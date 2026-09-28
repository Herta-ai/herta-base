use std::time::Duration;

use futures_util::StreamExt;
use herta_core::HbConfig;
use herta_db::{CollectionDef, DbClient, DbSession, RecordManager, SchemaManager};
use serde_json::{Value, json};

async fn values(session: DbSession<'_>) -> Vec<Value> {
    session
        .query("SELECT * FROM txn_probe ORDER BY value")
        .await
        .unwrap()
        .check()
        .unwrap()
        .take(0)
        .unwrap()
}

async fn exercise(engine: &str) {
    let directory = tempfile::tempdir().unwrap();
    let mut config = HbConfig::default();
    config.database.engine = engine.into();
    config.paths.data_dir = directory.path().to_string_lossy().into_owned();
    let db = DbClient::init(&config).await.unwrap();
    db.inner()
        .query("DEFINE TABLE txn_probe SCHEMALESS")
        .await
        .unwrap()
        .check()
        .unwrap();
    let mut live = db
        .inner()
        .select::<Vec<Value>>("txn_probe")
        .live()
        .await
        .unwrap();
    let transaction = db.inner().clone().begin().await.unwrap();
    transaction
        .query("CREATE txn_probe:one SET value = 1")
        .await
        .unwrap()
        .check()
        .unwrap();
    transaction
        .query("CREATE txn_probe:two SET value = 2")
        .await
        .unwrap()
        .check()
        .unwrap();
    assert_eq!(values((&transaction).into()).await.len(), 2);
    assert!(values((&db).into()).await.is_empty());
    assert!(
        tokio::time::timeout(Duration::from_millis(30), live.next())
            .await
            .is_err()
    );
    transaction.cancel().await.unwrap();
    assert!(values((&db).into()).await.is_empty());

    let transaction = db.inner().clone().begin().await.unwrap();
    transaction
        .query("CREATE txn_probe:one SET value = $value")
        .bind(("value", json!(7)))
        .await
        .unwrap()
        .check()
        .unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(values((&db).into()).await[0]["value"], 7);
    let notification = tokio::time::timeout(Duration::from_secs(3), live.next())
        .await
        .expect("explicit transaction commit must notify LIVE SELECT")
        .unwrap()
        .unwrap();
    assert_eq!(notification.data["value"], 7);

    // All manager operations, including Schema reads and Rules, use the same
    // explicit transaction. Collections created here do not exist outside it.
    let transaction = db.inner().clone().begin().await.unwrap();
    let schema: CollectionDef = serde_json::from_value(json!({
        "name":"managed", "type":"base", "schema_mode":"strict",
        "fields":[{"name":"value","type":"number","required":true}],
        "rules":{"create":"$record.value >= 1"}
    }))
    .unwrap();
    SchemaManager::new(&transaction)
        .create_collection(&schema)
        .await
        .unwrap();
    let record = RecordManager::new(&transaction)
        .create("managed", json!({"value":1}))
        .await
        .unwrap();
    let id = record["id"].as_str().unwrap();
    RecordManager::new(&transaction)
        .update("managed", id, json!({"value":2}))
        .await
        .unwrap();
    assert_eq!(
        RecordManager::new(&transaction)
            .get("managed", id, None)
            .await
            .unwrap()["value"],
        2
    );
    assert!(
        SchemaManager::new(&db)
            .get_collection("managed")
            .await
            .is_err()
    );
    transaction.cancel().await.unwrap();
    assert!(
        SchemaManager::new(&db)
            .get_collection("managed")
            .await
            .is_err()
    );

    let transaction = db.inner().clone().begin().await.unwrap();
    SchemaManager::new(&transaction)
        .create_collection(&schema)
        .await
        .unwrap();
    let context = herta_db::RuleContext {
        request_body: json!({"original":true}),
        ..Default::default()
    };
    RecordManager::new(&transaction)
        .create_authorized("managed", json!({"value":3}), &context)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(
        RecordManager::new(&db)
            .list("managed", &Default::default())
            .await
            .unwrap()
            .0[0]["value"],
        3
    );
    let manager = RecordManager::new(&db);
    for value in [4, 5] {
        manager
            .create("managed", json!({"value": value}))
            .await
            .unwrap();
    }
    let admin = herta_db::RuleContext {
        admin: true,
        ..Default::default()
    };
    let parameters = json!({"minimum":3,"count":1,"offset":1})
        .as_object()
        .unwrap()
        .clone();
    let rows = manager.select_authorized(
        "SELECT `value` FROM managed WHERE `value` >= $minimum ORDER BY `value` ASC LIMIT $count START $offset",
        &parameters, &admin).await.unwrap();
    assert_eq!(rows, vec![json!({"value":4})]);
    assert!(
        manager
            .select_authorized("SELECT * FROM _users", &Default::default(), &admin)
            .await
            .is_err()
    );
    assert!(
        db.inner()
            .query("RETURN http::get('http://127.0.0.1:9')")
            .await
            .unwrap()
            .check()
            .is_err()
    );
    assert!(
        db.inner()
            .query("RETURN function() { return 1; }")
            .await
            .unwrap()
            .check()
            .is_err()
    );

    let first = db.inner().clone().begin().await.unwrap();
    let second = db.inner().clone().begin().await.unwrap();
    first
        .query("UPDATE txn_probe:one SET value=11")
        .await
        .unwrap()
        .check()
        .unwrap();
    let second_write = second
        .query("UPDATE txn_probe:one SET value=12")
        .await
        .unwrap()
        .check();
    first.commit().await.unwrap();
    if second_write.is_ok() {
        assert!(second.commit().await.is_err());
    } else {
        second.cancel().await.unwrap();
    }
    assert_eq!(values((&db).into()).await[0]["value"], 11);
    exercise_owner(&db).await;
    drop(live);
    drop(db);
}

async fn exercise_owner(db: &DbClient) {
    use herta_core::HbError;
    use herta_db::transaction::{OperationState, TransactionOwner, operation_status};
    use std::sync::Arc;
    let owner = TransactionOwner::new(db.clone(), Some("users:original".into()));
    let receipt = owner.begin().await.unwrap();
    assert!(owner.begin().await.is_err());
    owner
        .execute(Some(receipt.id.clone()), true, |session| {
            Box::pin(async move {
                RecordManager::new(session)
                    .create("managed", json!({"value":99}))
                    .await
            })
        })
        .await
        .unwrap();
    // A caught persistent failure still poisons the whole owner transaction.
    let failed: Result<(), _> = owner
        .execute(Some(receipt.id.clone()), true, |_| {
            Box::pin(async { Err(HbError::validation("invalid nested write")) })
        })
        .await;
    assert!(failed.is_err());
    assert_eq!(
        owner.commit(&receipt.id).await.unwrap_err().error_code(),
        "HB_HOOK_ABORTED"
    );
    assert_eq!(
        operation_status(db, &receipt.operation_id, Some("users:original"), None)
            .await
            .unwrap()
            .state,
        OperationState::RolledBack
    );
    assert!(
        operation_status(db, &receipt.operation_id, Some("users:other"), None)
            .await
            .is_err()
    );
    assert_eq!(
        operation_status(
            db,
            &receipt.operation_id,
            None,
            Some(&receipt.check_credential)
        )
        .await
        .unwrap()
        .state,
        OperationState::RolledBack
    );
    assert!(
        RecordManager::new(db)
            .list("managed", &Default::default())
            .await
            .unwrap()
            .0
            .iter()
            .all(|row| row["value"] != 99)
    );

    let receipt = owner.begin().await.unwrap();
    owner
        .execute(Some(receipt.id.clone()), true, |session| {
            Box::pin(async move {
                RecordManager::new(session)
                    .create("managed", json!({"value":100}))
                    .await
            })
        })
        .await
        .unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let blocking = tokio::spawn({
        let owner = owner.clone();
        let id = receipt.id.clone();
        let entered = entered.clone();
        let release = release.clone();
        async move {
            owner
                .execute(Some(id), false, move |_| {
                    Box::pin(async move {
                        entered.notify_one();
                        release.notified().await;
                        Ok(())
                    })
                })
                .await
        }
    });
    entered.notified().await;
    {
        let mut commit = Box::pin(owner.commit(&receipt.id));
        assert!(futures_util::poll!(commit.as_mut()).is_pending());
        // The accepted commit task now outlives this dropped result future.
    }
    release.notify_one();
    blocking.await.unwrap().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if operation_status(db, &receipt.operation_id, Some("users:original"), None)
                .await
                .unwrap()
                .state
                == OperationState::Committed
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    owner.finish().await.unwrap();
    assert!(
        RecordManager::new(db)
            .list("managed", &Default::default())
            .await
            .unwrap()
            .0
            .iter()
            .any(|row| row["value"] == 100)
    );
}

#[tokio::test]
async fn memory_transactions_have_isolation_rollback_and_live_commit() {
    exercise("memory").await;
}

#[tokio::test]
async fn surrealkv_transactions_have_isolation_rollback_and_live_commit() {
    exercise("surrealkv").await;
}
