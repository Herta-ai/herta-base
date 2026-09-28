use herta_api::{ApiState, build_router, extensions::ApiHostFactory};
use herta_auth::{AuthResponse, Credentials};
use herta_core::{
    HbConfig,
    extension::{AuthMode, HostCall, HostContext, HostFactory},
    messages::PublishRequest,
};
use herta_db::{DbClient, SchemaManager, record::parse_record_id};
use herta_storage::ObjectStoreStorage;
use http_body_util::BodyExt;
use salvo::{prelude::*, test::TestClient};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

async fn setup() -> (tempfile::TempDir, Arc<ApiState>, AuthResponse, AuthResponse) {
    let directory = tempfile::tempdir().unwrap();
    let mut config = HbConfig::default();
    config.paths.data_dir = directory.path().join("data").to_string_lossy().into_owned();
    config.auth.jwt_secret = Some("application-message-test-signing-secret-32bytes".into());
    config.jsvm.enabled = true;
    config.jsvm.realtime.enabled = true;
    config.jsvm.realtime.connection_queue_capacity = 2;
    config.jsvm.realtime.connection_queue_bytes = 512;
    config.jsvm.realtime.max_message_bytes = 4096;
    config.realtime.max_connections_per_ip = 1;
    let db = DbClient::memory().await.unwrap();
    for name in ["readers", "writers"] {
        SchemaManager::new(&db)
            .create_collection(
                &serde_json::from_value(json!({"name":name,"type":"auth","schema_mode":"mixed"}))
                    .unwrap(),
            )
            .await
            .unwrap();
    }
    let state = Arc::new(
        ApiState::new_with_storage(db, config, Arc::new(ObjectStoreStorage::memory()))
            .await
            .unwrap(),
    );
    let credentials = || {
        serde_json::from_value::<Credentials>(
            json!({"email":"reader@example.com","password":"safe-password"}),
        )
        .unwrap()
    };
    let reader = state.auth.register("readers", credentials()).await.unwrap();
    let writer = state.auth.register("writers", credentials()).await.unwrap();
    (directory, state, reader, writer)
}
fn message(audience: Value) -> PublishRequest {
    serde_json::from_value(
        json!({"topic":"reports/ready","data":{"ready":true},"audience":audience}),
    )
    .unwrap()
}
async fn set(state: &ApiState, user: &AuthResponse, field: &str, value: &str) {
    assert!(matches!(field, "role" | "token_key"));
    state
        .db
        .inner()
        .query(format!("UPDATE $id SET {field} = $value"))
        .bind(("id", parse_record_id(&user.user.id).unwrap()))
        .bind(("value", value.to_owned()))
        .await
        .unwrap()
        .check()
        .unwrap();
}

#[tokio::test]
async fn audiences_are_exact_and_queued_messages_recheck_current_roles_and_tokens() {
    let (_directory, state, reader, writer) = setup().await;
    let mut readers = state
        .messages
        .subscribe("reports/ready".into(), reader.access_token.clone())
        .await
        .unwrap();
    let mut writers = state
        .messages
        .subscribe("reports/ready".into(), writer.access_token.clone())
        .await
        .unwrap();
    let audience = json!({"roles":[{"collection":"readers","role":"user"}]});
    let receipt = state
        .messages
        .publish(message(audience.clone()))
        .await
        .unwrap();
    assert_eq!(receipt.queued, 1);
    assert_eq!(
        readers.next().await.unwrap().unwrap().data,
        json!({"ready":true})
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(15), writers.next())
            .await
            .is_err()
    );
    state
        .messages
        .publish(message(audience.clone()))
        .await
        .unwrap();
    set(&state, &reader, "role", "moderator").await;
    assert!(
        tokio::time::timeout(Duration::from_millis(15), readers.next())
            .await
            .is_err(),
        "old role must not receive queued message"
    );
    assert_eq!(
        state
            .messages
            .publish(message(audience))
            .await
            .unwrap()
            .queued,
        0
    );
    assert_eq!(
        state
            .messages
            .publish(message(
                json!({"users":[{"collection":"readers","id":reader.user.id}]})
            ))
            .await
            .unwrap()
            .queued,
        1
    );
    set(&state, &reader, "token_key", "revoked").await;
    assert_eq!(
        readers.next().await.unwrap_err().error_code(),
        "HB_UNAUTHORIZED"
    );
    assert_eq!(
        state
            .messages
            .publish(message(json!({"connections":[writers.id()]})))
            .await
            .unwrap()
            .queued,
        1
    );
    assert_eq!(
        writers.next().await.unwrap().unwrap().topic,
        "reports/ready"
    );
    state.messages.shutdown();
    assert!(writers.next().await.unwrap().is_none());
}

#[tokio::test]
async fn slow_consumers_and_byte_exhaustion_disconnect_without_blocking_publishers() {
    let (_directory, state, reader, _) = setup().await;
    let mut connection = state
        .messages
        .subscribe("reports/ready".into(), reader.access_token.clone())
        .await
        .unwrap();
    let audience = json!({"connections":[connection.id()]});
    for _ in 0..2 {
        assert_eq!(
            state
                .messages
                .publish(message(audience.clone()))
                .await
                .unwrap()
                .queued,
            1
        );
    }
    let receipt = state.messages.publish(message(audience)).await.unwrap();
    assert_eq!(receipt.queued, 0);
    assert_eq!(receipt.dropped, 1);
    assert!(connection.next().await.unwrap().is_none());
    let mut connection = state
        .messages
        .subscribe("reports/ready".into(), reader.access_token)
        .await
        .unwrap();
    let mut request = message(json!({"connections":[connection.id()]}));
    request.data = json!("x".repeat(600));
    assert_eq!(state.messages.publish(request).await.unwrap().dropped, 1);
    assert!(connection.next().await.unwrap().is_none());
    assert_eq!(
        state
            .messages
            .publish(message(Value::Null))
            .await
            .unwrap_err()
            .error_code(),
        "HB_CAPABILITY_DENIED"
    );
    for audience in [
        json!({"users":[]}),
        json!({"roles":[],"connections":["one"]}),
        json!({"all":true}),
    ] {
        let value = serde_json::from_value::<PublishRequest>(
            json!({"topic":"reports/ready","data":{},"audience":audience}),
        );
        if let Ok(value) = value {
            assert!(state.messages.publish(value).await.is_err());
        }
    }
}

async fn frame(body: &mut salvo::http::ResBody) -> String {
    let frame = tokio::time::timeout(Duration::from_secs(2), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap()
}
#[tokio::test]
async fn application_sse_requires_auth_shares_connection_quota_and_releases_on_drop() {
    let (_directory, state, reader, _) = setup().await;
    let service = Service::new(build_router()).hoop(affix_state::inject(state.clone()));
    let endpoint = "http://localhost/api/events?topic=reports%2Fready";
    assert_eq!(
        TestClient::get(endpoint)
            .send(&service)
            .await
            .status_code
            .unwrap()
            .as_u16(),
        401
    );
    let mut response = TestClient::get(endpoint)
        .bearer_auth(&reader.access_token)
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 200);
    let mut body = response.take_body();
    let connected = frame(&mut body).await;
    assert!(connected.contains("event:connected"), "{connected}");
    assert!(connected.contains("connectionId"));
    assert!(connected.contains("reports/ready"));
    assert!(!connected.contains(&reader.access_token));
    assert_eq!(
        TestClient::get(endpoint)
            .bearer_auth(&reader.access_token)
            .send(&service)
            .await
            .status_code
            .unwrap()
            .as_u16(),
        429
    );
    state
        .messages
        .publish(message(
            json!({"users":[{"collection":"readers","id":reader.user.id}]}),
        ))
        .await
        .unwrap();
    let message = frame(&mut body).await;
    assert!(message.contains("event:message"));
    assert!(message.contains("\"ready\":true"));
    drop(body);
    let mut response = TestClient::get(endpoint)
        .bearer_auth(&reader.access_token)
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 200);
    let mut body = response.take_body();
    frame(&mut body).await;
    state.messages.shutdown();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), body.frame())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn message_bridge_uses_capability_adapter_and_native_transaction_gates() {
    let (_directory, state, _, _) = setup().await;
    let call = || HostCall {
        operation: "realtime.publish".into(),
        arguments: Value::Null,
        auth_mode: AuthMode::System,
        remaining_ms: 1000,
        script: "message.js".into(),
        transaction: None,
    };
    let factory = ApiHostFactory::new(
        state.db.clone(),
        state.config.clone(),
        state.mailer.clone(),
        state.storage.clone(),
    );
    let host = factory.create(HostContext::system());
    assert_eq!(
        host.call(call()).await.unwrap_err().error_code(),
        "HB_CAPABILITY_UNAVAILABLE"
    );
    let host = factory
        .with_messages(state.messages.clone())
        .create(HostContext::system());
    let mut begin = call();
    begin.operation = "transaction.begin".into();
    host.call(begin).await.unwrap();
    assert_eq!(
        host.call(call()).await.unwrap_err().error_code(),
        "HB_SIDE_EFFECT_IN_TRANSACTION"
    );
    host.finish(false).await.unwrap();
    assert_eq!(
        host.call(call()).await.unwrap_err().error_code(),
        "HB_VALIDATION_ERROR"
    );
}

#[tokio::test]
async fn expiry_is_exact_and_js_after_commit_publishes_once() {
    use herta_core::extension::{EventDispatcher, Invocation};
    let (directory, state, reader, _) = setup().await;
    let mut connection = state
        .messages
        .subscribe("reports/ready".into(), reader.access_token.clone())
        .await
        .unwrap();
    let script = format!(
        r#"routerAdd('POST','/api/custom/publish',async e=>{{
      let receipt;
      await $app.transaction(async tx=>{{
        try {{ await $app.realtime.publish('reports/ready',{{}},{{connections:['{}']}}); throw new Error('transaction send escaped') }}
        catch(error) {{ if(error.code!=='HB_SIDE_EFFECT_IN_TRANSACTION') throw error }}
        tx.afterCommit(async ()=>{{receipt=await $app.realtime.publish('reports/ready',{{committed:true}},{{connections:['{}']}})}});
      }});
      return e.json(200,receipt);
    }});"#,
        connection.id(),
        connection.id()
    );
    std::fs::write(directory.path().join("main.js"), script).unwrap();
    let runtime = herta_jsvm::JsRuntime::load(directory.path(), state.config.jsvm.clone())
        .await
        .unwrap();
    let host = ApiHostFactory::new(
        state.db.clone(),
        state.config.clone(),
        state.mailer.clone(),
        state.storage.clone(),
    )
    .with_messages(state.messages.clone())
    .create(HostContext::system());
    let result = runtime
        .dispatch(
            Invocation {
                name: "route".into(),
                payload: json!({}),
                auth_mode: AuthMode::System,
                request_id: None,
                registration: Some(0),
            },
            host,
        )
        .await
        .unwrap();
    assert_eq!(result["body"]["data"]["queued"], 1);
    assert_eq!(
        connection.next().await.unwrap().unwrap().data,
        json!({"committed":true})
    );
    runtime.shutdown().await.unwrap();

    // Sign a short-lived token with the fixture key, preserving its real account identity.
    let mut claims = jsonwebtoken::dangerous::insecure_decode::<Value>(&reader.access_token)
        .unwrap()
        .claims;
    let expires = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 2;
    claims["exp"] = json!(expires);
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(
            state.config.auth.jwt_secret.as_ref().unwrap().as_bytes(),
        ),
    )
    .unwrap();
    let service = Service::new(build_router()).hoop(affix_state::inject(state.clone()));
    let mut response = TestClient::get("http://localhost/api/events?topic=ready")
        .bearer_auth(&token)
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 200);
    let mut body = response.take_body();
    frame(&mut body).await;
    let event = frame(&mut body).await;
    assert!(event.contains("HB_TOKEN_EXPIRED"), "{event}");
    assert!(body.frame().await.is_none());
}
