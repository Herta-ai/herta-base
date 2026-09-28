//! Engine-independent configuration and error contracts for extensions.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

macro_rules! configuration {
    ($name:ident { $($field:ident: $ty:ty = $default:expr),* $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(default, deny_unknown_fields)]
        pub struct $name { $(pub $field: $ty),* }
        impl Default for $name {
            fn default() -> Self { Self { $($field: $default),* } }
        }
    };
}

configuration!(JsvmConfig {
    enabled: bool = false,
    memory_limit_mb: usize = 16,
    stack_limit_kb: usize = 512,
    execution_timeout_ms: u64 = 100,
    async_timeout_ms: u64 = 5000,
    startup_timeout_ms: u64 = 10000,
    queue_timeout_ms: u64 = 1000,
    shutdown_timeout_ms: u64 = 30000,
    pool_size: usize = 4,
    queue_capacity: usize = 128,
    max_response_bytes: usize = 4 * 1024 * 1024,
    max_bridge_bytes: usize = 4 * 1024 * 1024,
    max_host_buffer_bytes: usize = 16 * 1024 * 1024,
    max_pending_host_calls: usize = 32,
    max_host_calls: usize = 256,
    max_hook_depth: usize = 8,
    max_logs: usize = 100,
    max_log_bytes: usize = 65536,
    max_script_files: usize = 128,
    max_script_bytes: usize = 1024 * 1024,
    max_source_bytes: usize = 8 * 1024 * 1024,
    max_registrations: usize = 1024,
    max_snapshots: usize = 4,
    max_rate_limit_keys: usize = 10000,
    route_prefixes: Vec<String> = vec!["/api/".into()],
    raw_query_enabled: bool = false,
    env_allowlist: Vec<String> = Vec::new(),
    http: JsHttpConfig = JsHttpConfig::default(),
    files: JsFilesConfig = JsFilesConfig::default(),
    mail: JsMailConfig = JsMailConfig::default(),
    realtime: JsRealtimeConfig = JsRealtimeConfig::default(),
    cron: JsCronConfig = JsCronConfig::default(),
    outbox: JsOutboxConfig = JsOutboxConfig::default(),
});

configuration!(JsHttpConfig {
    enabled: bool = false,
    allowlist: Vec<String> = Vec::new(),
    max_redirects: usize = 3,
    max_request_bytes: usize = 1024 * 1024,
    max_response_bytes: usize = 4 * 1024 * 1024,
    connect_timeout_ms: u64 = 3000,
    timeout_ms: u64 = 5000,
});
configuration!(JsFilesConfig {
    enabled: bool = false,
    prefix: String = "extensions".into(),
    quota_bytes: u64 = 100 * 1024 * 1024,
    max_file_bytes: u64 = 10 * 1024 * 1024,
});
configuration!(JsMailConfig {
    enabled: bool = false,
    max_recipients: usize = 20,
    max_body_bytes: usize = 1024 * 1024,
});
configuration!(JsRealtimeConfig {
    enabled: bool = false,
    max_message_bytes: usize = 65536,
    publish_per_second: usize = 100,
    max_audience: usize = 100,
    connection_queue_capacity: usize = 64,
    connection_queue_bytes: usize = 262144,
});
configuration!(JsCronConfig {
    enabled: bool = true,
    timezone: String = "UTC".into(),
    max_runtime_ms: u64 = 30000,
    retries: usize = 0,
    max_retries: usize = 3,
});
configuration!(JsOutboxConfig {
    enabled: bool = false,
    lease_seconds: u64 = 60,
    renew_seconds: u64 = 20,
    max_retries: usize = 3,
    retention_days: u64 = 7,
    concurrency: usize = 4,
    max_jobs: usize = 10000,
    idempotent_origins: Vec<String> = Vec::new(),
});

impl JsvmConfig {
    /// Each leaf has exactly one environment spelling, including new nested options.
    pub fn apply_env(&mut self) -> anyhow::Result<()> {
        self.apply_overrides(std::env::vars().filter(|(key, _)| key.starts_with("HB_JS_")))
    }

    pub fn apply_overrides(
        &mut self,
        variables: impl IntoIterator<Item = (String, String)>,
    ) -> anyhow::Result<()> {
        let mut document = serde_json::to_value(&*self)?;
        let mut paths = std::collections::BTreeMap::new();
        collect_paths(&document, Vec::new(), &mut paths);
        for (name, value) in variables {
            let path = paths
                .get(&name)
                .with_context(|| format!("unknown JS environment option {name}"))?;
            let mut slot = &mut document;
            for part in path {
                slot = &mut slot[part];
            }
            *slot = if slot.is_string() {
                Value::String(value)
            } else {
                serde_json::from_str(&value)
                    .map_err(|_| anyhow::anyhow!("invalid value for {name}"))?
            };
        }
        *self = serde_json::from_value(document)
            .map_err(|_| anyhow::anyhow!("invalid JS environment option type"))?;
        self.validate()
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let document = serde_json::to_value(self)?;
        positive_limits(&document, "jsvm")?;
        if self.pool_size > 256 || self.queue_capacity > 65536 || self.max_snapshots < 2 {
            bail!("invalid JS pool, queue or snapshot capacity");
        }
        if self.memory_limit_mb.checked_mul(1024 * 1024).is_none()
            || self
                .stack_limit_kb
                .checked_mul(1024)
                .and_then(|stack| stack.checked_add(2 * 1024 * 1024))
                .is_none()
            || self.max_script_bytes > self.max_source_bytes
            || self.max_pending_host_calls > self.max_host_calls
            || self.files.max_file_bytes > self.files.quota_bytes
        {
            bail!("inconsistent JS resource limits");
        }
        if [
            self.max_bridge_bytes,
            self.max_host_buffer_bytes,
            self.max_response_bytes,
            self.max_script_bytes,
            self.max_source_bytes,
            self.realtime.connection_queue_bytes,
            self.realtime.max_message_bytes,
        ]
        .iter()
        .any(|value| *value > u32::MAX as usize)
        {
            bail!("JS byte limits must fit the native 32-bit buffer counters");
        }
        let now = std::time::Instant::now();
        for timeout in [
            self.execution_timeout_ms,
            self.async_timeout_ms,
            self.startup_timeout_ms,
            self.queue_timeout_ms,
            self.shutdown_timeout_ms,
            self.http.connect_timeout_ms,
            self.http.timeout_ms,
            self.cron.max_runtime_ms,
        ] {
            if now
                .checked_add(std::time::Duration::from_millis(timeout))
                .is_none()
            {
                bail!("JS timeout exceeds the platform clock range");
            }
        }
        if self.cron.max_retries > 3
            || self.cron.retries > self.cron.max_retries
            || self.outbox.max_retries > 3
            || self.outbox.renew_seconds >= self.outbox.lease_seconds
            || self.outbox.lease_seconds > i64::MAX as u64 / 1000
            || self.outbox.retention_days > i64::MAX as u64 / 86_400_000
        {
            bail!("invalid JS retry or lease limits");
        }
        self.cron
            .timezone
            .parse::<chrono_tz::Tz>()
            .context("invalid JS IANA timezone")?;
        for origin in self
            .http
            .allowlist
            .iter()
            .chain(&self.outbox.idempotent_origins)
        {
            exact_origin(origin)?;
        }
        for origin in &self.outbox.idempotent_origins {
            let normalized = exact_origin(origin)?;
            if !self
                .http
                .allowlist
                .iter()
                .any(|allowed| exact_origin(allowed).ok().as_ref() == Some(&normalized))
            {
                bail!("outbox idempotent origins must also be HTTP-authorized");
            }
        }
        let prefix = &self.files.prefix;
        if crate::files::validate_key(prefix, false).is_err()
            || prefix.is_empty()
            || prefix.contains(['\\', ':', '\0'])
            || prefix
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
            || matches!(prefix.split('/').next(), Some("records" | "web" | "_"))
        {
            bail!("invalid or reserved JS files prefix");
        }
        for prefix in &self.route_prefixes {
            if !prefix.starts_with('/')
                || prefix.contains(['%', '\\', '{', '}', '?', '#'])
                || prefix.contains("//")
                || prefix.split('/').any(|s| s == "." || s == "..")
            {
                bail!("invalid JS route prefix");
            }
        }
        for name in &self.env_allowlist {
            if name.is_empty()
                || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                || secret_environment(name)
            {
                bail!("invalid or credential-bearing JS environment allowlist entry");
            }
        }
        Ok(())
    }
}

pub fn exact_origin(input: &str) -> anyhow::Result<String> {
    let url = url::Url::parse(input).context("invalid outbound origin")?;
    if has_url_userinfo(input)
        || !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
        || input.contains('*')
    {
        bail!("outbound allowlist must contain exact HTTP(S) origins");
    }
    Ok(url.origin().ascii_serialization())
}

/// URL parsing normalizes away an empty username (`https://@host`). Preserve
/// the raw authority check for both absolute and network-path redirect URLs.
pub fn has_url_userinfo(input: &str) -> bool {
    input
        .split_once("://")
        .map(|(_, rest)| rest)
        .or_else(|| input.strip_prefix("//"))
        .is_some_and(|authority| {
            authority
                .split(['/', '?', '#'])
                .next()
                .unwrap_or_default()
                .contains('@')
        })
}

pub fn secret_environment(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.starts_with("HB_")
        || upper.starts_with("AWS_")
        || upper.starts_with("SMTP_")
        || [
            "TOKEN",
            "PASSWORD",
            "SECRET",
            "CREDENTIAL",
            "PRIVATE_KEY",
            "ACCESS_KEY",
            "PROXY",
        ]
        .iter()
        .any(|part| upper.contains(part))
}

fn collect_paths(
    value: &Value,
    path: Vec<String>,
    output: &mut std::collections::BTreeMap<String, Vec<String>>,
) {
    if let Value::Object(fields) = value {
        for (key, value) in fields {
            let mut child = path.clone();
            child.push(key.clone());
            collect_paths(value, child, output);
        }
    } else {
        output.insert(
            format!("HB_JS_{}", path.join("_").to_ascii_uppercase()),
            path,
        );
    }
}

fn positive_limits(value: &Value, path: &str) -> anyhow::Result<()> {
    if let Value::Object(fields) = value {
        for (name, value) in fields {
            positive_limits(value, &format!("{path}.{name}"))?;
        }
    } else if value.as_u64() == Some(0)
        && !path.ends_with("retries")
        && !path.ends_with("max_redirects")
    {
        bail!("{path} must be positive");
    }
    Ok(())
}

macro_rules! errors {
    ($($kind:ident => ($status:literal, $code:literal, $message:literal)),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum JsErrorKind { $($kind),* }
        impl JsErrorKind {
            pub fn status(self) -> u16 { match self { $(Self::$kind => $status),* } }
            pub fn code(self) -> &'static str { match self { $(Self::$kind => $code),* } }
            pub fn message(self) -> &'static str { match self { $(Self::$kind => $message),* } }
        }
    };
}
errors! {
    Load => (500, "HB_SCRIPT_LOAD_ERROR", "Extension loading failed"),
    Hook => (500, "HB_HOOK_ERROR", "Extension execution failed"),
    Timeout => (504, "HB_HOOK_TIMEOUT", "Extension execution timed out"),
    Oom => (500, "HB_HOOK_OOM", "Extension memory limit exceeded"),
    RouteConflict => (500, "HB_ROUTE_CONFLICT", "Extension route conflicts with another route"),
    Denied => (403, "HB_CAPABILITY_DENIED", "Extension capability is not authorized"),
    OutboundDenied => (403, "HB_OUTBOUND_DENIED", "Outbound destination is not authorized"),
    HttpFailed => (502, "HB_HTTP_SEND_FAILED", "Outbound HTTP request failed"),
    HttpTimeout => (504, "HB_HTTP_TIMEOUT", "Outbound HTTP request timed out"),
    FileDenied => (403, "HB_FILE_ACCESS_DENIED", "Extension file access denied"),
    Aborted => (409, "HB_HOOK_ABORTED", "Extension aborted the operation"),
    Recursion => (409, "HB_HOOK_RECURSION", "Recursive extension write rejected"),
    Busy => (503, "HB_JS_BUSY", "Extension workers are busy"),
    QueryUnsupported => (400, "HB_QUERY_UNSUPPORTED", "Query is outside the supported SELECT subset"),
    SideEffect => (409, "HB_SIDE_EFFECT_IN_TRANSACTION", "External side effects are prohibited in a transaction"),
    CommitUnknown => (503, "HB_COMMIT_UNKNOWN", "Transaction result requires verification"),
    MovePartial => (409, "HB_FILE_MOVE_PARTIAL", "Destination exists but source cleanup did not complete"),
    IdempotencyConflict => (409, "HB_IDEMPOTENCY_CONFLICT", "Idempotency key has a different payload"),
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct JsError {
    pub kind: JsErrorKind,
    pub message: String,
    pub details: Option<Value>,
}
impl JsError {
    pub fn new(kind: JsErrorKind) -> Self {
        Self {
            kind,
            message: kind.message().into(),
            details: None,
        }
    }
    pub fn diagnostic(kind: JsErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            details: None,
        }
    }
}
impl From<JsErrorKind> for crate::HbError {
    fn from(kind: JsErrorKind) -> Self {
        Self::Extension(JsError::new(kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_strict_environment_types() {
        let mut config = JsvmConfig::default();
        config.validate().unwrap();
        assert!(!config.enabled);
        config
            .apply_overrides([
                ("HB_JS_POOL_SIZE".into(), "1".into()),
                (
                    "HB_JS_HTTP_ALLOWLIST".into(),
                    "[\"https://example.com\"]".into(),
                ),
            ])
            .unwrap();
        assert_eq!(config.pool_size, 1);
        assert!(
            config
                .apply_overrides([("HB_JS_ENABLED".into(), "1".into())])
                .is_err()
        );
        assert!(
            serde_json::from_value::<JsvmConfig>(serde_json::json!({"http":{"surprise":true}}))
                .is_err()
        );
    }
    #[test]
    fn origins_and_credentials_are_constrained() {
        assert_eq!(
            exact_origin("https://example.com:443/").unwrap(),
            "https://example.com"
        );
        for origin in [
            "https://*.example.com",
            "https://user:pass@example.com",
            "https://@example.com",
            "https://example.com/path",
            "file:///a",
        ] {
            assert!(exact_origin(origin).is_err());
        }
        assert!(secret_environment("HB_SMTP_PASSWORD"));
        assert!(!secret_environment("PUBLIC_SITE_NAME"));
    }
}
