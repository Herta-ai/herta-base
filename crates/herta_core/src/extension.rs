//! Serializable host contracts. No engine objects or database handles cross this boundary.

use crate::host_buffer::{HostBudget, HostBuffer, HostReply};
use crate::{HbResult, JsErrorKind};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    System,
    #[default]
    Request,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invocation {
    pub name: String,
    pub payload: Value,
    pub auth_mode: AuthMode,
    pub request_id: Option<String>,
    pub registration: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCall {
    pub operation: String,
    pub arguments: Value,
    pub auth_mode: AuthMode,
    #[serde(default)]
    pub remaining_ms: u64,
    pub script: String,
    pub transaction: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    pub id: usize,
    pub kind: String,
    pub name: String,
    pub script: String,
    #[serde(default)]
    pub collections: Vec<String>,
    #[serde(default)]
    pub options: Value,
}

/// One instance is bound to one root operation's authenticated Rust identity and
/// transaction owner. The JS mode can only reduce that instance's authority.
#[async_trait]
pub trait HostServices: Send + Sync {
    async fn call(&self, call: HostCall) -> HbResult<Value>;
    /// Binary-aware transport. Legacy hosts remain compatible for JSON commands.
    async fn call_binary(
        &self,
        call: HostCall,
        bytes: Option<HostBuffer>,
        _budget: HostBudget,
    ) -> HbResult<HostReply> {
        if bytes.is_some() {
            return Err(crate::HbError::CapabilityUnavailable);
        }
        self.call(call).await.map(HostReply::from)
    }
    /// Stop accepting commands and signal in-flight operations. Transaction
    /// owners must still resolve commit/cancel outcomes in `finish`.
    fn cancel(&self) {}
    /// False after an uncertain commit or an external delivery that a fresh
    /// attempt cannot safely repeat. The scheduler checks this even if JS caught it.
    fn retry_safe(&self) -> bool {
        true
    }
    /// A commit may have crossed the database boundary before JS received its
    /// acknowledgement. Called without waiting for the owner lock on interruption.
    fn interrupted_commit(&self) -> Option<crate::HbError> {
        None
    }
    /// Called even when JS is interrupted or its result receiver disappears.
    async fn finish(&self, successful: bool) -> HbResult<()>;
}

pub struct UnavailableHost;
#[async_trait]
impl HostServices for UnavailableHost {
    async fn call(&self, _: HostCall) -> HbResult<Value> {
        Err(JsErrorKind::Denied.into())
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        Ok(())
    }
}

#[async_trait]
pub trait EventDispatcher: Send + Sync {
    async fn dispatch(
        &self,
        invocation: Invocation,
        host: Arc<dyn HostServices>,
    ) -> HbResult<Value>;
    async fn dispatch_frame(
        &self,
        mut invocation: Invocation,
        host: Arc<dyn HostServices>,
        body: Option<Vec<u8>>,
    ) -> HbResult<HostReply> {
        if let Some(body) = body {
            invocation.payload["request"]["body"] =
                serde_json::json!(String::from_utf8_lossy(&body));
            invocation.payload["request"]["bytes"] = serde_json::json!(body);
        }
        self.dispatch(invocation, host).await.map(HostReply::from)
    }
    fn registrations(&self) -> Arc<Vec<Registration>>;
    /// Pins both registration lookup and execution to one immutable revision.
    fn pin(self: Arc<Self>) -> Arc<dyn EventDispatcher>;
}

/// Rust-authenticated authority. Never deserialize this from extension input.
#[derive(Debug, Clone)]
pub struct HostContext {
    pub mode: AuthMode,
    pub principal: Option<String>,
    pub admin: bool,
    pub auth: Value,
    pub request_body: Value,
    pub uploads: Vec<RecordUpload>,
}

/// Created from checked multipart parts by Rust; never accepted over the JS bridge.
#[derive(Debug, Clone)]
pub struct RecordUpload {
    pub collection: String,
    pub record_id: String,
    pub field: String,
    pub filename: String,
    pub source: std::path::PathBuf,
}

impl HostContext {
    pub fn system() -> Self {
        Self {
            mode: AuthMode::System,
            principal: None,
            admin: true,
            auth: serde_json::json!({"admin":true,"role":"admin"}),
            request_body: Value::Null,
            uploads: Vec::new(),
        }
    }
}

pub trait HostFactory: Send + Sync {
    fn create(&self, context: HostContext) -> Arc<dyn HostServices>;
}

#[derive(Clone)]
pub struct Extensions {
    pub dispatcher: Arc<dyn EventDispatcher>,
    pub hosts: Arc<dyn HostFactory>,
}
