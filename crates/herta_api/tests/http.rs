use herta_api::{ApiState, build_router};
use herta_core::HbConfig;
use herta_db::DbClient;
use salvo::{
    http::StatusCode,
    prelude::*,
    test::{ResponseExt, TestClient},
};
use serde_json::{Value, json};

async fn service() -> Service {
    let db = DbClient::memory().await.unwrap();
    let mut config = HbConfig::default();
    config.database.engine = "memory".into();
    config.server.dev_mode = true;
    config.auth.bootstrap_admin_email = Some("admin@example.com".into());
    config.auth.bootstrap_admin_password = Some("correct horse battery staple".into());
    let state = ApiState::new(db, config).await.unwrap();
    Service::new(build_router()).hoop(affix_state::inject(state))
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

    let mut response = TestClient::get(
        "http://localhost/api/collections/posts/records?filter=status%20IN%20%5B%27active%27%5D",
    )
    .bearer_auth(&admin)
    .send(&service)
    .await;
    let listed: Value = response.take_json().await.unwrap();
    assert_eq!(listed["meta"]["total"], 1);
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);

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
