//! Logical extension files over immutable Storage objects and a durable quota journal.
use bytes::Bytes;
use futures_util::StreamExt;
use herta_core::{
    HbError, HbResult, JsError, JsErrorKind,
    files::{FileItem, FilePage, FileResolve, validate_key},
    host_buffer::{HostBudget, HostBuffer, HostReply},
    jsvm::JsFilesConfig,
};
use herta_db::DbClient;
use herta_storage::Storage;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Clone)]
pub struct FileService {
    db: DbClient,
    storage: Arc<dyn Storage>,
    config: JsFilesConfig,
}

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    prefix: String,
    object: String,
    state: String,
    #[serde(flatten)]
    item: FileItem,
}

fn database(error: impl std::fmt::Display) -> HbError {
    HbError::Database(error.to_string())
}
fn encoded(value: impl Serialize) -> HbResult<Value> {
    serde_json::to_value(value).map_err(|_| HbError::Internal)
}
fn string<'a>(input: &'a Value, field: &str) -> HbResult<&'a str> {
    input[field]
        .as_str()
        .ok_or_else(|| HbError::validation(format!("missing {field}")))
}

impl FileService {
    pub async fn call_binary(
        &self,
        operation: &str,
        arguments: Value,
        bytes: Option<HostBuffer>,
        budget: HostBudget,
    ) -> HbResult<HostReply> {
        match (operation, bytes) {
            ("files.write", Some(bytes)) => {
                let _guard = self.db.file_operations_lock().await;
                if !arguments["text"].is_null() || !arguments["bytes"].is_null() {
                    return Err(HbError::validation(
                        "write requires exactly one of text or bytes",
                    ));
                }
                let key = string(&arguments, "key")?;
                let content_type = arguments
                    .get("contentType")
                    .map(|_| string(&arguments, "contentType"))
                    .transpose()?
                    .unwrap_or("application/octet-stream");
                self.write(key, Bytes::from_owner(bytes), content_type)
                    .await
                    .and_then(encoded)
                    .map(HostReply::from)
            }
            ("files.readBytes" | "files.response", None) => {
                let _guard = self.db.file_operations_lock().await;
                let entry = self.required(string(&arguments, "key")?).await?;
                if entry.item.size > self.config.max_file_bytes {
                    return Err(HbError::PayloadTooLarge);
                }
                let size =
                    usize::try_from(entry.item.size).map_err(|_| HbError::PayloadTooLarge)?;
                let mut bytes = budget.allocate(size)?;
                // Reserve room for the provider's current chunk before beginning I/O.
                // Our object readers cannot accept a chunk larger than the file.
                let _chunk = budget.reserve(size)?;
                self.read_into(&entry, bytes.as_mut()).await?;
                Ok(HostReply {
                    value: if operation == "files.response" {
                        json!({"headers":{
                            "content-type":entry.item.content_type,"x-content-type-options":"nosniff",
                            "etag":format!("\"{}\"",entry.item.version)
                        }})
                    } else {
                        Value::Null
                    },
                    bytes: Some(bytes),
                })
            }
            (_, None) => self.call(operation, arguments).await.map(HostReply::from),
            _ => Err(HbError::validation("unexpected binary argument")),
        }
    }

    pub fn new(db: DbClient, storage: Arc<dyn Storage>, config: JsFilesConfig) -> Self {
        Self {
            db,
            storage,
            config,
        }
    }

    /// Administrative views deliberately omit physical object locations.
    pub async fn operations(&self, after: &str, limit: usize) -> HbResult<Value> {
        if !(1..=500).contains(&limit)
            || (!after.is_empty() && uuid::Uuid::parse_str(after).is_err())
        {
            return Err(HbError::validation("invalid file operation page"));
        }
        let mut response = self.db.inner().query("SELECT key, size, contentType, version, updatedAt, state FROM _extension_file_versions WHERE prefix = $prefix AND state IN ['writing','uncertain'] AND version > $after ORDER BY version LIMIT $limit")
            .bind(("prefix", self.config.prefix.clone())).bind(("after", after.to_owned()))
            .bind(("limit", limit + 1)).await.map_err(database)?.check().map_err(database)?;
        let mut items: Vec<Value> = response.take(0).map_err(database)?;
        let more = items.len() > limit;
        items.truncate(limit);
        let cursor = more.then(|| items.last().unwrap()["version"].clone());
        Ok(json!({"items":items,"nextCursor":cursor}))
    }

    pub async fn operation(&self, version: &str) -> HbResult<Value> {
        if uuid::Uuid::parse_str(version).is_err() {
            return Err(HbError::NotFound);
        }
        if let Some(mut receipt) = self.resolution(version).await? {
            receipt
                .as_object_mut()
                .ok_or(HbError::Internal)?
                .remove("id");
            receipt.as_object_mut().unwrap().remove("prefix");
            return Ok(receipt);
        }
        let entry = self.version(version).await?;
        let mut value = encoded(&entry.item)?;
        value["state"] = json!(entry.state);
        Ok(value)
    }

    async fn version(&self, version: &str) -> HbResult<Entry> {
        let value: Option<Value> = self
            .db
            .inner()
            .select(("_extension_file_versions", version))
            .await
            .map_err(database)?;
        let entry: Entry =
            serde_json::from_value(value.ok_or(HbError::NotFound)?).map_err(database)?;
        if entry.prefix != self.config.prefix {
            return Err(HbError::NotFound);
        }
        Ok(entry)
    }

    async fn resolution(&self, version: &str) -> HbResult<Option<Value>> {
        let value: Option<Value> = self
            .db
            .inner()
            .select(("_extension_file_resolutions", version))
            .await
            .map_err(database)?;
        Ok(value.filter(|v| v["prefix"].as_str() == Some(&self.config.prefix)))
    }

    pub async fn resolve(
        &self,
        version: &str,
        input: FileResolve,
        administrator: String,
    ) -> HbResult<Value> {
        if uuid::Uuid::parse_str(version).is_err() {
            return Err(HbError::NotFound);
        }
        if input.note.trim().is_empty() || input.note.len() > 2000 {
            return Err(HbError::validation(
                "resolution requires a note of 1..2000 bytes",
            ));
        }
        // Accepted native writes retain this gate after their JS caller is cancelled.
        let _guard = self.db.file_operations_lock().await;
        if self.resolution(version).await?.is_some() {
            return self.operation(version).await;
        }
        let entry = self.version(version).await?;
        if !matches!(entry.state.as_str(), "writing" | "uncertain") {
            return Err(HbError::Conflict(
                "only uncertain file writes can be resolved".into(),
            ));
        }
        match self.storage.head(&entry.object).await {
            Err(HbError::NotFound) => {}
            Ok(_) => {
                return Err(HbError::Conflict(
                    "object exists; reconcile it before releasing quota".into(),
                ));
            }
            Err(error) => return Err(error),
        }
        let mut receipt = encoded(&entry.item)?;
        receipt["state"] = json!("released");
        receipt["resolution"] = json!("not_written");
        receipt["resolvedBy"] = json!(administrator);
        receipt["resolvedAt"] = json!(chrono::Utc::now().to_rfc3339());
        receipt["note"] = json!(input.note);
        let mut stored = receipt.clone();
        stored["prefix"] = json!(entry.prefix);
        // Auditing and releasing the reservation are one atomic change. A lost
        // acknowledgement is safe to retry using the immutable version id.
        self.db.inner().query("BEGIN TRANSACTION; CREATE ONLY type::record('_extension_file_resolutions', $version) CONTENT $receipt; DELETE type::record('_extension_file_versions', $version); COMMIT TRANSACTION")
            .bind(("version", version.to_owned())).bind(("receipt", stored))
            .await.map_err(database)?.check().map_err(database)?;
        Ok(receipt)
    }

    /// Hosts invoke mutations through TransactionOwner::side_effect, whose accepted
    /// task survives waiter cancellation. This gate is shared across all host factories.
    pub async fn call(&self, operation: &str, arguments: Value) -> HbResult<Value> {
        let _guard = self.db.file_operations_lock().await;
        match operation {
            "files.list" => self.list(&arguments).await.and_then(encoded),
            "files.exists" => Ok(json!(
                self.lookup(string(&arguments, "key")?).await?.is_some()
            )),
            "files.stat" => encoded(self.required(string(&arguments, "key")?).await?.item),
            "files.readBytes" | "files.readText" | "files.response" => {
                let entry = self.required(string(&arguments, "key")?).await?;
                let bytes = self.read(&entry).await?;
                match operation {
                    "files.readText" => String::from_utf8(bytes)
                        .map(Value::String)
                        .map_err(|_| HbError::validation("file is not UTF-8")),
                    "files.response" => Ok(json!({"bytes": bytes, "headers": {
                        "content-type":entry.item.content_type,
                        "x-content-type-options":"nosniff",
                        "etag":format!("\"{}\"",entry.item.version)
                    }})),
                    _ => Ok(json!(bytes)),
                }
            }
            "files.write" => {
                let key = string(&arguments, "key")?;
                validate_key(key, false)?;
                let bytes = match (&arguments["text"], &arguments["bytes"]) {
                    (Value::String(text), Value::Null) => Bytes::copy_from_slice(text.as_bytes()),
                    (Value::Null, Value::Array(_)) => Bytes::from(
                        serde_json::from_value::<Vec<u8>>(arguments["bytes"].clone())
                            .map_err(|_| HbError::validation("invalid byte array"))?,
                    ),
                    _ => {
                        return Err(HbError::validation(
                            "write requires exactly one of text or bytes",
                        ));
                    }
                };
                let content_type = arguments
                    .get("contentType")
                    .map(|_| string(&arguments, "contentType"))
                    .transpose()?
                    .unwrap_or("application/octet-stream");
                self.write(key, bytes, content_type).await.and_then(encoded)
            }
            "files.copy" | "files.move" => {
                let source = string(&arguments, "source")?;
                let destination = string(&arguments, "destination")?;
                validate_key(destination, false)?;
                if source == destination {
                    return Err(HbError::validation("source and destination must differ"));
                }
                let source = self.required(source).await?;
                let entry = self
                    .reserve(destination, source.item.size, &source.item.content_type)
                    .await?;
                let result = self
                    .storage
                    .copy(&source.object, &entry.object, self.config.max_file_bytes)
                    .await;
                let item = self
                    .complete_write(&entry, result.map(|metadata| metadata.size))
                    .await?;
                if operation == "files.move" && self.remove(&source).await.is_err() {
                    let state = match self.storage.head(&source.object).await {
                        Ok(_) => "present",
                        Err(HbError::NotFound) => "absent",
                        Err(_) => "unknown",
                    };
                    return Err(JsError {
                        kind: JsErrorKind::MovePartial,
                        message: JsErrorKind::MovePartial.message().into(),
                        details: Some(json!({"destination":item,"destinationExists":true,
                                "source":source.item.key,"sourceState":state})),
                    }
                    .into());
                }
                encoded(item)
            }
            "files.remove" => {
                if let Some(entry) = self.lookup(string(&arguments, "key")?).await? {
                    self.remove(&entry).await?;
                }
                Ok(Value::Null)
            }
            _ => Err(HbError::CapabilityUnavailable),
        }
    }

    fn id(&self, key: &str) -> String {
        format!(
            "{:x}",
            Sha256::digest(format!("{}/{}", self.config.prefix, key))
        )
    }

    async fn lookup(&self, key: &str) -> HbResult<Option<Entry>> {
        validate_key(key, false)?;
        let value: Option<Value> = self
            .db
            .inner()
            .select(("_extension_files", self.id(key)))
            .await
            .map_err(database)?;
        value
            .map(serde_json::from_value)
            .transpose()
            .map_err(database)
    }
    async fn required(&self, key: &str) -> HbResult<Entry> {
        self.lookup(key).await?.ok_or(HbError::NotFound)
    }

    async fn read(&self, entry: &Entry) -> HbResult<Vec<u8>> {
        if entry.item.size > self.config.max_file_bytes {
            return Err(HbError::PayloadTooLarge);
        }
        let size = usize::try_from(entry.item.size).map_err(|_| HbError::PayloadTooLarge)?;
        let mut bytes = vec![0; size];
        self.read_into(entry, &mut bytes).await?;
        Ok(bytes)
    }

    async fn read_into(&self, entry: &Entry, bytes: &mut [u8]) -> HbResult<()> {
        let mut object = self.storage.get(&entry.object, None).await?;
        if object.metadata.size != entry.item.size {
            return Err(HbError::Storage("file size changed".into()));
        }
        let mut offset = 0;
        while let Some(chunk) = object.stream.next().await {
            let chunk = chunk.map_err(|e| HbError::Storage(e.to_string()))?;
            if chunk.len() > bytes.len().saturating_sub(offset) {
                return Err(HbError::PayloadTooLarge);
            }
            bytes[offset..offset + chunk.len()].copy_from_slice(&chunk);
            offset += chunk.len();
        }
        if offset != bytes.len() {
            return Err(HbError::Storage("incomplete file".into()));
        }
        Ok(())
    }

    async fn list(&self, arguments: &Value) -> HbResult<FilePage> {
        let prefix = arguments
            .get("prefix")
            .map(|_| string(arguments, "prefix"))
            .transpose()?
            .unwrap_or("");
        if let Some(directory) = prefix.strip_suffix('/') {
            validate_key(directory, false)?;
        } else {
            validate_key(prefix, true)?;
        }
        let limit = arguments
            .get("limit")
            .map(|v| {
                v.as_u64()
                    .filter(|v| (1..=500).contains(v))
                    .ok_or_else(|| HbError::validation("file list limit must be 1..500"))
            })
            .transpose()?
            .unwrap_or(100);
        let after = match arguments.get("cursor").filter(|v| !v.is_null()) {
            None => String::new(),
            Some(Value::String(cursor)) if cursor.len() <= 4096 => {
                let (scope, filter, after): (String, String, String) = serde_json::from_str(cursor)
                    .map_err(|_| HbError::validation("invalid file cursor"))?;
                if scope != self.config.prefix || filter != prefix {
                    return Err(HbError::validation("file cursor belongs to another prefix"));
                }
                validate_key(&after, false)?;
                after
            }
            _ => return Err(HbError::validation("invalid file cursor")),
        };
        let mut result = self.db.inner().query("SELECT * FROM _extension_files WHERE prefix = $scope AND string::starts_with(key, $prefix) AND key > $after ORDER BY key LIMIT $limit")
            .bind(("scope",self.config.prefix.clone())).bind(("prefix",prefix.to_owned()))
            .bind(("after",after)).bind(("limit",limit+1)).await.map_err(database)?.check().map_err(database)?;
        let values: Vec<Value> = result.take(0).map_err(database)?;
        let mut entries: Vec<Entry> = values
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<_, _>>()
            .map_err(database)?;
        let more = entries.len() > limit as usize;
        entries.truncate(limit as usize);
        let next_cursor = if more {
            Some(
                serde_json::to_string(&(
                    &self.config.prefix,
                    prefix,
                    &entries.last().unwrap().item.key,
                ))
                .map_err(database)?,
            )
        } else {
            None
        };
        Ok(FilePage {
            items: entries.into_iter().map(|entry| entry.item).collect(),
            next_cursor,
        })
    }

    async fn reserve(&self, key: &str, size: u64, content_type: &str) -> HbResult<Entry> {
        validate_key(key, false)?;
        if size > self.config.max_file_bytes {
            return Err(HbError::PayloadTooLarge);
        }
        if content_type.len() > 256
            || content_type.chars().any(char::is_control)
            || content_type.parse::<mime::Mime>().is_err()
        {
            return Err(HbError::validation("invalid contentType"));
        }
        let version = uuid::Uuid::now_v7().to_string();
        let entry = Entry {
            prefix: self.config.prefix.clone(),
            object: format!("{}/objects/{version}", self.config.prefix),
            state: "writing".into(),
            item: FileItem {
                key: key.into(),
                size,
                content_type: content_type.into(),
                version: version.clone(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
        };
        let transaction = self.db.inner().clone().begin().await.map_err(database)?;
        let mut totals = transaction
            .query("SELECT math::sum(size) AS used FROM _extension_file_versions GROUP ALL")
            .await
            .map_err(database)?
            .check()
            .map_err(database)?;
        let totals: Vec<Value> = totals.take(0).map_err(database)?;
        let used = totals.first().and_then(|v| v["used"].as_u64()).unwrap_or(0);
        if size > self.config.quota_bytes.saturating_sub(used) {
            transaction.cancel().await.map_err(database)?;
            return Err(HbError::PayloadTooLarge);
        }
        transaction
            .query("CREATE ONLY type::record('_extension_file_versions', $version) CONTENT $entry")
            .bind(("version", version))
            .bind(("entry", encoded(&entry)?))
            .await
            .map_err(database)?
            .check()
            .map_err(database)?;
        transaction.commit().await.map_err(database)?;
        Ok(entry)
    }

    async fn write(&self, key: &str, bytes: Bytes, content_type: &str) -> HbResult<FileItem> {
        let entry = self.reserve(key, bytes.len() as u64, content_type).await?;
        let result = self.storage.put_bytes(&entry.object, bytes).await;
        self.complete_write(&entry, result.map(|metadata| metadata.size))
            .await
    }

    async fn complete_write(&self, entry: &Entry, result: HbResult<u64>) -> HbResult<FileItem> {
        if !matches!(&result, Ok(size) if *size == entry.item.size) {
            // A failed PUT/COPY may have been accepted. Retain both object intent and quota.
            self.db.inner().query("UPDATE type::record('_extension_file_versions', $version) SET state = 'uncertain'")
                .bind(("version",entry.item.version.clone())).await.map_err(database)?.check().map_err(database)?;
            return Err(result
                .err()
                .unwrap_or_else(|| HbError::Storage("stored size mismatch".into())));
        }
        let old = self.lookup(&entry.item.key).await?;
        let transaction = self.db.inner().clone().begin().await.map_err(database)?;
        if let Some(old) = &old {
            transaction.query("UPDATE type::record('_extension_file_versions', $version) SET state = 'garbage'")
                .bind(("version",old.item.version.clone())).await.map_err(database)?.check().map_err(database)?;
        }
        let mut live = entry.clone();
        live.state = "live".into();
        transaction.query("UPSERT ONLY type::record('_extension_files', $id) CONTENT $entry; UPDATE type::record('_extension_file_versions', $version) SET state = 'live'")
            .bind(("id",self.id(&entry.item.key))).bind(("entry",encoded(live)?))
            .bind(("version",entry.item.version.clone())).await.map_err(database)?.check().map_err(database)?;
        transaction.commit().await.map_err(database)?;
        if let Some(old) = old
            && let Err(error) = self.collect(&old).await
        {
            tracing::warn!(%error, "old extension file retained in quota journal");
        }
        Ok(entry.item.clone())
    }

    async fn remove(&self, entry: &Entry) -> HbResult<()> {
        self.db
            .inner()
            .query(
                "UPDATE type::record('_extension_file_versions', $version) SET state = 'deleting'",
            )
            .bind(("version", entry.item.version.clone()))
            .await
            .map_err(database)?
            .check()
            .map_err(database)?;
        self.delete_object(&entry.object).await?;
        self.db.inner().query("BEGIN TRANSACTION; DELETE _extension_files WHERE version = $version; DELETE type::record('_extension_file_versions', $version); COMMIT TRANSACTION")
            .bind(("version",entry.item.version.clone())).await.map_err(database)?.check().map_err(database)?;
        Ok(())
    }
    async fn delete_object(&self, object: &str) -> HbResult<()> {
        match self.storage.delete(object).await {
            Ok(()) | Err(HbError::NotFound) => Ok(()),
            Err(error) => Err(error),
        }
    }
    async fn collect(&self, entry: &Entry) -> HbResult<()> {
        self.delete_object(&entry.object).await?;
        self.db
            .inner()
            .query("DELETE type::record('_extension_file_versions', $version)")
            .bind(("version", entry.item.version.clone()))
            .await
            .map_err(database)?
            .check()
            .map_err(database)?;
        Ok(())
    }

    /// Bounded keyset scans; a failed object never prevents later entries being checked.
    /// No pending writer exists while this gate is held. Missing uncertain remote
    /// objects retain their reservations: absence alone does not prove a PUT cannot arrive.
    pub async fn reconcile(&self) -> HbResult<()> {
        let _guard = self.db.file_operations_lock().await;
        let mut after = String::new();
        loop {
            let mut response = self.db.inner().query("SELECT * FROM _extension_file_versions WHERE version > $after AND state != 'live' ORDER BY version LIMIT 100")
                .bind(("after",after.clone())).await.map_err(database)?.check().map_err(database)?;
            let values: Vec<Value> = response.take(0).map_err(database)?;
            let entries: Vec<Entry> = values
                .into_iter()
                .map(serde_json::from_value)
                .collect::<Result<_, _>>()
                .map_err(database)?;
            if entries.is_empty() {
                break;
            }
            for entry in entries {
                after = entry.item.version.clone();
                let result = match entry.state.as_str() {
                    "deleting" => self.remove(&entry).await,
                    "garbage" => self.collect(&entry).await,
                    "writing" | "uncertain" => match self.storage.head(&entry.object).await {
                        Ok(_) => self.collect(&entry).await,
                        Err(HbError::NotFound) => continue,
                        Err(error) => Err(error),
                    },
                    _ => Err(HbError::Internal),
                };
                if let Err(error) = result {
                    tracing::warn!(%error,version=%entry.item.version,"extension file reconciliation deferred");
                }
            }
        }
        self.reconcile_orphans().await
    }

    /// Called under the shared writer gate. Only our generated physical object and
    /// temporary filenames qualify; an unrelated object is never garbage-collected.
    async fn reconcile_orphans(&self) -> HbResult<()> {
        let prefix = format!("{}/objects", self.config.prefix);
        let mut cursor = None;
        loop {
            let page = match self
                .storage
                .list_page(&prefix, 100, cursor.as_deref())
                .await
            {
                Ok(page) => page,
                // Older custom Storage adapters can still recover their operation ledger.
                Err(HbError::CapabilityUnavailable) => return Ok(()),
                Err(error) => return Err(error),
            };
            for (object, metadata) in page.items {
                let Some(name) = object.strip_prefix(&format!("{prefix}/")) else {
                    continue;
                };
                let identifier = name
                    .strip_prefix(".hb-")
                    .and_then(|s| s.strip_suffix(".tmp"))
                    .unwrap_or(name);
                if !uuid::Uuid::parse_str(identifier)
                    .is_ok_and(|id| id.get_version_num() == 7 && id.to_string() == identifier)
                {
                    continue;
                }
                // Check both tables: a missing version row must never destroy a live file.
                let mut response = self.db.inner().query("SELECT VALUE object FROM _extension_file_versions WHERE object = $object LIMIT 1; SELECT VALUE object FROM _extension_files WHERE object = $object LIMIT 1")
                    .bind(("object", object.clone())).await.map_err(database)?.check().map_err(database)?;
                let versions: Vec<String> = response.take(0).map_err(database)?;
                let live: Vec<String> = response.take(1).map_err(database)?;
                if !versions.is_empty() || !live.is_empty() {
                    continue;
                }
                // Persist the orphan's full size before deletion. Failed deletes retain
                // quota and become ordinary restart-recoverable garbage operations.
                let version = uuid::Uuid::now_v7().to_string();
                let entry = Entry {
                    prefix: self.config.prefix.clone(),
                    object,
                    state: "garbage".into(),
                    item: FileItem {
                        key: format!("__orphan/{version}"),
                        size: metadata.size,
                        content_type: "application/octet-stream".into(),
                        version,
                        updated_at: chrono::Utc::now().to_rfc3339(),
                    },
                };
                self.db.inner().query("CREATE ONLY type::record('_extension_file_versions', $version) CONTENT $entry")
                    .bind(("version",entry.item.version.clone())).bind(("entry",encoded(&entry)?))
                    .await.map_err(database)?.check().map_err(database)?;
                if let Err(error) = self.collect(&entry).await {
                    tracing::warn!(%error,version=%entry.item.version,"orphan extension file retained in quota journal");
                }
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(());
            }
        }
    }
}
