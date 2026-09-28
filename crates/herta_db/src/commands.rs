//! Database-only extension commands. The server composes this with other host
//! services; nested hooks stay in the JS worker and never re-enter a dispatcher.

use crate::{
    CollectionDef, CollectionType, DbClient, FieldType, RecordManager, RecordQuery, RuleContext,
    SchemaManager, transaction::TransactionOwner, validation::validate_record,
};
use herta_core::{
    HbError, HbResult, JsErrorKind,
    extension::{AuthMode, HostCall},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Clone)]
pub struct DatabaseCommands {
    pub owner: TransactionOwner,
    request: RuleContext,
    root_mode: AuthMode,
    raw_query_enabled: bool,
    uploads: std::sync::Arc<Vec<herta_core::extension::RecordUpload>>,
}

impl DatabaseCommands {
    pub fn new(
        db: DbClient,
        request: RuleContext,
        root_mode: AuthMode,
        principal: Option<String>,
        raw_query_enabled: bool,
    ) -> Self {
        Self {
            owner: TransactionOwner::new(db, principal),
            request,
            root_mode,
            raw_query_enabled,
            uploads: Default::default(),
        }
    }

    pub fn with_uploads(mut self, uploads: Vec<herta_core::extension::RecordUpload>) -> Self {
        self.uploads = std::sync::Arc::new(uploads);
        self
    }

    fn context(&self, mode: AuthMode) -> RuleContext {
        if self.root_mode == AuthMode::Request || mode == AuthMode::Request {
            self.request.clone()
        } else {
            RuleContext {
                admin: true,
                auth: json!({"admin":true,"role":"admin"}),
                auth_record: None,
                request_body: self.request.request_body.clone(),
            }
        }
    }

    pub async fn call(&self, call: HostCall) -> HbResult<Value> {
        let context = self.context(call.auth_mode);
        match call.operation.as_str() {
            operation if operation.starts_with("collections.") => {
                let persistent = matches!(operation, "collections.prepare" | "collections.write");
                let operation = operation.to_owned();
                let transaction = call.transaction.clone();
                let result = self
                    .owner
                    .execute(transaction.clone(), persistent, move |session| {
                        Box::pin(async move {
                            if !context.admin {
                                return Err(HbError::Forbidden);
                            }
                            let service = crate::collections::CollectionService::new(session);
                            let value = call.arguments["value"].clone();
                            match operation.as_str() {
                                "collections.findByName" => {
                                    service
                                        .get(value.as_str().ok_or_else(|| {
                                            HbError::validation("collection name is required")
                                        })?)
                                        .await
                                }
                                "collections.list" => service.list().await,
                                "collections.prepare" => {
                                    service
                                        .prepare(
                                            call.arguments["action"]
                                                .as_str()
                                                .ok_or(HbError::Internal)?,
                                            value,
                                            call.arguments["patch"].as_bool().unwrap_or(false),
                                        )
                                        .await
                                }
                                "collections.write" => {
                                    service
                                        .write(
                                            transaction.as_deref().ok_or(HbError::Internal)?,
                                            call.arguments["action"]
                                                .as_str()
                                                .ok_or(HbError::Internal)?,
                                            call.arguments["original"].clone(),
                                            call.arguments["candidate"].clone(),
                                        )
                                        .await
                                }
                                _ => Err(HbError::CapabilityUnavailable),
                            }
                        })
                    })
                    .await;
                if persistent && result.is_err() {
                    self.owner.mark_rollback_only().await;
                }
                result
            }
            "transaction.begin" => serde_json::to_value(self.owner.begin().await?)
                .map_err(crate::schema::database_error),
            "transaction.commit" => {
                self.owner
                    .commit(
                        call.transaction.as_deref().ok_or_else(|| {
                            HbError::Conflict("missing transaction identity".into())
                        })?,
                    )
                    .await?;
                Ok(Value::Null)
            }
            "transaction.rollback" => {
                self.owner.finish().await?;
                Ok(Value::Null)
            }
            "transaction.markRollbackOnly" => {
                self.owner.mark_rollback_only().await;
                Ok(Value::Null)
            }
            "record.find" => {
                let arguments: Find = decode(call.arguments)?;
                self.owner
                    .execute(call.transaction, false, move |session| {
                        Box::pin(async move {
                            RecordManager::new(session)
                                .get_authorized(
                                    &arguments.collection,
                                    &arguments.id,
                                    arguments.expand.as_deref(),
                                    &context,
                                )
                                .await
                        })
                    })
                    .await
            }
            "record.list" | "record.listPage" => {
                let page = call.operation == "record.listPage";
                let arguments: List = decode(call.arguments)?;
                self.owner
                    .execute(call.transaction, false, move |session| {
                        Box::pin(async move {
                            let (rows, total) = RecordManager::new(session)
                                .query_authorized(&arguments.collection, &arguments.query, &context)
                                .await?;
                            Ok(if page {
                                json!({"rows":rows,"total":total})
                            } else {
                                Value::Array(rows)
                            })
                        })
                    })
                    .await
            }
            "db.query" => {
                if !self.raw_query_enabled {
                    return Err(JsErrorKind::Denied.into());
                }
                let arguments: Select = decode(call.arguments)?;
                self.owner
                    .execute(call.transaction, false, move |session| {
                        Box::pin(async move {
                            RecordManager::new(session)
                                .select_authorized(&arguments.query, &arguments.bindings, &context)
                                .await
                                .map(Value::Array)
                        })
                    })
                    .await
            }
            "record.prepare" | "record.write" => {
                // Decode/validation failure also poisons the owning transaction.
                let result = self.mutate(call, context).await;
                if result.is_err() {
                    self.owner.mark_rollback_only().await;
                }
                result
            }
            _ => Err(HbError::CapabilityUnavailable),
        }
    }

    async fn mutate(&self, call: HostCall, context: RuleContext) -> HbResult<Value> {
        let input: Mutation = decode(call.arguments)?;
        let prepare = call.operation == "record.prepare";
        let operation = call
            .transaction
            .clone()
            .ok_or_else(|| HbError::Conflict("record writes need a transaction".into()))?;
        let uploads = self.uploads.clone();
        self.owner
            .execute(call.transaction, true, move |session| {
                Box::pin(async move {
                    let schema = SchemaManager::new(session)
                        .get_collection(&input.collection)
                        .await?;
                    let manager = RecordManager::new(session);
                    let original = if input.action == "create" {
                        None
                    } else {
                        Some(
                            manager
                                .get_authorized(&input.collection, &input.id, None, &context)
                                .await?,
                        )
                    };
                    if !matches!(input.action.as_str(), "create" | "update" | "delete") {
                        return Err(HbError::validation("unknown record action"));
                    }
                    check_patch_safety(
                        &schema,
                        &input.dirty,
                        &input.unset,
                        original.as_ref(),
                        &input.id,
                        &uploads,
                    )?;
                    let mut candidate = original.clone().unwrap_or_else(|| json!({"id":input.id}));
                    let object = candidate.as_object_mut().ok_or(HbError::Internal)?;
                    for (field, value) in &input.dirty {
                        object.insert(field.clone(), value.clone());
                    }
                    for field in &input.unset {
                        object.remove(field);
                    }
                    if prepare {
                        // Required/type validation is deliberately deferred until the
                        // entire pre-next hook chain has built its final candidate.
                        return Ok(
                            json!({"candidate":candidate,"original":original,"collection":schema}),
                        );
                    }
                    if input.action == "delete" {
                        let result = manager
                            .delete_authorized(&input.collection, &input.id, &context)
                            .await?;
                        crate::uploads::save_references(
                            session,
                            &operation,
                            &schema,
                            &input.id,
                            &Value::Null,
                        )
                        .await?;
                        return Ok(result);
                    }
                    let object = candidate.as_object_mut().ok_or(HbError::Internal)?;
                    for field in SYSTEM_FIELDS {
                        object.remove(field);
                    }
                    if schema.collection_type == CollectionType::Auth {
                        for &field in herta_core::models::AUTH_MANAGED_FIELDS {
                            object.remove(field);
                        }
                    }
                    validate_record(&schema, &mut candidate, true)?;
                    // Relation targets created by earlier hooks are visible in this session.
                    for field in schema
                        .fields
                        .iter()
                        .filter(|field| field.field_type == FieldType::Relation)
                    {
                        let Some(value) =
                            candidate.get(&field.name).filter(|value| !value.is_null())
                        else {
                            continue;
                        };
                        let target = field
                            .options
                            .as_ref()
                            .and_then(|options| options["collection"].as_str())
                            .ok_or(HbError::Internal)?;
                        let values = value
                            .as_array()
                            .map_or_else(|| vec![value], |values| values.iter().collect());
                        for value in values {
                            manager
                                .get_authorized(
                                    target,
                                    value.as_str().ok_or(HbError::Internal)?,
                                    None,
                                    &context,
                                )
                                .await?;
                        }
                    }
                    let saved = if input.action == "create" {
                        manager
                            .create_authorized_with_id(
                                &input.collection,
                                &input.id,
                                candidate,
                                &context,
                            )
                            .await
                    } else {
                        let mut patch = input.dirty;
                        for field in input.unset {
                            patch.insert(field, Value::Null);
                        }
                        manager
                            .update_authorized(
                                &input.collection,
                                &input.id,
                                Value::Object(patch),
                                &context,
                            )
                            .await
                    }?;
                    crate::uploads::save_references(
                        session, &operation, &schema, &input.id, &saved,
                    )
                    .await?;
                    Ok(saved)
                })
            })
            .await
    }
}

const SYSTEM_FIELDS: [&str; 4] = ["id", "created_at", "updated_at", "deleted_at"];
const AUTH_FIELDS: &[&str] = herta_core::models::AUTH_MANAGED_FIELDS;

fn check_patch_safety(
    schema: &CollectionDef,
    dirty: &Map<String, Value>,
    unset: &[String],
    original: Option<&Value>,
    id: &str,
    uploads: &[herta_core::extension::RecordUpload],
) -> HbResult<()> {
    for field in dirty.keys().chain(unset) {
        crate::validation::validate_identifier("record field", field)?;
        if SYSTEM_FIELDS.contains(&field.as_str())
            || (schema.collection_type == CollectionType::Auth
                && AUTH_FIELDS.contains(&field.as_str()))
        {
            return Err(HbError::validation(format!(
                "field '{field}' is managed by the host"
            )));
        }
    }
    // An extension can retain or remove an existing attachment. Upload handles
    // must be supplied by the HTTP upload owner, never fabricated as strings.
    for field in schema
        .fields
        .iter()
        .filter(|field| field.field_type == FieldType::File)
    {
        let Some(value) = dirty.get(&field.name).filter(|value| !value.is_null()) else {
            continue;
        };
        let names = value
            .as_array()
            .map_or_else(|| vec![value], |values| values.iter().collect());
        for name in names {
            let old = original.and_then(|record| record.get(&field.name));
            let authorized = uploads.iter().any(|upload| {
                upload.collection == schema.name
                    && upload.record_id == id
                    && upload.field == field.name
                    && name.as_str() == Some(upload.filename.as_str())
            });
            if !authorized
                && !old.is_some_and(|old| {
                    old == name || old.as_array().is_some_and(|values| values.contains(name))
                })
            {
                return Err(JsErrorKind::FileDenied.into());
            }
        }
    }
    Ok(())
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> HbResult<T> {
    serde_json::from_value(value).map_err(|error| HbError::validation(error.to_string()))
}
#[derive(Deserialize)]
struct Find {
    collection: String,
    id: String,
    #[serde(default)]
    expand: Option<String>,
}
#[derive(Deserialize)]
struct List {
    collection: String,
    #[serde(flatten)]
    query: RecordQuery,
}
#[derive(Deserialize)]
struct Select {
    query: String,
    #[serde(default)]
    bindings: Map<String, Value>,
}
#[derive(Deserialize)]
struct Mutation {
    action: String,
    collection: String,
    id: String,
    #[serde(default)]
    dirty: Map<String, Value>,
    #[serde(default)]
    unset: Vec<String>,
}
