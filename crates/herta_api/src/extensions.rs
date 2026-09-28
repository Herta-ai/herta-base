//! Host composition used by the server. No service handle retains a dispatcher.
use herta_auth::{AuthIdentity, AuthResponse, PreparedAuth};
use herta_core::{
    HbConfig, HbError, HbResult, JsErrorKind, MailMessage, Mailer,
    extension::{AuthMode, HostCall, HostContext, HostFactory, HostServices},
    host_buffer::{HostBudget, HostBuffer, HostReply},
};
use herta_db::{DbClient, RuleContext, commands::DatabaseCommands};
use herta_storage::Storage;
use salvo::async_trait;
use serde_json::Value;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub struct ApiHostFactory {
    db: DbClient,
    config: Arc<HbConfig>,
    mailer: Arc<dyn Mailer>,
    storage: Arc<dyn Storage>,
    docs: crate::docs::OpenApiCache,
    messages: Option<crate::messages::RealtimeBus>,
}
impl ApiHostFactory {
    pub fn new(
        db: DbClient,
        config: Arc<HbConfig>,
        mailer: Arc<dyn Mailer>,
        storage: Arc<dyn Storage>,
    ) -> Self {
        Self {
            db,
            config,
            mailer,
            storage,
            docs: crate::docs::OpenApiCache::empty(),
            messages: None,
        }
    }
    pub fn with_docs(mut self, docs: crate::docs::OpenApiCache) -> Self {
        self.docs = docs;
        self
    }
    pub fn with_messages(mut self, messages: crate::messages::RealtimeBus) -> Self {
        self.messages = Some(messages);
        self
    }
    fn host(&self, context: HostContext) -> ApiHost {
        ApiHost {
            database: DatabaseCommands::new(
                self.db.clone(),
                RuleContext {
                    admin: context.admin,
                    auth: context.auth,
                    auth_record: context
                        .principal
                        .as_deref()
                        .and_then(|id| herta_db::record::parse_record_id(id).ok()),
                    request_body: context.request_body,
                },
                context.mode,
                context.principal,
                self.config.jsvm.raw_query_enabled,
            )
            .with_uploads(context.uploads.clone()),
            config: self.config.clone(),
            mailer: self.mailer.clone(),
            db: self.db.clone(),
            storage: self.storage.clone(),
            uploads: Arc::new(tokio::sync::Mutex::new(context.uploads)),
            operations: Mutex::new(Vec::new()),
            auth: None,
            docs: self.docs.clone(),
            retry_safe: Arc::new(AtomicBool::new(true)),
            messages: self.messages.clone(),
        }
    }

    /// Only the native AuthService can provide this one-use completion.
    pub fn create_auth(
        &self,
        context: HostContext,
        prepared: PreparedAuth,
    ) -> (Arc<dyn HostServices>, AuthOutput) {
        let request = RuleContext {
            admin: context.admin,
            auth: context.auth.clone(),
            auth_record: context
                .principal
                .as_deref()
                .and_then(|id| herta_db::record::parse_record_id(id).ok()),
            request_body: context.request_body.clone(),
        };
        let output = AuthOutput::default();
        let mut host = self.host(context);
        host.auth = Some(AuthCompletion {
            prepared: Mutex::new(Some(prepared)),
            request,
            output: output.clone(),
        });
        (Arc::new(host), output)
    }
}
impl HostFactory for ApiHostFactory {
    fn create(&self, context: HostContext) -> Arc<dyn HostServices> {
        Arc::new(self.host(context))
    }
}

#[derive(Clone, Default)]
pub struct AuthOutput(Arc<Mutex<(Option<AuthResponse>, bool)>>);
impl AuthOutput {
    pub fn take(&self) -> HbResult<AuthResponse> {
        let mut output = self.0.lock().map_err(|_| HbError::Internal)?;
        if !output.1 {
            return Err(JsErrorKind::Aborted.into());
        }
        output.0.take().ok_or(HbError::Internal)
    }
}
struct AuthCompletion {
    prepared: Mutex<Option<PreparedAuth>>,
    request: RuleContext,
    output: AuthOutput,
}

pub fn request_context(identity: &AuthIdentity, body: Value) -> HostContext {
    HostContext {
        mode: AuthMode::Request,
        principal: identity.record_id().map(str::to_owned),
        admin: identity.is_admin(),
        auth: identity.as_rule_value(),
        request_body: body,
        uploads: Vec::new(),
    }
}

struct ApiHost {
    database: DatabaseCommands,
    config: Arc<HbConfig>,
    mailer: Arc<dyn Mailer>,
    db: DbClient,
    storage: Arc<dyn Storage>,
    uploads: Arc<tokio::sync::Mutex<Vec<herta_core::extension::RecordUpload>>>,
    operations: Mutex<Vec<String>>,
    auth: Option<AuthCompletion>,
    docs: crate::docs::OpenApiCache,
    retry_safe: Arc<AtomicBool>,
    messages: Option<crate::messages::RealtimeBus>,
}

#[async_trait]
impl HostServices for ApiHost {
    async fn call_binary(
        &self,
        call: HostCall,
        bytes: Option<HostBuffer>,
        budget: HostBudget,
    ) -> HbResult<HostReply> {
        if matches!(
            call.operation.as_str(),
            "files.write" | "files.readBytes" | "files.response"
        ) {
            if !self.config.jsvm.files.enabled {
                return Err(JsErrorKind::Denied.into());
            }
            let files = crate::files::FileService::new(
                self.db.clone(),
                self.storage.clone(),
                self.config.jsvm.files.clone(),
            );
            if call.operation == "files.write" {
                let retry_safe = self.retry_safe.clone();
                self.database
                    .owner
                    .side_effect(async move {
                        retry_safe.store(false, Ordering::Release);
                        files
                            .call_binary(&call.operation, call.arguments, bytes, budget)
                            .await
                    })
                    .await
            } else {
                self.database
                    .owner
                    .execute(call.transaction, false, move |_| {
                        Box::pin(async move {
                            tokio::time::timeout(
                                Duration::from_millis(call.remaining_ms),
                                files.call_binary(&call.operation, call.arguments, bytes, budget),
                            )
                            .await
                            .map_err(|_| HbError::from(JsErrorKind::Timeout))?
                        })
                    })
                    .await
            }
        } else if bytes.is_some() {
            Err(HbError::validation("unexpected binary argument"))
        } else {
            self.call(call).await.map(HostReply::from)
        }
    }

    async fn call(&self, call: HostCall) -> HbResult<Value> {
        match call.operation.as_str() {
            "auth.complete" => {
                let auth = self.auth.as_ref().ok_or(HbError::Forbidden)?;
                let prepared = auth.prepared.lock().map_err(|_| HbError::Internal)?.take();
                let output = auth.output.clone();
                let request = auth.request.clone();
                self.database
                    .owner
                    .execute(call.transaction, true, move |session| {
                        Box::pin(async move {
                            let prepared = prepared.ok_or_else(|| {
                                HbError::Conflict("Auth core already executed".into())
                            })?;
                            let response = prepared
                                .execute(session, call.arguments["profile"].clone(), &request)
                                .await?;
                            let account = serde_json::to_value(&response.user)
                                .map_err(|_| HbError::Internal)?;
                            output.0.lock().map_err(|_| HbError::Internal)?.0 = Some(response);
                            Ok(account)
                        })
                    })
                    .await
            }
            "transaction.commit" => {
                let result = self.database.call(call).await;
                if result
                    .as_ref()
                    .is_err_and(|error| error.error_code() == "HB_COMMIT_UNKNOWN")
                {
                    self.retry_safe.store(false, Ordering::Release);
                }
                let value = result?;
                if let Some(auth) = &self.auth {
                    let mut output = auth.output.0.lock().map_err(|_| HbError::Internal)?;
                    if output.0.is_some() {
                        output.1 = true;
                    }
                }
                Ok(value)
            }
            "transaction.begin" => {
                let receipt = self.database.call(call).await?;
                if let Some(id) = receipt["id"].as_str() {
                    self.operations
                        .lock()
                        .map_err(|_| HbError::Internal)?
                        .push(id.into());
                }
                Ok(receipt)
            }
            "record.prepare" => {
                let transaction = call.transaction.clone();
                let id = call.arguments["id"]
                    .as_str()
                    .ok_or_else(|| HbError::validation("missing record ID"))?
                    .to_owned();
                let prepared = self.database.call(call).await?;
                let schema: herta_db::CollectionDef =
                    serde_json::from_value(prepared["collection"].clone())
                        .map_err(|_| HbError::Internal)?;
                let original = prepared["original"].clone();
                let operation = transaction.clone().ok_or(HbError::Internal)?;
                let db = self.db.clone();
                let storage = self.storage.clone();
                let uploads = self.uploads.clone();
                self.database
                    .owner
                    .execute(transaction, true, move |_session| {
                        Box::pin(async move {
                            let mut pending = uploads.lock().await;
                            let matching: Vec<_> = pending
                                .iter()
                                .filter(|upload| {
                                    upload.collection == schema.name && upload.record_id == id
                                })
                                .cloned()
                                .collect();
                            herta_db::uploads::track(
                                &db, &operation, &schema, &id, &original, &matching,
                            )
                            .await?;
                            for upload in &matching {
                                let key = herta_db::uploads::storage_key(
                                    &upload.collection,
                                    &upload.record_id,
                                    &upload.field,
                                    &upload.filename,
                                );
                                storage.put_file(&key, &upload.source).await?;
                            }
                            pending.retain(|upload| {
                                upload.collection != schema.name || upload.record_id != id
                            });
                            Ok(())
                        })
                    })
                    .await?;
                Ok(prepared)
            }
            "mail.send" => {
                if !self.config.jsvm.mail.enabled {
                    return Err(JsErrorKind::Denied.into());
                }
                if self.config.mail.driver == "disabled" {
                    return Err(HbError::CapabilityUnavailable);
                }
                let mailer = self.mailer.clone();
                let config = self.config.clone();
                let retry_safe = self.retry_safe.clone();
                self.database
                    .owner
                    .side_effect(async move {
                        let message: MailMessage = serde_json::from_value(call.arguments)
                            .map_err(|error| HbError::validation(error.to_string()))?;
                        let mut limits = config.mail.clone();
                        limits.max_recipients =
                            limits.max_recipients.min(config.jsvm.mail.max_recipients);
                        limits.max_body_bytes =
                            limits.max_body_bytes.min(config.jsvm.mail.max_body_bytes);
                        let message = limits.prepare(message)?;
                        // SMTP does not provide receiver idempotency. Even an accepted
                        // message must not be repeated by a later failed cron attempt.
                        retry_safe.store(false, Ordering::Release);
                        let receipt = tokio::time::timeout(
                            Duration::from_millis(call.remaining_ms.min(config.mail.timeout_ms)),
                            mailer.send(message),
                        )
                        .await
                        .map_err(|_| HbError::MailTimeout)??;
                        serde_json::to_value(receipt).map_err(|_| HbError::Internal)
                    })
                    .await
            }
            "http.send" => {
                if !self.config.jsvm.http.enabled {
                    return Err(JsErrorKind::Denied.into());
                }
                let config = self.config.jsvm.http.clone();
                let retry_safe = self.retry_safe.clone();
                self.database
                    .owner
                    .side_effect(async move {
                        let request = serde_json::from_value(call.arguments).map_err(|error| {
                            HbError::validation(format!("invalid HTTP request: {error}"))
                        })?;
                        let result = herta_http::HttpService::new(config)?
                            .send(request, call.remaining_ms)
                            .await;
                        if !result
                            .as_ref()
                            .is_err_and(|error| error.delivery == herta_http::Delivery::NotSent)
                        {
                            retry_safe.store(false, Ordering::Release);
                        }
                        let response = result.map_err(HbError::from)?;
                        serde_json::to_value(response).map_err(|_| HbError::Internal)
                    })
                    .await
            }
            "realtime.publish" => {
                if !self.config.jsvm.realtime.enabled {
                    return Err(JsErrorKind::Denied.into());
                }
                let messages = self
                    .messages
                    .clone()
                    .ok_or(HbError::CapabilityUnavailable)?;
                self.database
                    .owner
                    .side_effect(async move {
                        let request = serde_json::from_value(call.arguments)
                            .map_err(|_| HbError::validation("invalid application message"))?;
                        let receipt = tokio::time::timeout(
                            Duration::from_millis(call.remaining_ms),
                            messages.publish(request),
                        )
                        .await
                        .map_err(|_| HbError::from(JsErrorKind::Timeout))??;
                        serde_json::to_value(receipt).map_err(|_| HbError::Internal)
                    })
                    .await
            }
            "outbox.enqueue" => {
                let config = self.config.clone();
                self.database
                    .owner
                    .execute(call.transaction, true, move |session| {
                        Box::pin(async move {
                            let input = tokio::time::timeout(
                                Duration::from_millis(call.remaining_ms),
                                crate::outbox::validate(&config, call.arguments),
                            )
                            .await
                            .map_err(|_| HbError::from(JsErrorKind::Timeout))??;
                            let receipt = herta_db::outbox::enqueue(
                                session,
                                input,
                                config.jsvm.outbox.max_jobs,
                            )
                            .await?;
                            serde_json::to_value(receipt).map_err(|_| HbError::Internal)
                        })
                    })
                    .await
            }
            operation if operation.starts_with("files.") => {
                if !self.config.jsvm.files.enabled {
                    return Err(JsErrorKind::Denied.into());
                }
                let files = crate::files::FileService::new(
                    self.db.clone(),
                    self.storage.clone(),
                    self.config.jsvm.files.clone(),
                );
                let operation = operation.to_owned();
                if matches!(
                    operation.as_str(),
                    "files.write" | "files.copy" | "files.move" | "files.remove"
                ) {
                    let retry_safe = self.retry_safe.clone();
                    self.database
                        .owner
                        .side_effect(async move {
                            retry_safe.store(false, Ordering::Release);
                            files.call(&operation, call.arguments).await
                        })
                        .await
                } else {
                    self.database
                        .owner
                        .execute(call.transaction, false, move |_| {
                            Box::pin(async move {
                                tokio::time::timeout(
                                    Duration::from_millis(call.remaining_ms),
                                    files.call(&operation, call.arguments),
                                )
                                .await
                                .map_err(|_| HbError::from(JsErrorKind::Timeout))?
                            })
                        })
                        .await
                }
            }
            operation
                if operation.starts_with("http.")
                    || operation.starts_with("files.")
                    || operation.starts_with("realtime.")
                    || operation.starts_with("outbox.") =>
            {
                let enabled = if operation.starts_with("http.") {
                    self.config.jsvm.http.enabled
                } else if operation.starts_with("files.") {
                    self.config.jsvm.files.enabled
                } else if operation.starts_with("realtime.") {
                    self.config.jsvm.realtime.enabled
                } else {
                    self.config.jsvm.outbox.enabled
                };
                Err(if enabled {
                    HbError::CapabilityUnavailable
                } else {
                    JsErrorKind::Denied.into()
                })
            }
            _ => self.database.call(call).await,
        }
    }
    fn cancel(&self) {
        self.database.owner.cancel();
    }
    fn retry_safe(&self) -> bool {
        self.retry_safe.load(Ordering::Acquire)
    }
    fn interrupted_commit(&self) -> Option<HbError> {
        self.database.owner.interrupted_commit()
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        let result = self.database.owner.finish().await;
        let operations = self
            .operations
            .lock()
            .map_err(|_| HbError::Internal)?
            .clone();
        for operation in operations {
            if let Err(error) =
                reconcile_uploads(&self.db, self.storage.as_ref(), Some(&operation)).await
            {
                tracing::error!(%operation,%error,"record upload cleanup remains in the durable journal");
            }
        }
        if result.is_ok() {
            match tokio::time::timeout(
                Duration::from_secs(5),
                reconcile_collections(&self.db, self.storage.as_ref(), &self.docs),
            )
            .await
            {
                Ok(Ok(())) => {}
                outcome => tracing::error!(
                    ?outcome,
                    "collection post-commit work remains in its durable journal"
                ),
            }
        }
        result
    }
}

/// The journal is written with DDL. A failed cleanup blocks reuse of that name;
/// a restart resumes idempotent cleanup before accepting requests.
pub async fn reconcile_collections(
    db: &DbClient,
    storage: &dyn Storage,
    docs: &crate::docs::OpenApiCache,
) -> HbResult<()> {
    let _guard = docs.collection_effects_lock().await;
    loop {
        let mut response = db
            .inner()
            .query("SELECT * FROM _collection_effects LIMIT 100")
            .await
            .map_err(|error| HbError::Database(error.to_string()))?
            .check()
            .map_err(|error| HbError::Database(error.to_string()))?;
        let rows: Vec<Value> = response
            .take(0)
            .map_err(|error| HbError::Database(error.to_string()))?;
        if rows.is_empty() {
            return Ok(());
        }
        docs.refresh_db(db).await?;
        for row in rows {
            let name = row["name"].as_str().ok_or(HbError::Internal)?;
            if row["cleanup"].as_bool() == Some(true) {
                storage.delete_prefix(&format!("records/{name}")).await?;
            }
            db.inner()
                .query("DELETE _collection_effects WHERE operation = $operation AND name = $name")
                .bind(("operation", row["operation"].clone()))
                .bind(("name", name.to_owned()))
                .await
                .map_err(|error| HbError::Database(error.to_string()))?
                .check()
                .map_err(|error| HbError::Database(error.to_string()))?;
        }
    }
}

/// Run before accepting work at startup, or for a single finished owner.
pub async fn reconcile_uploads(
    db: &DbClient,
    storage: &dyn Storage,
    operation: Option<&str>,
) -> HbResult<()> {
    let mut offset = 0;
    loop {
        let entries = herta_db::uploads::entries(db, operation, offset).await?;
        if entries.is_empty() {
            return Ok(());
        }
        for entry in entries {
            if let Some(delete) = herta_db::uploads::should_delete(db, &entry).await? {
                if delete {
                    storage.delete(&entry.key).await?;
                }
                herta_db::uploads::resolved(db, &entry).await?;
            } else {
                offset += 1;
            }
        }
    }
}
