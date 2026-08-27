use std::collections::{HashMap, VecDeque};

use futures_util::StreamExt;
use herta_core::{HbError, HbResult};
use serde_json::{Value, json};
use surrealdb::{
    Notification,
    types::{Action, Value as SurrealValue},
};
use tokio::{
    sync::mpsc,
    task::JoinHandle,
    time::{Duration, Instant, Interval, interval_at},
};

use crate::{
    DbClient, SchemaManager,
    filter::compile_filter,
    models::{CollectionDef, RuleContext},
    record::{compile_rule, normalize_value, sanitize_record},
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
    receiver: mpsc::Receiver<HbResult<RealtimeEvent>>,
    worker: JoinHandle<()>,
    db: DbClient,
    schema: CollectionDef,
    poll_sql: String,
    filter_bindings: Vec<(String, SurrealValue)>,
    context: RuleContext,
    snapshot: HashMap<String, Value>,
    pending: VecDeque<RealtimeEvent>,
    poll: Interval,
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
            .map(|filter| compile_filter(filter, &schema))
            .transpose()?;

        if let Some(filter) = &compiled_filter {
            let table = quote_identifier(collection);
            let preflight_sql = format!(
                "SELECT count() AS total FROM {table} WHERE deleted_at IS NONE AND ({filter_sql}) GROUP ALL;\n\
                 SELECT count() AS total FROM {table} WHERE deleted_at IS NONE AND ({rule}) AND ({filter_sql}) GROUP ALL;",
                filter_sql = filter.sql,
            );
            let mut query = self
                .db
                .inner()
                .query(preflight_sql)
                .bind(("hb_auth", context.auth.clone()))
                .bind(("hb_auth_record", context.auth_record.clone()))
                .bind(("hb_request", json!({"body": context.request_body})));
            for (name, value) in &filter.bindings {
                query = query.bind((name.clone(), value.clone()));
            }
            let mut response = query
                .await
                .map_err(database_error)?
                .check()
                .map_err(database_error)?;
            let matching: Vec<Value> = response.take(0).map_err(database_error)?;
            let authorized: Vec<Value> = response.take(1).map_err(database_error)?;
            if count(&matching) > 0 && count(&authorized) == 0 {
                return Err(HbError::Forbidden);
            }
        }

        let mut where_sql = format!("({rule})");
        let mut poll_where_sql = format!("deleted_at IS NONE AND ({rule})");
        if let Some(filter) = &compiled_filter {
            where_sql.push_str(" AND (");
            where_sql.push_str(&filter.live_sql);
            where_sql.push(')');
            poll_where_sql.push_str(" AND (");
            poll_where_sql.push_str(&filter.sql);
            poll_where_sql.push(')');
        }
        let sql = format!(
            "LIVE SELECT * FROM {} WHERE {where_sql}",
            quote_identifier(collection)
        );
        let query = self
            .db
            .inner()
            .query(sql)
            .bind(("hb_auth", context.auth.clone()))
            .bind(("hb_auth_record", context.auth_record.clone()))
            .bind(("hb_request", json!({"body": context.request_body})));
        let mut response = query
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let stream = response
            .stream::<SurrealValue>(0usize)
            .map_err(database_error)?;
        let (sender, receiver) = mpsc::channel(64);
        let worker_schema = schema.clone();
        let worker = tokio::spawn(async move {
            let mut stream = stream;
            while let Some(notification) = stream.next().await {
                let event = notification
                    .map_err(database_error)
                    .and_then(|notification| map_notification(&worker_schema, notification));
                let done = event.is_err();
                match event {
                    Ok(Some(event)) => {
                        if sender.send(Ok(event)).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let _ = sender.send(Err(error)).await;
                    }
                }
                if done {
                    break;
                }
            }
        });
        let poll_sql = format!(
            "SELECT * FROM {} WHERE {poll_where_sql}",
            quote_identifier(collection)
        );
        let filter_bindings = compiled_filter
            .as_ref()
            .map_or_else(Vec::new, |filter| filter.bindings.clone());
        let snapshot =
            fetch_matching(self.db, &schema, &poll_sql, &filter_bindings, context).await?;
        let poll_period = Duration::from_millis(100);
        Ok(RealtimeSubscription {
            receiver,
            worker,
            db: self.db.clone(),
            schema,
            poll_sql,
            filter_bindings,
            context: context.clone(),
            snapshot,
            pending: VecDeque::new(),
            poll: interval_at(Instant::now() + poll_period, poll_period),
        })
    }
}

fn count(rows: &[Value]) -> u64 {
    rows.first()
        .and_then(|row| row.get("total"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

impl RealtimeSubscription {
    pub async fn next(&mut self) -> HbResult<Option<RealtimeEvent>> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }
            enum Source {
                Live(Option<HbResult<RealtimeEvent>>),
                Poll,
            }
            let source = tokio::select! {
                live = self.receiver.recv() => Source::Live(live),
                _ = self.poll.tick() => Source::Poll,
            };
            match source {
                Source::Live(Some(Ok(event))) => {
                    if self.apply_live(&event) {
                        return Ok(Some(event));
                    }
                }
                Source::Live(Some(Err(error))) => return Err(error),
                Source::Live(None) => self.refresh().await?,
                Source::Poll => self.refresh().await?,
            }
        }
    }

    fn apply_live(&mut self, event: &RealtimeEvent) -> bool {
        let Some(id) = event.record.get("id").and_then(Value::as_str) else {
            return true;
        };
        match event.action {
            RealtimeAction::Create | RealtimeAction::Update => {
                if self.snapshot.get(id) == Some(&event.record) {
                    false
                } else {
                    self.snapshot.insert(id.to_owned(), event.record.clone());
                    true
                }
            }
            RealtimeAction::Delete => self.snapshot.remove(id).is_some(),
        }
    }

    async fn refresh(&mut self) -> HbResult<()> {
        let current = fetch_matching(
            &self.db,
            &self.schema,
            &self.poll_sql,
            &self.filter_bindings,
            &self.context,
        )
        .await?;
        for (id, record) in &current {
            match self.snapshot.get(id) {
                None => self.pending.push_back(RealtimeEvent {
                    action: RealtimeAction::Create,
                    record: record.clone(),
                }),
                Some(previous) if previous != record => self.pending.push_back(RealtimeEvent {
                    action: RealtimeAction::Update,
                    record: record.clone(),
                }),
                Some(_) => {}
            }
        }
        for id in self.snapshot.keys() {
            if !current.contains_key(id) {
                self.pending.push_back(RealtimeEvent {
                    action: RealtimeAction::Delete,
                    record: serde_json::json!({"id": id}),
                });
            }
        }
        self.snapshot = current;
        Ok(())
    }
}

async fn fetch_matching(
    db: &DbClient,
    schema: &CollectionDef,
    sql: &str,
    filter_bindings: &[(String, SurrealValue)],
    context: &RuleContext,
) -> HbResult<HashMap<String, Value>> {
    let mut query = db
        .inner()
        .query(sql)
        .bind(("hb_auth", context.auth.clone()))
        .bind(("hb_auth_record", context.auth_record.clone()))
        .bind(("hb_request", json!({"body": context.request_body})));
    for (name, value) in filter_bindings {
        query = query.bind((name.clone(), value.clone()));
    }
    let mut response = query
        .await
        .map_err(database_error)?
        .check()
        .map_err(database_error)?;
    let mut records: Vec<Value> = response.take(0).map_err(database_error)?;
    let mut matching = HashMap::new();
    for mut record in records.drain(..) {
        normalize_value(&mut record);
        sanitize_record(schema, &mut record);
        if let Some(id) = record.get("id").and_then(Value::as_str) {
            matching.insert(id.to_owned(), record);
        }
    }
    Ok(matching)
}

impl Drop for RealtimeSubscription {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

fn map_notification(
    schema: &CollectionDef,
    notification: Notification<SurrealValue>,
) -> HbResult<Option<RealtimeEvent>> {
    let mut record = notification.data.into_json_value();
    normalize_value(&mut record);

    match notification.action {
        Action::Create => {
            sanitize_record(schema, &mut record);
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
            sanitize_record(schema, &mut record);
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

fn id_only(record: Value) -> Value {
    match record.get("id").cloned() {
        Some(id) => json!({"id": id}),
        None if record.is_string() => json!({"id": record}),
        None => json!({"id": Value::Null}),
    }
}
