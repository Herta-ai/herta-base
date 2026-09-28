use crate::host_queue::HostQueue;
use crate::source::{Snapshot, load_error};
use herta_core::{
    HbError, HbResult, JsError, JsErrorKind, JsvmConfig,
    extension::{Invocation, Registration},
    host_buffer::{BufferPermit, HostBudget, HostBuffer, HostReply},
};
use rquickjs::{
    AsyncContext, AsyncRuntime, Ctx, Function, Object, Persistent, Promise, TypedArray, Value,
    context::EvalOptions,
    function::Opt,
    promise::{PromiseHookType, Promised},
};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    future::{Future, poll_fn},
    hash::{Hash, Hasher},
    pin::pin,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

struct ActiveBudget {
    used: Cell<Duration>,
    segment: Cell<Option<Instant>>,
    exceeded: Cell<bool>,
    limit: Duration,
    deadline: Instant,
}
impl ActiveBudget {
    fn interrupted(&self) -> bool {
        let active = self.used.get()
            + self
                .segment
                .get()
                .map_or(Duration::ZERO, |start| start.elapsed());
        let exceeded = active >= self.limit || Instant::now() >= self.deadline;
        if exceeded {
            self.exceeded.set(true);
        }
        exceeded
    }

    async fn meter<T>(
        &self,
        cancellation: &CancellationToken,
        future: impl Future<Output = HbResult<T>>,
    ) -> HbResult<T> {
        let mut future = pin!(future);
        poll_fn(|cx| {
            if cancellation.is_cancelled() || self.interrupted() {
                return std::task::Poll::Ready(Err(JsErrorKind::Timeout.into()));
            }
            self.segment.set(Some(Instant::now()));
            let result = future.as_mut().poll(cx);
            self.used.set(
                self.used.get()
                    + self
                        .segment
                        .take()
                        .map_or(Duration::ZERO, |start| start.elapsed()),
            );
            if self.interrupted() {
                std::task::Poll::Ready(Err(JsErrorKind::Timeout.into()))
            } else {
                result
            }
        })
        .await
    }
}

// Do not use AsyncContext::async_with here: its poll drains an unbounded number
// of microtasks. A loop of short Promise callbacks would bypass the interrupt
// counter and starve cancellation and the cumulative execution budget.
async fn settle(
    runtime: &AsyncRuntime,
    context: &AsyncContext,
    budget: &ActiveBudget,
    cancellation: &CancellationToken,
    value: Persistent<Value<'static>>,
) -> HbResult<Persistent<Value<'static>>> {
    loop {
        let result = budget
            .meter(
                cancellation,
                context.with(|ctx| {
                    let value = value.clone().restore(&ctx).map_err(engine_error)?;
                    match value.as_promise() {
                        Some(promise) => promise
                            .result::<Value>()
                            .map(|result| {
                                result
                                    .map(|value| Persistent::save(&ctx, value))
                                    .map_err(|error| caught_error(&ctx, error))
                            })
                            .transpose(),
                        None => Ok(Some(Persistent::save(&ctx, value))),
                    }
                }),
            )
            .await?;
        if let Some(result) = result {
            return Ok(result);
        }
        let progressed = budget
            .meter(cancellation, async {
                match runtime.execute_pending_job().await {
                    Ok(progress) => Ok(progress),
                    Err(error) => Err(error
                        .0
                        .with(|ctx| caught_error(&ctx, rquickjs::Error::Exception))
                        .await),
                }
            })
            .await?;
        if progressed {
            tokio::task::yield_now().await;
        } else {
            // The scheduler registers its waker when polling host futures. The
            // short timer also covers a pending JS promise with no host I/O.
            tokio::select! {
                _ = cancellation.cancelled() => return Err(JsErrorKind::Timeout.into()),
                _ = tokio::time::sleep(Duration::from_millis(1)) => {},
            }
        }
    }
}

async fn drain(
    runtime: &AsyncRuntime,
    budget: &ActiveBudget,
    cancellation: &CancellationToken,
) -> HbResult<()> {
    while runtime.is_job_pending().await {
        budget
            .meter(cancellation, async {
                match runtime.execute_pending_job().await {
                    Ok(true) => Ok(()),
                    Ok(false) => Err(JsErrorKind::Hook.into()),
                    Err(error) => Err(error
                        .0
                        .with(|ctx| caught_error(&ctx, rquickjs::Error::Exception))
                        .await),
                }
            })
            .await?;
        tokio::task::yield_now().await;
    }
    Ok(())
}

fn caught_error(ctx: &Ctx<'_>, error: rquickjs::Error) -> HbError {
    let diagnostic = if error.is_exception() {
        let exception = ctx.catch();
        exception
            .as_object()
            .and_then(|object| object.get::<_, String>("stack").ok())
            .or_else(|| {
                exception
                    .as_object()
                    .and_then(|object| object.get::<_, String>("message").ok())
            })
            .unwrap_or_else(|| "JavaScript exception".into())
    } else {
        error.to_string()
    };
    tracing::error!(source="jsvm", %diagnostic, "JavaScript execution failed");
    let kind =
        if matches!(error, rquickjs::Error::Allocation) || diagnostic.contains("out of memory") {
            JsErrorKind::Oom
        } else {
            JsErrorKind::Hook
        };
    HbError::Extension(JsError::diagnostic(kind, diagnostic))
}

struct Bridge {
    config: Arc<JsvmConfig>,
    host: Arc<HostQueue>,
    cancellation: CancellationToken,
    deadline: Instant,
    calls: AtomicUsize,
    request_mode: bool,
    buffers: HostBudget,
}

struct Reply {
    value: String,
    binary: Option<HostBuffer>,
    _reservation: Option<BufferPermit>,
}
impl<'js> rquickjs::IntoJs<'js> for Reply {
    fn into_js(self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let reply = Object::new(ctx.clone())?;
        reply.set(
            "json",
            rquickjs::String::from_str(ctx.clone(), &self.value)?,
        )?;
        if let Some(binary) = self.binary {
            // Copy into QuickJS-owned memory so retained arrays count against the JS
            // heap limit. External ArrayBuffers would bypass that limit.
            reply.set(
                "bytes",
                TypedArray::<u8>::new_copy(ctx.clone(), binary.as_ref())?,
            )?;
        }
        Ok(reply.into_value())
    }
}

impl Bridge {
    async fn send(
        self: Arc<Self>,
        raw: String,
        reservation: BufferPermit,
        binary: Option<HostBuffer>,
    ) -> Reply {
        let invalid = self.cancellation.is_cancelled() || Instant::now() >= self.deadline;
        let count = self.calls.fetch_add(1, Ordering::AcqRel);
        if invalid || count >= self.config.max_host_calls {
            return self.error(if invalid {
                JsErrorKind::Timeout.into()
            } else {
                HbError::PayloadTooLarge
            });
        }
        let result = self
            .host
            .call(
                raw,
                reservation,
                binary,
                self.buffers.clone(),
                self.request_mode,
                self.deadline,
            )
            .await;
        let (envelope, binary) = match result {
            Ok(reply) => (json!({"ok":true,"value":reply.value}), reply.bytes),
            Err(error) => (error_envelope(&error), None),
        };
        let limit = self
            .config
            .max_bridge_bytes
            .saturating_sub(binary.as_ref().map_or(0, |bytes| bytes.as_ref().len()));
        let size = match json_size(&envelope, limit) {
            Ok(size) => size,
            Err(error) => return self.error(error),
        };
        let reservation = match self.buffers.reserve(size) {
            Ok(permit) => permit,
            Err(error) => return self.error(error),
        };
        match bounded_json(&envelope, size) {
            Ok(value) => Reply {
                value,
                binary,
                _reservation: Some(reservation),
            },
            Err(error) => self.error(error),
        }
    }
    fn error(&self, error: HbError) -> Reply {
        Reply {
            value: error_envelope(&error).to_string(),
            binary: None,
            _reservation: None,
        }
    }
}

fn json_size(value: &impl serde::Serialize, limit: usize) -> HbResult<usize> {
    struct Counter {
        size: usize,
        limit: usize,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.size) {
                return Err(std::io::Error::other("JSON byte limit exceeded"));
            }
            self.size += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter { size: 0, limit };
    serde_json::to_writer(&mut counter, value).map_err(|_| HbError::PayloadTooLarge)?;
    Ok(counter.size)
}

pub(crate) fn error_envelope(error: &HbError) -> serde_json::Value {
    let details = match error {
        HbError::Validation { details, .. } => details.clone(),
        HbError::Extension(error) => error.details.clone(),
        _ => None,
    };
    json!({"ok":false,"error":{"code":error.error_code(),"status":error.status_code(),"message":error.public_message(false),"details":details}})
}

pub(crate) fn bounded_json(value: &impl serde::Serialize, limit: usize) -> HbResult<String> {
    let limit = json_size(value, limit)?;
    struct Writer {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl std::io::Write for Writer {
        fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
            if input.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other("JSON byte limit exceeded"));
            }
            self.bytes.extend_from_slice(input);
            Ok(input.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Writer {
        bytes: Vec::with_capacity(limit),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| HbError::PayloadTooLarge)?;
    String::from_utf8(writer.bytes).map_err(|_| HbError::Internal)
}

pub(crate) struct Input {
    pub invocation: Invocation,
    pub bytes: Option<HostBuffer>,
    pub buffers: HostBudget,
}

pub(crate) async fn execute(
    snapshot: Arc<Snapshot>,
    config: Arc<JsvmConfig>,
    input: Option<Input>,
    host: Arc<HostQueue>,
    cancellation: CancellationToken,
    total_ms: u64,
) -> HbResult<(Vec<Registration>, HostReply)> {
    let buffers = input
        .as_ref()
        .map(|input| input.buffers.clone())
        .unwrap_or_else(|| HostBudget::new(config.max_host_buffer_bytes));
    let (invocation, request_bytes) = input
        .map(|input| (Some(input.invocation), input.bytes))
        .unwrap_or((None, None));
    let runtime = AsyncRuntime::new().map_err(engine_error)?;
    runtime
        .set_memory_limit(config.memory_limit_mb * 1024 * 1024)
        .await;
    runtime
        .set_max_stack_size(config.stack_limit_kb * 1024)
        .await;
    let budget = Rc::new(ActiveBudget {
        used: Cell::new(Duration::ZERO),
        segment: Cell::new(None),
        exceeded: Cell::new(false),
        limit: Duration::from_millis(if invocation.is_some() {
            config.execution_timeout_ms
        } else {
            config.startup_timeout_ms
        }),
        deadline: Instant::now() + Duration::from_millis(total_ms),
    });
    let interrupt_budget = budget.clone();
    let interrupt_cancel = cancellation.clone();
    runtime
        .set_interrupt_handler(Some(Box::new(move || {
            interrupt_cancel.is_cancelled() || interrupt_budget.interrupted()
        })))
        .await;
    let context = AsyncContext::full(&runtime).await.map_err(engine_error)?;
    let bridge = Arc::new(Bridge {
        config: config.clone(),
        host,
        cancellation: cancellation.clone(),
        deadline: budget.deadline,
        calls: AtomicUsize::new(0),
        request_mode: invocation.as_ref().is_some_and(|invocation| {
            invocation.auth_mode == herta_core::extension::AuthMode::Request
        }),
        buffers,
    });
    let log_count = Rc::new(Cell::new(0usize));
    let log_bytes = Rc::new(Cell::new(0usize));
    let environment = snapshot.environment.clone();
    let setup_bridge = bridge.clone();
    let setup_config = config.clone();
    // Private controller checkpoint exists only while running root afterCommit
    // callbacks. Hard interruption here cannot turn confirmed persistence into a failure.
    let committed = Rc::new(RefCell::new(None::<serde_json::Value>));
    let checkpoint = committed.clone();
    let setup = context.with(|ctx| {
        let bridge = setup_bridge;
        let native = Function::new(ctx.clone(), move |input: rquickjs::String<'_>, binary: Opt<TypedArray<'_, u8>>| {
            let input = input.to_cstring()?;
            let prepared = (|| -> HbResult<_> {
                let bytes = binary.0.as_ref().map(|bytes| bytes.as_bytes().ok_or_else(|| HbError::validation("detached byte array"))).transpose()?;
                if bytes.map_or(0, |bytes| bytes.len()) > bridge.config.max_bridge_bytes.saturating_sub(input.len())
                    || input.len() > bridge.config.max_bridge_bytes { return Err(HbError::PayloadTooLarge); }
                let permit = bridge.buffers.reserve(input.len())?;
                let binary = bytes.map(|bytes| bridge.buffers.copy(bytes)).transpose()?;
                Ok((input.as_str().to_owned(), permit, binary))
            })();
            let bridge = bridge.clone();
            Ok::<_, rquickjs::Error>(Promised(async move {
                match prepared {
                    Ok((raw, permit, binary)) => bridge.send(raw, permit, binary).await,
                    Err(error) => bridge.error(error),
                }
            }))
        })?;
        let log_config = setup_config.clone();
        let logger = Function::new(ctx.clone(), move |level: String, message: rquickjs::String<'_>, fields: rquickjs::String<'_>, script: String| {
            let message = message.to_cstring()?; let fields = fields.to_cstring()?;
            let bytes = message.len().saturating_add(fields.len());
            if log_count.get() >= log_config.max_logs || bytes > log_config.max_log_bytes.saturating_sub(log_bytes.get()) { return Ok::<_, rquickjs::Error>(()); }
            log_count.set(log_count.get()+1); log_bytes.set(log_bytes.get()+bytes);
            let mut fields: serde_json::Value = serde_json::from_str(fields.as_str()).unwrap_or(json!({}));
            redact(&mut fields, 0);
            match level.as_str() {
                "trace" => tracing::trace!(source="jsvm", %script, message=%message.as_str(), fields=%fields),
                "debug" => tracing::debug!(source="jsvm", %script, message=%message.as_str(), fields=%fields),
                "warn" => tracing::warn!(source="jsvm", %script, message=%message.as_str(), fields=%fields),
                "error" => tracing::error!(source="jsvm", %script, message=%message.as_str(), fields=%fields),
                _ => tracing::info!(source="jsvm", %script, message=%message.as_str(), fields=%fields),
            }
            Ok(())
        })?;
        let env = Function::new(ctx.clone(), move |name: String| {
            if name == "__HB_NEW_RECORD_ID" { Some(uuid::Uuid::now_v7().to_string()) } else { environment.get(&name).cloned() }
        })?;
        let factory: Function = ctx.eval(include_str!("bootstrap.js"))?;
        let cfg = ctx.json_parse(serde_json::to_vec(&*setup_config).expect("configuration is JSON"))?;
        let checkpoint = Function::new(ctx.clone(), move |value: Option<rquickjs::String<'_>>| {
            *checkpoint.borrow_mut() = match value {
                Some(value) => {
                    let value = value.to_cstring()?;
                    if value.len() <= setup_config.max_response_bytes { serde_json::from_str(value.as_str()).ok() } else { None }
                },
                None => None,
            };
            Ok::<_, rquickjs::Error>(())
        })?;
        let controller: Object = factory.call((native, logger, env, cfg, checkpoint))?;
        Ok::<_, rquickjs::Error>(Persistent::save(&ctx, controller))
    });
    let mut setup = pin!(setup);
    let controller = poll_fn(|cx| {
        budget.segment.set(Some(Instant::now()));
        let result = setup.as_mut().poll(cx);
        budget.used.set(
            budget.used.get()
                + budget
                    .segment
                    .take()
                    .map_or(Duration::ZERO, |start| start.elapsed()),
        );
        result
    })
    .await
    .map_err(engine_error)?;

    let promise_controller = Rc::new(RefCell::new(Some(controller.clone())));
    let hook_controller = promise_controller.clone();
    runtime
        .set_promise_hook(Some(Box::new(move |ctx, kind, promise, parent| {
            let kind = match kind {
                PromiseHookType::Init => 0,
                PromiseHookType::Before => 1,
                PromiseHookType::After => 2,
                PromiseHookType::Resolve => 3,
            };
            if let Some(controller) = hook_controller.borrow().as_ref()
                && let Ok(controller) = controller.clone().restore(&ctx)
                && let Ok(hook) = controller.get::<_, Function>("promiseHook")
            {
                let _ = hook.call::<_, ()>((kind, promise, parent));
            }
        })))
        .await;
    let rejections = Rc::new(RefCell::new(HashSet::new()));
    let rejected = rejections.clone();
    let reject_cancel = cancellation.clone();
    runtime
        .set_host_promise_rejection_tracker(Some(Box::new(
            move |_ctx, promise, _reason, handled| {
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                promise.hash(&mut hash);
                let id = hash.finish();
                if handled {
                    rejected.borrow_mut().remove(&id);
                } else if rejected.borrow().len() < 1024 {
                    rejected.borrow_mut().insert(id);
                } else {
                    reject_cancel.cancel();
                }
            },
        )))
        .await;
    let running = async {
        for source in &snapshot.sources {
            let value = budget
                .meter(
                    &cancellation,
                    context.with(|ctx| {
                        (|| {
                            let controller = controller.clone().restore(&ctx)?;
                            controller
                                .get::<_, Function>("script")?
                                .call::<_, ()>((source.path.as_str(),))?;
                            let mut options = EvalOptions::default();
                            options.filename = Some(source.path.clone());
                            let value: Value =
                                ctx.eval_with_options(source.text.as_bytes(), options)?;
                            Ok(Persistent::save(&ctx, value))
                        })()
                        .map_err(|error| caught_error(&ctx, error))
                    }),
                )
                .await?;
            settle(&runtime, &context, &budget, &cancellation, value).await?;
            drain(&runtime, &budget, &cancellation).await?;
        }
        if !rejections.borrow().is_empty() {
            return Err(load_error("unhandled rejection during initialization"));
        }
        let registration_text = budget
            .meter(
                &cancellation,
                context.with(|ctx| {
                    (|| {
                        let controller = controller.clone().restore(&ctx)?;
                        let registrations: rquickjs::String =
                            controller.get::<_, Function>("registrations")?.call(())?;
                        registrations.to_string()
                    })()
                    .map_err(|error| caught_error(&ctx, error))
                }),
            )
            .await?;
        let registrations: Vec<Registration> =
            serde_json::from_str(&registration_text).map_err(load_error)?;
        // Initialization is validated before any invocation can perform host I/O,
        // including when the validated snapshot has an empty manifest.
        if invocation.is_some() && registrations != *snapshot.registrations {
            return Err(load_error(
                "initialization replay changed the registration manifest",
            ));
        }
        let output = if let Some(invocation) = invocation {
            let size = json_size(
                &invocation,
                config.max_bridge_bytes.saturating_sub(
                    request_bytes
                        .as_ref()
                        .map_or(0, |bytes| bytes.as_ref().len()),
                ),
            )?;
            let _input_permit = bridge.buffers.reserve(size)?;
            let input = bounded_json(&invocation, size)?;
            let _text_permit = request_bytes
                .as_ref()
                .map(|bytes| {
                    bridge
                        .buffers
                        .reserve(if std::str::from_utf8(bytes.as_ref()).is_ok() {
                            0
                        } else {
                            bytes.as_ref().len().saturating_mul(3)
                        })
                })
                .transpose()?;
            let request_text = request_bytes
                .as_ref()
                .map(|bytes| String::from_utf8_lossy(bytes.as_ref()));
            let value = budget
                .meter(
                    &cancellation,
                    context.with(|ctx| {
                        (|| {
                            let controller = controller.clone().restore(&ctx)?;
                            let bytes = request_bytes
                                .as_ref()
                                .map(|bytes| {
                                    TypedArray::<u8>::new_copy(ctx.clone(), bytes.as_ref())
                                })
                                .transpose()?;
                            let text = request_text
                                .as_ref()
                                .map(|text| rquickjs::String::from_str(ctx.clone(), text))
                                .transpose()?;
                            let promise: Promise = controller
                                .get::<_, Function>("run")?
                                .call((input, bytes, text))?;
                            Ok(Persistent::save(&ctx, promise.into_value()))
                        })()
                        .map_err(|error| caught_error(&ctx, error))
                    }),
                )
                .await?;
            drop(request_text);
            drop(_text_permit);
            let value = settle(&runtime, &context, &budget, &cancellation, value).await?;
            Some(
                budget
                    .meter(
                        &cancellation,
                        context.with(|ctx| {
                            let value = value.restore(&ctx).map_err(engine_error)?;
                            let result = value.into_object().ok_or(HbError::Internal)?;
                            let binary: Option<TypedArray<u8>> =
                                result.get("bytes").map_err(engine_error)?;
                            let binary = binary
                                .as_ref()
                                .map(|bytes| {
                                    let bytes = bytes.as_bytes().ok_or(HbError::Internal)?;
                                    if bytes.len() > config.max_response_bytes {
                                        return Err(HbError::PayloadTooLarge);
                                    }
                                    bridge.buffers.copy(bytes)
                                })
                                .transpose()?;
                            let value: rquickjs::String =
                                result.get("json").map_err(engine_error)?;
                            let value = value.to_cstring().map_err(engine_error)?;
                            if value.len()
                                > config.max_response_bytes.saturating_sub(
                                    binary.as_ref().map_or(0, |bytes| bytes.as_ref().len()),
                                )
                            {
                                return Err(HbError::PayloadTooLarge);
                            }
                            let permit = bridge.buffers.reserve(value.len())?;
                            Ok((value.as_str().to_owned(), binary, permit))
                        }),
                    )
                    .await?,
            )
        } else {
            None
        };
        // Drain remaining work through the same bounded driver; abandoned Promise
        // chains cannot silently escape validation or keep a worker alive forever.
        drain(&runtime, &budget, &cancellation).await?;
        Ok((registrations, output))
    };
    let result = tokio::select! {
        result = tokio::time::timeout_at(tokio::time::Instant::from_std(budget.deadline), running) => result.unwrap_or_else(|_| Err(JsErrorKind::Timeout.into())),
        _ = cancellation.cancelled() => Err(JsErrorKind::Timeout.into()),
    };
    if result.is_err() {
        cancellation.cancel();
        bridge.host.cancel();
    }
    // Runtime callbacks must release persistent context references before destruction.
    runtime.set_promise_hook(None).await;
    runtime.set_host_promise_rejection_tracker(None).await;
    promise_controller.borrow_mut().take();
    if let Some(value) = committed.borrow_mut().take() {
        tracing::error!(
            source = "jsvm",
            "afterCommit execution interrupted; root commit remains successful"
        );
        return Ok((snapshot.registrations.as_ref().clone(), value.into()));
    }
    if budget.exceeded.get() {
        return Err(JsErrorKind::Timeout.into());
    }
    let (registrations, output) = result?;
    if !rejections.borrow().is_empty() || bridge.host.pending() != 0 {
        return Err(JsErrorKind::Hook.into());
    }

    let output = output
        .map(|(value, binary, _permit)| {
            serde_json::from_str::<serde_json::Value>(&value).map(|value| (value, binary))
        })
        .transpose()
        .map_err(load_error)?
        .unwrap_or((json!({"ok":true,"value":null}), None));
    let (mut output, bytes) = output;
    if output["ok"] == true {
        Ok((
            registrations,
            HostReply {
                value: output["value"].take(),
                bytes,
            },
        ))
    } else {
        Err(decode_error(&output["error"]))
    }
}

fn engine_error(error: rquickjs::Error) -> HbError {
    JsError::diagnostic(
        if matches!(error, rquickjs::Error::Allocation) {
            JsErrorKind::Oom
        } else {
            JsErrorKind::Hook
        },
        error.to_string(),
    )
    .into()
}

fn decode_error(value: &serde_json::Value) -> HbError {
    let message = value["message"]
        .as_str()
        .unwrap_or("Extension failed")
        .to_owned();
    let details = value
        .get("details")
        .filter(|value| !value.is_null())
        .cloned();
    let code = value["code"].as_str().unwrap_or("HB_HOOK_ERROR");
    let kind = match code {
        "HB_CAPABILITY_DENIED" => JsErrorKind::Denied,
        "HB_HOOK_TIMEOUT" => JsErrorKind::Timeout,
        "HB_HOOK_OOM" => JsErrorKind::Oom,
        "HB_HOOK_ABORTED" => JsErrorKind::Aborted,
        "HB_HOOK_RECURSION" => JsErrorKind::Recursion,
        "HB_JS_BUSY" => JsErrorKind::Busy,
        "HB_SIDE_EFFECT_IN_TRANSACTION" => JsErrorKind::SideEffect,
        "HB_COMMIT_UNKNOWN" => JsErrorKind::CommitUnknown,
        "HB_OUTBOUND_DENIED" => JsErrorKind::OutboundDenied,
        "HB_HTTP_SEND_FAILED" => JsErrorKind::HttpFailed,
        "HB_HTTP_TIMEOUT" => JsErrorKind::HttpTimeout,
        "HB_FILE_ACCESS_DENIED" => JsErrorKind::FileDenied,
        "HB_FILE_MOVE_PARTIAL" => JsErrorKind::MovePartial,
        "HB_QUERY_UNSUPPORTED" => JsErrorKind::QueryUnsupported,
        "HB_IDEMPOTENCY_CONFLICT" => JsErrorKind::IdempotencyConflict,
        "HB_VALIDATION_ERROR" => return HbError::Validation { message, details },
        "HB_FORBIDDEN" => return HbError::Forbidden,
        "HB_NOT_FOUND" => return HbError::NotFound,
        "HB_RECORD_NOT_FOUND" => return HbError::RecordNotFound,
        "HB_COLLECTION_NOT_FOUND" => return HbError::CollectionNotFound(message),
        "HB_INVALID_FILTER" => return HbError::InvalidFilter(message),
        "HB_INVALID_SORT" => return HbError::InvalidSort(message),
        "HB_AUTH_REQUIRED" => return HbError::AuthRequired,
        "HB_UNAUTHORIZED" => return HbError::Unauthorized,
        "HB_TOKEN_EXPIRED" => return HbError::TokenExpired,
        "HB_ACCOUNT_LOCKED" => return HbError::AccountLocked,
        "HB_RATE_LIMITED" => return HbError::RateLimited,
        "HB_CONFLICT" => return HbError::Conflict(message),
        "HB_PAYLOAD_TOO_LARGE" => return HbError::PayloadTooLarge,
        "HB_UNSUPPORTED_MEDIA_TYPE" => return HbError::UnsupportedMediaType(message),
        "HB_RANGE_NOT_SATISFIABLE" => return HbError::RangeNotSatisfiable,
        "HB_DB_ERROR" => return HbError::Database(message),
        "HB_STORAGE_ERROR" => return HbError::Storage(message),
        "HB_INTERNAL_ERROR" => return HbError::Internal,
        "HB_CAPABILITY_UNAVAILABLE" => return HbError::CapabilityUnavailable,
        "HB_MAIL_TIMEOUT" => return HbError::MailTimeout,
        "HB_MAIL_SEND_FAILED" => return HbError::MailSendFailed,
        _ => JsErrorKind::Hook,
    };
    JsError {
        kind,
        message,
        details,
    }
    .into()
}

fn redact(value: &mut serde_json::Value, depth: usize) {
    if depth >= 8 {
        *value = json!("[truncated]");
        return;
    }
    match value {
        serde_json::Value::Object(fields) => {
            let excess: Vec<_> = fields.keys().skip(64).cloned().collect();
            for key in excess {
                fields.remove(&key);
            }
            for (key, value) in fields {
                let key = key.to_ascii_lowercase();
                if ["password", "token", "authorization", "secret", "cookie"]
                    .iter()
                    .any(|part| key.contains(part))
                {
                    *value = json!("[redacted]");
                } else {
                    redact(value, depth + 1);
                }
            }
        }
        serde_json::Value::Array(values) => {
            values.truncate(64);
            for value in values {
                redact(value, depth + 1);
            }
        }
        serde_json::Value::String(text) if text.len() > 4096 => {
            let end = text.floor_char_boundary(4096);
            text.truncate(end);
        }
        _ => {}
    }
}
