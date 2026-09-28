//! One definition/delta contract for HTTP management and extension hooks.
use crate::{
    CollectionDef, DbClient, DbSession, SchemaManager, UpdateCollectionRequest,
    schema::database_error, transaction::TransactionOwner, validation::validate_identifier,
};
use herta_core::{HbError, HbResult};
use serde_json::{Value, json};

pub struct CollectionService<'a> {
    session: DbSession<'a>,
}
impl<'a> CollectionService<'a> {
    pub fn new(session: impl Into<DbSession<'a>>) -> Self {
        Self {
            session: session.into(),
        }
    }

    pub async fn get(&self, name: &str) -> HbResult<Value> {
        let mut response = self
            .session
            .query("SELECT * OMIT id FROM ONLY type::record('_collections', $name)")
            .bind(("name", name.to_owned()))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let value: Option<Value> = response.take(0).map_err(database_error)?;
        value.ok_or_else(|| HbError::CollectionNotFound(name.into()))
    }

    pub async fn list(&self) -> HbResult<Value> {
        let mut response = self
            .session
            .query("SELECT * OMIT id FROM _collections ORDER BY name")
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let rows: Vec<Value> = response.take(0).map_err(database_error)?;
        Ok(rows.into())
    }

    pub async fn prepare(&self, action: &str, value: Value, patch: bool) -> HbResult<Value> {
        let name = value
            .as_str()
            .or_else(|| value["name"].as_str())
            .ok_or_else(|| HbError::validation("collection name is required"))?;
        validate_identifier("collection name", name)?;
        let original = if action == "create" {
            let mut response = self.session.query("SELECT * FROM _collection_effects WHERE name = $name AND cleanup = true LIMIT 1")
                .bind(("name",name.to_owned())).await.map_err(database_error)?.check().map_err(database_error)?;
            let pending: Vec<Value> = response.take(0).map_err(database_error)?;
            if !pending.is_empty() {
                return Err(HbError::Conflict(
                    "collection file cleanup is still pending".into(),
                ));
            }
            match self.get(name).await {
                Ok(_) => return Err(HbError::Conflict("collection already exists".into())),
                Err(HbError::CollectionNotFound(_)) => Value::Null,
                Err(error) => return Err(error),
            }
        } else {
            self.get(name).await?
        };
        if !value["version"].is_null() && value["version"] != original["version"] {
            return Err(HbError::Conflict("collection version changed".into()));
        }
        let candidate = match action {
            "delete" => original.clone(),
            "create" => {
                if !value["version"].is_null() {
                    return Err(HbError::validation("new collection cannot set version"));
                }
                let mut candidate = value;
                candidate
                    .as_object_mut()
                    .ok_or_else(|| HbError::validation("definition must be an object"))?
                    .remove("version");
                candidate
            }
            "update" if patch => {
                let patch: UpdateCollectionRequest =
                    serde_json::from_value(value).map_err(database_error)?;
                let mut updated: CollectionDef =
                    serde_json::from_value(original.clone()).map_err(database_error)?;
                updated.fields.extend(patch.fields);
                updated.indexes.extend(patch.indexes);
                if let Some(rules) = patch.rules {
                    updated.rules = rules;
                }
                let mut candidate = serde_json::to_value(updated).map_err(database_error)?;
                candidate["version"] = original["version"].clone();
                candidate
            }
            "update" => {
                if value["version"].is_null() {
                    return Err(HbError::validation(
                        "save requires the original collection version",
                    ));
                }
                value
            }
            _ => return Err(HbError::validation("unknown collection operation")),
        };
        Ok(json!({"original":original,"candidate":candidate}))
    }

    pub async fn write(
        &self,
        operation: &str,
        action: &str,
        original: Value,
        candidate: Value,
    ) -> HbResult<Value> {
        if !self.session.is_transaction() {
            return Err(HbError::Conflict(
                "Schema writes require a transaction".into(),
            ));
        }
        let object = candidate
            .as_object()
            .ok_or_else(|| HbError::validation("definition must be an object"))?;
        if object.keys().any(|key| {
            ![
                "name",
                "type",
                "schema_mode",
                "fields",
                "indexes",
                "rules",
                "version",
            ]
            .contains(&key.as_str())
        }) {
            return Err(HbError::validation(
                "unknown collection definition property",
            ));
        }
        let definition: CollectionDef = serde_json::from_value(candidate.clone())
            .map_err(|error| HbError::validation(error.to_string()))?;
        validate_identifier("collection name", &definition.name)?;
        let schema = SchemaManager::new(self.session);
        if action == "create" {
            schema.create_collection(&definition).await?;
        } else {
            if original["name"] != candidate["name"] || original["version"] != candidate["version"]
            {
                return Err(HbError::validation(
                    "collection name and version are immutable",
                ));
            }
            let latest = self.get(&definition.name).await?;
            if latest != original {
                return Err(HbError::Conflict("collection version changed".into()));
            }
            if action == "delete" {
                schema.delete_collection(&definition.name).await?;
            } else if action == "update" {
                let previous: CollectionDef =
                    serde_json::from_value(original.clone()).map_err(database_error)?;
                if definition.collection_type != previous.collection_type
                    || definition.schema_mode != previous.schema_mode
                    || !definition.fields.starts_with(&previous.fields)
                    || !definition.indexes.starts_with(&previous.indexes)
                {
                    return Err(HbError::validation(
                        "save only permits appended fields/indexes and rule changes",
                    ));
                }
                let patch = UpdateCollectionRequest {
                    fields: definition.fields[previous.fields.len()..].into(),
                    indexes: definition.indexes[previous.indexes.len()..].into(),
                    rules: (definition.rules != previous.rules).then(|| definition.rules.clone()),
                };
                if patch.fields.is_empty() && patch.indexes.is_empty() && patch.rules.is_none() {
                    return Ok(original);
                }
                schema.update_collection(&definition.name, &patch).await?;
            } else {
                return Err(HbError::validation("unknown collection operation"));
            }
        }
        self.session.query("CREATE _collection_effects SET operation = $operation, name = $name, cleanup = $cleanup")
            .bind(("operation",operation.to_owned())).bind(("name",definition.name.clone())).bind(("cleanup",action=="delete"))
            .await.map_err(database_error)?.check().map_err(database_error)?;
        if action == "delete" {
            Ok(candidate)
        } else {
            self.get(&definition.name).await
        }
    }

    /// The extension-free HTTP path uses the same owner and definition checks.
    pub async fn apply(
        db: DbClient,
        principal: Option<String>,
        action: String,
        value: Value,
        patch: bool,
    ) -> HbResult<Value> {
        let owner = TransactionOwner::new(db, principal);
        let receipt = owner.begin().await?;
        let operation = receipt.id.clone();
        let result = owner
            .execute(Some(operation.clone()), true, move |session| {
                Box::pin(async move {
                    let service = CollectionService::new(session);
                    let prepared = service.prepare(&action, value, patch).await?;
                    service
                        .write(
                            &operation,
                            &action,
                            prepared["original"].clone(),
                            prepared["candidate"].clone(),
                        )
                        .await
                })
            })
            .await;
        match result {
            Ok(value) => {
                owner.commit(&receipt.id).await?;
                Ok(value)
            }
            Err(error) => {
                owner.finish().await?;
                Err(error)
            }
        }
    }
}
