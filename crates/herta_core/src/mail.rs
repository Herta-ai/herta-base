use std::{collections::BTreeMap, fmt};

use anyhow::{Context, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{HbError, HbResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MailConfig {
    pub driver: String,
    pub from_address: String,
    pub from_name: String,
    /// Additional addresses allowed alongside from_address.
    pub allowed_from_addresses: Vec<String>,
    pub max_recipients: usize,
    pub max_subject_bytes: usize,
    pub max_body_bytes: usize,
    pub max_header_bytes: usize,
    pub timeout_ms: u64,
    pub smtp: SmtpConfig,
}

impl Default for MailConfig {
    fn default() -> Self {
        Self {
            driver: "disabled".into(),
            from_address: "noreply@example.com".into(),
            from_name: "HertaBase".into(),
            allowed_from_addresses: Vec::new(),
            max_recipients: 50,
            max_subject_bytes: 998,
            max_body_bytes: 1024 * 1024,
            max_header_bytes: 8 * 1024,
            timeout_ms: 10_000,
            smtp: SmtpConfig::default(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub tls: String,
    #[serde(skip_serializing)]
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password: Option<String>,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 587,
            tls: "starttls".into(),
            username: None,
            password: None,
        }
    }
}

impl fmt::Debug for SmtpConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmtpConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("tls", &self.tls)
            .field("username", &"[redacted]")
            .field("password", &"[redacted]")
            .finish()
    }
}

impl MailConfig {
    pub(crate) fn apply_env(&mut self) -> anyhow::Result<()> {
        self.apply_env_with(|name| std::env::var(name).ok())
    }

    fn apply_env_with(&mut self, get: impl Fn(&str) -> Option<String>) -> anyhow::Result<()> {
        for (name, target) in [
            ("HB_MAIL_DRIVER", &mut self.driver),
            ("HB_MAIL_FROM_ADDRESS", &mut self.from_address),
            ("HB_MAIL_FROM_NAME", &mut self.from_name),
            ("HB_SMTP_HOST", &mut self.smtp.host),
            ("HB_SMTP_TLS", &mut self.smtp.tls),
        ] {
            if let Some(value) = get(name) {
                *target = value;
            }
        }
        for (name, target) in [
            ("HB_MAIL_MAX_RECIPIENTS", &mut self.max_recipients),
            ("HB_MAIL_MAX_SUBJECT_BYTES", &mut self.max_subject_bytes),
            ("HB_MAIL_MAX_BODY_BYTES", &mut self.max_body_bytes),
            ("HB_MAIL_MAX_HEADER_BYTES", &mut self.max_header_bytes),
        ] {
            if let Some(value) = get(name) {
                *target = value
                    .parse()
                    .with_context(|| format!("{name} must be a positive integer"))?;
            }
        }
        if let Some(value) = get("HB_MAIL_TIMEOUT_MS") {
            self.timeout_ms = value
                .parse()
                .context("HB_MAIL_TIMEOUT_MS must be a positive integer")?;
        }
        if let Some(value) = get("HB_SMTP_PORT") {
            self.smtp.port = value.parse().context("HB_SMTP_PORT must be a valid port")?;
        }
        if let Some(value) = get("HB_MAIL_ALLOWED_FROM_ADDRESSES") {
            self.allowed_from_addresses = value
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
        }
        for (name, target) in [
            ("HB_SMTP_USERNAME", &mut self.smtp.username),
            ("HB_SMTP_PASSWORD", &mut self.smtp.password),
        ] {
            if let Some(value) = get(name) {
                *target = (!value.is_empty()).then_some(value);
            }
        }
        Ok(())
    }

    pub fn validate(&self, dev_mode: bool) -> anyhow::Result<()> {
        if !matches!(self.driver.as_str(), "disabled" | "smtp") {
            bail!("mail.driver must be disabled or smtp");
        }
        if !matches!(self.smtp.tls.as_str(), "required" | "starttls" | "none") {
            bail!("mail.smtp.tls must be required, starttls, or none");
        }
        if self.max_recipients == 0
            || self.max_subject_bytes == 0
            || self.max_body_bytes == 0
            || self.max_header_bytes == 0
            || self.timeout_ms == 0
        {
            bail!("mail limits and timeout must be greater than zero");
        }
        if self.driver == "smtp" {
            if self.smtp.tls == "none" && !dev_mode {
                bail!("mail.smtp.tls=none requires server.dev_mode=true or --dev");
            }
            if self.smtp.host.trim().is_empty()
                || !safe_header(&self.smtp.host)
                || self.smtp.port == 0
            {
                bail!("mail.smtp requires a host and nonzero port");
            }
            if !valid_address(&self.from_address)
                || !safe_header(&self.from_name)
                || self.from_name.len() > 998
                || self
                    .allowed_from_addresses
                    .iter()
                    .any(|address| !valid_address(address))
            {
                bail!("mail sender configuration is invalid");
            }
            let username = self.smtp.username.as_ref().is_some_and(|v| !v.is_empty());
            let password = self.smtp.password.as_ref().is_some_and(|v| !v.is_empty());
            if username != password {
                bail!("SMTP username and password must be provided together");
            }
        }
        Ok(())
    }

    /// Validate and resolve the default sender before a transport sees the message.
    pub fn prepare(&self, mut message: MailMessage) -> HbResult<MailMessage> {
        let from = message.from.get_or_insert_with(|| MailAddress {
            address: self.from_address.clone(),
            name: Some(self.from_name.clone()),
        });
        if from.address != self.from_address && !self.allowed_from_addresses.contains(&from.address)
        {
            return Err(HbError::validation("mail sender is not allowed"));
        }
        if message.to.is_empty() || message.to.len() > self.max_recipients {
            return Err(HbError::validation(
                "mail recipient count is outside configured limits",
            ));
        }
        for address in std::iter::once(&*from).chain(message.to.iter()) {
            if !valid_address(&address.address)
                || address
                    .name
                    .as_ref()
                    .is_some_and(|name| !safe_header(name) || name.len() > 998)
            {
                return Err(HbError::validation("invalid mail address or display name"));
            }
        }
        if message.subject.trim().is_empty()
            || !safe_header(&message.subject)
            || message.subject.len() > self.max_subject_bytes
        {
            return Err(HbError::validation("invalid mail subject"));
        }
        let text_len = message.text.as_ref().map_or(0, String::len);
        let html_len = message.html.as_ref().map_or(0, String::len);
        if text_len.saturating_add(html_len) == 0 {
            return Err(HbError::validation(
                "mail requires a nonempty text or html body",
            ));
        }
        if text_len.saturating_add(html_len) > self.max_body_bytes {
            return Err(HbError::PayloadTooLarge);
        }
        let mut header_bytes = 0usize;
        for (name, value) in &message.headers {
            if name.len() <= 2
                || !name
                    .get(..2)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("x-"))
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || !safe_header(value)
            {
                return Err(HbError::validation(
                    "only valid X-* custom mail headers are allowed",
                ));
            }
            header_bytes = header_bytes
                .saturating_add(name.len())
                .saturating_add(value.len());
        }
        if header_bytes > self.max_header_bytes {
            return Err(HbError::PayloadTooLarge);
        }
        Ok(message)
    }
}

fn safe_header(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

fn valid_address(value: &str) -> bool {
    safe_header(value) && value.len() <= 254 && email_address::EmailAddress::is_valid(value)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MailAddress {
    pub address: String,
    pub name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailMessage {
    pub from: Option<MailAddress>,
    pub to: Vec<MailAddress>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MailReceipt {
    pub message_id: String,
    pub status: MailStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MailStatus {
    Accepted,
}

/// Direct, nontransactional SMTP submission. Success means SMTP acceptance only.
/// Callers must not invoke this from an active database transaction.
#[async_trait]
pub trait Mailer: Send + Sync {
    async fn send(&self, message: MailMessage) -> HbResult<MailReceipt>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message() -> MailMessage {
        serde_json::from_value(serde_json::json!({
            "to": [{"address": "reader@example.com"}], "subject": "你好", "text": "正文"
        }))
        .unwrap()
    }

    #[test]
    fn config_defaults_overrides_and_secret_redaction() {
        let mut config: MailConfig =
            toml::from_str("[smtp]\nusername='private-user'\npassword='private-password'").unwrap();
        assert_eq!(config.driver, "disabled");
        config
            .apply_env_with(|name| match name {
                "HB_SMTP_PASSWORD" => Some("env-password".into()),
                "HB_MAIL_MAX_BODY_BYTES" => Some("2048".into()),
                "HB_SMTP_PORT" => Some("1025".into()),
                _ => None,
            })
            .unwrap();
        assert_eq!(config.smtp.password.as_deref(), Some("env-password"));
        assert_eq!(config.smtp.username.as_deref(), Some("private-user"));
        assert_eq!(config.max_body_bytes, 2048);
        assert_eq!(config.smtp.port, 1025);
        for output in [
            format!("{config:?}"),
            serde_json::to_string(&config).unwrap(),
            toml::to_string(&config).unwrap(),
        ] {
            assert!(!output.contains("private-user"));
            assert!(!output.contains("env-password"));
        }
    }

    #[test]
    fn tls_and_credentials_are_validated() {
        let mut config = MailConfig {
            driver: "smtp".into(),
            ..Default::default()
        };
        config.smtp.host = "localhost".into();
        for tls in ["required", "starttls"] {
            config.smtp.tls = tls.into();
            assert!(config.validate(false).is_ok());
        }
        config.smtp.tls = "none".into();
        assert!(config.validate(false).is_err());
        assert!(config.validate(true).is_ok());
        config.smtp.username = Some("user".into());
        assert!(config.validate(true).is_err());
        config.smtp.password = Some("password".into());
        assert!(config.validate(true).is_ok());
    }

    #[test]
    fn message_limits_senders_and_injection_are_checked() {
        let mut config = MailConfig::default();
        assert_eq!(
            config.prepare(message()).unwrap().from.unwrap().address,
            config.from_address
        );
        for field in [
            "subject",
            "address",
            "name",
            "header",
            "unicode_header",
            "reserved_header",
        ] {
            let mut mail = message();
            match field {
                "subject" => mail.subject = "hello\r\nBcc: victim@example.com".into(),
                "address" => mail.to[0].address = "invalid".into(),
                "name" => mail.to[0].name = Some("name\nBcc: victim@example.com".into()),
                "header" => {
                    mail.headers.insert("X-Test".into(), "a\r\nb".into());
                }
                "unicode_header" => {
                    mail.headers.insert("你-Test".into(), "v".into());
                }
                _ => {
                    mail.headers
                        .insert("Bcc".into(), "victim@example.com".into());
                }
            }
            assert!(config.prepare(mail).is_err(), "{field}");
        }
        let mut mail = message();
        mail.from = Some(MailAddress {
            address: "other@example.com".into(),
            name: None,
        });
        assert!(config.prepare(mail.clone()).is_err());
        config
            .allowed_from_addresses
            .push("other@example.com".into());
        assert!(config.prepare(mail).is_ok());
        let mut mail = message();
        mail.to.clear();
        assert!(config.prepare(mail).is_err());
        let mut mail = message();
        mail.text = None;
        assert!(config.prepare(mail).is_err());
        config.max_body_bytes = 5;
        assert!(matches!(
            config.prepare(message()),
            Err(HbError::PayloadTooLarge)
        ));
        config.max_body_bytes = 6;
        assert!(config.prepare(message()).is_ok());
        config.max_subject_bytes = 5;
        assert!(config.prepare(message()).is_err());
        config.max_subject_bytes = 6;
        let mut mail = message();
        mail.headers.insert("X-Id".into(), "123".into());
        config.max_header_bytes = 6;
        assert!(matches!(
            config.prepare(mail),
            Err(HbError::PayloadTooLarge)
        ));
        assert!(
            serde_json::from_value::<MailMessage>(serde_json::json!({
                "to": [], "subject": "test", "bcc": []
            }))
            .is_err()
        );
    }
}
