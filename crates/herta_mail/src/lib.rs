use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use herta_core::{
    HbConfig, HbError, HbResult, MailAddress, MailConfig, MailMessage, MailReceipt, MailStatus,
    Mailer,
};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{
        Mailbox, MultiPart, SinglePart,
        header::{HeaderName, HeaderValue},
    },
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use uuid::Uuid;

pub struct DisabledMailer;

#[async_trait]
impl Mailer for DisabledMailer {
    async fn send(&self, _message: MailMessage) -> HbResult<MailReceipt> {
        Err(HbError::CapabilityUnavailable)
    }
}

pub struct SmtpMailer {
    config: MailConfig,
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

pub fn mailer_from_config(config: &HbConfig) -> HbResult<Arc<dyn Mailer>> {
    config
        .mail
        .validate(config.server.dev_mode)
        .map_err(|_| HbError::validation("invalid mail configuration"))?;
    match config.mail.driver.as_str() {
        "disabled" => Ok(Arc::new(DisabledMailer)),
        "smtp" => Ok(Arc::new(SmtpMailer::new(
            &config.mail,
            config.server.dev_mode,
        )?)),
        _ => Err(HbError::validation("invalid mail driver")),
    }
}

impl SmtpMailer {
    pub fn new(config: &MailConfig, dev_mode: bool) -> HbResult<Self> {
        config
            .validate(dev_mode)
            .map_err(|_| HbError::validation("invalid mail configuration"))?;
        if config.driver != "smtp" {
            return Err(HbError::validation("SMTP mailer requires mail.driver=smtp"));
        }
        let tls = match config.smtp.tls.as_str() {
            "none" => Tls::None,
            mode => {
                let params = TlsParameters::new(config.smtp.host.clone())
                    .map_err(|_| HbError::validation("invalid SMTP TLS configuration"))?;
                if mode == "required" {
                    Tls::Wrapper(params)
                } else {
                    Tls::Required(params)
                }
            }
        };
        let mut builder =
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp.host)
                .port(config.smtp.port)
                .tls(tls)
                .timeout(Some(Duration::from_millis(config.timeout_ms)));
        if let (Some(username), Some(password)) = (&config.smtp.username, &config.smtp.password)
            && !username.is_empty()
            && !password.is_empty()
        {
            builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
        }
        Ok(Self {
            config: config.clone(),
            transport: builder.build(),
        })
    }

    fn build_message(&self, message: MailMessage, message_id: &str) -> HbResult<Message> {
        let message = self.config.prepare(message)?;
        let mut builder = Message::builder()
            .from(mailbox(message.from.ok_or(HbError::Internal)?)?)
            .subject(message.subject)
            .message_id(Some(message_id.to_owned()));
        for address in message.to {
            builder = builder.to(mailbox(address)?);
        }
        for (name, value) in message.headers {
            let name = HeaderName::new_from_ascii(name)
                .map_err(|_| HbError::validation("invalid custom mail header"))?;
            builder = builder.raw_header(HeaderValue::new(name, value));
        }
        let result = match (message.text, message.html) {
            (Some(text), Some(html)) => builder.multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(text))
                    .singlepart(SinglePart::html(html)),
            ),
            (Some(text), None) => builder.singlepart(SinglePart::plain(text)),
            (None, Some(html)) => builder.singlepart(SinglePart::html(html)),
            (None, None) => return Err(HbError::validation("mail body is required")),
        };
        result.map_err(|_| HbError::validation("cannot construct mail message"))
    }
}

#[async_trait]
impl Mailer for SmtpMailer {
    async fn send(&self, message: MailMessage) -> HbResult<MailReceipt> {
        let message_id = format!("<{}@hertabase.local>", Uuid::now_v7());
        let message = self.build_message(message, &message_id)?;
        let started = Instant::now();
        // No retry: after a connection failure or timeout the peer may have accepted DATA.
        let result = tokio::time::timeout(
            Duration::from_millis(self.config.timeout_ms),
            self.transport.send(message),
        )
        .await;
        let error = match result {
            Ok(Ok(_)) => {
                tracing::info!(%message_id, elapsed_ms = started.elapsed().as_millis() as u64, "SMTP accepted mail");
                return Ok(MailReceipt {
                    message_id,
                    status: MailStatus::Accepted,
                });
            }
            Ok(Err(error)) if error.is_timeout() => HbError::MailTimeout,
            Ok(Err(_)) => HbError::MailSendFailed,
            Err(_) => HbError::MailTimeout,
        };
        tracing::warn!(%message_id, elapsed_ms = started.elapsed().as_millis() as u64,
            error_code = error.error_code(), "SMTP submission failed; acceptance may be unknown");
        Err(error)
    }
}

fn mailbox(address: MailAddress) -> HbResult<Mailbox> {
    Ok(Mailbox::new(
        address.name,
        address
            .address
            .parse()
            .map_err(|_| HbError::validation("invalid mail address"))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message() -> MailMessage {
        serde_json::from_value(serde_json::json!({
            "to": [{"address": "reader@example.com"}], "subject": "Hello", "text": "Plain body",
            "html": "<b>HTML body</b>", "headers": {"X-Event-Id": "event-123"}
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn disabled_mailer_returns_service_unavailable() {
        let mailer = mailer_from_config(&HbConfig::default()).unwrap();
        assert!(matches!(
            mailer.send(message()).await,
            Err(HbError::CapabilityUnavailable)
        ));
    }

    #[test]
    fn builds_multipart_and_keeps_correlation_id() {
        let mut config = MailConfig {
            driver: "smtp".into(),
            ..Default::default()
        };
        config.smtp.host = "localhost".into();
        config.smtp.tls = "none".into();
        assert!(SmtpMailer::new(&config, false).is_err());
        let mailer = SmtpMailer::new(&config, true).unwrap();
        let mail = mailer
            .build_message(message(), "<test@hertabase.local>")
            .unwrap();
        let raw = String::from_utf8(mail.formatted()).unwrap();
        for expected in [
            "multipart/alternative",
            "text/plain",
            "text/html",
            "Message-ID: <test@hertabase.local>",
            "X-Event-Id: event-123",
            "noreply@example.com",
            "reader@example.com",
        ] {
            assert!(raw.contains(expected), "missing {expected}: {raw}");
        }
    }

    #[tokio::test]
    async fn local_smtp_mailbox_observes_the_receipt_message_id() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let inbox = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (read, mut write) = stream.into_split();
            let mut lines = BufReader::new(read).lines();
            write.write_all(b"220 mailbox ESMTP\r\n").await.unwrap();
            let mut message = String::new();
            while let Some(line) = lines.next_line().await.unwrap() {
                if line == "DATA" {
                    write.write_all(b"354 send data\r\n").await.unwrap();
                    while let Some(line) = lines.next_line().await.unwrap() {
                        if line == "." {
                            break;
                        }
                        message.push_str(&line);
                        message.push('\n');
                    }
                    write.write_all(b"250 queued\r\n").await.unwrap();
                } else if line == "QUIT" {
                    write.write_all(b"221 bye\r\n").await.unwrap();
                    break;
                } else {
                    write.write_all(b"250 OK\r\n").await.unwrap();
                }
            }
            message
        });
        let mut config = MailConfig {
            driver: "smtp".into(),
            timeout_ms: 3000,
            ..Default::default()
        };
        config.smtp.host = "127.0.0.1".into();
        config.smtp.port = port;
        config.smtp.tls = "none".into();
        let mailer = SmtpMailer::new(&config, true).unwrap();
        let receipt = mailer.send(message()).await.unwrap();
        drop(mailer);
        let received = tokio::time::timeout(Duration::from_secs(5), inbox)
            .await
            .unwrap()
            .unwrap();
        assert!(
            received.contains(&format!("Message-ID: {}", receipt.message_id)),
            "{received}"
        );
        assert!(received.contains("X-Event-Id: event-123"));
        assert_eq!(receipt.status, MailStatus::Accepted);
    }
}
