//! Native, leased outbox delivery. An unknown transport outcome never means unsent.
use herta_core::{
    HbConfig, HbError, HbResult, JsErrorKind, MailMessage, Mailer,
    http::HttpRequest,
    outbox::{OutboxEnqueue, OutboxKind, OutboxState},
};
use herta_db::{
    DbClient,
    outbox::{self, Job},
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

fn encoded(value: impl serde::Serialize) -> HbResult<Value> {
    serde_json::to_value(value).map_err(|_| HbError::Internal)
}

/// Reused at enqueue and immediately before delivery, including current target policy.
pub async fn validate(config: &HbConfig, input: Value) -> HbResult<OutboxEnqueue> {
    validate_with_http(config, input, herta_http::HttpService::new).await
}

async fn validate_with_http(
    config: &HbConfig,
    input: Value,
    http: fn(herta_core::jsvm::JsHttpConfig) -> HbResult<herta_http::HttpService>,
) -> HbResult<OutboxEnqueue> {
    if !config.jsvm.outbox.enabled {
        return Err(JsErrorKind::Denied.into());
    }
    let kind: OutboxKind = serde_json::from_value(input["kind"].clone())
        .map_err(|_| HbError::validation("unsupported outbox kind"))?;
    match kind {
        OutboxKind::Mail if !config.jsvm.mail.enabled => return Err(JsErrorKind::Denied.into()),
        OutboxKind::Mail if config.mail.driver != "smtp" => {
            return Err(HbError::CapabilityUnavailable);
        }
        OutboxKind::Http if !config.jsvm.http.enabled => return Err(JsErrorKind::Denied.into()),
        _ => {}
    }
    let mut input: OutboxEnqueue =
        serde_json::from_value(input).map_err(|_| HbError::validation("invalid outbox request"))?;
    input.payload = match kind {
        OutboxKind::Mail => {
            let message: MailMessage = serde_json::from_value(input.payload)
                .map_err(|_| HbError::validation("invalid mail message"))?;
            let mut limits = config.mail.clone();
            limits.max_recipients = limits.max_recipients.min(config.jsvm.mail.max_recipients);
            limits.max_body_bytes = limits.max_body_bytes.min(config.jsvm.mail.max_body_bytes);
            encoded(limits.prepare(message)?)?
        }
        OutboxKind::Http => {
            let request: HttpRequest = serde_json::from_value(input.payload)
                .map_err(|_| HbError::validation("invalid HTTP request"))?;
            http(config.jsvm.http.clone())?.validate(&request).await?;
            encoded(request)?
        }
    };
    Ok(input)
}

#[derive(Clone)]
pub struct OutboxService {
    db: DbClient,
    config: Arc<HbConfig>,
    mailer: Arc<dyn Mailer>,
    #[cfg(test)]
    http: Option<fn(herta_core::jsvm::JsHttpConfig) -> HbResult<herta_http::HttpService>>,
}
impl OutboxService {
    pub fn new(db: DbClient, config: Arc<HbConfig>, mailer: Arc<dyn Mailer>) -> Self {
        Self {
            db,
            config,
            mailer,
            #[cfg(test)]
            http: None,
        }
    }

    fn http_factory(
        &self,
    ) -> fn(herta_core::jsvm::JsHttpConfig) -> HbResult<herta_http::HttpService> {
        #[cfg(test)]
        if let Some(http) = self.http {
            return http;
        }
        herta_http::HttpService::new
    }

    /// Claims at most one job. Public for deterministic embedding and integration tests.
    pub async fn deliver_one(&self) -> HbResult<bool> {
        if !self.config.jsvm.enabled || !self.config.jsvm.outbox.enabled {
            return Ok(false);
        }
        let Some(mut job) = outbox::claim(&self.db, self.config.jsvm.outbox.lease_seconds).await?
        else {
            return Ok(false);
        };
        let input = json!({"kind":job.receipt.kind,"payload":job.payload,"idempotencyKey":job.idempotency_key});
        if let Err(error) = validate_with_http(&self.config, input, self.http_factory()).await {
            finish_preflight(&mut job, error, self.config.jsvm.outbox.max_retries);
            outbox::update_owned(&self.db, &job).await?;
            return Ok(true);
        }
        outbox::sending(&self.db, &mut job).await?;
        // Sending marker is durable before entering either transport.
        let outcome = self.deliver_with_renewal(&job).await;
        finish(&mut job, outcome, self.config.jsvm.outbox.max_retries);
        outbox::update_owned(&self.db, &job).await?;
        Ok(true)
    }

    async fn deliver_with_renewal(&self, job: &Job) -> Outcome {
        let send = self.deliver(job);
        tokio::pin!(send);
        let mut interval =
            tokio::time::interval(Duration::from_secs(self.config.jsvm.outbox.renew_seconds));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            tokio::select! {
                result=&mut send=>return result,
                _=interval.tick()=>{
                    if let Err(error)=outbox::renew(&self.db,job,self.config.jsvm.outbox.lease_seconds).await {
                        tracing::error!(%error,job_id=%job.receipt.job_id,"outbox lease renewal failed; waiting for transport without retry");
                        // Keep waiting: dropping the future is not evidence that delivery stopped.
                        let _=send.await;
                        return Outcome::Unknown("HB_OUTBOX_LEASE_LOST".into());
                    }
                }
            }
        }
    }

    async fn deliver(&self, job: &Job) -> Outcome {
        match job.receipt.kind {
            OutboxKind::Mail => {
                let Ok(message) = serde_json::from_value::<MailMessage>(job.payload.clone()) else {
                    return Outcome::Failed("HB_VALIDATION_ERROR".into());
                };
                match tokio::time::timeout(
                    Duration::from_millis(self.config.mail.timeout_ms),
                    self.mailer.send(message),
                )
                .await
                {
                    Ok(Ok(receipt)) => Outcome::Accepted(
                        json!({"messageId":receipt.message_id,"status":"accepted"}),
                    ),
                    Ok(Err(error)) => Outcome::Unknown(error.error_code().into()),
                    Err(_) => Outcome::Unknown("HB_MAIL_TIMEOUT".into()),
                }
            }
            OutboxKind::Http => {
                let Ok(mut request) = serde_json::from_value::<HttpRequest>(job.payload.clone())
                else {
                    return Outcome::Failed("HB_VALIDATION_ERROR".into());
                };
                let origin = url::Url::parse(&request.url)
                    .ok()
                    .map(|url| url.origin().ascii_serialization());
                let idempotent = self
                    .config
                    .jsvm
                    .outbox
                    .idempotent_origins
                    .iter()
                    .any(|allowed| herta_core::jsvm::exact_origin(allowed).ok() == origin);
                let mut config = self.config.jsvm.http.clone();
                if idempotent {
                    // A redirect may change the receiver's idempotency scope. Do not follow it.
                    config.max_redirects = 0;
                    request
                        .headers
                        .retain(|key, _| !key.eq_ignore_ascii_case("idempotency-key"));
                    request
                        .headers
                        .insert("Idempotency-Key".into(), job.idempotency_key.clone());
                }
                let service = match (self.http_factory())(config) {
                    Ok(service) => service,
                    Err(error) => return Outcome::Failed(error.error_code().into()),
                };
                match service
                    .send(request, self.config.jsvm.http.timeout_ms)
                    .await
                {
                    Ok(response) => Outcome::Accepted(json!({"status":response.status})),
                    Err(error) if error.delivery == herta_http::Delivery::NotSent => {
                        if matches!(
                            error.error.error_code(),
                            "HB_HTTP_SEND_FAILED" | "HB_HTTP_TIMEOUT"
                        ) {
                            Outcome::Retry(error.error.error_code().into())
                        } else {
                            Outcome::Failed(error.error.error_code().into())
                        }
                    }
                    Err(error) if idempotent => {
                        Outcome::RetryUncertain(error.error.error_code().into())
                    }
                    Err(error) => Outcome::Unknown(error.error.error_code().into()),
                }
            }
        }
    }

    pub fn start(self) -> OutboxWorker {
        let stop = CancellationToken::new();
        let stopped = stop.clone();
        let timeout = self.config.jsvm.shutdown_timeout_ms;
        let task = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            let mut next_purge = std::time::Instant::now();
            let mut interval = tokio::time::interval(Duration::from_millis(250));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _=stopped.cancelled()=>break,
                    _=interval.tick()=>{
                        if let Err(error)=outbox::recover_expired(&self.db).await { tracing::error!(%error,"outbox lease recovery failed"); continue; }
                        if std::time::Instant::now() >= next_purge {
                            if let Err(error)=outbox::purge(&self.db,self.config.jsvm.outbox.retention_days).await { tracing::warn!(%error,"outbox retention cleanup deferred"); }
                            next_purge=std::time::Instant::now()+Duration::from_secs(60);
                        }
                        while tasks.len()<self.config.jsvm.outbox.concurrency {
                            let service=self.clone(); let stop=stopped.clone();
                            tasks.spawn(async move {
                                if !stop.is_cancelled() && let Err(error)=service.deliver_one().await { tracing::error!(%error,"outbox delivery deferred to durable lease recovery"); }
                            });
                        }
                    },
                    _=tasks.join_next(),if !tasks.is_empty()=>{}
                }
            }
            if tokio::time::timeout(Duration::from_millis(timeout), async {
                while tasks.join_next().await.is_some() {}
            })
            .await
            .is_err()
            {
                // Abort cancels waits only; durable sending leases become unknown after expiry.
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
            }
        });
        OutboxWorker {
            stop,
            task: Some(task),
        }
    }
}

enum Outcome {
    Accepted(Value),
    Failed(String),
    Retry(String),
    RetryUncertain(String),
    Unknown(String),
}
fn finish(job: &mut Job, outcome: Outcome, max_retries: usize) {
    job.receipt.updated_at = outbox::now();
    job.lease_until = 0;
    if matches!(&outcome, Outcome::Unknown(_) | Outcome::RetryUncertain(_)) {
        job.uncertain = true;
    }
    match outcome {
        Outcome::Accepted(receipt) => {
            job.uncertain = false;
            job.receipt.state = OutboxState::Accepted;
            job.receipt.result = Some(receipt);
            job.receipt.error_code = None;
        }
        Outcome::Retry(error) | Outcome::RetryUncertain(error)
            if job.receipt.attempts <= max_retries =>
        {
            job.receipt.state = OutboxState::Pending;
            job.receipt.error_code = Some(error);
            job.receipt.next_attempt_at =
                outbox::now() + (1i64 << job.receipt.attempts.saturating_sub(1).min(2)) * 1000;
        }
        Outcome::Unknown(error) | Outcome::RetryUncertain(error) => {
            job.receipt.state = OutboxState::Unknown;
            job.receipt.error_code = Some(error);
        }
        Outcome::Failed(error) | Outcome::Retry(error) => {
            job.receipt.state = if job.uncertain {
                OutboxState::Unknown
            } else {
                OutboxState::Failed
            };
            job.receipt.error_code = Some(error);
        }
    }
}

fn finish_preflight(job: &mut Job, error: HbError, max_retries: usize) {
    // DNS/timeouts here are provably before transport entry. Keep their retry
    // counter separate so the public attempts still counts transport entries.
    job.preflight_failures += 1;
    let retry = job.receipt.kind == OutboxKind::Http
        && matches!(
            error.error_code(),
            "HB_HTTP_SEND_FAILED" | "HB_HTTP_TIMEOUT"
        )
        && job.preflight_failures <= max_retries;
    finish(job, Outcome::Failed(error.error_code().into()), max_retries);
    if retry {
        job.receipt.state = OutboxState::Pending;
        job.receipt.next_attempt_at =
            outbox::now() + (1i64 << (job.preflight_failures - 1).min(2)) * 1000;
    }
}
pub struct OutboxWorker {
    stop: CancellationToken,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl OutboxWorker {
    pub fn stop(&self) {
        self.stop.cancel();
    }
    pub async fn shutdown(&mut self) -> HbResult<()> {
        self.stop();
        if let Some(task) = self.task.take() {
            task.await.map_err(|_| HbError::Internal)?;
        }
        Ok(())
    }
}
impl Drop for OutboxWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use herta_core::outbox::OutboxReceipt;

    fn job() -> Job {
        Job {
            receipt: OutboxReceipt {
                job_id: "one".into(),
                kind: OutboxKind::Http,
                state: OutboxState::Sending,
                attempts: 1,
                created_at: 0,
                updated_at: 0,
                next_attempt_at: 0,
                error_code: None,
                result: None,
            },
            payload: Value::Null,
            payload_hash: String::new(),
            idempotency_key: "stable".into(),
            lease_owner: Some("attempt".into()),
            lease_until: 1,
            resolved_by: None,
            resolution_note: None,
            uncertain: false,
            preflight_failures: 0,
        }
    }
    #[test]
    fn retry_schedule_has_three_delays_and_preserves_uncertainty_when_exhausted() {
        let mut job = job();
        for (attempt, delay) in [(1, 1000), (2, 2000), (3, 4000)] {
            job.receipt.attempts = attempt;
            let before = outbox::now();
            finish(&mut job, Outcome::Retry("HB_HTTP_SEND_FAILED".into()), 3);
            assert_eq!(job.receipt.state, OutboxState::Pending);
            assert!(
                (before + delay..=outbox::now() + delay).contains(&job.receipt.next_attempt_at)
            );
        }
        job.receipt.attempts = 4;
        finish(&mut job, Outcome::Retry("HB_HTTP_SEND_FAILED".into()), 3);
        assert_eq!(job.receipt.state, OutboxState::Failed);
        finish(
            &mut job,
            Outcome::RetryUncertain("HB_HTTP_TIMEOUT".into()),
            3,
        );
        assert_eq!(job.receipt.state, OutboxState::Unknown);
        job.receipt.attempts = 1;
        finish(&mut job, Outcome::Unknown("HB_MAIL_TIMEOUT".into()), 3);
        assert_eq!(job.receipt.state, OutboxState::Unknown);
        assert_eq!(job.idempotency_key, "stable");
    }

    #[test]
    fn later_unsent_failures_cannot_erase_an_earlier_uncertain_delivery() {
        let mut job = job();
        finish(
            &mut job,
            Outcome::RetryUncertain("HB_HTTP_TIMEOUT".into()),
            3,
        );
        assert_eq!(job.receipt.state, OutboxState::Pending);
        // The flag must survive storage and lease recovery between attempts.
        let mut job: Job = serde_json::from_value(serde_json::to_value(job).unwrap()).unwrap();
        job.receipt.attempts = 4;
        finish(&mut job, Outcome::Retry("HB_HTTP_SEND_FAILED".into()), 3);
        assert_eq!(job.receipt.state, OutboxState::Unknown);
        finish_preflight(&mut job, JsErrorKind::Denied.into(), 3);
        assert_eq!(job.receipt.state, OutboxState::Unknown);
        finish(&mut job, Outcome::Accepted(json!({"status":200})), 3);
        assert_eq!(job.receipt.state, OutboxState::Accepted);
        assert!(!job.uncertain);
    }

    #[test]
    fn temporary_preflight_failures_have_a_bounded_independent_retry_budget() {
        let mut job = job();
        job.receipt.attempts = 0;
        for failure in 1..=4 {
            finish_preflight(&mut job, JsErrorKind::HttpTimeout.into(), 3);
            assert_eq!(job.preflight_failures, failure);
            assert_eq!(job.receipt.attempts, 0);
            assert_eq!(
                job.receipt.state,
                if failure <= 3 {
                    OutboxState::Pending
                } else {
                    OutboxState::Failed
                }
            );
        }
    }

    #[tokio::test]
    async fn http_outbox_lost_ack_retries_only_idempotent_receivers_with_a_stable_key() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for (idempotent, acknowledge_retry, sends) in
            [(false, false, 1), (true, true, 2), (true, false, 4)]
        {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let receiver = tokio::spawn(async move {
                let mut accepted = std::collections::BTreeMap::new();
                let mut requests = Vec::new();
                for index in 0..sends {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let mut bytes = Vec::new();
                    while !bytes.ends_with(b"\r\n\r\n") {
                        bytes.push(stream.read_u8().await.unwrap());
                        assert!(bytes.len() < 8192);
                    }
                    let headers = String::from_utf8(bytes).unwrap().to_ascii_lowercase();
                    let header = |name: &str| {
                        headers
                            .lines()
                            .find_map(|line| line.strip_prefix(name))
                            .map(str::trim)
                            .unwrap()
                            .to_owned()
                    };
                    let key = header("idempotency-key:");
                    let length: usize = header("content-length:").parse().unwrap();
                    let mut body = vec![0; length];
                    stream.read_exact(&mut body).await.unwrap();
                    if let Some(prior) = accepted.get(&key) {
                        assert_eq!(prior, &body);
                    } else {
                        accepted.insert(key.clone(), body.clone());
                    }
                    requests.push((key, body));
                    // The receiver durably accepts the first request but loses the response.
                    if index > 0 && acknowledge_retry {
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").await.unwrap();
                    }
                    stream.shutdown().await.unwrap();
                }
                (accepted.len(), requests)
            });
            let db = DbClient::memory().await.unwrap();
            let mut config = HbConfig::default();
            config.jsvm.enabled = true;
            config.jsvm.outbox.enabled = true;
            config.jsvm.http.enabled = true;
            config.jsvm.http.allowlist = vec![origin.clone()];
            if idempotent {
                config.jsvm.outbox.idempotent_origins = vec![origin.clone()];
            }
            let input = json!({"kind":"http.send","idempotencyKey":"stable-native-key","payload":{
                "url":format!("{origin}/receive"),"method":"POST","headers":{"iDeMpOtEnCy-KeY":"untrusted-key"},"body":"private-payload"}});
            let input =
                validate_with_http(&config, input, herta_http::HttpService::for_loopback_test)
                    .await
                    .unwrap();
            let tx = db.inner().clone().begin().await.unwrap();
            let receipt = outbox::enqueue((&tx).into(), input, 100).await.unwrap();
            tx.commit().await.unwrap();
            let mut service = OutboxService::new(
                db.clone(),
                Arc::new(config),
                Arc::new(herta_mail::DisabledMailer),
            );
            service.http = Some(herta_http::HttpService::for_loopback_test);
            for attempt in 1..=sends {
                assert!(service.deliver_one().await.unwrap());
                let job = outbox::get((&db).into(), &receipt.job_id).await.unwrap();
                assert_eq!(job.receipt.attempts, attempt);
                if attempt < sends {
                    assert_eq!(job.receipt.state, OutboxState::Pending);
                    assert!(!service.deliver_one().await.unwrap());
                    // Move only the test clock boundary; retries still traverse claim/send/CAS.
                    db.inner().query("UPDATE _extension_outbox SET nextAttemptAt = 0 WHERE state = 'pending'")
                        .await.unwrap().check().unwrap();
                }
            }
            let job = outbox::get((&db).into(), &receipt.job_id).await.unwrap();
            assert_eq!(
                job.receipt.state,
                if acknowledge_retry {
                    OutboxState::Accepted
                } else {
                    OutboxState::Unknown
                }
            );
            assert!(!service.deliver_one().await.unwrap());
            let (effects, requests) = tokio::time::timeout(Duration::from_secs(2), receiver)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(effects, 1);
            assert_eq!(requests.len(), sends);
            assert!(requests.iter().all(|(key, body)| key
                == if idempotent {
                    "stable-native-key"
                } else {
                    "untrusted-key"
                }
                && body == b"private-payload"));
        }
    }
}
