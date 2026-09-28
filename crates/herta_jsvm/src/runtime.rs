use crate::{
    engine,
    source::{Snapshot, load_error},
};
use async_trait::async_trait;
use herta_core::{
    HbError, HbResult, JsErrorKind, JsvmConfig,
    extension::{EventDispatcher, HostServices, Invocation, Registration, UnavailableHost},
    host_buffer::{HostBudget, HostReply},
};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, RwLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

struct Job {
    snapshot: Arc<Snapshot>,
    invocation: engine::Input,
    host: Arc<dyn HostServices>,
    cancellation: CancellationToken,
    deadline: Instant,
    total_ms: u64,
    result: oneshot::Sender<HbResult<HostReply>>,
    started: oneshot::Sender<()>,
}

pub struct JsRuntime {
    config: Arc<JsvmConfig>,
    path: PathBuf,
    snapshot: RwLock<Arc<Snapshot>>,
    snapshots: Mutex<Vec<Weak<Snapshot>>>,
    sender: mpsc::Sender<Job>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    stopping: CancellationToken,
    cancellation: CancellationToken,
    accepting: AtomicBool,
    reload_lock: tokio::sync::Mutex<()>,
    executor: tokio::runtime::Handle,
}

impl JsRuntime {
    pub async fn load(path: impl AsRef<Path>, config: JsvmConfig) -> HbResult<Arc<Self>> {
        config.validate().map_err(load_error)?;
        if !config.enabled {
            return Err(JsErrorKind::Denied.into());
        }
        let config = Arc::new(config);
        let path = path.as_ref().to_owned();
        let executor = tokio::runtime::Handle::current();
        let snapshot = compile(path.clone(), config.clone(), executor.clone(), None).await?;
        let (sender, receiver) = mpsc::channel::<Job>(config.queue_capacity);
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        let stopping = CancellationToken::new();
        // Accepted native operations keep their slot until settlement finishes,
        // even if the JS worker stops waiting. This bounds detached cleanup tasks.
        let settlements = Arc::new(Semaphore::new(config.pool_size));
        let cancellation = CancellationToken::new();
        let runtime = Arc::new(Self {
            config: config.clone(),
            path,
            snapshot: RwLock::new(snapshot.clone()),
            snapshots: Mutex::new(vec![Arc::downgrade(&snapshot)]),
            sender,
            threads: Mutex::new(Vec::new()),
            stopping: stopping.clone(),
            cancellation,
            accepting: AtomicBool::new(true),
            reload_lock: tokio::sync::Mutex::new(()),
            executor: executor.clone(),
        });
        for index in 0..config.pool_size {
            let receiver = receiver.clone();
            let config = config.clone();
            let stopping = stopping.clone();
            let executor = executor.clone();
            let settlements = settlements.clone();
            let thread=std::thread::Builder::new().name(format!("herta-js-{index}")).stack_size(2*1024*1024 + config.stack_limit_kb*1024)
                .spawn(move || {
                    let local=match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                        Ok(local)=>local, Err(error)=>{tracing::error!(%error,"failed to start JS worker executor");return;}
                    };
                    local.block_on(async move {
                        loop {
                            let job={
                                let mut receiver=receiver.lock().await;
                                tokio::select! {
                                    _=stopping.cancelled()=>{
                                        receiver.close();
                                        while let Ok(job)=receiver.try_recv() {let _=job.result.send(Err(JsErrorKind::Busy.into()));}
                                        None
                                    },
                                    job=receiver.recv()=>job,
                                }
                            };
                            let Some(job)=job else {break};
                            if Instant::now()>=job.deadline || job.cancellation.is_cancelled() || job.result.is_closed() {
                                let _=job.result.send(Err(JsErrorKind::Busy.into()));continue;
                            }
                            let settlement=match settlements.clone().try_acquire_owned() {
                                Ok(permit)=>permit,
                                Err(_)=>{let _=job.result.send(Err(JsErrorKind::Busy.into()));continue;}
                            };
                            let _=job.started.send(());
                            let host=crate::host_queue::HostQueue::new(job.host,&executor,config.max_pending_host_calls,job.cancellation.clone());
                            let result=engine::execute(job.snapshot,config.clone(),Some(job.invocation),host.clone(),job.cancellation,job.total_ms).await
                                .map(|(_,value)|value);
                            let successful=result.is_ok();
                            if !successful { host.cancel(); }
                            let finishing=host.clone();
                            let cleanup=executor.spawn(async move {
                                let _settlement=settlement;
                                finishing.finish(successful).await
                            });
                            let cleanup=tokio::time::timeout(Duration::from_millis(config.shutdown_timeout_ms.min(job.total_ms)),cleanup).await;
                            let result=match (result,cleanup) {
                                // Unknown commit always takes precedence over a JS timeout.
                                (_,Ok(Ok(Err(error)))) if error.error_code()=="HB_COMMIT_UNKNOWN"=>Err(error),
                                (Err(error),_) if error.error_code()=="HB_HOOK_TIMEOUT"=>Err(host.interrupted_commit().unwrap_or(error)),
                                (Err(error),_)=>Err(error),
                                (Ok(value),Ok(Ok(Ok(()))))=>Ok(value),
                                (_,Ok(Ok(Err(error))))=>Err(error),
                                (_,Ok(Err(_)))=>Err(HbError::Internal),
                                (_,Err(_))=>{host.cancel();Err(host.interrupted_commit().unwrap_or_else(||JsErrorKind::Timeout.into()))},
                            };
                            let _=job.result.send(result);
                        }
                    });
                }).map_err(load_error)?;
            runtime
                .threads
                .lock()
                .map_err(|_| HbError::Internal)?
                .push(thread);
        }
        Ok(runtime)
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot
            .read()
            .expect("snapshot lock poisoned")
            .clone()
    }

    pub async fn invoke(
        &self,
        invocation: Invocation,
        host: Arc<dyn HostServices>,
        total_ms: Option<u64>,
    ) -> HbResult<Value> {
        self.invoke_snapshot(self.snapshot(), invocation, host, total_ms)
            .await
    }

    pub async fn invoke_snapshot(
        &self,
        snapshot: Arc<Snapshot>,
        invocation: Invocation,
        host: Arc<dyn HostServices>,
        total_ms: Option<u64>,
    ) -> HbResult<Value> {
        self.invoke_frame(snapshot, invocation, host, total_ms, None)
            .await?
            .into_response_value()
    }

    async fn invoke_frame(
        &self,
        snapshot: Arc<Snapshot>,
        invocation: Invocation,
        host: Arc<dyn HostServices>,
        total_ms: Option<u64>,
        body: Option<Vec<u8>>,
    ) -> HbResult<HostReply> {
        if !self.accepting.load(Ordering::Acquire) {
            return Err(JsErrorKind::Busy.into());
        }
        let (sender, receiver) = oneshot::channel();
        let (start_sender, start_receiver) = oneshot::channel();
        let cancellation = self.cancellation.child_token();
        struct CancelOnDrop(CancellationToken);
        impl Drop for CancelOnDrop {
            fn drop(&mut self) {
                self.0.cancel();
            }
        }
        let _guard = CancelOnDrop(cancellation.clone());
        let buffers = HostBudget::new(self.config.max_host_buffer_bytes);
        let bytes = body
            .map(|body| {
                if body.len() > self.config.max_bridge_bytes {
                    return Err(HbError::PayloadTooLarge);
                }
                buffers.adopt(body)
            })
            .transpose()?;
        let job = Job {
            snapshot,
            invocation: engine::Input {
                invocation,
                bytes,
                buffers,
            },
            host,
            cancellation,
            deadline: Instant::now() + Duration::from_millis(self.config.queue_timeout_ms),
            total_ms: total_ms.unwrap_or(self.config.async_timeout_ms),
            result: sender,
            started: start_sender,
        };
        self.sender
            .try_send(job)
            .map_err(|_| HbError::from(JsErrorKind::Busy))?;
        tokio::time::timeout(
            Duration::from_millis(self.config.queue_timeout_ms),
            start_receiver,
        )
        .await
        .map_err(|_| HbError::from(JsErrorKind::Busy))?
        .map_err(|_| HbError::from(JsErrorKind::Busy))?;
        receiver.await.unwrap_or(Err(JsErrorKind::Busy.into()))
    }

    pub async fn reload(&self) -> HbResult<bool> {
        let _reload = self.reload_lock.lock().await;
        {
            let mut snapshots = self.snapshots.lock().map_err(|_| HbError::Internal)?;
            snapshots.retain(|snapshot| snapshot.strong_count() > 0);
            if snapshots.len() >= self.config.max_snapshots {
                return Err(JsErrorKind::Busy.into());
            }
        }
        let candidate = compile(
            self.path.clone(),
            self.config.clone(),
            self.executor.clone(),
            Some(self.snapshot()),
        )
        .await?;
        let mut snapshot = self.snapshot.write().map_err(|_| HbError::Internal)?;
        if snapshot.hash == candidate.hash {
            return Ok(false);
        }
        *snapshot = candidate.clone();
        self.snapshots
            .lock()
            .map_err(|_| HbError::Internal)?
            .push(Arc::downgrade(&candidate));
        tracing::info!(source="jsvm",snapshot=%candidate.hash,"extension registry reloaded");
        Ok(true)
    }

    pub async fn shutdown(&self) -> HbResult<()> {
        self.accepting.store(false, Ordering::Release);
        self.stopping.cancel();
        let threads = std::mem::take(&mut *self.threads.lock().map_err(|_| HbError::Internal)?);
        let cancellation = self.cancellation.clone();
        let deadline = self.config.shutdown_timeout_ms;
        let mut joined = tokio::task::spawn_blocking(move || {
            for thread in threads {
                let _ = thread.join();
            }
        });
        if tokio::time::timeout(Duration::from_millis(deadline), &mut joined)
            .await
            .is_err()
        {
            cancellation.cancel();
            return Err(JsErrorKind::Timeout.into());
        }
        Ok(())
    }
}

impl Drop for JsRuntime {
    fn drop(&mut self) {
        self.stopping.cancel();
        self.cancellation.cancel();
    }
}

#[async_trait]
impl EventDispatcher for JsRuntime {
    async fn dispatch_frame(
        &self,
        invocation: Invocation,
        host: Arc<dyn HostServices>,
        body: Option<Vec<u8>>,
    ) -> HbResult<HostReply> {
        self.invoke_frame(self.snapshot(), invocation, host, None, body)
            .await
    }
    async fn dispatch(
        &self,
        invocation: Invocation,
        host: Arc<dyn HostServices>,
    ) -> HbResult<Value> {
        self.invoke(invocation, host, None).await
    }
    fn registrations(&self) -> Arc<Vec<Registration>> {
        self.snapshot().registrations.clone()
    }
    fn pin(self: Arc<Self>) -> Arc<dyn EventDispatcher> {
        Arc::new(PinnedDispatcher {
            snapshot: self.snapshot(),
            runtime: self,
        })
    }
}

struct PinnedDispatcher {
    runtime: Arc<JsRuntime>,
    snapshot: Arc<Snapshot>,
}

#[async_trait]
impl EventDispatcher for PinnedDispatcher {
    async fn dispatch_frame(
        &self,
        invocation: Invocation,
        host: Arc<dyn HostServices>,
        body: Option<Vec<u8>>,
    ) -> HbResult<HostReply> {
        self.runtime
            .invoke_frame(self.snapshot.clone(), invocation, host, None, body)
            .await
    }
    async fn dispatch(
        &self,
        invocation: Invocation,
        host: Arc<dyn HostServices>,
    ) -> HbResult<Value> {
        self.runtime
            .invoke_snapshot(self.snapshot.clone(), invocation, host, None)
            .await
    }
    fn registrations(&self) -> Arc<Vec<Registration>> {
        self.snapshot.registrations.clone()
    }
    fn pin(self: Arc<Self>) -> Arc<dyn EventDispatcher> {
        self
    }
}

async fn compile(
    path: PathBuf,
    config: Arc<JsvmConfig>,
    executor: tokio::runtime::Handle,
    previous: Option<Arc<Snapshot>>,
) -> HbResult<Arc<Snapshot>> {
    let (sender, receiver) = oneshot::channel();
    let timeout = config.startup_timeout_ms;
    let cancellation = CancellationToken::new();
    let compiler_cancel = cancellation.clone();
    std::thread::Builder::new()
        .name("herta-js-validate".into())
        .stack_size(2 * 1024 * 1024 + config.stack_limit_kb * 1024)
        .spawn(move || {
            let result = (|| {
                let snapshot = Arc::new(Snapshot::read(&path, &config)?);
                if let Some(previous) = previous.filter(|previous| previous.hash == snapshot.hash) {
                    return Ok(previous);
                }
                let local = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(load_error)?;
                let (registrations, _) = local.block_on(engine::execute(
                    snapshot.clone(),
                    config.clone(),
                    None,
                    crate::host_queue::HostQueue::new(
                        Arc::new(UnavailableHost),
                        &executor,
                        config.max_pending_host_calls,
                        compiler_cancel.clone(),
                    ),
                    compiler_cancel,
                    timeout,
                ))?;
                validate_registrations(&registrations, &config)?;
                let mut snapshot = Arc::try_unwrap(snapshot)
                    .map_err(|_| load_error("validation retained a source snapshot"))?;
                snapshot.registrations = Arc::new(registrations);
                Ok(Arc::new(snapshot))
            })();
            let _ = sender.send(result);
        })
        .map_err(load_error)?;
    match tokio::time::timeout(Duration::from_millis(timeout), receiver).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(load_error("validation worker exited")),
        Err(_) => {
            cancellation.cancel();
            Err(load_error("script startup timeout"))
        }
    }
}

fn validate_registrations(registrations: &[Registration], config: &JsvmConfig) -> HbResult<()> {
    let mut cron_names = std::collections::HashSet::new();
    for registration in registrations.iter().filter(|entry| entry.kind == "cron") {
        let task = herta_core::cron::CronTask::parse(registration, &config.cron)?;
        if !cron_names.insert(task.name) {
            return Err(load_error("duplicate cron task name"));
        }
    }
    let mut routes: Vec<herta_core::routes::CustomRoute> = Vec::new();
    for registration in registrations.iter().filter(|entry| entry.kind == "route") {
        let route = herta_core::routes::CustomRoute::parse(registration, config)?;
        if routes.iter().any(|other| route.overlaps(other)) {
            return Err(JsErrorKind::RouteConflict.into());
        }
        routes.push(route);
    }
    Ok(())
}
