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
}
