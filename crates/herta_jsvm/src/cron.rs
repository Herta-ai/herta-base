//! Calendar scheduling belongs to the host, independently of source snapshots.
use crate::{JsRuntime, Snapshot};
use chrono::{DateTime, Timelike, Utc};
use herta_core::{
    HbResult, JsErrorKind, JsvmConfig,
    cron::CronTask,
    extension::{AuthMode, HostContext, HostFactory, Invocation},
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    task::{JoinHandle, JoinSet},
    time::timeout,
};
use tokio_util::sync::CancellationToken;

pub struct CronScheduler {
    stop: CancellationToken,
    driver: Option<JoinHandle<()>>,
    drain_ms: u64,
}
impl CronScheduler {
    /// Start only after bootstrap and serve have succeeded.
    pub fn start(
        runtime: Arc<JsRuntime>,
        hosts: Arc<dyn HostFactory>,
        config: &JsvmConfig,
    ) -> Self {
        let stop = CancellationToken::new();
        let mut driver = Driver::new(runtime, hosts, config.clone(), stop.clone());
        let task = tokio::spawn(async move {
            loop {
                let now = Utc::now();
                let delay = Duration::from_millis(1000 - u64::from(now.timestamp_subsec_millis()));
                tokio::select! {
                    biased;
                    _ = driver.stop.cancelled() => break,
                    Some(result) = driver.tasks.join_next(), if !driver.tasks.is_empty() => {
                        if let Err(error) = result { tracing::error!(%error, "cron task panicked"); }
                    }
                    _ = tokio::time::sleep(delay) => driver.tick(Utc::now()),
                }
            }
            // No new attempts or retries after stop. Accepted work gets bounded drain time.
            while let Some(result) = driver.tasks.join_next().await {
                if let Err(error) = result {
                    tracing::error!(%error, "cron drain failed");
                }
            }
        });
        Self {
            stop,
            driver: Some(task),
            drain_ms: config.shutdown_timeout_ms,
        }
    }
    pub fn stop(&self) {
        self.stop.cancel();
    }
    pub async fn shutdown(&mut self) -> HbResult<()> {
        self.stop();
        if let Some(mut task) = self.driver.take() {
            match timeout(Duration::from_millis(self.drain_ms), &mut task).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => return Err(JsErrorKind::Hook.into()),
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    return Err(JsErrorKind::Timeout.into());
                }
            }
        }
        Ok(())
    }
}
impl Drop for CronScheduler {
    fn drop(&mut self) {
        self.stop();
        if let Some(task) = self.driver.take() {
            task.abort();
        }
    }
}

struct Driver {
    runtime: Arc<JsRuntime>,
    hosts: Arc<dyn HostFactory>,
    config: JsvmConfig,
    stop: CancellationToken,
    tasks: JoinSet<()>,
    active: Arc<Mutex<HashSet<String>>>,
    last_second: i64,
}
impl Driver {
    fn new(
        runtime: Arc<JsRuntime>,
        hosts: Arc<dyn HostFactory>,
        config: JsvmConfig,
        stop: CancellationToken,
    ) -> Self {
        Self {
            runtime,
            hosts,
            config,
            stop,
            tasks: JoinSet::new(),
            active: Arc::new(Mutex::new(HashSet::new())),
            last_second: Utc::now().timestamp(),
        }
    }
    fn tick(&mut self, time: DateTime<Utc>) {
        let second = time.timestamp();
        if self.stop.is_cancelled() || second <= self.last_second {
            return;
        }
        self.last_second = second;
        if !self.config.cron.enabled {
            return;
        }
        let time = time.with_nanosecond(0).expect("zero nanoseconds is valid");
        let snapshot = self.runtime.snapshot();
        for registration in snapshot
            .registrations
            .iter()
            .filter(|entry| entry.kind == "cron")
        {
            // This was validated before publishing the snapshot.
            let task = match CronTask::parse(registration, &self.config.cron) {
                Ok(task) => task,
                Err(error) => {
                    tracing::error!(%error, "invalid cron snapshot");
                    continue;
                }
            };
            if !task.schedule.matches(time) {
                continue;
            }
            if !self
                .active
                .lock()
                .expect("cron active lock")
                .insert(task.name.clone())
            {
                tracing::debug!(task=%task.name, "overlapping cron occurrence skipped");
                continue;
            }
            let guard = Active {
                name: task.name.clone(),
                active: self.active.clone(),
            };
            let runtime = self.runtime.clone();
            let hosts = self.hosts.clone();
            let stop = self.stop.clone();
            let snapshot = snapshot.clone();
            self.tasks.spawn(async move {
                let _guard = guard;
                run(runtime, hosts, snapshot, task, time, stop).await;
            });
        }
    }
}
struct Active {
    name: String,
    active: Arc<Mutex<HashSet<String>>>,
}
impl Drop for Active {
    fn drop(&mut self) {
        self.active
            .lock()
            .expect("cron active lock")
            .remove(&self.name);
    }
}
async fn run(
    runtime: Arc<JsRuntime>,
    hosts: Arc<dyn HostFactory>,
    snapshot: Arc<Snapshot>,
    task: CronTask,
    scheduled_at: DateTime<Utc>,
    stop: CancellationToken,
) {
    let run_id = uuid::Uuid::now_v7().to_string();
    for attempt in 0..=task.retries {
        if stop.is_cancelled() {
            break;
        }
        let host = hosts.create(HostContext::system());
        let invocation = Invocation {
            name: "cron".into(),
            auth_mode: AuthMode::System,
            registration: Some(task.registration),
            request_id: None,
            payload: serde_json::json!({"name":task.name, "runId":run_id, "attemptId":uuid::Uuid::now_v7().to_string(), "scheduledAt":scheduled_at.to_rfc3339()}),
        };
        let result = runtime
            .invoke_snapshot(
                snapshot.clone(),
                invocation,
                host.clone(),
                Some(task.max_runtime_ms),
            )
            .await;
        let error = match result {
            Ok(_) => break,
            Err(error) => error,
        };
        tracing::warn!(task=%task.name, %run_id, attempt, code=error.error_code(), "cron attempt failed");
        if attempt == task.retries
            || !host.retry_safe()
            || matches!(
                error.error_code(),
                "HB_COMMIT_UNKNOWN"
                    | "HB_MAIL_SEND_FAILED"
                    | "HB_MAIL_TIMEOUT"
                    | "HB_HTTP_SEND_FAILED"
                    | "HB_HTTP_TIMEOUT"
            )
        {
            break;
        }
        tokio::select! {
            biased;
            _ = stop.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(1 << attempt)) => {},
        }
    }
}

#[cfg(test)]
mod tests;
