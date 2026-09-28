use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OutboxKind {
    #[serde(rename = "mail.send")]
    Mail,
    #[serde(rename = "http.send")]
    Http,
}
impl OutboxKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mail => "mail.send",
            Self::Http => "http.send",
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutboxEnqueue {
    pub kind: OutboxKind,
    pub payload: Value,
    pub idempotency_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutboxState {
    Pending,
    Leased,
    Sending,
    Accepted,
    Failed,
    Unknown,
}

/// No message payload, recipient, credentials, or response body is exposed in receipts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxReceipt {
    pub job_id: String,
    pub kind: OutboxKind,
    pub state: OutboxState,
    pub attempts: usize,
    pub created_at: i64,
    pub updated_at: i64,
    pub next_attempt_at: i64,
    pub error_code: Option<String>,
    pub result: Option<Value>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxResolution {
    Accepted,
    NotSent,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboxResolve {
    pub resolution: OutboxResolution,
    pub note: String,
}
