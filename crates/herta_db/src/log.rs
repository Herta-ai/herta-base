use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::DbClient;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogType {
    Server,
    Request,
}

pub type LogSender = mpsc::Sender<LogEntry>;
pub type LogReceiver = mpsc::Receiver<LogEntry>;

pub fn log_channel() -> (LogSender, LogReceiver) {
    mpsc::channel(4096)
}

struct LogManager<'a> {
    db: &'a DbClient,
}

impl<'a> LogManager<'a> {
    fn new(db: &'a DbClient) -> Self {
        Self { db }
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
