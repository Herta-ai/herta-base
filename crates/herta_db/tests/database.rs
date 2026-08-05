use herta_db::{
    ApiRule, CollectionDef, CollectionType, DbClient, FieldDef, FieldType, ListParams, LogEntry,
    LogType, RealtimeAction, RealtimeManager, RecordManager, RuleContext, SchemaManager,
    SchemaMode, UpdateCollectionRequest, log_channel, spawn_log_worker,
};
use serde_json::json;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn log_worker_persists_server_and_request_metadata() {
    let db = DbClient::memory().await.unwrap();
    let (sender, receiver) = log_channel();
    let worker = spawn_log_worker(db.clone(), receiver);
    sender
        .send(LogEntry {
            log_type: LogType::Request,
            level: "info".into(),
            message: "GET /health -> 200".into(),
            target: "test".into(),
            method: Some("GET".into()),
            path: Some("/health".into()),
            status_code: Some(200),
            referer: Some("https://example.test".into()),
            remote_ip: Some("192.0.2.1".into()),
            user_agent: Some("test-agent".into()),
            auth_type: Some("anonymous".into()),
            user_id: None,
            user_collection: None,
        })
        .await
        .unwrap();
    drop(sender);
    timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();

    let mut response = db.inner().query("SELECT * FROM _logs").await.unwrap();
    let rows: Vec<serde_json::Value> = response.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["path"], "/health");
    assert_eq!(rows[0]["remote_ip"], "192.0.2.1");
    assert_eq!(rows[0]["auth_type"], "anonymous");
}

fn posts_collection() -> CollectionDef {
    CollectionDef {
        name: "posts".into(),
        collection_type: CollectionType::Base,
        schema_mode: SchemaMode::Strict,
        fields: vec![
            FieldDef {
                name: "title".into(),
                field_type: FieldType::Text,
                required: true,
                options: None,
            },
            FieldDef {
                name: "status".into(),
                field_type: FieldType::Select,
                required: true,
                options: Some(json!({"values": ["draft", "active"]})),
            },
        ],
        indexes: vec![],
        rules: Default::default(),
    }
}

#[tokio::test]
async fn collection_and_record_lifecycle() {
    let db = DbClient::memory().await.unwrap();
    let schema = SchemaManager::new(&db);
    schema.create_collection(&posts_collection()).await.unwrap();

    let records = RecordManager::new(&db);
    let created = records
        .create("posts", json!({"title": "Hello", "status": "active"}))
        .await
        .unwrap();
    let id = created["id"]
        .as_str()
        .unwrap()
        .strip_prefix("posts:")
        .unwrap();

    let params = ListParams {
        filter: Some("status IN ['active'] AND title CONTAINS 'Hell'".into()),
        ..Default::default()
    };
    let (listed, total) = records.list("posts", &params).await.unwrap();
    assert_eq!(total, 1);
    assert_eq!(listed.len(), 1);

    schema
        .update_collection(
            "posts",
            &UpdateCollectionRequest {
                fields: vec![FieldDef {
                    name: "summary".into(),
                    field_type: FieldType::Text,
                    required: false,
                    options: None,
                }],
                indexes: vec![],
                rules: None,
            },
        )
        .await
        .unwrap();
    let updated = records
        .update("posts", id, json!({"summary": "short"}))
        .await
        .unwrap();
    assert_eq!(updated["summary"], "short");

    records.delete("posts", id).await.unwrap();
    assert!(records.get("posts", id, None).await.is_err());
}

#[tokio::test]
async fn creates_auth_collections_and_rejects_unknown_fields() {
    let db = DbClient::memory().await.unwrap();
    let mut collection = posts_collection();
    collection.collection_type = CollectionType::Auth;
    SchemaManager::new(&db)
        .create_collection(&collection)
        .await
        .unwrap();

    let mut collection = posts_collection();
    collection.name = "other_posts".into();
    SchemaManager::new(&db)
        .create_collection(&collection)
        .await
        .unwrap();
    assert!(
        RecordManager::new(&db)
            .create(
                "other_posts",
                json!({"title": "Hello", "status": "active", "unknown": true}),
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn expands_relation_records_without_replacing_relation_ids() {
    let db = DbClient::memory().await.unwrap();
    let schema = SchemaManager::new(&db);
    schema
        .create_collection(&CollectionDef {
            name: "users".into(),
            collection_type: CollectionType::Base,
            schema_mode: SchemaMode::Strict,
            fields: vec![FieldDef {
                name: "name".into(),
                field_type: FieldType::Text,
                required: true,
                options: None,
            }],
            indexes: vec![],
            rules: Default::default(),
        })
        .await
        .unwrap();
    schema
        .create_collection(&CollectionDef {
            name: "articles".into(),
            collection_type: CollectionType::Base,
            schema_mode: SchemaMode::Strict,
            fields: vec![
                FieldDef {
                    name: "title".into(),
                    field_type: FieldType::Text,
                    required: true,
                    options: None,
                },
                FieldDef {
                    name: "author".into(),
                    field_type: FieldType::Relation,
                    required: true,
                    options: Some(json!({"collection": "users", "maxSelect": 1})),
                },
            ],
            indexes: vec![],
            rules: herta_db::CollectionRules {
                view: ApiRule::Boolean(true),
                ..Default::default()
            },
        })
        .await
        .unwrap();

    let records = RecordManager::new(&db);
    let user = records
        .create("users", json!({"name": "Ada"}))
        .await
        .unwrap();
    let user_id = user["id"].as_str().unwrap();
    let article = records
        .create("articles", json!({"title": "Graphs", "author": user_id}))
        .await
        .unwrap();
    let article_id = article["id"]
        .as_str()
        .unwrap()
        .strip_prefix("articles:")
        .unwrap();
    let expanded = records
        .get("articles", article_id, Some("author"))
        .await
        .unwrap();
    assert_eq!(expanded["author"], user_id);
    assert_eq!(expanded["expand"]["author"]["name"], "Ada");

    let anonymous = RuleContext::default();
    let restricted = records
        .get_authorized("articles", article_id, Some("author"), &anonymous)
        .await
        .unwrap();
    assert_eq!(restricted["author"], user_id);
    assert!(restricted["expand"]["author"].is_null());
}

#[tokio::test]
async fn stores_rule_patches_and_rejects_statement_rules() {
    let db = DbClient::memory().await.unwrap();
    let schema = SchemaManager::new(&db);
    schema.create_collection(&posts_collection()).await.unwrap();

    let rules: herta_db::CollectionRules = serde_json::from_value(json!({
        "list": true,
        "view": "$auth.id = $record.owner",
        "create": false,
        "update": null,
        "delete": ""
    }))
    .unwrap();
    let updated = schema
        .update_collection(
            "posts",
            &UpdateCollectionRequest {
                fields: vec![],
                indexes: vec![],
                rules: Some(rules.clone()),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.rules, rules);

    let invalid: herta_db::CollectionRules = serde_json::from_value(json!({
        "list": "true; DELETE posts"
    }))
    .unwrap();
    assert!(
        schema
            .update_collection(
                "posts",
                &UpdateCollectionRequest {
                    fields: vec![],
                    indexes: vec![],
                    rules: Some(invalid),
                },
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn realtime_maps_record_changes_and_sanitizes_auth_records() {
    let db = DbClient::memory().await.unwrap();
    let schema = SchemaManager::new(&db);
    schema.create_collection(&posts_collection()).await.unwrap();
    let admin = RuleContext {
        admin: true,
        ..Default::default()
    };
    let mut subscription = RealtimeManager::new(&db)
        .subscribe("posts", None, &admin)
        .await
        .unwrap();
    let writer = db.clone();
    let records = RecordManager::new(&writer);
    let created = records
        .create("posts", json!({"title": "Hello", "status": "active"}))
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap().to_owned();
    let key = id.strip_prefix("posts:").unwrap();

    let event = timeout(Duration::from_secs(3), subscription.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(event.action, RealtimeAction::Create);
    assert_eq!(event.record["id"], id);

    records
        .update("posts", key, json!({"title": "Updated"}))
        .await
        .unwrap();
    let event = timeout(Duration::from_secs(3), subscription.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(event.action, RealtimeAction::Update);
    assert_eq!(event.record["title"], "Updated");

    records.delete("posts", key).await.unwrap();
    let event = timeout(Duration::from_secs(3), subscription.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(event.action, RealtimeAction::Delete);
    assert_eq!(event.record, json!({"id": id}));

    db.inner()
        .query("DELETE type::record('posts', $id)")
        .bind(("id", key.to_owned()))
        .await
        .unwrap()
        .check()
        .unwrap();
    let event = timeout(Duration::from_secs(3), subscription.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(event.action, RealtimeAction::Delete);
    assert_eq!(event.record, json!({"id": id}));

    let mut auth = posts_collection();
    auth.name = "members".into();
    auth.collection_type = CollectionType::Auth;
    auth.rules.view = ApiRule::Boolean(true);
    schema.create_collection(&auth).await.unwrap();
    let mut auth_subscription = RealtimeManager::new(&db)
        .subscribe("members", None, &RuleContext::default())
        .await
        .unwrap();
    db.inner()
        .query(
            "CREATE type::record('members', 'one') CONTENT { title: 'Member', status: 'active', \
             email: 'member@example.com', password_hash: 'secret', token_key: 'key', \
             failed_attempts: 2, locked_until: 123 }",
        )
        .await
        .unwrap()
        .check()
        .unwrap();
    let event = timeout(Duration::from_secs(3), auth_subscription.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(event.action, RealtimeAction::Create);
    for field in [
        "password_hash",
        "token_key",
        "failed_attempts",
        "locked_until",
    ] {
        assert!(event.record.get(field).is_none());
    }
}

#[tokio::test]
async fn realtime_uses_view_and_filter_native_matching_semantics() {
    let db = DbClient::memory().await.unwrap();
    let mut collection = posts_collection();
    collection.rules.view = ApiRule::Expression("$record.status = 'active'".into());
    SchemaManager::new(&db)
        .create_collection(&collection)
        .await
        .unwrap();
    let mut subscription = RealtimeManager::new(&db)
        .subscribe(
            "posts",
            Some("title CONTAINS 'match'"),
            &RuleContext::default(),
        )
        .await
        .unwrap();
    let records = RecordManager::new(&db);
    records
        .create("posts", json!({"title": "match me", "status": "draft"}))
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_millis(150), subscription.next())
            .await
            .is_err()
    );

    let created = records
        .create("posts", json!({"title": "match me", "status": "active"}))
        .await
        .unwrap();
    let key = created["id"]
        .as_str()
        .unwrap()
        .strip_prefix("posts:")
        .unwrap();
    let event = timeout(Duration::from_secs(3), subscription.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(event.action, RealtimeAction::Create);

    records
        .update("posts", key, json!({"status": "draft"}))
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_millis(150), subscription.next())
            .await
            .is_err()
    );
}
