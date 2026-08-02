use herta_db::{
    ApiRule, CollectionDef, CollectionType, DbClient, FieldDef, FieldType, ListParams,
    RecordManager, RuleContext, SchemaManager, SchemaMode, UpdateCollectionRequest,
};
use serde_json::json;

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
