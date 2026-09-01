use std::collections::{HashMap, VecDeque};

use futures_util::StreamExt;
use herta_core::{HbError, HbResult};
use serde_json::{Value, json};
use surrealdb::types::{Action, Value as SurrealValue};
use tokio::{
    sync::mpsc::{self, error::TrySendError},
    task::JoinHandle,
    time::{Duration, Instant, Interval, interval_at, timeout},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    DbClient, SchemaManager,
    filter::compile_filter,
    models::{CollectionDef, RuleContext},
    record::{compile_rule, normalize_value, sanitize_record},
    schema::database_error,
    validation::quote_identifier,
};

const LIVE_QUERY_DRAIN_QUIET_PERIOD: Duration = Duration::from_millis(50);

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
    receiver: mpsc::Receiver<HbResult<()>>,
    worker: Option<JoinHandle<HbResult<()>>>,
    cancellation: CancellationToken,
    db: DbClient,
    schema: CollectionDef,
    poll_sql: String,
    filter_bindings: Vec<(String, SurrealValue)>,
    context: RuleContext,
    snapshot: HashMap<String, Value>,
    pending: VecDeque<RealtimeEvent>,
    poll: Interval,
}

pub struct RealtimeManager {
    db: DbClient,
    reconciliation_period: Duration,
}

impl RealtimeManager {
    pub fn new(db: &DbClient) -> Self {
        Self {
            db: db.clone(),
            reconciliation_period: Duration::from_secs(30),
        }
    }

    pub fn with_reconciliation_period(mut self, period: Duration) -> Self {
        assert!(!period.is_zero(), "reconciliation period must be positive");
        self.reconciliation_period = period;
        self
    }

    pub async fn subscribe(
        &self,
        collection: &str,
        filter: Option<&str>,
        context: &RuleContext,
    ) -> HbResult<RealtimeSubscription> {
        let schema = SchemaManager::new(&self.db)
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

        let mut poll_where_sql = format!("deleted_at IS NONE AND ({rule})");
        if let Some(filter) = &compiled_filter {
            poll_where_sql.push_str(" AND (");
            poll_where_sql.push_str(&filter.sql);
            poll_where_sql.push(')');
        }
        let sql = format!("LIVE SELECT id FROM {}", quote_identifier(collection));
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
        let live_query_id = response
            .take::<Option<Uuid>>(0)
            .map_err(database_error)?
            .ok_or_else(|| HbError::Database("live query did not return an id".into()))?;
        let stream = match response.stream::<SurrealValue>(0usize) {
            Ok(stream) => stream,
            Err(error) => {
                let error = database_error(error);
                let _ = kill_live_query(&self.db, live_query_id).await;
                return Err(error);
            }
        };
        let poll_sql = format!(
            "SELECT * FROM {} WHERE {poll_where_sql}",
            quote_identifier(collection)
        );
        let filter_bindings = compiled_filter
            .as_ref()
            .map_or_else(Vec::new, |filter| filter.bindings.clone());
        let snapshot =
            match fetch_matching(&self.db, &schema, &poll_sql, &filter_bindings, context).await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    let _ = kill_live_query(&self.db, live_query_id).await;
                    return Err(error);
                }
            };
        let (sender, receiver) = mpsc::channel(1);
        let cancellation = CancellationToken::new();
        let worker = tokio::spawn(run_live_worker(
            stream,
            sender,
            cancellation.clone(),
            self.db.clone(),
            live_query_id,
        ));
        Ok(RealtimeSubscription {
            receiver,
            worker: Some(worker),
            cancellation,
            db: self.db.clone(),
            schema,
            poll_sql,
            filter_bindings,
            context: context.clone(),
            snapshot,
            pending: VecDeque::new(),
            poll: interval_at(
                Instant::now() + self.reconciliation_period,
                self.reconciliation_period,
            ),
        })
    }
}

async fn run_live_worker(
    mut stream: surrealdb::method::QueryStream<SurrealValue>,
    sender: mpsc::Sender<HbResult<()>>,
    cancellation: CancellationToken,
    db: DbClient,
    live_query_id: Uuid,
) -> HbResult<()> {
    loop {
        let notification = tokio::select! {
            _ = cancellation.cancelled() => break,
            notification = stream.next() => notification,
        };
        let Some(notification) = notification else {
            break;
        };
        match notification.map_err(database_error) {
            Ok(notification) if notification.action == Action::Killed => break,
            Ok(notification) if notification.action == Action::Error => {
                let error = HbError::Database(format!(
                    "live query failed: {}",
                    notification.data.into_json_value()
                ));
                send_worker_error(&sender, &cancellation, error).await;
                break;
            }
            Ok(_) => match sender.try_send(Ok(())) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Closed(_)) => break,
            },
            Err(error) => {
                send_worker_error(&sender, &cancellation, error).await;
                break;
            }
        }
    }

    let result = kill_live_query(&db, live_query_id).await;
    if let Err(error) = &result {
        tracing::warn!(%live_query_id, %error, "failed to clean up realtime live query");
    } else {
        drain_queued_notifications(&mut stream).await;
    }
    result
}

async fn drain_queued_notifications(stream: &mut surrealdb::method::QueryStream<SurrealValue>) {
    // Keep the receiver alive while notifications already queued by SurrealDB's
    // local router drain. Dropping it immediately races the router and makes
    // SurrealDB 3.2.3 retry cleanup with a malformed bare-UUID KILL statement.
    while let Ok(Some(_)) = timeout(LIVE_QUERY_DRAIN_QUIET_PERIOD, stream.next()).await {}
}

async fn send_worker_error(
    sender: &mpsc::Sender<HbResult<()>>,
    cancellation: &CancellationToken,
    error: HbError,
) {
    tokio::select! {
        _ = cancellation.cancelled() => {}
        _ = sender.send(Err(error)) => {}
    }
}

async fn kill_live_query(db: &DbClient, live_query_id: Uuid) -> HbResult<()> {
    db.inner()
        .query("KILL $live_query_id")
        .bind(("live_query_id", live_query_id))
        .await
        .map_err(database_error)?
        .check()
        .map_err(database_error)?;
    Ok(())
}

fn count(rows: &[Value]) -> u64 {
    rows.first()
        .and_then(|row| row.get("total"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

impl RealtimeSubscription {
    pub async fn close(&mut self) -> HbResult<()> {
        self.cancellation.cancel();
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        worker.await.map_err(|_| HbError::Internal)?
    }

    pub async fn next(&mut self) -> HbResult<Option<RealtimeEvent>> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }
            enum Source {
                Live(Option<HbResult<()>>),
                Poll,
            }
            let source = tokio::select! {
                live = self.receiver.recv() => Source::Live(live),
                _ = self.poll.tick() => Source::Poll,
            };
            match source {
                Source::Live(Some(Ok(()))) => self.refresh().await?,
                Source::Live(Some(Err(error))) => return Err(error),
                Source::Live(None) => return Ok(None),
                Source::Poll => self.refresh().await?,
            }
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
        self.cancellation.cancel();
    }
}
