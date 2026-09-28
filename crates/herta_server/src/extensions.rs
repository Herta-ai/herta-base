use herta_api::{ApiState, extensions::ApiHostFactory};
use herta_core::{
    HbResult,
    extension::{AuthMode, Extensions, HostContext, Invocation},
};
use herta_jsvm::{CronScheduler, JsRuntime};
use std::{sync::Arc, time::Duration};

pub struct RuntimeServices {
    runtime: Arc<JsRuntime>,
    services: Extensions,
    watcher: Option<tokio::task::JoinHandle<()>>,
    cron: Option<CronScheduler>,
    outbox: Option<herta_api::outbox::OutboxWorker>,
    outbox_service: herta_api::outbox::OutboxService,
    config: herta_core::JsvmConfig,
}

impl RuntimeServices {
    pub async fn start(state: &mut ApiState) -> HbResult<Option<Self>> {
        if !state.config.jsvm.enabled {
            return Ok(None);
        }
        let runtime =
            JsRuntime::load(&state.config.paths.hooks_dir, state.config.jsvm.clone()).await?;
        let services = Extensions {
            dispatcher: runtime.clone(),
            hosts: Arc::new(
                ApiHostFactory::new(
                    state.db.clone(),
                    state.config.clone(),
                    state.mailer.clone(),
                    state.storage.clone(),
                )
                .with_docs(state.docs.clone())
                .with_messages(state.messages.clone()),
            ),
        };
        let mut running = Self {
            runtime,
            services,
            watcher: None,
            cron: None,
            outbox: None,
            outbox_service: herta_api::outbox::OutboxService::new(
                state.db.clone(),
                state.config.clone(),
                state.mailer.clone(),
            ),
            config: state.config.jsvm.clone(),
        };
        if let Err(error) = running.lifecycle("bootstrap").await {
            let _ = running.runtime.shutdown().await;
            return Err(error);
        }
        state.extensions = Some(running.services.clone());
        if state.config.server.dev_mode {
            let runtime = running.runtime.clone();
            running.watcher = Some(tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    if let Err(error) = runtime.reload().await {
                        tracing::warn!(%error, "extension reload rejected; previous snapshot remains active");
                    }
                }
            }));
        }
        Ok(Some(running))
    }

    pub async fn lifecycle(&self, name: &str) -> HbResult<()> {
        self.services
            .dispatcher
            .clone()
            .pin()
            .dispatch(
                Invocation {
                    name: name.into(),
                    payload: serde_json::Value::Null,
                    auth_mode: AuthMode::System,
                    request_id: None,
                    registration: None,
                },
                self.services.hosts.create(HostContext::system()),
            )
            .await
            .map(|_| ())
    }

    pub fn stop_background(&mut self) {
        if let Some(outbox) = &self.outbox {
            outbox.stop();
        }
        if let Some(cron) = &self.cron {
            cron.stop();
        }
        if let Some(watcher) = self.watcher.take() {
            watcher.abort();
        }
    }

    pub fn start_scheduled_tasks(&mut self) {
        if self.config.outbox.enabled && self.outbox.is_none() {
            self.outbox = Some(self.outbox_service.clone().start());
        }
        if self.config.cron.enabled && self.cron.is_none() {
            self.cron = Some(CronScheduler::start(
                self.runtime.clone(),
                self.services.hosts.clone(),
                &self.config,
            ));
        }
    }

    pub async fn shutdown(mut self) -> HbResult<()> {
        self.stop_background();
        let outbox = if let Some(worker) = &mut self.outbox {
            worker.shutdown().await
        } else {
            Ok(())
        };
        let tasks = if let Some(cron) = &mut self.cron {
            cron.shutdown().await
        } else {
            Ok(())
        };
        let event = self.lifecycle("shutdown").await;
        let workers = self.runtime.shutdown().await;
        tasks.and(outbox).and(event).and(workers)
    }
}

impl Drop for RuntimeServices {
    fn drop(&mut self) {
        self.stop_background();
    }
}

pub async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! { result = tokio::signal::ctrl_c() => result, _ = terminate.recv() => Ok(()) }
    }
    #[cfg(windows)]
    {
        // CTRL_BREAK can target a dedicated console process group, allowing service
        // supervisors to request a graceful stop without terminating the process.
        let mut interrupt = tokio::signal::windows::ctrl_break()?;
        tokio::select! { result = tokio::signal::ctrl_c() => result, _ = interrupt.recv() => Ok(()) }
    }
    #[cfg(not(any(unix, windows)))]
    {
        tokio::signal::ctrl_c().await
    }
}
