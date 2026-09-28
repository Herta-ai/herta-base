//! Durable upload intent and final attachment references. Storage is owned by the host.
use crate::{CollectionDef, DbClient, DbSession, FieldType, schema::database_error};
use herta_core::{HbResult, extension::RecordUpload};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadEntry {
    pub journal_id: String,
    pub operation_id: String,
    pub key: String,
    pub state_id: String,
    pub created: bool,
}

pub fn storage_key(collection: &str, id: &str, field: &str, filename: &str) -> String {
    let id = id.strip_prefix(&format!("{collection}:")).unwrap_or(id);
    format!("records/{collection}/{id}/{field}/{filename}")
}
pub fn state_id(operation: &str, collection: &str, id: &str) -> String {
    let id = id.strip_prefix(&format!("{collection}:")).unwrap_or(id);
    format!(
        "{:x}",
        Sha256::digest(format!("{operation}/{collection}/{id}"))
    )
}
pub fn attachment_keys(schema: &CollectionDef, id: &str, record: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    for field in schema
        .fields
        .iter()
        .filter(|field| field.field_type == FieldType::File)
    {
        if let Some(value) = record.get(&field.name) {
            for name in value.as_str().into_iter().chain(
                value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            ) {
                keys.push(storage_key(&schema.name, id, &field.name, name));
            }
        }
    }
    keys
}

pub async fn track(
    db: &DbClient,
    operation: &str,
    schema: &CollectionDef,
    id: &str,
    original: &Value,
    uploads: &[RecordUpload],
) -> HbResult<()> {
    let state_id = state_id(operation, &schema.name, id);
    let keys = attachment_keys(schema, id, original)
        .into_iter()
        .map(|key| (key, false))
        .chain(uploads.iter().map(|upload| {
            (
                storage_key(
                    &upload.collection,
                    &upload.record_id,
                    &upload.field,
                    &upload.filename,
                ),
                true,
            )
        }));
    for (key, created) in keys {
        let journal_id = format!(
            "{:x}",
            Sha256::digest(format!("{operation}/{key}/{created}"))
        );
        let entry = UploadEntry {
            journal_id: journal_id.clone(),
            operation_id: operation.into(),
            key,
            state_id: state_id.clone(),
            created,
        };
        db.inner()
            .query("UPSERT ONLY type::record('_record_uploads', $id) CONTENT $entry")
            .bind(("id", journal_id))
            .bind((
                "entry",
                serde_json::to_value(entry).map_err(database_error)?,
            ))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
    }
    Ok(())
}

pub async fn save_references(
    session: DbSession<'_>,
    operation: &str,
    schema: &CollectionDef,
    id: &str,
    record: &Value,
) -> HbResult<()> {
    session
        .query("UPSERT ONLY type::record('_record_attachment_states', $id) CONTENT $entry")
        .bind(("id", state_id(operation, &schema.name, id)))
        .bind(("entry", json!({"keys":attachment_keys(schema,id,record)})))
        .await
        .map_err(database_error)?
        .check()
        .map_err(database_error)?;
    Ok(())
}

pub async fn entries(
    db: &DbClient,
    operation: Option<&str>,
    offset: usize,
) -> HbResult<Vec<UploadEntry>> {
    let mut result = db.inner().query("SELECT * FROM _record_uploads WHERE $operation = NONE OR operationId = $operation ORDER BY id LIMIT 500 START $offset")
        .bind(("operation",operation.map(str::to_owned))).bind(("offset",offset))
        .await.map_err(database_error)?.check().map_err(database_error)?;
    let values: Vec<Value> = result.take(0).map_err(database_error)?;
    values
        .into_iter()
        .map(|value| serde_json::from_value(value).map_err(database_error))
        .collect()
}

/// None preserves an unresolved object's current storage state.
pub async fn should_delete(db: &DbClient, entry: &UploadEntry) -> HbResult<Option<bool>> {
    let operation: Option<Value> = db
        .inner()
        .select(("_operations", entry.operation_id.as_str()))
        .await
        .map_err(database_error)?;
    match operation.as_ref().and_then(|value| value["state"].as_str()) {
        Some("rolled_back") => Ok(Some(entry.created)),
        Some("committed") => {
            let state: Option<Value> = db
                .inner()
                .select(("_record_attachment_states", entry.state_id.as_str()))
                .await
                .map_err(database_error)?;
            // No marker means this record's core never ran in the transaction.
            Ok(Some(state.as_ref().map_or(entry.created, |value| {
                !value["keys"]
                    .as_array()
                    .is_some_and(|keys| keys.contains(&Value::String(entry.key.clone())))
            })))
        }
        _ => Ok(None),
    }
}
pub async fn resolved(db: &DbClient, entry: &UploadEntry) -> HbResult<()> {
    db.inner()
        .query("DELETE type::record('_record_uploads', $id)")
        .bind(("id", entry.journal_id.clone()))
        .await
        .map_err(database_error)?
        .check()
        .map_err(database_error)?;
    Ok(())
}
