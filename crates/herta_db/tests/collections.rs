use herta_core::{HbConfig, HbError};
use herta_db::{
    DbClient, SchemaManager,
    collections::CollectionService,
    transaction::{OperationState, TransactionOwner, operation_status},
};
use serde_json::{Value, json};

async fn exercise(engine: &str) {
    let directory = tempfile::tempdir().unwrap();
    let mut config = HbConfig::default();
    config.database.engine = engine.into();
    config.paths.data_dir = directory.path().to_string_lossy().into_owned();
    let db = DbClient::init(&config).await.unwrap();
    let definition = json!({"name":"managed","type":"base","schema_mode":"strict","fields":[{"name":"title","type":"text"}]});
    let created = CollectionService::apply(db.clone(), None, "create".into(), definition, false)
        .await
        .unwrap();
    assert!(created["version"].is_string());
    let update = |original: &Value, rule: bool| {
        let mut value = original.clone();
        value["rules"]["list"] = rule.into();
        value
    };
    let updated = CollectionService::apply(
        db.clone(),
        None,
        "update".into(),
        update(&created, true),
        false,
    )
    .await
    .unwrap();
    assert_ne!(updated["version"], created["version"]);
    assert!(matches!(
        CollectionService::apply(
            db.clone(),
            None,
            "update".into(),
            update(&created, false),
            false
        )
        .await,
        Err(HbError::Conflict(_))
    ));
    assert_eq!(
        CollectionService::apply(db.clone(), None, "update".into(), updated.clone(), false)
            .await
            .unwrap(),
        updated
    );
    let mut invalid = updated.clone();
    invalid["fields"] = json!([]);
    assert!(
        CollectionService::apply(db.clone(), None, "update".into(), invalid, false)
            .await
            .is_err()
    );
    let mut invalid = updated.clone();
    invalid["type"] = "auth".into();
    assert!(
        CollectionService::apply(db.clone(), None, "update".into(), invalid, false)
            .await
            .is_err()
    );

    let first = TransactionOwner::new(db.clone(), None);
    let second = TransactionOwner::new(db.clone(), None);
    let a = first.begin().await.unwrap();
    let b = second.begin().await.unwrap();
    for (owner, id, rule) in [(&first, &a.id, false), (&second, &b.id, true)] {
        let original = updated.clone();
        let mut candidate = update(&original, rule);
        candidate["rules"]["view"] = rule.into();
        let id = id.clone();
        owner
            .execute(Some(id.clone()), true, move |session| {
                Box::pin(async move {
                    CollectionService::new(session)
                        .write(&id, "update", original, candidate)
                        .await
                })
            })
            .await
            .unwrap();
    }
    first.commit(&a.id).await.unwrap();
    let error = second.commit(&b.id).await.unwrap_err();
    assert!(
        matches!(error, HbError::Conflict(_)),
        "typed database conflict must be known rollback: {error:?}"
    );
    assert_eq!(
        operation_status(&db, &b.operation_id, None, Some(&b.check_credential))
            .await
            .unwrap()
            .state,
        OperationState::RolledBack
    );

    let owner = TransactionOwner::new(db.clone(), None);
    let receipt = owner.begin().await.unwrap();
    let id = receipt.id.clone();
    owner
        .execute(Some(id.clone()), true, move |session| {
            Box::pin(async move {
                let service = CollectionService::new(session);
                let original = service.get("managed").await?;
                service
                    .write(&id, "delete", original.clone(), original)
                    .await
            })
        })
        .await
        .unwrap();
    owner.finish().await.unwrap();
    assert!(
        SchemaManager::new(&db)
            .get_collection("managed")
            .await
            .is_ok()
    );
    CollectionService::apply(db.clone(), None, "delete".into(), "managed".into(), false)
        .await
        .unwrap();
    assert!(
        CollectionService::apply(
            db.clone(),
            None,
            "create".into(),
            json!({"name":"managed","type":"base","schema_mode":"strict"}),
            false
        )
        .await
        .is_err(),
        "pending cleanup blocks name reuse"
    );
}

#[tokio::test]
async fn memory_collection_versions_conflicts_and_rollback() {
    exercise("memory").await;
}
#[tokio::test]
async fn surrealkv_collection_versions_conflicts_and_rollback() {
    exercise("surrealkv").await;
}
