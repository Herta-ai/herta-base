use std::path::Path;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct HbConfig {
    pub server: ServerConfig,
    pub paths: PathsConfig,
    pub database: DatabaseConfig,
    pub log: LogConfig,
    pub auth: AuthConfig,
    pub realtime: RealtimeConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct StorageConfig {
    #[serde(rename = "type")]
    pub storage_type: String,
    pub max_file_size: usize,
    pub file_token_ttl_seconds: u64,
    pub s3: S3Config,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: "local".into(),
            max_file_size: 10 * 1024 * 1024,
            file_token_ttl_seconds: 5 * 60,
            s3: S3Config::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct S3Config {
    pub endpoint: Option<String>,
    pub bucket: String,
    pub region: String,
    pub prefix: String,
    pub force_path_style: bool,
    pub allow_http: bool,
    #[serde(skip)]
    pub access_key: Option<String>,
    #[serde(skip)]
    pub secret_key: Option<String>,
    #[serde(skip)]
    pub session_token: Option<String>,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint: None,
            bucket: String::new(),
            region: "us-east-1".into(),
            prefix: "hertabase".into(),
            force_path_style: true,
            allow_http: false,
            access_key: None,
            secret_key: None,
            session_token: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RealtimeConfig {
    pub max_connections: usize,
    pub max_connections_per_ip: usize,
    pub heartbeat_seconds: u64,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            max_connections_per_ip: 20,
            heartbeat_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AuthConfig {
    pub access_token_ttl_seconds: u64,
    pub refresh_token_ttl_seconds: u64,
    pub lockout_threshold: u32,
    pub lockout_seconds: u64,
    pub register_rate_limit_per_minute: u32,
    pub login_rate_limit_per_minute: u32,
    pub refresh_rate_limit_per_minute: u32,
    #[serde(skip)]
    pub jwt_secret: Option<String>,
    #[serde(skip)]
    pub bootstrap_admin_email: Option<String>,
    #[serde(skip)]
    pub bootstrap_admin_password: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            access_token_ttl_seconds: 15 * 60,
            refresh_token_ttl_seconds: 7 * 24 * 60 * 60,
            lockout_threshold: 5,
            lockout_seconds: 15 * 60,
            register_rate_limit_per_minute: 5,
            login_rate_limit_per_minute: 10,
            refresh_rate_limit_per_minute: 30,
            jwt_secret: None,
            bootstrap_admin_email: None,
            bootstrap_admin_password: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub dev_mode: bool,
    pub max_body_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PathsConfig {
    pub data_dir: String,
    pub hooks_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DatabaseConfig {
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LogConfig {
    pub level: String,
    pub format: String,
    /// Whether application/server tracing events are persisted in `_logs`.
    pub server_persist_enabled: bool,
    /// Minimum application/server tracing level persisted in `_logs`.
    pub server_persist_level: String,
    /// Whether HTTP request metadata is persisted in `_logs`.
    pub http_persist_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 8080,
            dev_mode: false,
            max_body_size: 10 * 1024 * 1024,
        }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            data_dir: "./hb_data".into(),
            hooks_dir: "./hb_hooks".into(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            engine: "surrealkv".into(),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: "pretty".into(),
            server_persist_enabled: true,
            server_persist_level: "info".into(),
            http_persist_enabled: true,
        }
    }
}

impl HbConfig {
    pub fn load(config_path: Option<&Path>) -> anyhow::Result<Self> {
        let path = config_path.map(Path::to_path_buf).or_else(|| {
            Path::new("hertabase.toml")
                .exists()
                .then(|| "hertabase.toml".into())
        });

        let mut config = if let Some(path) = path {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read config file {}", path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("failed to parse config file {}", path.display()))?
        } else {
            Self::default()
        };

        config.apply_env()?;
        config.validate()?;
        Ok(config)
    }

    fn apply_env(&mut self) -> anyhow::Result<()> {
        if let Ok(value) = std::env::var("HB_HOST") {
            self.server.host = value;
        }
        if let Ok(value) = std::env::var("HB_PORT") {
            self.server.port = value.parse().context("HB_PORT must be a valid u16")?;
        }
        if let Ok(value) = std::env::var("HB_DATA_DIR") {
            self.paths.data_dir = value;
        }
        if let Ok(value) = std::env::var("HB_HOOKS_DIR") {
            self.paths.hooks_dir = value;
        }
        if let Ok(value) = std::env::var("HB_DB_ENGINE") {
            self.database.engine = value;
        }
        if let Ok(value) = std::env::var("HB_LOG_LEVEL") {
            self.log.level = value;
        }
        if let Ok(value) = std::env::var("HB_LOG_FORMAT") {
            self.log.format = value;
        }
        if let Ok(value) = std::env::var("HB_LOG_SERVER_PERSIST_ENABLED") {
            self.log.server_persist_enabled = value
                .parse()
                .context("HB_LOG_SERVER_PERSIST_ENABLED must be true or false")?;
        }
        if let Ok(value) = std::env::var("HB_LOG_SERVER_PERSIST_LEVEL") {
            self.log.server_persist_level = value;
        }
        if let Ok(value) = std::env::var("HB_LOG_HTTP_PERSIST_ENABLED") {
            self.log.http_persist_enabled = value
                .parse()
                .context("HB_LOG_HTTP_PERSIST_ENABLED must be true or false")?;
        }
        if let Ok(value) = std::env::var("HB_MAX_REQUEST_BODY_SIZE") {
            self.server.max_body_size = value
                .parse()
                .context("HB_MAX_REQUEST_BODY_SIZE must be a positive integer")?;
        }
        apply_u64_env(
            "HB_AUTH_ACCESS_TOKEN_TTL_SECONDS",
            &mut self.auth.access_token_ttl_seconds,
        )?;
        apply_u64_env(
            "HB_AUTH_REFRESH_TOKEN_TTL_SECONDS",
            &mut self.auth.refresh_token_ttl_seconds,
        )?;
        apply_u32_env(
            "HB_AUTH_LOCKOUT_THRESHOLD",
            &mut self.auth.lockout_threshold,
        )?;
        apply_u64_env("HB_AUTH_LOCKOUT_SECONDS", &mut self.auth.lockout_seconds)?;
        apply_u32_env(
            "HB_AUTH_REGISTER_RATE_LIMIT_PER_MINUTE",
            &mut self.auth.register_rate_limit_per_minute,
        )?;
        apply_u32_env(
            "HB_AUTH_LOGIN_RATE_LIMIT_PER_MINUTE",
            &mut self.auth.login_rate_limit_per_minute,
        )?;
        apply_u32_env(
            "HB_AUTH_REFRESH_RATE_LIMIT_PER_MINUTE",
            &mut self.auth.refresh_rate_limit_per_minute,
        )?;
        apply_usize_env(
            "HB_REALTIME_MAX_CONNECTIONS",
            &mut self.realtime.max_connections,
        )?;
        apply_usize_env(
            "HB_REALTIME_MAX_CONNECTIONS_PER_IP",
            &mut self.realtime.max_connections_per_ip,
        )?;
        apply_u64_env(
            "HB_REALTIME_HEARTBEAT_SECONDS",
            &mut self.realtime.heartbeat_seconds,
        )?;
        if let Ok(value) = std::env::var("HB_STORAGE_TYPE") {
            self.storage.storage_type = value;
        }
        apply_usize_env("HB_STORAGE_MAX_FILE_SIZE", &mut self.storage.max_file_size)?;
        apply_u64_env(
            "HB_STORAGE_FILE_TOKEN_TTL_SECONDS",
            &mut self.storage.file_token_ttl_seconds,
        )?;
        if let Ok(value) = std::env::var("HB_S3_ENDPOINT") {
            self.storage.s3.endpoint = (!value.trim().is_empty()).then_some(value);
        }
        if let Ok(value) = std::env::var("HB_S3_BUCKET") {
            self.storage.s3.bucket = value;
        }
        if let Ok(value) = std::env::var("HB_S3_REGION") {
            self.storage.s3.region = value;
        }
        if let Ok(value) = std::env::var("HB_S3_PREFIX") {
            self.storage.s3.prefix = value;
        }
        if let Ok(value) = std::env::var("HB_S3_FORCE_PATH_STYLE") {
            self.storage.s3.force_path_style = value
                .parse()
                .context("HB_S3_FORCE_PATH_STYLE must be true or false")?;
        }
        if let Ok(value) = std::env::var("HB_S3_ALLOW_HTTP") {
            self.storage.s3.allow_http = value
                .parse()
                .context("HB_S3_ALLOW_HTTP must be true or false")?;
        }
        self.storage.s3.access_key = std::env::var("HB_S3_ACCESS_KEY").ok();
        self.storage.s3.secret_key = std::env::var("HB_S3_SECRET_KEY").ok();
        self.storage.s3.session_token = std::env::var("HB_S3_SESSION_TOKEN").ok();
        self.auth.jwt_secret = std::env::var("HB_JWT_SECRET").ok();
        self.auth.bootstrap_admin_email = std::env::var("HB_BOOTSTRAP_ADMIN_EMAIL").ok();
        self.auth.bootstrap_admin_password = std::env::var("HB_BOOTSTRAP_ADMIN_PASSWORD").ok();
        Ok(())
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.server.host.trim().is_empty() {
            bail!("server.host cannot be empty");
        }
        if self.server.max_body_size == 0 {
            bail!("server.max_body_size must be greater than zero");
        }
        if !matches!(self.database.engine.as_str(), "memory" | "surrealkv") {
            bail!("database.engine must be either 'memory' or 'surrealkv'");
        }
        if !matches!(self.log.format.as_str(), "pretty" | "json") {
            bail!("log.format must be either 'pretty' or 'json'");
        }
        if !is_log_level(&self.log.level) {
            bail!("log.level must be trace, debug, info, warn, or error");
        }
        if !is_log_level(&self.log.server_persist_level) {
            bail!("log.server_persist_level must be trace, debug, info, warn, or error");
        }
        if self.auth.access_token_ttl_seconds == 0
            || self.auth.refresh_token_ttl_seconds == 0
            || self.auth.lockout_threshold == 0
            || self.auth.lockout_seconds == 0
            || self.auth.register_rate_limit_per_minute == 0
            || self.auth.login_rate_limit_per_minute == 0
            || self.auth.refresh_rate_limit_per_minute == 0
        {
            bail!("auth durations, thresholds, and rate limits must be greater than zero");
        }
        if self
            .auth
            .jwt_secret
            .as_ref()
            .is_some_and(|secret| secret.len() < 32)
        {
            bail!("HB_JWT_SECRET must contain at least 32 bytes");
        }
        if self.realtime.max_connections == 0
            || self.realtime.max_connections_per_ip == 0
            || self.realtime.heartbeat_seconds == 0
        {
            bail!("realtime connection limits and heartbeat must be greater than zero");
        }
        if !matches!(self.storage.storage_type.as_str(), "local" | "s3") {
            bail!("storage.type must be either 'local' or 's3'");
        }
        if self.storage.max_file_size == 0 {
            bail!("storage.max_file_size must be greater than zero");
        }
        if !(1..=86_400).contains(&self.storage.file_token_ttl_seconds) {
            bail!("storage.file_token_ttl_seconds must be between 1 and 86400");
        }
        if self.storage.storage_type == "s3" {
            if self.storage.s3.bucket.trim().is_empty() {
                bail!("storage.s3.bucket is required when storage.type is 's3'");
            }
            if self.storage.s3.region.trim().is_empty() {
                bail!("storage.s3.region cannot be empty");
            }
            if self.storage.s3.access_key.is_some() != self.storage.s3.secret_key.is_some() {
                bail!("HB_S3_ACCESS_KEY and HB_S3_SECRET_KEY must be provided together");
            }
            if self.storage.s3.prefix.contains('\\')
                || self.storage.s3.prefix.contains('\0')
                || self.storage.s3.prefix.split('/').any(|part| part == "..")
            {
                bail!("storage.s3.prefix contains an invalid path segment");
            }
            if let Some(endpoint) = &self.storage.s3.endpoint {
                let endpoint =
                    url::Url::parse(endpoint).context("storage.s3.endpoint must be a valid URL")?;
                if !matches!(endpoint.scheme(), "http" | "https") {
                    bail!("storage.s3.endpoint must use http or https");
                }
                if endpoint.scheme() == "http"
                    && (!self.storage.s3.allow_http || !self.server.dev_mode)
                {
                    bail!(
                        "HTTP S3 endpoints require storage.s3.allow_http=true and server.dev_mode=true"
                    );
                }
            }
        }
        Ok(())
    }
}

fn is_log_level(level: &str) -> bool {
    matches!(level, "trace" | "debug" | "info" | "warn" | "error")
}

fn apply_u64_env(name: &str, target: &mut u64) -> anyhow::Result<()> {
    if let Ok(value) = std::env::var(name) {
        *target = value
            .parse()
            .with_context(|| format!("{name} must be a positive integer"))?;
    }
    Ok(())
}

fn apply_u32_env(name: &str, target: &mut u32) -> anyhow::Result<()> {
    if let Ok(value) = std::env::var(name) {
        *target = value
            .parse()
            .with_context(|| format!("{name} must be a positive integer"))?;
    }
    Ok(())
}

fn apply_usize_env(name: &str, target: &mut usize) -> anyhow::Result<()> {
    if let Ok(value) = std::env::var(name) {
        *target = value
            .parse()
            .with_context(|| format!("{name} must be a positive integer"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_toml_uses_nested_defaults() {
        let config: HbConfig = toml::from_str("[server]\nport = 9000").unwrap();
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.database.engine, "surrealkv");
        assert_eq!(config.realtime.heartbeat_seconds, 30);
        assert_eq!(config.storage.storage_type, "local");
        assert_eq!(config.storage.file_token_ttl_seconds, 300);
    }

    #[test]
    fn realtime_values_must_be_positive() {
        let mut config = HbConfig::default();
        config.realtime.max_connections = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn log_persistence_defaults_are_enabled_at_info() {
        let config = HbConfig::default();
        assert!(config.log.server_persist_enabled);
        assert_eq!(config.log.server_persist_level, "info");
        assert!(config.log.http_persist_enabled);
    }

    #[test]
    fn invalid_log_persistence_level_is_rejected() {
        let mut config = HbConfig::default();
        config.log.server_persist_level = "verbose".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn s3_requires_a_bucket_and_rejects_http_in_production() {
        let mut config = HbConfig::default();
        config.storage.storage_type = "s3".into();
        assert!(config.validate().is_err());
        config.storage.s3.bucket = "files".into();
        config.storage.s3.endpoint = Some("http://127.0.0.1:9000".into());
        config.storage.s3.allow_http = true;
        assert!(config.validate().is_err());
        config.server.dev_mode = true;
        assert!(config.validate().is_ok());
    }
}
