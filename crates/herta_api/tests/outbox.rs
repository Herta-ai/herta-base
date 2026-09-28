use herta_api::{extensions::ApiHostFactory, outbox::OutboxService};
use herta_core::{
    HbConfig, HbError, HbResult, MailMessage, MailReceipt, MailStatus, Mailer,
    extension::{AuthMode, HostCall, HostContext, HostFactory, HostServices},
    outbox::{OutboxResolution, OutboxResolve, OutboxState},
};
use herta_db::{DbClient, outbox};
use herta_storage::ObjectStoreStorage;
use salvo::async_trait;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
struct Mailbox {
    sends: AtomicUsize,
    mode: AtomicUsize,
    entered: tokio::sync::Notify,
}
#[async_trait]
impl Mailer for Mailbox {
    async fn send(&self, _message: MailMessage) -> HbResult<MailReceipt> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        match self.mode.load(Ordering::SeqCst) {
            1 => Err(HbError::MailTimeout),
            2 => {
                std::future::pending::<()>().await;
                unreachable!()
            }
            _ => Ok(MailReceipt {
                message_id: "<outbox@test.invalid>".into(),
                status: MailStatus::Accepted,
            }),
        }
    }
}
fn settings() -> HbConfig {
    let mut config = HbConfig::default();
    config.jsvm.enabled = true;
    config.jsvm.outbox.enabled = true;
    config.jsvm.mail.enabled = true;
    config.mail.driver = "smtp".into();
    config.mail.from_address = "sender@example.com".into();
    config
}
fn host(db: &DbClient, config: &HbConfig, mailbox: Arc<Mailbox>) -> Arc<dyn HostServices> {
    ApiHostFactory::new(
        db.clone(),
        Arc::new(config.clone()),
        mailbox,
        Arc::new(ObjectStoreStorage::memory()),
    )
    .create(HostContext::system())
}
fn call(operation: &str, transaction: Option<&str>, arguments: Value) -> HostCall {
    HostCall {
        operation: operation.into(),
        transaction: transaction.map(str::to_owned),
        arguments,
        auth_mode: AuthMode::System,
        remaining_ms: 3000,
        script: "outbox.js".into(),
    }
}
fn payload(key: &str) -> Value {
    json!({"kind":"mail.send","idempotencyKey":key,"payload":{"to":[{"address":"reader@example.com"}],"subject":"hi","text":"private body"}})
}
async fn begin(host: &Arc<dyn HostServices>) -> String {
    host.call(call("transaction.begin", None, Value::Null))
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .into()
}
async fn enqueue(host: &Arc<dyn HostServices>, input: Value) -> Value {
    let id = begin(host).await;
    let receipt = host
        .call(call("outbox.enqueue", Some(&id), input))
        .await
        .unwrap();
    host.call(call("transaction.commit", Some(&id), Value::Null))
        .await
        .unwrap();
    receipt
}

#[tokio::test]
async fn outbox_enqueue_is_transactional_idempotent_and_caught_conflicts_poison_the_owner() {
    for engine in ["memory", "surrealkv"] {
        let directory = tempfile::tempdir().unwrap();
        let mut config = settings();
        config.database.engine = engine.into();
        config.paths.data_dir = directory.path().to_string_lossy().into_owned();
        config.jsvm.outbox.max_jobs = 1;
        let db = DbClient::init(&config).await.unwrap();
        let mailbox = Arc::new(Mailbox::default());
        let host = host(&db, &config, mailbox.clone());
        let tx = begin(&host).await;
        let first = host
            .call(call("outbox.enqueue", Some(&tx), payload("same")))
            .await
            .unwrap();
        let id = first["jobId"].as_str().unwrap();
        assert!(matches!(
            outbox::get((&db).into(), id).await,
            Err(HbError::NotFound)
        ));
        assert!(outbox::claim(&db, 60).await.unwrap().is_none());
        let again = host
            .call(call("outbox.enqueue", Some(&tx), payload("same")))
            .await
            .unwrap();
        assert_eq!(again["jobId"], first["jobId"]);
        let mut changed = payload("same");
        changed["payload"]["text"] = json!("different");
        assert_eq!(
            host.call(call("outbox.enqueue", Some(&tx), changed))
                .await
                .unwrap_err()
                .error_code(),
            "HB_IDEMPOTENCY_CONFLICT"
        );
        assert_eq!(
            host.call(call("transaction.commit", Some(&tx), Value::Null))
                .await
                .unwrap_err()
                .error_code(),
            "HB_HOOK_ABORTED"
        );
        assert!(matches!(
            outbox::get((&db).into(), id).await,
            Err(HbError::NotFound)
        ));
        let accepted = enqueue(&host, payload("same")).await;
        assert_eq!(
            enqueue(&host, payload("same")).await["jobId"],
            accepted["jobId"]
        );
        let tx = begin(&host).await;
        assert!(matches!(
            host.call(call("outbox.enqueue", Some(&tx), payload("full")))
                .await,
            Err(HbError::RateLimited)
        ));
        host.finish(false).await.unwrap();
        assert_eq!(mailbox.sends.load(Ordering::SeqCst), 0);
        let service = OutboxService::new(db.clone(), Arc::new(config), mailbox.clone());
        assert!(service.deliver_one().await.unwrap());
        assert!(!service.deliver_one().await.unwrap());
        let job = outbox::get((&db).into(), id).await.unwrap();
        assert_eq!(job.receipt.state, OutboxState::Accepted);
        assert_eq!(job.receipt.attempts, 1);
        assert_eq!(
            job.receipt.result.unwrap()["messageId"],
            "<outbox@test.invalid>"
        );
        assert_eq!(mailbox.sends.load(Ordering::SeqCst), 1);
        let page = outbox::list(&db, "", 100).await.unwrap();
        assert!(!page.to_string().contains("private body"));
        assert!(!page.to_string().contains("reader@example.com"));
    }
}

#[tokio::test]
async fn uncertain_mail_requires_explicit_resolution_and_old_attempt_cannot_ack_new_lease() {
    let db = DbClient::memory().await.unwrap();
    let config = settings();
    let mailbox = Arc::new(Mailbox::default());
    mailbox.mode.store(1, Ordering::SeqCst);
    let host = host(&db, &config, mailbox.clone());
    let receipt = enqueue(&host, payload("unknown")).await;
    let id = receipt["jobId"].as_str().unwrap();
    let service = OutboxService::new(db.clone(), Arc::new(config), mailbox.clone());
    assert!(service.deliver_one().await.unwrap());
    assert_eq!(
        outbox::get((&db).into(), id).await.unwrap().receipt.state,
        OutboxState::Unknown
    );
    assert!(!service.deliver_one().await.unwrap());
    assert_eq!(mailbox.sends.load(Ordering::SeqCst), 1);
    let old = outbox::get((&db).into(), id).await.unwrap();
    outbox::resolve(
        &db,
        id,
        OutboxResolve {
            resolution: OutboxResolution::NotSent,
            note: "receiver confirmed no acceptance".into(),
        },
        "_admins:one".into(),
    )
    .await
    .unwrap();
    assert!(outbox::update_owned(&db, &old).await.is_err());
    mailbox.mode.store(0, Ordering::SeqCst);
    service.deliver_one().await.unwrap();
    assert_eq!(mailbox.sends.load(Ordering::SeqCst), 2);
    assert!(
        outbox::resolve(
            &db,
            id,
            OutboxResolve {
                resolution: OutboxResolution::Accepted,
                note: "duplicate resolution".into()
            },
            "_admins:one".into()
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn expired_lease_and_sending_recover_differently_and_unknown_is_never_purged() {
    let db = DbClient::memory().await.unwrap();
    let config = settings();
    let mailbox = Arc::new(Mailbox::default());
    let host = host(&db, &config, mailbox.clone());
    let first = enqueue(&host, payload("unsent")).await;
    let second = enqueue(&host, payload("sending")).await;
    let leased = outbox::claim(&db, 60).await.unwrap().unwrap();
    let mut sending = outbox::claim(&db, 60).await.unwrap().unwrap();
    outbox::sending(&db, &mut sending).await.unwrap();
    let before = outbox::now();
    outbox::renew(&db, &sending, 90).await.unwrap();
    assert!(
        outbox::get((&db).into(), &sending.receipt.job_id)
            .await
            .unwrap()
            .lease_until
            >= before + 90000
    );
    db.inner()
        .query("UPDATE _extension_outbox SET leaseUntil = 0")
        .await
        .unwrap()
        .check()
        .unwrap();
    outbox::recover_expired(&db).await.unwrap();
    assert_eq!(
        outbox::get((&db).into(), &leased.receipt.job_id)
            .await
            .unwrap()
            .receipt
            .state,
        OutboxState::Pending
    );
    assert_eq!(
        outbox::get((&db).into(), &sending.receipt.job_id)
            .await
            .unwrap()
            .receipt
            .state,
        OutboxState::Unknown
    );
    let service = OutboxService::new(db.clone(), Arc::new(config), mailbox.clone());
    service.deliver_one().await.unwrap();
    db.inner()
        .query("UPDATE _extension_outbox SET updatedAt = 0")
        .await
        .unwrap()
        .check()
        .unwrap();
    outbox::purge(&db, 7).await.unwrap();
    let remaining = outbox::list(&db, "", 100).await.unwrap();
    assert_eq!(remaining["items"].as_array().unwrap().len(), 1);
    assert_eq!(remaining["items"][0]["state"], "unknown");
    assert_ne!(first["jobId"], second["jobId"]);
}

#[tokio::test]
async fn revoked_capability_prevents_delivery_and_shutdown_leaves_durable_unknown_intent() {
    let db = DbClient::memory().await.unwrap();
    let mut config = settings();
    let mailbox = Arc::new(Mailbox::default());
    let host = host(&db, &config, mailbox.clone());
    let receipt = enqueue(&host, payload("revoked")).await;
    config.jsvm.mail.enabled = false;
    OutboxService::new(db.clone(), Arc::new(config.clone()), mailbox.clone())
        .deliver_one()
        .await
        .unwrap();
    assert_eq!(
        outbox::get((&db).into(), receipt["jobId"].as_str().unwrap())
            .await
            .unwrap()
            .receipt
            .state,
        OutboxState::Failed
    );
    assert_eq!(mailbox.sends.load(Ordering::SeqCst), 0);
    let receipt = enqueue(&host, payload("shutdown")).await;
    config.jsvm.mail.enabled = true;
    config.jsvm.shutdown_timeout_ms = 50;
    mailbox.mode.store(2, Ordering::SeqCst);
    let mut worker = OutboxService::new(db.clone(), Arc::new(config), mailbox.clone()).start();
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        mailbox.entered.notified(),
    )
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), worker.shutdown())
        .await
        .unwrap()
        .unwrap();
    let id = receipt["jobId"].as_str().unwrap();
    assert_eq!(
        outbox::get((&db).into(), id).await.unwrap().receipt.state,
        OutboxState::Sending
    );
    db.inner()
        .query("UPDATE _extension_outbox SET leaseUntil = 0")
        .await
        .unwrap()
        .check()
        .unwrap();
    outbox::recover_expired(&db).await.unwrap();
    assert_eq!(
        outbox::get((&db).into(), id).await.unwrap().receipt.state,
        OutboxState::Unknown
    );
}
