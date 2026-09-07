use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use herta_api::{ApiState, build_router};
use herta_core::{HbConfig, HbError, HbResult, MailMessage, MailReceipt, MailStatus, Mailer};
use herta_db::DbClient;
use herta_storage::ObjectStoreStorage;
use salvo::{
    prelude::*,
    test::{ResponseExt, TestClient},
};
use serde_json::{Value, json};

#[derive(Default)]
struct TestMailer {
    sent: Mutex<Vec<MailMessage>>,
    failure: AtomicUsize,
}

#[async_trait]
impl Mailer for TestMailer {
    async fn send(&self, message: MailMessage) -> HbResult<MailReceipt> {
        match self.failure.load(Ordering::SeqCst) {
            1 => Err(HbError::CapabilityUnavailable),
            2 => Err(HbError::MailSendFailed),
            3 => Err(HbError::MailTimeout),
            _ => {
                self.sent.lock().unwrap().push(message);
                Ok(MailReceipt {
                    message_id: "<api-test@hertabase.local>".into(),
                    status: MailStatus::Accepted,
                })
            }
        }
    }
}

#[tokio::test]
async fn mail_endpoint_enforces_admin_and_preserves_receipt_and_errors() {
    let directory = tempfile::tempdir().unwrap();
    let mut config = HbConfig::default();
    config.paths.data_dir = directory.path().to_string_lossy().into_owned();
    config.database.engine = "memory".into();
    config.auth.jwt_secret = Some("mail-api-test-signing-secret-at-least-32-bytes".into());
    config.auth.bootstrap_admin_email = Some("admin@example.com".into());
    config.auth.bootstrap_admin_password = Some("correct horse battery staple".into());
    let mailer = Arc::new(TestMailer::default());
    let state = ApiState::new_with_services(
        DbClient::memory().await.unwrap(),
        config,
        Arc::new(ObjectStoreStorage::memory()),
        mailer.clone(),
    )
    .await
    .unwrap();
    let service = Service::new(build_router()).hoop(affix_state::inject(Arc::new(state)));
    let body =
        json!({"to": [{"address": "reader@example.com"}], "subject": "Test", "text": "Body"});
    let endpoint = "http://localhost/api/admin/mail/send";
    let response = TestClient::post(endpoint).json(&body).send(&service).await;
    assert_eq!(response.status_code, Some(StatusCode::UNAUTHORIZED));
    let mut registered = TestClient::post("http://localhost/api/auth/register")
        .json(&json!({"email": "reader@example.com", "password": "correct horse battery staple"}))
        .send(&service)
        .await;
    assert_eq!(registered.status_code, Some(StatusCode::CREATED));
    let user: Value = registered.take_json().await.unwrap();
    let response = TestClient::post(endpoint)
        .json(&body)
        .bearer_auth(user["data"]["accessToken"].as_str().unwrap())
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::FORBIDDEN));
    assert!(mailer.sent.lock().unwrap().is_empty());

    let mut login = TestClient::post("http://localhost/api/admin/auth/login")
        .json(&json!({"email": "admin@example.com", "password": "correct horse battery staple"}))
        .send(&service)
        .await;
    let login: Value = login.take_json().await.unwrap();
    let token = login["data"]["accessToken"].as_str().unwrap();
    let mut response = TestClient::post(endpoint)
        .json(&body)
        .bearer_auth(token)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let result: Value = response.take_json().await.unwrap();
    assert_eq!(
        result["data"],
        json!({"messageId": "<api-test@hertabase.local>", "status": "accepted"})
    );
    assert_eq!(mailer.sent.lock().unwrap().len(), 1);
    assert_eq!(mailer.sent.lock().unwrap()[0].text.as_deref(), Some("Body"));

    for (mode, status, code) in [
        (1, 503, "HB_CAPABILITY_UNAVAILABLE"),
        (2, 502, "HB_MAIL_SEND_FAILED"),
        (3, 504, "HB_MAIL_TIMEOUT"),
    ] {
        mailer.failure.store(mode, Ordering::SeqCst);
        let mut response = TestClient::post(endpoint)
            .json(&body)
            .bearer_auth(token)
            .send(&service)
            .await;
        assert_eq!(response.status_code.unwrap().as_u16(), status);
        let result: Value = response.take_json().await.unwrap();
        assert!(result["data"].is_null());
        assert_eq!(result["error"]["error"], code);
    }
}
