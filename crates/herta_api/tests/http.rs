use herta_api::{ApiState, build_router, build_router_with_logger};
use herta_core::HbConfig;
use herta_db::{
    CollectionDef, CollectionType, DbClient, FieldDef, FieldType, LogEntry, SchemaManager,
    SchemaMode, log_channel,
};
use herta_storage::ObjectStoreStorage;
use http_body_util::BodyExt;
use salvo::{
    http::StatusCode,
    prelude::*,
    test::{ResponseExt, TestClient},
};
use serde_json::{Value, json};
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn request_logger_records_metadata_only() {
    let db = DbClient::memory().await.unwrap();
    let config = HbConfig::default();
    let state = ApiState::new_with_storage(
        db,
        config,
        std::sync::Arc::new(ObjectStoreStorage::memory()),
    )
    .await
    .unwrap();
    let (sender, mut receiver) = log_channel();
    let service = Service::new(build_router_with_logger(Some(
        herta_api::handlers::logging::RequestLogger::new(sender),
    )))
    .hoop(affix_state::inject(state));

    let mut response = TestClient::get("http://localhost/api-doc/openapi.json?token=secret")
        .add_header("referer", "https://example.test", true)
        .add_header("user-agent", "test-agent", true)
        .send(&service)
        .await;
    let _: Value = response.take_json().await.unwrap();
    let entry: LogEntry = timeout(Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entry.method.as_deref(), Some("GET"));
    assert_eq!(entry.path.as_deref(), Some("/api-doc/openapi.json"));
    assert_eq!(entry.status_code, Some(200));
    assert_eq!(entry.referer.as_deref(), Some("https://example.test"));
    assert_eq!(entry.user_agent.as_deref(), Some("test-agent"));
    assert_eq!(entry.auth_type.as_deref(), Some("anonymous"));
}

#[tokio::test]
async fn request_logger_skips_log_stream_reads() {
    let db = DbClient::memory().await.unwrap();
    let config = HbConfig::default();
    let state = ApiState::new(db, config).await.unwrap();
    let (sender, mut receiver) = log_channel();
    let service = Service::new(build_router_with_logger(Some(
        herta_api::handlers::logging::RequestLogger::new(sender),
    )))
    .hoop(affix_state::inject(state));

    let mut response = TestClient::get("http://localhost/api/admin/logs")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::UNAUTHORIZED));
    let _: Value = response.take_json().await.unwrap();
    assert!(
        timeout(Duration::from_millis(100), receiver.recv())
            .await
            .is_err()
    );
}

async fn service() -> Service {
    service_with_settings(1000, 20, 900).await
}

async fn service_with_db() -> (Service, DbClient) {
    let db = DbClient::memory().await.unwrap();
    let mut config = HbConfig::default();
    config.database.engine = "memory".into();
    config.server.dev_mode = true;
    config.auth.bootstrap_admin_email = Some("admin@example.com".into());
    config.auth.bootstrap_admin_password = Some("correct horse battery staple".into());
    let state = ApiState::new(db.clone(), config).await.unwrap();
    (
        Service::new(build_router()).hoop(affix_state::inject(state)),
        db,
    )
}

async fn service_with_realtime_limits(max_connections: usize, per_ip: usize) -> Service {
    service_with_settings(max_connections, per_ip, 900).await
}

async fn service_with_settings(
    max_connections: usize,
    per_ip: usize,
    access_token_ttl_seconds: u64,
) -> Service {
    let db = DbClient::memory().await.unwrap();
    let mut config = HbConfig::default();
    config.database.engine = "memory".into();
    config.server.dev_mode = true;
    config.auth.bootstrap_admin_email = Some("admin@example.com".into());
    config.auth.bootstrap_admin_password = Some("correct horse battery staple".into());
    config.auth.access_token_ttl_seconds = access_token_ttl_seconds;
    config.realtime.max_connections = max_connections;
    config.realtime.max_connections_per_ip = per_ip;
    let state = ApiState::new_with_storage(
        db,
        config,
        std::sync::Arc::new(ObjectStoreStorage::memory()),
    )
    .await
    .unwrap();
    Service::new(build_router()).hoop(affix_state::inject(state))
}

async fn realtime_service(access_token_ttl_seconds: u64, heartbeat_seconds: u64) -> Service {
    let db = DbClient::memory().await.unwrap();
    let mut config = HbConfig::default();
    config.database.engine = "memory".into();
    config.server.dev_mode = true;
    config.auth.access_token_ttl_seconds = access_token_ttl_seconds;
    config.realtime.heartbeat_seconds = heartbeat_seconds;
    config.auth.bootstrap_admin_email = Some("admin@example.com".into());
    config.auth.bootstrap_admin_password = Some("correct horse battery staple".into());
    SchemaManager::new(&db)
        .create_collection(&CollectionDef {
            name: "public_posts".into(),
            collection_type: CollectionType::Base,
            schema_mode: SchemaMode::Strict,
            fields: vec![FieldDef {
                name: "title".into(),
                field_type: FieldType::Text,
                required: true,
                options: None,
            }],
            indexes: vec![],
            rules: herta_db::CollectionRules {
                view: herta_db::ApiRule::Boolean(true),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let state = ApiState::new_with_storage(
        db,
        config,
        std::sync::Arc::new(ObjectStoreStorage::memory()),
    )
    .await
    .unwrap();
    Service::new(build_router()).hoop(affix_state::inject(state))
}

async fn create_public_posts(service: &Service, admin: &str) {
    let mut response = TestClient::post("http://localhost/_/collections")
        .bearer_auth(admin)
        .json(&json!({
            "name": "public_posts",
            "type": "base",
            "schema_mode": "strict",
            "fields": [{"name": "title", "type": "text", "required": true}],
            "indexes": [],
            "rules": {"view": true}
        }))
        .send(service)
        .await;
    let status = response.status_code;
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(status, Some(StatusCode::CREATED), "{body}");
}

async fn first_body_frame(response: &mut Response) -> String {
    let mut body = response.take_body();
    next_body_frame(&mut body).await
}

async fn next_body_frame(body: &mut salvo::http::ResBody) -> String {
    let frame = timeout(Duration::from_secs(7), body.frame())
        .await
        .expect("SSE frame timed out")
        .expect("SSE body ended")
        .expect("SSE body failed");
    String::from_utf8(frame.into_data().expect("expected data frame").to_vec()).unwrap()
}

async fn admin_token(service: &Service) -> String {
    let mut response = TestClient::post("http://localhost/api/admin/auth/login")
        .json(&json!({
            "email": " ADMIN@example.com ",
            "password": "correct horse battery staple"
        }))
        .send(service)
        .await;
    let status = response.status_code;
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(status, Some(StatusCode::OK), "{body}");
    let token = body["data"]["accessToken"].as_str().unwrap().to_owned();
    let mut me = TestClient::get("http://localhost/api/admin/auth/me")
        .bearer_auth(&token)
        .send(service)
        .await;
    let me_status = me.status_code;
    let me_body: Value = me.take_json().await.unwrap();
    assert_eq!(me_status, Some(StatusCode::OK), "{me_body}");
    token
}

#[tokio::test]
async fn admin_logs_endpoint_requires_admin_and_applies_filters() {
    let (service, db) = service_with_db().await;
    db.inner()
        .query("CREATE _logs CONTENT $entry")
        .bind((
            "entry",
            json!({
                "log_type": "request",
                "level": "error",
                "message": "GET /admin failed",
                "target": "herta_api::request",
                "method": "GET",
                "path": "/admin",
                "status_code": 500,
                "referer": "https://example.test",
                "remote_ip": "192.0.2.10",
                "user_agent": "admin-browser",
                "auth_type": "admin",
                "user_id": "_admins:one",
                "user_collection": "_admins"
            }),
        ))
        .await
        .unwrap()
        .check()
        .unwrap();

    let mut response = TestClient::get("http://localhost/api/admin/logs")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::UNAUTHORIZED));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_AUTH_REQUIRED");

    let mut response = TestClient::post("http://localhost/api/auth/register")
        .json(&json!({
            "email": "user@example.com",
            "password": "a sufficiently long password"
        }))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::CREATED));
    let registered: Value = response.take_json().await.unwrap();
    let user_access = registered["data"]["accessToken"].as_str().unwrap();
    let mut response = TestClient::get("http://localhost/api/admin/logs")
        .bearer_auth(user_access)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::FORBIDDEN));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_FORBIDDEN");

    let admin = admin_token(&service).await;
    let mut response = TestClient::get(
        "http://localhost/api/admin/logs?level=ERROR&logType=request&q=ADMIN-BROWSER&target=herta_api%3A%3Arequest&path=%2Fadmin&statusCode=500",
    )
    .bearer_auth(&admin)
    .send(&service)
    .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["meta"]["total"], 1);
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert!(body["data"][0]["id"].as_str().is_some());
    assert!(body["data"][0]["created_at"].as_str().is_some());
    assert_eq!(body["data"][0]["status_code"], 500);

    let mut response = TestClient::get("http://localhost/api/admin/logs?from=not-a-date")
        .bearer_auth(&admin)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::BAD_REQUEST));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_VALIDATION_ERROR");
}

fn multipart_body(
    data: Option<&str>,
    files: &[(&str, &str, &str, &str)],
) -> (String, &'static str) {
    const BOUNDARY: &str = "hertabase-phase5-boundary";
    let mut body = String::new();
    if let Some(data) = data {
        body.push_str(&format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"data\"\r\nContent-Type: application/json\r\n\r\n{data}\r\n"
        ));
    }
    for (field, filename, content_type, content) in files {
        body.push_str(&format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n{content}\r\n"
        ));
    }
    body.push_str(&format!("--{BOUNDARY}--\r\n"));
    (
        body,
        "multipart/form-data; boundary=hertabase-phase5-boundary",
    )
}

#[tokio::test]
async fn collection_record_and_openapi_flow() {
    let service = service().await;
    let admin = admin_token(&service).await;
    let collection = json!({
        "name": "posts",
        "type": "base",
        "schema_mode": "strict",
        "fields": [
            {"name": "title", "type": "text", "required": true},
            {"name": "status", "type": "select", "required": true,
             "options": {"values": ["draft", "active"]}}
        ],
        "indexes": []
    });
    let mut response = TestClient::post("http://localhost/_/collections")
        .bearer_auth(&admin)
        .json(&collection)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::CREATED));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["data"]["name"], "posts");
    assert!(body["error"].is_null());

    let mut response = TestClient::get("http://localhost/api-doc/openapi.json")
        .bearer_auth(&admin)
        .send(&service)
        .await;
    let document: Value = response.take_json().await.unwrap();
    assert!(document["paths"]["/api/collections/posts/records"].is_object());
    assert_eq!(
        document["components"]["schemas"]["postsCreate"]["required"][0],
        "title"
    );
    assert_eq!(
        document["components"]["securitySchemes"]["bearerAuth"]["scheme"],
        "bearer"
    );
    assert!(document["paths"]["/api/auth/register"].is_object());
    assert!(document["paths"]["/api/admin/auth/login"].is_object());
    assert!(document["paths"]["/api/admin/logs"].is_object());
    assert_eq!(
        document["paths"]["/api/admin/logs"]["get"]["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|parameter| parameter["name"] == "q")
            .unwrap()["schema"]["maxLength"],
        256
    );
    assert!(document["components"]["schemas"]["LogRecord"].is_object());
    assert!(document["paths"]["/api/realtime/{collection}"].is_object());

    let mut response = TestClient::patch("http://localhost/_/collections/posts")
        .bearer_auth(&admin)
        .json(&json!({
            "fields": [{"name": "summary", "type": "text"}],
            "indexes": []
        }))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let _: Value = response.take_json().await.unwrap();
    let mut response = TestClient::get("http://localhost/api-doc/openapi.json")
        .bearer_auth(&admin)
        .send(&service)
        .await;
    let document: Value = response.take_json().await.unwrap();
    assert!(document["components"]["schemas"]["postsCreate"]["properties"]["summary"].is_object());

    let mut response = TestClient::post("http://localhost/api/collections/posts/records")
        .bearer_auth(&admin)
        .json(&json!({"title": "Hello", "status": "active"}))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::CREATED));
    let created: Value = response.take_json().await.unwrap();
    assert!(
        created["data"]["id"]
            .as_str()
            .unwrap()
            .starts_with("posts:")
    );
    let full_id = created["data"]["id"].as_str().unwrap().to_owned();
    let bare_id = full_id.strip_prefix("posts:").unwrap();
    for id in [&full_id, bare_id] {
        let mut response = TestClient::get(format!(
            "http://localhost/api/collections/posts/records/{id}"
        ))
        .bearer_auth(&admin)
        .send(&service)
        .await;
        assert_eq!(response.status_code, Some(StatusCode::OK));
        let body: Value = response.take_json().await.unwrap();
        assert_eq!(body["data"]["id"], full_id);
    }
    let mut response = TestClient::get(format!(
        "http://localhost/api/collections/posts/records/other:{bare_id}"
    ))
    .bearer_auth(&admin)
    .send(&service)
    .await;
    assert_eq!(response.status_code, Some(StatusCode::NOT_FOUND));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_NOT_FOUND");

    let mut response = TestClient::patch(format!(
        "http://localhost/api/collections/posts/records/{full_id}"
    ))
    .bearer_auth(&admin)
    .json(&json!({"summary": "updated through a full id"}))
    .send(&service)
    .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let updated: Value = response.take_json().await.unwrap();
    assert_eq!(updated["data"]["summary"], "updated through a full id");

    let mut response = TestClient::get(
        "http://localhost/api/collections/posts/records?filter=status%20IN%20%5B%27active%27%5D",
    )
    .bearer_auth(&admin)
    .send(&service)
    .await;
    let listed: Value = response.take_json().await.unwrap();
    assert_eq!(listed["meta"]["total"], 1);
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);

    let record_url = format!("http://localhost/api/collections/posts/records/{full_id}");
    let mut response = TestClient::delete(&record_url)
        .bearer_auth(&admin)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let _: Value = response.take_json().await.unwrap();
    for mut response in [
        TestClient::patch(&record_url)
            .bearer_auth(&admin)
            .json(&json!({"summary": "too late"}))
            .send(&service)
            .await,
        TestClient::delete(&record_url)
            .bearer_auth(&admin)
            .send(&service)
            .await,
    ] {
        assert_eq!(response.status_code, Some(StatusCode::FORBIDDEN));
        let body: Value = response.take_json().await.unwrap();
        assert_eq!(body["error"]["error"], "HB_FORBIDDEN");
    }
    let mut response = TestClient::get(&record_url)
        .bearer_auth(&admin)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::NOT_FOUND));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_NOT_FOUND");
    let mut response = TestClient::get("http://localhost/api/collections/posts/records")
        .bearer_auth(&admin)
        .send(&service)
        .await;
    let empty: Value = response.take_json().await.unwrap();
    assert_eq!(empty["meta"]["total"], 0);
    assert!(empty["data"].as_array().unwrap().is_empty());

    let mut response = TestClient::delete("http://localhost/_/collections/posts")
        .bearer_auth(&admin)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let _: Value = response.take_json().await.unwrap();
    let mut response = TestClient::get("http://localhost/api-doc/openapi.json")
        .bearer_auth(&admin)
        .send(&service)
        .await;
    let document: Value = response.take_json().await.unwrap();
    assert!(document["paths"]["/api/collections/posts/records"].is_null());
}

#[tokio::test]
async fn record_file_upload_token_download_replace_and_clear_flow() {
    let service = service().await;
    let admin = admin_token(&service).await;
    let mut response = TestClient::post("http://localhost/_/collections")
        .bearer_auth(&admin)
        .json(&json!({
            "name": "assets",
            "type": "base",
            "schema_mode": "strict",
            "fields": [
                {"name": "title", "type": "text", "required": true},
                {"name": "avatar", "type": "file", "options": {
                    "maxSelect": 1, "maxSize": 1024,
                    "mimeTypes": ["text/plain"], "extensions": ["txt"]
                }},
                {"name": "attachments", "type": "file", "options": {
                    "maxSelect": 2, "mimeTypes": ["text/plain"], "extensions": ["txt"]
                }}
            ],
            "indexes": []
        }))
        .send(&service)
        .await;
    let status = response.status_code;
    let collection_body: Value = response.take_json().await.unwrap();
    assert_eq!(status, Some(StatusCode::CREATED), "{collection_body}");

    let mut response = TestClient::post("http://localhost/api/collections/assets/records")
        .bearer_auth(&admin)
        .json(&json!({"title": "forged", "avatar": "client-name.txt"}))
        .send(&service)
        .await;
    assert_eq!(
        response.status_code,
        Some(StatusCode::UNSUPPORTED_MEDIA_TYPE)
    );
    let forged: Value = response.take_json().await.unwrap();
    assert_eq!(forged["error"]["error"], "HB_UNSUPPORTED_MEDIA_TYPE");

    let (body, content_type) = multipart_body(
        Some(r#"{"title":"one"}"#),
        &[
            ("avatar", "hello.txt", "text/plain", "hello world"),
            ("attachments", "one.txt", "text/plain", "first"),
            ("attachments", "two.txt", "text/plain", "second"),
        ],
    );
    let mut response = TestClient::post("http://localhost/api/collections/assets/records")
        .bearer_auth(&admin)
        .add_header("content-type", content_type, true)
        .body(body)
        .send(&service)
        .await;
    let status = response.status_code;
    let created: Value = response.take_json().await.unwrap();
    assert_eq!(status, Some(StatusCode::CREATED), "{created}");
    let record_id = created["data"]["id"].as_str().unwrap();
    let raw_id = record_id.split_once(':').unwrap().1;
    let avatar = created["data"]["avatar"].as_str().unwrap().to_owned();
    assert!(avatar.ends_with(".txt"));
    assert_eq!(created["data"]["attachments"].as_array().unwrap().len(), 2);

    let mut openapi = TestClient::get("http://localhost/api-doc/openapi.json")
        .send(&service)
        .await;
    let openapi: Value = openapi.take_json().await.unwrap();
    assert!(openapi["paths"]["/api/collections/assets/records"]["post"]["requestBody"]
        ["content"]["multipart/form-data"]
        .is_object());
    assert_eq!(
        openapi["components"]["schemas"]["assetsRecord"]["properties"]["attachments"]["type"],
        "array"
    );

    let file_url = format!("http://localhost/api/files/assets/{raw_id}/avatar/{avatar}");
    let mut response = TestClient::get(&file_url)
        .bearer_auth(&admin)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/plain"
    );
    assert_eq!(
        response.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    let etag = response
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(response.take_bytes(None).await.unwrap(), "hello world");

    let mut response = TestClient::head(&file_url)
        .bearer_auth(&admin)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    assert_eq!(response.headers().get("content-length").unwrap(), "11");
    assert!(response.take_bytes(None).await.unwrap().is_empty());

    let mut response = TestClient::get(&file_url)
        .bearer_auth(&admin)
        .add_header("range", "bytes=0-4", true)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::PARTIAL_CONTENT));
    assert_eq!(
        response.headers().get("content-range").unwrap(),
        "bytes 0-4/11"
    );
    assert_eq!(response.take_bytes(None).await.unwrap(), "hello");

    let mut response = TestClient::get(&file_url)
        .bearer_auth(&admin)
        .add_header("range", "bytes=99-", true)
        .send(&service)
        .await;
    assert_eq!(
        response.status_code,
        Some(StatusCode::RANGE_NOT_SATISFIABLE)
    );
    assert_eq!(
        response.headers().get("content-range").unwrap(),
        "bytes */11"
    );
    let range_error: Value = response.take_json().await.unwrap();
    assert_eq!(range_error["error"]["error"], "HB_RANGE_NOT_SATISFIABLE");

    let response = TestClient::get(&file_url)
        .bearer_auth(&admin)
        .add_header("if-none-match", &etag, true)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::NOT_MODIFIED));

    let mut response = TestClient::post("http://localhost/api/files/token")
        .bearer_auth(&admin)
        .json(&json!({"collection": "assets", "recordId": raw_id, "field": "avatar"}))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let token: Value = response.take_json().await.unwrap();
    let file_token = token["data"]["token"].as_str().unwrap();
    let mut response = TestClient::get(format!("{file_url}?token={file_token}"))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    assert_eq!(response.take_bytes(None).await.unwrap(), "hello world");
    let mut response = TestClient::get(format!("{file_url}?token={file_token}"))
        .bearer_auth("invalid")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::UNAUTHORIZED));
    let _: Value = response.take_json().await.unwrap();

    let (body, content_type) = multipart_body(
        None,
        &[("avatar", "replacement.txt", "text/plain", "replacement")],
    );
    let mut response = TestClient::patch(format!(
        "http://localhost/api/collections/assets/records/{raw_id}"
    ))
    .bearer_auth(&admin)
    .add_header("content-type", content_type, true)
    .body(body)
    .send(&service)
    .await;
    let status = response.status_code;
    let replaced: Value = response.take_json().await.unwrap();
    assert_eq!(status, Some(StatusCode::OK), "{replaced}");
    let replacement = replaced["data"]["avatar"].as_str().unwrap().to_owned();
    assert_ne!(replacement, avatar);
    let mut old = TestClient::get(&file_url)
        .bearer_auth(&admin)
        .send(&service)
        .await;
    assert_eq!(old.status_code, Some(StatusCode::NOT_FOUND));
    let _: Value = old.take_json().await.unwrap();

    let mut response = TestClient::patch(format!(
        "http://localhost/api/collections/assets/records/{raw_id}"
    ))
    .bearer_auth(&admin)
    .json(&json!({"avatar": []}))
    .send(&service)
    .await;
    let status = response.status_code;
    let cleared: Value = response.take_json().await.unwrap();
    assert_eq!(status, Some(StatusCode::OK), "{cleared}");
    assert!(cleared["data"]["avatar"].is_null());
}

#[tokio::test]
async fn errors_use_the_standard_envelope() {
    let service = service().await;
    let mut response = TestClient::get("http://localhost/api/collections/missing/records")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::NOT_FOUND));
    let body: Value = response.take_json().await.unwrap();
    assert!(body["data"].is_null());
    assert!(body["meta"].is_null());
    assert_eq!(body["error"]["error"], "HB_COLLECTION_NOT_FOUND");
    assert_eq!(body["error"]["code"], 404);
}

#[tokio::test]
async fn openapi_is_public_but_management_routes_require_admin() {
    let service = service().await;

    let mut response = TestClient::get("http://localhost/api-doc/openapi.json")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let document: Value = response.take_json().await.unwrap();
    assert_eq!(document["openapi"], "3.1.0");
    assert_eq!(
        document["components"]["securitySchemes"]["bearerAuth"]["scheme"],
        "bearer"
    );

    let mut response = TestClient::get("http://localhost/_/collections")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::UNAUTHORIZED));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_AUTH_REQUIRED");
}

#[tokio::test]
async fn user_auth_rotation_replay_and_default_rules() {
    let service = service().await;

    let mut response = TestClient::post("http://localhost/api/auth/register")
        .json(&json!({
            "email": " User@Example.COM ",
            "password": "a sufficiently long password"
        }))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::CREATED));
    let registered: Value = response.take_json().await.unwrap();
    assert_eq!(registered["data"]["user"]["email"], "user@example.com");
    assert!(registered["data"]["user"]["password_hash"].is_null());
    let access = registered["data"]["accessToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let refresh = registered["data"]["refreshToken"]
        .as_str()
        .unwrap()
        .to_owned();

    let mut response = TestClient::post("http://localhost/api/auth/register")
        .json(&json!({
            "email": "user@example.com",
            "password": "another sufficiently long password"
        }))
        .send(&service)
        .await;
    let status = response.status_code;
    let duplicate: Value = response.take_json().await.unwrap();
    assert_eq!(status, Some(StatusCode::CONFLICT), "{duplicate}");

    let mut response = TestClient::get("http://localhost/api/auth/me")
        .bearer_auth(format!("{access}invalid"))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::UNAUTHORIZED));
    let _: Value = response.take_json().await.unwrap();

    let mut response = TestClient::get("http://localhost/api/auth/me")
        .bearer_auth(&access)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let me: Value = response.take_json().await.unwrap();
    assert_eq!(me["data"]["email"], "user@example.com");

    let mut response = TestClient::get("http://localhost/_/collections")
        .bearer_auth(&access)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::FORBIDDEN));
    let _: Value = response.take_json().await.unwrap();

    let mut response = TestClient::post("http://localhost/api/auth/refresh")
        .json(&json!({"refreshToken": refresh}))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let rotated: Value = response.take_json().await.unwrap();
    let rotated_access = rotated["data"]["accessToken"].as_str().unwrap().to_owned();

    let mut response = TestClient::post("http://localhost/api/auth/refresh")
        .json(&json!({"refreshToken": registered["data"]["refreshToken"]}))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::UNAUTHORIZED));
    let _: Value = response.take_json().await.unwrap();

    let mut response = TestClient::get("http://localhost/api/auth/me")
        .bearer_auth(&rotated_access)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::UNAUTHORIZED));
    let _: Value = response.take_json().await.unwrap();

    let admin = admin_token(&service).await;
    let collection = json!({
        "name": "notes",
        "type": "base",
        "schema_mode": "strict",
        "fields": [{"name": "owner", "type": "text", "required": true}],
        "indexes": [],
        "rules": {
            "list": "$auth.id = $record.owner",
            "view": "$auth.id = $record.owner",
            "create": "$auth.id = $record.owner",
            "update": "$auth.id = $record.owner",
            "delete": "$auth.id = $record.owner"
        }
    });
    let mut response = TestClient::post("http://localhost/_/collections")
        .bearer_auth(&admin)
        .json(&collection)
        .send(&service)
        .await;
    let status = response.status_code;
    let created_collection: Value = response.take_json().await.unwrap();
    assert_eq!(status, Some(StatusCode::CREATED), "{created_collection}");

    let owner = registered["data"]["user"]["id"].as_str().unwrap();
    let mut response = TestClient::post("http://localhost/api/collections/notes/records")
        .json(&json!({"owner": owner}))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::FORBIDDEN));
    let _: Value = response.take_json().await.unwrap();

    let mut response = TestClient::post("http://localhost/api/auth/login")
        .json(&json!({
            "email": "user@example.com",
            "password": "a sufficiently long password"
        }))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let logged_in: Value = response.take_json().await.unwrap();
    let user_access = logged_in["data"]["accessToken"]
        .as_str()
        .unwrap()
        .to_owned();

    let mut response = TestClient::post("http://localhost/api/collections/notes/records")
        .bearer_auth(&user_access)
        .json(&json!({"owner": owner}))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::CREATED));
    let note: Value = response.take_json().await.unwrap();
    let note_id = note["data"]["id"].as_str().unwrap().to_owned();

    let mut response = TestClient::get("http://localhost/api/collections/notes/records")
        .bearer_auth(&user_access)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let listed: Value = response.take_json().await.unwrap();
    assert_eq!(listed["meta"]["total"], 1);

    let key = note_id.split_once(':').unwrap().1;
    let mut response = TestClient::patch(format!(
        "http://localhost/api/collections/notes/records/{key}"
    ))
    .bearer_auth(&user_access)
    .json(&json!({"owner": owner}))
    .send(&service)
    .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let _: Value = response.take_json().await.unwrap();

    let mut response = TestClient::delete(format!(
        "http://localhost/api/collections/notes/records/{key}"
    ))
    .bearer_auth(&user_access)
    .send(&service)
    .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let _: Value = response.take_json().await.unwrap();
}

#[tokio::test]
async fn realtime_sse_preflight_authentication_and_connected_frame() {
    let service = service().await;
    let admin = admin_token(&service).await;
    create_public_posts(&service, &admin).await;

    let mut response = TestClient::get("http://localhost/api/realtime/public_posts")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-cache");
    assert_eq!(response.headers().get("x-accel-buffering").unwrap(), "no");
    let frame = first_body_frame(&mut response).await;
    assert!(frame.contains("event:connected"), "{frame}");
    assert!(frame.contains("\"collection\":\"public_posts\""), "{frame}");
    assert!(frame.contains("\"subscriptionId\":"), "{frame}");

    let mut response = TestClient::get(format!(
        "http://localhost/api/realtime/public_posts?token={admin}"
    ))
    .send(&service)
    .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    drop(response.take_body());

    let mut response = TestClient::get(format!(
        "http://localhost/api/realtime/public_posts?token={admin}"
    ))
    .bearer_auth("invalid")
    .send(&service)
    .await;
    assert_eq!(response.status_code, Some(StatusCode::UNAUTHORIZED));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_AUTH_REQUIRED");

    let mut response =
        TestClient::get("http://localhost/api/realtime/public_posts?filter=unknown%20%3D%201")
            .send(&service)
            .await;
    assert_eq!(response.status_code, Some(StatusCode::BAD_REQUEST));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_INVALID_FILTER");

    let mut response = TestClient::get("http://localhost/api/realtime/_users")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::FORBIDDEN));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_FORBIDDEN");
}

#[tokio::test]
async fn realtime_connection_limit_is_released_with_response_body() {
    let service = service_with_realtime_limits(1, 1).await;
    let admin = admin_token(&service).await;
    create_public_posts(&service, &admin).await;

    let mut first = TestClient::get("http://localhost/api/realtime/public_posts")
        .send(&service)
        .await;
    assert_eq!(first.status_code, Some(StatusCode::OK));

    let mut limited = TestClient::get("http://localhost/api/realtime/public_posts")
        .send(&service)
        .await;
    assert_eq!(limited.status_code, Some(StatusCode::TOO_MANY_REQUESTS));
    let body: Value = limited.take_json().await.unwrap();
    assert_eq!(body["error"]["error"], "HB_RATE_LIMITED");

    drop(first.take_body());
    let mut released = TestClient::get("http://localhost/api/realtime/public_posts")
        .send(&service)
        .await;
    assert_eq!(released.status_code, Some(StatusCode::OK));
    drop(released.take_body());
}

#[tokio::test]
async fn realtime_token_expiry_sends_error_and_closes() {
    let service = realtime_service(2, 30).await;
    let admin = admin_token(&service).await;
    let mut response = TestClient::get("http://localhost/api/realtime/public_posts")
        .bearer_auth(&admin)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let mut body = response.take_body();
    let connected = next_body_frame(&mut body).await;
    assert!(connected.contains("event:connected"), "{connected}");

    let expired = next_body_frame(&mut body).await;
    assert!(expired.contains("event:error"), "{expired}");
    assert!(expired.contains("HB_TOKEN_EXPIRED"), "{expired}");
    let ended = timeout(Duration::from_secs(1), body.frame())
        .await
        .expect("expired stream did not close");
    assert!(ended.is_none());
}

#[tokio::test]
async fn realtime_sends_configured_ping_events() {
    let service = realtime_service(900, 1).await;
    let mut response = TestClient::get("http://localhost/api/realtime/public_posts")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let mut body = response.take_body();
    let connected = next_body_frame(&mut body).await;
    assert!(connected.contains("event:connected"), "{connected}");
    let ping = next_body_frame(&mut body).await;
    assert!(ping.contains("event:ping"), "{ping}");
    assert!(ping.contains("\"timestamp\":"), "{ping}");
}
