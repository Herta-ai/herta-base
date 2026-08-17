use futures_util::StreamExt;
use herta_core::{HbError, HbResult};
use serde_json::{Value, json};
use surrealdb::{
    method::QueryStream,
    types::{Action, Value as SurrealValue},
};

use crate::{
    DbClient, SchemaManager,
    filter::compile_filter,
    models::{CollectionDef, RuleContext},
    record::{allowed_fields, compile_rule, normalize_value, sanitize_record},
    schema::database_error,
    validation::quote_identifier,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeAction {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RealtimeEvent {
    pub action: RealtimeAction,
    pub record: Value,
}

pub struct RealtimeSubscription {
    stream: QueryStream<SurrealValue>,
    schema: CollectionDef,
}

pub struct RealtimeManager<'a> {
    db: &'a DbClient,
}

impl<'a> RealtimeManager<'a> {
    pub fn new(db: &'a DbClient) -> Self {
        Self { db }
    }

    pub async fn subscribe(
        &self,
        collection: &str,
        filter: Option<&str>,
        context: &RuleContext,
    ) -> HbResult<RealtimeSubscription> {
        let schema = SchemaManager::new(self.db)
            .get_collection(collection)
            .await?;
        let rule = compile_rule(&schema.rules.view, context, true)?;
        let compiled_filter = filter
            .map(|filter| compile_filter(filter, &allowed_fields(&schema)))
            .transpose()?;

        let mut where_sql = format!("({rule})");
        if let Some(filter) = &compiled_filter {
            where_sql.push_str(" AND (");
            where_sql.push_str(&filter.sql);
            where_sql.push(')');
        }
        let sql = format!(
            "LIVE SELECT * FROM {} WHERE {where_sql}",
            quote_identifier(collection)
        );
        let mut query = self
            .db
            .inner()
            .query(sql)
            .bind(("hb_auth", context.auth.clone()))
            .bind(("hb_auth_record", context.auth_record.clone()))
            .bind(("hb_request", json!({"body": context.request_body})));
        if let Some(filter) = compiled_filter {
            for (name, value) in filter.bindings {
                query = query.bind((name, value));
            }
        }
        let mut response = query
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let stream = response
            .stream::<SurrealValue>(0usize)
            .map_err(database_error)?;
        Ok(RealtimeSubscription { stream, schema })
    }
}

impl RealtimeSubscription {
    pub async fn next(&mut self) -> HbResult<Option<RealtimeEvent>> {
        let Some(notification) = self.stream.next().await else {
            return Ok(None);
        };
        let notification = notification.map_err(database_error)?;
        let mut record = notification.data.into_json_value();
        normalize_value(&mut record);

        match notification.action {
            Action::Create => {
                sanitize_record(&self.schema, &mut record);
                Ok(Some(RealtimeEvent {
                    action: RealtimeAction::Create,
                    record,
                }))
            }
            Action::Update
                if record
                    .get("deleted_at")
                    .is_some_and(|value| !value.is_null()) =>
            {
                Ok(Some(RealtimeEvent {
                    action: RealtimeAction::Delete,
                    record: id_only(record),
                }))
            }
            Action::Update => {
                sanitize_record(&self.schema, &mut record);
                Ok(Some(RealtimeEvent {
                    action: RealtimeAction::Update,
                    record,
                }))
            }
            Action::Delete => Ok(Some(RealtimeEvent {
                action: RealtimeAction::Delete,
                record: id_only(record),
            })),
            Action::Killed => Ok(None),
            Action::Error => Err(HbError::Database(format!("live query failed: {record}"))),
        }
    }
}

fn id_only(record: Value) -> Value {
    match record.get("id").cloned() {
        Some(id) => json!({"id": id}),
        None if record.is_string() => json!({"id": record}),
        None => json!({"id": Value::Null}),
    }
}
