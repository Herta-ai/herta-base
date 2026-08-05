use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::types::Datetime;
use tokio::sync::mpsc;

use crate::{DbClient, record::normalize_value, schema::database_error};
use herta_core::{HbError, HbResult};

const DEFAULT_LOG_PAGE: u64 = 1;
const DEFAULT_LOG_PER_PAGE: u64 = 30;
const MAX_LOG_PER_PAGE: u64 = 500;
const MAX_LOG_KEYWORD_BYTES: usize = 256;

/// A structured entry persisted in the `_logs` system table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub log_type: LogType,
    pub level: String,
    pub message: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_collection: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogType {
    Server,
    Request,
}

/// Parameters accepted by the administrator log list query.
#[derive(Debug, Clone, Default)]
pub struct LogQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub level: Option<String>,
    pub log_type: Option<String>,
    pub keyword: Option<String>,
    pub target: Option<String>,
    pub path: Option<String>,
    pub status_code: Option<u16>,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl LogQuery {
    pub fn page(&self) -> u64 {
        self.page.unwrap_or(DEFAULT_LOG_PAGE)
    }

    pub fn per_page(&self) -> u64 {
        self.per_page.unwrap_or(DEFAULT_LOG_PER_PAGE)
    }

    fn validate(
        &self,
    ) -> HbResult<(
        Option<String>,
        Option<String>,
        Option<Datetime>,
        Option<Datetime>,
    )> {
        if self.page() == 0 {
            return Err(HbError::validation("page must be at least 1"));
        }
        if !(1..=MAX_LOG_PER_PAGE).contains(&self.per_page()) {
            return Err(HbError::validation(format!(
                "perPage must be between 1 and {MAX_LOG_PER_PAGE}"
            )));
        }

        let level = self
            .level
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase);
        if let Some(value) = level.as_deref()
            && !matches!(value, "trace" | "debug" | "info" | "warn" | "error")
        {
            return Err(HbError::validation(
                "level must be trace, debug, info, warn, or error",
            ));
        }

        let log_type = self
            .log_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase);
        if let Some(value) = log_type.as_deref()
            && !matches!(value, "server" | "request")
        {
            return Err(HbError::validation("logType must be server or request"));
        }

        if self
            .keyword
            .as_deref()
            .is_some_and(|value| value.len() > MAX_LOG_KEYWORD_BYTES)
        {
            return Err(HbError::validation(format!(
                "q must not exceed {MAX_LOG_KEYWORD_BYTES} bytes"
            )));
        }

        if self
            .status_code
            .is_some_and(|status| !(100..=599).contains(&status))
        {
            return Err(HbError::validation(
                "statusCode must be between 100 and 599",
            ));
        }

        let from = parse_datetime(self.from.as_deref(), "from")?;
        let to = parse_datetime(self.to.as_deref(), "to")?;
        if let (Some(from), Some(to)) = (from, to)
            && from > to
        {
            return Err(HbError::validation(
                "from must be earlier than or equal to to",
            ));
        }

        Ok((level, log_type, from, to))
    }
}

pub type LogSender = mpsc::Sender<LogEntry>;
pub type LogReceiver = mpsc::Receiver<LogEntry>;

pub fn log_channel() -> (LogSender, LogReceiver) {
    mpsc::channel(4096)
}

pub struct LogManager<'a> {
    db: &'a DbClient,
}

impl<'a> LogManager<'a> {
    pub fn new(db: &'a DbClient) -> Self {
        Self { db }
    }

    /// List persisted logs using the administrator-only query contract.
    pub async fn list(&self, params: &LogQuery) -> HbResult<(Vec<Value>, u64)> {
        let (level, log_type, from, to) = params.validate()?;
        let keyword = params
            .keyword
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase);

        let mut clauses = Vec::new();
        if level.is_some() {
            clauses.push("level = $level".to_owned());
        }
        if log_type.is_some() {
            clauses.push("log_type = $log_type".to_owned());
        }
        if keyword.is_some() {
            clauses.push(
                "(string::lowercase(message) CONTAINS $keyword OR \
                 string::lowercase(target) CONTAINS $keyword OR \
                 string::lowercase(method) CONTAINS $keyword OR \
                 string::lowercase(path) CONTAINS $keyword OR \
                 string::lowercase(referer) CONTAINS $keyword OR \
                 string::lowercase(remote_ip) CONTAINS $keyword OR \
                 string::lowercase(user_agent) CONTAINS $keyword OR \
                 string::lowercase(auth_type) CONTAINS $keyword OR \
                 string::lowercase(user_id) CONTAINS $keyword OR \
                 string::lowercase(user_collection) CONTAINS $keyword)"
                    .replace('\n', " "),
            );
        }
        if params
            .target
            .as_deref()
            .is_some_and(|value| !value.is_empty())
        {
            clauses.push("target = $target".to_owned());
        }
        if params
            .path
            .as_deref()
            .is_some_and(|value| !value.is_empty())
        {
            clauses.push("path = $path".to_owned());
        }
        if params.status_code.is_some() {
            clauses.push("status_code = $status_code".to_owned());
        }
        if from.is_some() {
            clauses.push("created_at >= $from".to_owned());
        }
        if to.is_some() {
            clauses.push("created_at <= $to".to_owned());
        }

        let where_sql = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        let sql = format!(
            "SELECT count() AS total FROM _logs{where_sql} GROUP ALL;\n\
             SELECT * FROM _logs{where_sql} ORDER BY created_at DESC, id DESC \
             LIMIT $limit START $offset;"
        );
        let mut query = self
            .db
            .inner()
            .query(sql)
            .bind(("limit", params.per_page()))
            .bind(("offset", (params.page() - 1) * params.per_page()));
        if let Some(value) = level {
            query = query.bind(("level", value));
        }
        if let Some(value) = log_type {
            query = query.bind(("log_type", value));
        }
        if let Some(value) = keyword {
            query = query.bind(("keyword", value));
        }
        if let Some(value) = params.target.as_deref().filter(|value| !value.is_empty()) {
            query = query.bind(("target", value.to_owned()));
        }
        if let Some(value) = params.path.as_deref().filter(|value| !value.is_empty()) {
            query = query.bind(("path", value.to_owned()));
        }
        if let Some(value) = params.status_code {
            query = query.bind(("status_code", value));
        }
        if let Some(value) = from {
            query = query.bind(("from", value));
        }
        if let Some(value) = to {
            query = query.bind(("to", value));
        }

        let mut response = query
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let counts: Vec<Value> = response.take(0).map_err(database_error)?;
        let total = counts
            .first()
            .and_then(|value| value.get("total"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let mut records: Vec<Value> = response.take(1).map_err(database_error)?;
        for record in &mut records {
            normalize_value(record);
        }
        Ok((records, total))
    }

    async fn insert_batch(&self, entries: &[LogEntry]) -> anyhow::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let values: Vec<Value> = entries
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?;
        self.db
            .inner()
            .query("INSERT INTO _logs $entries")
            .bind(("entries", values))
            .await?
            .check()?;
        Ok(())
    }
}

fn parse_datetime(value: Option<&str>, field: &str) -> HbResult<Option<Datetime>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<Datetime>()
                .map_err(|_| HbError::validation(format!("{field} must be an RFC3339 datetime")))
        })
        .transpose()
}

/// Start the non-blocking batch writer. It flushes every 500 ms or at 100 entries.
pub fn spawn_log_worker(db: DbClient, mut rx: LogReceiver) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut buffer = Vec::with_capacity(100);
        loop {
            let deadline = tokio::time::sleep(std::time::Duration::from_millis(500));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    entry = rx.recv() => match entry {
                        Some(entry) => {
                            buffer.push(entry);
                            if buffer.len() >= 100 { break; }
                        }
                        None => {
                            if !buffer.is_empty()
                                && let Err(error) = LogManager::new(&db).insert_batch(&buffer).await
                            {
                                eprintln!("[log_worker] failed to flush logs on shutdown: {error}");
                            }
                            return;
                        }
                    },
                    _ = &mut deadline => break,
                }
            }
            if !buffer.is_empty() {
                if let Err(error) = LogManager::new(&db).insert_batch(&buffer).await {
                    eprintln!("[log_worker] failed to flush logs: {error}");
                }
                buffer.clear();
            }
        }
    })
}
