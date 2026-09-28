//! A bounded worker-to-Tokio command channel. Native operations outlive JS waiters.
use herta_core::{
    HbError, HbResult, JsErrorKind,
    extension::{AuthMode, HostCall, HostServices},
    host_buffer::{BufferPermit, HostBudget, HostBuffer, HostReply},
};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

struct Command {
    raw: String,
    binary: Option<HostBuffer>,
    budget: HostBudget,
    request_mode: bool,
    deadline: Instant,
    _bytes: BufferPermit,
    _slot: OwnedSemaphorePermit,
    reply: oneshot::Sender<HbResult<HostReply>>,
}

pub(crate) struct HostQueue {
    host: Arc<dyn HostServices>,
    sender: Mutex<Option<mpsc::Sender<Command>>>,
    task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    slots: Arc<Semaphore>,
    capacity: usize,
    cancellation: CancellationToken,
}

impl HostQueue {
    pub fn new(
        host: Arc<dyn HostServices>,
        executor: &tokio::runtime::Handle,
        capacity: usize,
        cancellation: CancellationToken,
    ) -> Arc<Self> {
        let (sender, mut receiver) = mpsc::channel::<Command>(capacity);
        let services = host.clone();
        let cancelled = cancellation.clone();
        let task = executor.spawn(async move {
            let mut calls = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    command = receiver.recv() => {
                        let Some(command) = command else { break; };
                        let host = services.clone();
                        let cancelled = cancelled.clone();
                        // The slot is held through completion, including a dropped JS receiver.
                        calls.spawn(async move {
                            let result = async {
                                if cancelled.is_cancelled() || Instant::now() >= command.deadline {
                                    return Err(JsErrorKind::Timeout.into());
                                }
                                let mut call: HostCall = serde_json::from_str(&command.raw)
                                    .map_err(|_| HbError::validation("invalid host command"))?;
                                call.remaining_ms = command.deadline.saturating_duration_since(Instant::now())
                                    .as_millis().min(u64::MAX as u128) as u64;
                                if command.request_mode { call.auth_mode = AuthMode::Request; }
                                host.call_binary(call, command.binary, command.budget).await
                            }.await;
                            // Send only after freeing the pending slot. A sequential caller can
                            // immediately issue its next command when capacity is one.
                            drop(command.raw);
                            drop(command._bytes);
                            drop(command._slot);
                            let _ = command.reply.send(result);
                        });
                    },
                    result = calls.join_next(), if !calls.is_empty() => {
                        if result.is_some_and(|result| result.is_err()) {
                            cancelled.cancel();
                            services.cancel();
                        }
                    },
                }
            }
            while let Some(result) = calls.join_next().await {
                if result.is_err() { cancelled.cancel(); services.cancel(); }
            }
        });
        Arc::new(Self {
            host,
            sender: Mutex::new(Some(sender)),
            task: tokio::sync::Mutex::new(Some(task)),
            slots: Arc::new(Semaphore::new(capacity)),
            capacity,
            cancellation,
        })
    }

    pub async fn call(
        &self,
        raw: String,
        bytes: BufferPermit,
        binary: Option<HostBuffer>,
        budget: HostBudget,
        request_mode: bool,
        deadline: Instant,
    ) -> HbResult<HostReply> {
        if self.cancellation.is_cancelled() || Instant::now() >= deadline {
            return Err(JsErrorKind::Timeout.into());
        }
        let slot = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| HbError::PayloadTooLarge)?;
        let (reply, received) = oneshot::channel();
        {
            let sender = self.sender.lock().map_err(|_| HbError::Internal)?;
            sender
                .as_ref()
                .ok_or_else(|| HbError::from(JsErrorKind::Timeout))?
                .try_send(Command {
                    raw,
                    binary,
                    budget,
                    request_mode,
                    deadline,
                    _bytes: bytes,
                    _slot: slot,
                    reply,
                })
                .map_err(|_| HbError::from(JsErrorKind::Busy))?;
        }
        received.await.unwrap_or(Err(HbError::Internal))
    }

    pub fn pending(&self) -> usize {
        self.capacity - self.slots.available_permits()
    }
    pub fn cancel(&self) {
        self.cancellation.cancel();
        self.host.cancel();
    }
    pub fn interrupted_commit(&self) -> Option<HbError> {
        self.host.interrupted_commit()
    }

    pub async fn finish(&self, successful: bool) -> HbResult<()> {
        self.sender.lock().map_err(|_| HbError::Internal)?.take();
        // The runtime's settlement permit is retained while native I/O drains.
        let drained = if let Some(task) = self.task.lock().await.take() {
            task.await.is_ok()
        } else {
            true
        };
        let result = self.host.finish(successful && drained).await;
        if !drained {
            result?;
            return Err(HbError::Internal);
        }
        result
    }
}
