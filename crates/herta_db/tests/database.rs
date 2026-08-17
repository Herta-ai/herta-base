use herta_core::HbError;
use herta_db::{
    ApiRule, CollectionDef, CollectionType, DbClient, FieldDef, FieldType, ListParams, LogEntry,
    LogManager, LogQuery, LogType, RealtimeAction, RealtimeManager, RecordManager, RuleContext,
    SchemaManager, SchemaMode, UpdateCollectionRequest, log_channel, spawn_log_worker,
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

#[tokio::test]
async fn log_query_supports_pagination_filters_and_time_range() {
    let db = DbClient::memory().await.unwrap();
    let (sender, receiver) = log_channel();
    let worker = spawn_log_worker(db.clone(), receiver);
    for entry in [
        LogEntry {
            log_type: LogType::Server,
            level: "info".into(),
            message: "Database started".into(),
            target: "herta_server::startup".into(),
            method: None,
            path: None,
            status_code: None,
            referer: None,
            remote_ip: None,
            user_agent: None,
            auth_type: None,
            user_id: None,
            user_collection: None,
        },
        LogEntry {
            log_type: LogType::Request,
            level: "error".into(),
            message: "GET /admin failed".into(),
            target: "herta_api::request".into(),
            method: Some("GET".into()),
            path: Some("/admin".into()),
            status_code: Some(500),
            referer: Some("https://example.test".into()),
            remote_ip: Some("192.0.2.10".into()),
            user_agent: Some("admin-browser".into()),
            auth_type: Some("admin".into()),
            user_id: Some("_admins:one".into()),
            user_collection: Some("_admins".into()),
        },
        LogEntry {
            log_type: LogType::Request,
            level: "warn".into(),
            message: "GET /health -> 429".into(),
            target: "herta_api::request".into(),
            method: Some("GET".into()),
            path: Some("/health".into()),
            status_code: Some(429),
            referer: None,
            remote_ip: Some("192.0.2.11".into()),
            user_agent: Some("probe".into()),
            auth_type: Some("anonymous".into()),
            user_id: None,
            user_collection: None,
        },
    ] {
        sender.send(entry).await.unwrap();
    }
    drop(sender);
    timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();

    let manager = LogManager::new(&db);
    let (page, total) = manager
        .list(&LogQuery {
            per_page: Some(2),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(total, 3);
    assert_eq!(page.len(), 2);
    let (repeat, _) = manager.list(&LogQuery::default()).await.unwrap();
    let (repeat_again, _) = manager.list(&LogQuery::default()).await.unwrap();
    assert_eq!(repeat[0]["id"], repeat_again[0]["id"]);

    let (errors, total) = manager
        .list(&LogQuery {
            level: Some("ERROR".into()),
            log_type: Some("REQUEST".into()),
            keyword: Some("ADMIN-BROWSER".into()),
            target: Some("herta_api::request".into()),
            path: Some("/admin".into()),
            status_code: Some(500),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(errors[0]["message"], "GET /admin failed");

    let (_, total) = manager
        .list(&LogQuery {
            from: Some("2000-01-01T00:00:00Z".into()),
            to: Some("2100-01-01T00:00:00Z".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(total, 3);
    let (_, total) = manager
        .list(&LogQuery {
            to: Some("2000-01-01T00:00:00Z".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(total, 0);

    for invalid in [
        LogQuery {
            page: Some(0),
            ..Default::default()
        },
        LogQuery {
            per_page: Some(501),
            ..Default::default()
        },
        LogQuery {
            level: Some("verbose".into()),
            ..Default::default()
        },
        LogQuery {
            from: Some("not-a-date".into()),
            ..Default::default()
        },
        LogQuery {
            from: Some("2026-01-02T00:00:00Z".into()),
            to: Some("2026-01-01T00:00:00Z".into()),
            ..Default::default()
        },
    ] {
        assert!(manager.list(&invalid).await.is_err());
    }
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
    let full_id = created["id"].as_str().unwrap();
    let id = full_id.strip_prefix("posts:").unwrap();
    assert_eq!(
        records.get("posts", full_id, None).await.unwrap()["id"],
        full_id
    );
    assert_eq!(records.get("posts", id, None).await.unwrap()["id"], full_id);
    assert!(matches!(
        records.get("posts", &format!("other:{id}"), None).await,
        Err(HbError::NotFound)
    ));

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

    records.delete("posts", full_id).await.unwrap();
    assert!(matches!(
        records.get("posts", full_id, None).await,
        Err(HbError::NotFound)
    ));
    assert!(matches!(
        records
            .update("posts", full_id, json!({"summary": "too late"}))
            .await,
        Err(HbError::Forbidden)
    ));
    assert!(matches!(
        records.delete("posts", full_id).await,
        Err(HbError::Forbidden)
    ));
    let (remaining, total) = records.list("posts", &ListParams::default()).await.unwrap();
    assert_eq!(total, 0);
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn auth_record_rules_cover_owner_crud() {
    let db = DbClient::memory().await.unwrap();
    let schema = SchemaManager::new(&db);
    schema
        .create_collection(&CollectionDef {
            name: "owners".into(),
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
    let owner_rule = ApiRule::Expression("$record.owner = $auth.record".into());
    schema
        .create_collection(&CollectionDef {
            name: "documents".into(),
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
                    name: "owner".into(),
                    field_type: FieldType::Relation,
                    required: true,
                    options: Some(json!({"collection": "owners", "maxSelect": 1})),
                },
            ],
            indexes: vec![],
            rules: herta_db::CollectionRules {
                list: owner_rule.clone(),
                view: owner_rule.clone(),
                create: owner_rule.clone(),
                update: owner_rule.clone(),
                delete: owner_rule,
            },
        })
        .await
        .unwrap();

    let records = RecordManager::new(&db);
    let owner = records
        .create("owners", json!({"name": "Owner"}))
        .await
        .unwrap();
    let stranger = records
        .create("owners", json!({"name": "Stranger"}))
        .await
        .unwrap();
    let owner_id = owner["id"].as_str().unwrap();
    let stranger_id = stranger["id"].as_str().unwrap();
    let context = |id: &str| RuleContext {
        admin: false,
        auth: json!({"id": id, "admin": false}),
        auth_record: Some(herta_db::record::parse_record_id(id).unwrap()),
        request_body: serde_json::Value::Null,
    };
    let owner_context = context(owner_id);
    let stranger_context = context(stranger_id);

    assert!(matches!(
        records
            .create_authorized(
                "documents",
                json!({"title": "Forged", "owner": owner_id}),
                &stranger_context,
            )
            .await,
        Err(HbError::Forbidden)
    ));
    let document = records
        .create_authorized(
            "documents",
            json!({"title": "Owned", "owner": owner_id}),
            &owner_context,
        )
        .await
        .unwrap();
    let document_id = document["id"].as_str().unwrap();
    assert_eq!(document["owner"], owner_id);

    let (owned, total) = records
        .list_authorized("documents", &ListParams::default(), &owner_context)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(owned[0]["id"], document_id);
    let (hidden, total) = records
        .list_authorized("documents", &ListParams::default(), &stranger_context)
        .await
        .unwrap();
    assert_eq!(total, 0);
    assert!(hidden.is_empty());
    assert!(
        records
            .get_authorized("documents", document_id, None, &owner_context)
            .await
            .is_ok()
    );
    assert!(matches!(
        records
            .get_authorized("documents", document_id, None, &stranger_context)
            .await,
        Err(HbError::NotFound)
    ));
    assert!(matches!(
        records
            .update_authorized(
                "documents",
                document_id,
                json!({"title": "Denied"}),
                &stranger_context,
            )
            .await,
        Err(HbError::Forbidden)
    ));
    assert!(matches!(
        records
            .delete_authorized("documents", document_id, &stranger_context)
            .await,
        Err(HbError::Forbidden)
    ));
    assert_eq!(
        records
            .update_authorized(
                "documents",
                document_id,
                json!({"title": "Updated"}),
                &owner_context,
            )
            .await
            .unwrap()["title"],
        "Updated"
    );
    records
        .delete_authorized("documents", document_id, &owner_context)
        .await
        .unwrap();
}

#[tokio::test]
async fn create_rules_can_follow_native_relation_paths() {
    let db = DbClient::memory().await.unwrap();
    let schema = SchemaManager::new(&db);
    schema
        .create_collection(&CollectionDef {
            name: "authors".into(),
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
            name: "rule_posts".into(),
            collection_type: CollectionType::Base,
            schema_mode: SchemaMode::Strict,
            fields: vec![
                FieldDef {
                    name: "is_public".into(),
                    field_type: FieldType::Bool,
                    required: true,
                    options: None,
                },
                FieldDef {
                    name: "author".into(),
                    field_type: FieldType::Relation,
                    required: true,
                    options: Some(json!({"collection": "authors", "maxSelect": 1})),
                },
            ],
            indexes: vec![],
            rules: Default::default(),
        })
        .await
        .unwrap();
    schema
        .create_collection(&CollectionDef {
            name: "rule_comments".into(),
            collection_type: CollectionType::Base,
            schema_mode: SchemaMode::Strict,
            fields: vec![
                FieldDef {
                    name: "post".into(),
                    field_type: FieldType::Relation,
                    required: true,
                    options: Some(json!({"collection": "rule_posts", "maxSelect": 1})),
                },
                FieldDef {
                    name: "author".into(),
                    field_type: FieldType::Relation,
                    required: true,
                    options: Some(json!({"collection": "authors", "maxSelect": 1})),
                },
            ],
            indexes: vec![],
            rules: herta_db::CollectionRules {
                create: ApiRule::Expression(
                    "$record.author = $auth.record AND ($record.post.is_public = true OR $record.post.author = $auth.record)".into(),
                ),
                ..Default::default()
            },
        })
        .await
        .unwrap();

    let records = RecordManager::new(&db);
    let owner = records
        .create("authors", json!({"name": "Owner"}))
        .await
        .unwrap();
    let stranger = records
        .create("authors", json!({"name": "Stranger"}))
        .await
        .unwrap();
    let owner_id = owner["id"].as_str().unwrap();
    let stranger_id = stranger["id"].as_str().unwrap();
    let public_post = records
        .create("rule_posts", json!({"is_public": true, "author": owner_id}))
        .await
        .unwrap();
    let private_post = records
        .create(
            "rule_posts",
            json!({"is_public": false, "author": owner_id}),
        )
        .await
        .unwrap();
    let context = |id: &str| RuleContext {
        admin: false,
        auth: json!({"id": id, "admin": false}),
        auth_record: Some(herta_db::record::parse_record_id(id).unwrap()),
        request_body: serde_json::Value::Null,
    };

    assert!(
        records
            .create_authorized(
                "rule_comments",
                json!({"post": public_post["id"], "author": stranger_id}),
                &context(stranger_id),
            )
            .await
            .is_ok()
    );
    assert!(matches!(
        records
            .create_authorized(
                "rule_comments",
                json!({"post": private_post["id"], "author": stranger_id}),
                &context(stranger_id),
            )
            .await,
        Err(HbError::Forbidden)
    ));
    assert!(
        records
            .create_authorized(
                "rule_comments",
                json!({"post": private_post["id"], "author": owner_id}),
                &context(owner_id),
            )
            .await
            .is_ok()
    );
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
async fn filters_unauthorized_records_from_relation_array_expansion() {
    let db = DbClient::memory().await.unwrap();
    let schema = SchemaManager::new(&db);
    schema
        .create_collection(&CollectionDef {
            name: "profiles".into(),
            collection_type: CollectionType::Base,
            schema_mode: SchemaMode::Strict,
            fields: vec![
                FieldDef {
                    name: "name".into(),
                    field_type: FieldType::Text,
                    required: true,
                    options: None,
                },
                FieldDef {
                    name: "visible".into(),
                    field_type: FieldType::Bool,
                    required: true,
                    options: None,
                },
            ],
            indexes: vec![],
            rules: herta_db::CollectionRules {
                view: ApiRule::Expression("$record.visible = true".into()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    schema
        .create_collection(&CollectionDef {
            name: "teams".into(),
            collection_type: CollectionType::Base,
            schema_mode: SchemaMode::Strict,
            fields: vec![FieldDef {
                name: "members".into(),
                field_type: FieldType::Relation,
                required: true,
                options: Some(json!({"collection": "profiles", "maxSelect": 2})),
            }],
            indexes: vec![],
            rules: herta_db::CollectionRules {
                view: ApiRule::Boolean(true),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    let records = RecordManager::new(&db);
    let visible = records
        .create("profiles", json!({"name": "Visible", "visible": true}))
        .await
        .unwrap();
    let hidden = records
        .create("profiles", json!({"name": "Hidden", "visible": false}))
        .await
        .unwrap();
    let member_ids = json!([visible["id"], hidden["id"]]);
    let team = records
        .create("teams", json!({"members": member_ids.clone()}))
        .await
        .unwrap();
    let expanded = records
        .get_authorized(
            "teams",
            team["id"].as_str().unwrap(),
            Some("members"),
            &RuleContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(expanded["members"], member_ids);
    assert_eq!(expanded["expand"]["members"].as_array().unwrap().len(), 1);
    assert_eq!(expanded["expand"]["members"][0]["id"], visible["id"]);
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
