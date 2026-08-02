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
    let state = ApiState::new(db, config).await.unwrap();
    Service::new(build_router()).hoop(affix_state::inject(state))
}

#[tokio::test]
async fn collection_record_and_openapi_flow() {
    let service = service().await;
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
        .json(&collection)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::CREATED));
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(body["data"]["name"], "posts");
    assert!(body["error"].is_null());

    let mut response = TestClient::get("http://localhost/api-doc/openapi.json")
        .send(&service)
        .await;
    let document: Value = response.take_json().await.unwrap();
    assert!(document["paths"]["/api/collections/posts/records"].is_object());
    assert_eq!(
        document["components"]["schemas"]["postsCreate"]["required"][0],
        "title"
    );

    let mut response = TestClient::patch("http://localhost/_/collections/posts")
        .json(&json!({
            "fields": [{"name": "summary", "type": "text"}],
            "indexes": []
        }))
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let _: Value = response.take_json().await.unwrap();
    let mut response = TestClient::get("http://localhost/api-doc/openapi.json")
        .send(&service)
        .await;
    let document: Value = response.take_json().await.unwrap();
    assert!(document["components"]["schemas"]["postsCreate"]["properties"]["summary"].is_object());

    let mut response = TestClient::post("http://localhost/api/collections/posts/records")
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
    .send(&service)
    .await;
    let listed: Value = response.take_json().await.unwrap();
    assert_eq!(listed["meta"]["total"], 1);
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);

    let mut response = TestClient::delete("http://localhost/_/collections/posts")
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let _: Value = response.take_json().await.unwrap();
    let mut response = TestClient::get("http://localhost/api-doc/openapi.json")
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
