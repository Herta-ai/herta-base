//! Deterministic database interleavings at the Auth hook/issuance boundary.
use herta_auth::{AuthService, Credentials};
use herta_core::{HbConfig, HbError};
use herta_db::{
    DbClient, RuleContext, SchemaManager,
    transaction::{OperationState, TransactionOwner, operation_status},
};
use serde_json::{Value, json};

fn credentials(email: &str) -> Credentials {
    Credentials {
        email: email.into(),
        password: "correct horse battery staple".into(),
        profile: Default::default(),
    }
}

async fn count_tokens(db: &DbClient) -> usize {
    let mut response = db
        .inner()
        .query("SELECT id FROM _auth_refresh_tokens")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Value> = response.take(0).unwrap();
    rows.len()
}

#[tokio::test]
async fn login_and_refresh_issuance_conflict_with_concurrent_account_changes() {
    for engine in ["memory", "surrealkv"] {
        let directory = tempfile::tempdir().unwrap();
        let mut config = HbConfig::default();
        config.database.engine = engine.into();
        config.paths.data_dir = directory.path().to_string_lossy().into_owned();
        config.auth.jwt_secret = Some("auth-barrier-test-secret-at-least-32-bytes".into());
        let db = DbClient::init(&config).await.unwrap();
        SchemaManager::new(&db)
            .create_collection(
                &serde_json::from_value(json!({
                    "name":"people","type":"auth","schema_mode":"strict","fields":[],"rules":{"create":true}
                }))
                .unwrap(),
            )
            .await
            .unwrap();
        let auth = AuthService::new(db.clone(), &config).await.unwrap();
        let registered = auth
            .register("people", credentials("visible@example.com"))
            .await
            .unwrap();
        let login = auth
            .login("people", credentials("visible@example.com"), false)
            .await
            .unwrap();
        assert!(
            serde_json::to_value(login.user)
                .unwrap()
                .get("_hb_auth_issuance")
                .is_none()
        );
        let record = herta_db::RecordManager::new(&db)
            .get("people", &registered.user.id, None)
            .await
            .unwrap();
        assert!(record.get("_hb_auth_issuance").is_none());
        for refresh in [false, true] {
            for change in ["token_key", "deleted_at", "role"] {
                let email = format!("{refresh}-{change}@example.com");
                let initial = auth.register("people", credentials(&email)).await.unwrap();
                let prepared = if refresh {
                    auth.prepare_refresh(&initial.refresh_token, false, Some("people"))
                        .await
                        .unwrap()
                } else {
                    auth.prepare_login("people", credentials(&email), false)
                        .await
                        .unwrap()
                };
                let tokens = count_tokens(&db).await;
                let owner = TransactionOwner::new(db.clone(), prepared.principal());
                let receipt = owner.begin().await.unwrap();
                // execute() writes the account fence and token row, while commit remains blocked by this test.
                let issued = owner
                    .execute(Some(receipt.id.clone()), true, move |session| {
                        Box::pin(async move {
                            prepared
                                .execute(session, Value::Null, &RuleContext::default())
                                .await
                        })
                    })
                    .await
                    .unwrap();
                assert_eq!(count_tokens(&db).await, tokens);
                let query = match change {
                    "token_key" => {
                        "UPDATE people SET token_key = 'revoked-by-admin' WHERE email = $email"
                    }
                    "deleted_at" => {
                        "UPDATE people SET deleted_at = time::now() WHERE email = $email"
                    }
                    _ => "UPDATE people SET role = 'restricted' WHERE email = $email",
                };
                db.inner()
                    .query(query)
                    .bind(("email", email))
                    .await
                    .unwrap()
                    .check()
                    .unwrap();
                let committed = owner.commit(&receipt.id).await;
                assert!(
                    matches!(committed, Err(HbError::Conflict(_))),
                    "{engine} {refresh} {change}: {committed:?}"
                );
                assert_eq!(
                    operation_status(
                        &db,
                        &receipt.operation_id,
                        None,
                        Some(&receipt.check_credential)
                    )
                    .await
                    .unwrap()
                    .state,
                    OperationState::RolledBack
                );
                assert_eq!(count_tokens(&db).await, tokens);
                // An uncommitted refresh token is unusable, even if its JWT was generated in Rust.
                assert!(
                    auth.prepare_refresh(&issued.refresh_token, false, Some("people"))
                        .await
                        .is_err()
                );
            }
        }
    }
}

#[tokio::test]
async fn old_auth_schema_child() {
    let Ok(path) = std::env::var("HB_TEST_AUTH_SCHEMA_DIRECTORY") else {
        return;
    };
    let mut config = HbConfig::default();
    config.database.engine = "surrealkv".into();
    config.paths.data_dir = path;
    config.auth.jwt_secret = Some("auth-migration-test-secret-at-least-32-bytes".into());
    let db = DbClient::init(&config).await.unwrap();
    SchemaManager::new(&db).create_collection(&serde_json::from_value(json!({
        "name":"people","type":"auth","schema_mode":"strict","fields":[],"rules":{"create":true}
    })).unwrap()).await.unwrap();
    let auth = AuthService::new(db.clone(), &config).await.unwrap();
    auth.register("people", credentials("migrate@example.com"))
        .await
        .unwrap();
    db.inner()
        .query("REMOVE FIELD _hb_auth_issuance ON TABLE people")
        .await
        .unwrap()
        .check()
        .unwrap();
    std::process::exit(0);
}

#[tokio::test]
async fn existing_strict_auth_collections_gain_the_issuance_fence_on_restart() {
    let directory = tempfile::tempdir().unwrap();
    let result = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "old_auth_schema_child", "--nocapture"])
        .env("HB_TEST_AUTH_SCHEMA_DIRECTORY", directory.path())
        .output()
        .await
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut config = HbConfig::default();
    config.database.engine = "surrealkv".into();
    config.paths.data_dir = directory.path().to_string_lossy().into_owned();
    config.auth.jwt_secret = Some("auth-migration-test-secret-at-least-32-bytes".into());
    let db = DbClient::init(&config).await.unwrap();
    let auth = AuthService::new(db.clone(), &config).await.unwrap();
    let response = auth
        .login("people", credentials("migrate@example.com"), false)
        .await
        .unwrap();
    let mut rows = db
        .inner()
        .query("SELECT _hb_auth_issuance FROM people")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Value> = rows.take(0).unwrap();
    assert!(rows[0]["_hb_auth_issuance"].is_string());
    assert!(
        serde_json::to_value(response.user)
            .unwrap()
            .get("_hb_auth_issuance")
            .is_none()
    );
}

#[tokio::test]
async fn refresh_replay_revocation_fences_a_pair_already_prepared_in_another_transaction() {
    for engine in ["memory", "surrealkv"] {
        let directory = tempfile::tempdir().unwrap();
        let mut config = HbConfig::default();
        config.database.engine = engine.into();
        config.paths.data_dir = directory.path().to_string_lossy().into_owned();
        config.auth.jwt_secret = Some("auth-replay-test-secret-at-least-32-bytes".into());
        let db = DbClient::init(&config).await.unwrap();
        SchemaManager::new(&db)
            .create_collection(
                &serde_json::from_value(json!({
                    "name":"people","type":"auth","schema_mode":"strict","fields":[],"rules":{"create":true}
                }))
                .unwrap(),
            )
            .await
            .unwrap();
        let auth = AuthService::new(db.clone(), &config).await.unwrap();
        let initial = auth
            .register("people", credentials("replay@example.com"))
            .await
            .unwrap();
        let prepared = auth
            .prepare_refresh(&initial.refresh_token, false, Some("people"))
            .await
            .unwrap();
        let owner = TransactionOwner::new(db.clone(), prepared.principal());
        let receipt = owner.begin().await.unwrap();
        let issued = owner
            .execute(Some(receipt.id.clone()), true, move |session| {
                Box::pin(async move {
                    prepared
                        .execute(session, Value::Null, &RuleContext::default())
                        .await
                })
            })
            .await
            .unwrap();
        assert!(matches!(
            auth.prepare_refresh(&initial.refresh_token, false, Some("people"))
                .await,
            Err(HbError::Unauthorized)
        ));
        let committed = owner.commit(&receipt.id).await;
        assert!(
            matches!(committed, Err(HbError::Conflict(_))),
            "{engine}: {committed:?}"
        );
        assert_eq!(count_tokens(&db).await, 1);
        assert!(auth.authenticate(&initial.access_token).await.is_err());
        assert!(auth.authenticate(&issued.access_token).await.is_err());
        assert!(
            auth.prepare_refresh(&issued.refresh_token, false, Some("people"))
                .await
                .is_err()
        );
    }
}
