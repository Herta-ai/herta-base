use bytes::Bytes;
use herta_api::{extensions::ApiHostFactory, files::FileService};
use herta_core::{
    HbConfig, HbError, HbResult,
    extension::{AuthMode, HostCall, HostContext, HostFactory},
};
use herta_db::DbClient;
use herta_storage::{ObjectMetadata, ObjectPage, ObjectStoreStorage, Storage, StoredObject};
use salvo::async_trait;
use serde_json::{Value, json};
use std::{
    ops::Range,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct FaultStorage {
    inner: ObjectStoreStorage,
    fail_put: AtomicBool,
    fail_delete: AtomicBool,
    discard_put: AtomicBool,
    fail_head: AtomicBool,
    fail_delete_after: AtomicBool,
}
impl FaultStorage {
    fn new() -> Self {
        Self {
            inner: ObjectStoreStorage::memory(),
            fail_put: AtomicBool::new(false),
            fail_delete: AtomicBool::new(false),
            discard_put: AtomicBool::new(false),
            fail_head: AtomicBool::new(false),
            fail_delete_after: AtomicBool::new(false),
        }
    }
}
#[async_trait]
impl Storage for FaultStorage {
    async fn list_page(
        &self,
        prefix: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> HbResult<ObjectPage> {
        self.inner.list_page(prefix, limit, cursor).await
    }
    async fn put_bytes(&self, key: &str, bytes: Bytes) -> HbResult<ObjectMetadata> {
        if self.discard_put.load(Ordering::SeqCst) {
            return Err(HbError::Storage(
                "connection lost before acknowledgement".into(),
            ));
        }
        let result = self.inner.put_bytes(key, bytes).await?;
        if self.fail_put.load(Ordering::SeqCst) {
            return Err(HbError::Storage("acknowledgement lost".into()));
        }
        Ok(result)
    }
    async fn copy(
        &self,
        source: &str,
        destination: &str,
        max_bytes: u64,
    ) -> HbResult<ObjectMetadata> {
        self.inner.copy(source, destination, max_bytes).await
    }
    async fn put_file(&self, key: &str, source: &Path) -> HbResult<ObjectMetadata> {
        self.inner.put_file(key, source).await
    }
    async fn head(&self, key: &str) -> HbResult<ObjectMetadata> {
        if self.fail_head.load(Ordering::SeqCst) {
            return Err(HbError::Storage("metadata unavailable".into()));
        }
        self.inner.head(key).await
    }
    async fn get(&self, key: &str, range: Option<Range<u64>>) -> HbResult<StoredObject> {
        self.inner.get(key, range).await
    }
    async fn delete(&self, key: &str) -> HbResult<()> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(HbError::Storage("delete unavailable".into()));
        }
        self.inner.delete(key).await?;
        if self.fail_delete_after.load(Ordering::SeqCst) {
            return Err(HbError::Storage("delete acknowledgement lost".into()));
        }
        Ok(())
    }
    async fn delete_prefix(&self, prefix: &str) -> HbResult<()> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(HbError::Storage("delete prefix unavailable".into()));
        }
        self.inner.delete_prefix(prefix).await?;
        if self.fail_delete_after.load(Ordering::SeqCst) {
            return Err(HbError::Storage(
                "delete prefix acknowledgement lost".into(),
            ));
        }
        Ok(())
    }
}
fn config(quota: u64) -> herta_core::jsvm::JsFilesConfig {
    herta_core::jsvm::JsFilesConfig {
        enabled: true,
        quota_bytes: quota,
        max_file_bytes: quota,
        ..Default::default()
    }
}
async fn write(files: &FileService, key: &str, text: &str) -> HbResult<Value> {
    files
        .call(
            "files.write",
            json!({"key":key,"text":text,"contentType":"text/plain"}),
        )
        .await
}
async fn count(db: &DbClient) -> usize {
    let mut response = db
        .inner()
        .query("SELECT * FROM _extension_file_versions")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Value> = response.take(0).unwrap();
    rows.len()
}

#[tokio::test]
async fn physical_orphans_are_paged_journaled_and_never_confused_with_live_files() {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = HbConfig::default();
    settings.database.engine = "surrealkv".into();
    settings.paths.data_dir = directory.path().to_string_lossy().into_owned();
    for db in [
        DbClient::memory().await.unwrap(),
        DbClient::init(&settings).await.unwrap(),
    ] {
        let storage = Arc::new(FaultStorage::new());
        let files = FileService::new(db.clone(), storage.clone(), config(1000));
        let live = write(&files, "live", "keep").await.unwrap();
        // Simulate damaged metadata: the live directory must independently protect the object.
        db.inner()
            .query("DELETE type::record('_extension_file_versions', $version)")
            .bind(("version", live["version"].as_str().unwrap().to_owned()))
            .await
            .unwrap()
            .check()
            .unwrap();
        for _ in 0..205 {
            storage
                .inner
                .put_bytes(
                    &format!("extensions/objects/{}", uuid::Uuid::now_v7()),
                    Bytes::from_static(b"orphan"),
                )
                .await
                .unwrap();
        }
        let temporary = format!("extensions/objects/.hb-{}.tmp", uuid::Uuid::now_v7());
        storage
            .inner
            .put_bytes(&temporary, Bytes::from_static(b"partial"))
            .await
            .unwrap();
        let unrelated = [
            "extensions/objects/operator-note".to_owned(),
            format!("extensions/objects/nested/{}", uuid::Uuid::now_v7()),
            format!("other/objects/{}", uuid::Uuid::now_v7()),
        ];
        for key in &unrelated {
            storage
                .inner
                .put_bytes(key, Bytes::from_static(b"keep"))
                .await
                .unwrap();
        }
        storage.fail_delete.store(true, Ordering::SeqCst);
        files.reconcile().await.unwrap();
        assert_eq!(
            count(&db).await,
            206,
            "failed cleanup retains all orphan reservations across pages"
        );
        assert!(matches!(
            write(&files, "over-quota", "x").await,
            Err(HbError::PayloadTooLarge)
        ));
        assert_eq!(
            files
                .call("files.readText", json!({"key":"live"}))
                .await
                .unwrap(),
            "keep"
        );
        drop(files);
        storage.fail_delete.store(false, Ordering::SeqCst);
        let files = FileService::new(db.clone(), storage.clone(), config(1000));
        files.reconcile().await.unwrap();
        assert_eq!(count(&db).await, 0);
        assert_eq!(
            storage
                .inner
                .list("extensions/objects", 1000)
                .await
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            files
                .call("files.readText", json!({"key":"live"}))
                .await
                .unwrap(),
            "keep"
        );
        for key in unrelated {
            assert!(storage.head(&key).await.is_ok());
        }
    }
}

#[tokio::test]
async fn memory_and_local_files_have_stable_pagination_and_immutable_versions() {
    let directory = tempfile::tempdir().unwrap();
    for storage in [
        ObjectStoreStorage::memory(),
        ObjectStoreStorage::local(directory.path()).unwrap(),
    ] {
        let db = DbClient::memory().await.unwrap();
        let files = FileService::new(db.clone(), Arc::new(storage), config(100));
        let first = write(&files, "exports/a", "old").await.unwrap();
        let second = write(&files, "exports/a", "new").await.unwrap();
        assert_ne!(first["version"], second["version"]);
        assert_eq!(count(&db).await, 1);
        write(&files, "exports/c", "c").await.unwrap();
        let page = files.call("files.list", json!({"prefix":"exports/"})).await;
        assert_eq!(page.unwrap()["items"].as_array().unwrap().len(), 2);
        let page = files
            .call("files.list", json!({"prefix":"exports","limit":1}))
            .await
            .unwrap();
        assert_eq!(page["items"][0]["key"], "exports/a");
        write(&files, "exports/b", "b").await.unwrap();
        let next = files
            .call(
                "files.list",
                json!({"prefix":"exports","limit":1,"cursor":page["nextCursor"]}),
            )
            .await
            .unwrap();
        assert_eq!(next["items"][0]["key"], "exports/b");
        assert!(
            files
                .call(
                    "files.list",
                    json!({"prefix":"other","cursor":page["nextCursor"]})
                )
                .await
                .is_err()
        );
        for limit in [0, 501] {
            assert!(
                files
                    .call("files.list", json!({"limit":limit}))
                    .await
                    .is_err()
            );
        }
        let copy = files
            .call(
                "files.copy",
                json!({"source":"exports/a","destination":"copy"}),
            )
            .await
            .unwrap();
        assert_ne!(second["version"], copy["version"]);
        assert_eq!(
            files
                .call("files.readText", json!({"key":"copy"}))
                .await
                .unwrap(),
            "new"
        );
        files
            .call("files.move", json!({"source":"copy","destination":"moved"}))
            .await
            .unwrap();
        assert_eq!(
            files
                .call("files.exists", json!({"key":"copy"}))
                .await
                .unwrap(),
            false
        );
        assert_eq!(
            files
                .call("files.readBytes", json!({"key":"moved"}))
                .await
                .unwrap(),
            json!([110, 101, 119])
        );
        for key in [
            "../records/a",
            "/root",
            "a\\b",
            "a/../b",
            "CON.txt",
            "a:",
            "a%2fb",
            "x/",
            "a.",
            "a\n",
        ] {
            assert_eq!(
                write(&files, key, "x").await.unwrap_err().error_code(),
                "HB_FILE_ACCESS_DENIED"
            );
        }
        assert!(
            files
                .call("files.write", json!({"key":"invalid","bytes":[256]}))
                .await
                .is_err()
        );
        assert!(
            files
                .call(
                    "files.write",
                    json!({"key":"invalid","text":"x","contentType":"text/plain\r\nx-foo: yes"})
                )
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn overwrite_reserves_full_size_and_failed_cleanup_keeps_quota() {
    let db = DbClient::memory().await.unwrap();
    let storage = Arc::new(FaultStorage::new());
    let files = FileService::new(db.clone(), storage.clone(), config(6));
    write(&files, "a", "1234").await.unwrap();
    assert!(matches!(
        write(&files, "a", "456").await,
        Err(HbError::PayloadTooLarge)
    ));
    assert_eq!(
        files
            .call("files.readText", json!({"key":"a"}))
            .await
            .unwrap(),
        "1234"
    );
    storage.fail_delete.store(true, Ordering::SeqCst);
    write(&files, "a", "12").await.unwrap();
    assert_eq!(count(&db).await, 2);
    assert!(matches!(
        write(&files, "b", "1").await,
        Err(HbError::PayloadTooLarge)
    ));
    storage.fail_delete.store(false, Ordering::SeqCst);
    files.reconcile().await.unwrap();
    assert_eq!(count(&db).await, 1);
    write(&files, "b", "1234").await.unwrap();
}

#[tokio::test]
async fn partial_move_preserves_destination_and_reports_confirmed_source() {
    let db = DbClient::memory().await.unwrap();
    let storage = Arc::new(FaultStorage::new());
    let files = FileService::new(db.clone(), storage.clone(), config(20));
    write(&files, "source", "value").await.unwrap();
    storage.fail_delete.store(true, Ordering::SeqCst);
    let error = files
        .call(
            "files.move",
            json!({"source":"source","destination":"dest"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.error_code(), "HB_FILE_MOVE_PARTIAL");
    let HbError::Extension(error) = error else {
        panic!()
    };
    let details = error.details.unwrap();
    assert_eq!(details["destinationExists"], true);
    assert_eq!(details["sourceState"], "present");
    assert_eq!(
        files
            .call("files.readText", json!({"key":"dest"}))
            .await
            .unwrap(),
        "value"
    );
    storage.fail_delete.store(false, Ordering::SeqCst);
    files.reconcile().await.unwrap();
    assert_eq!(count(&db).await, 1);
    assert_eq!(
        files
            .call("files.exists", json!({"key":"source"}))
            .await
            .unwrap(),
        false
    );
}

#[tokio::test]
async fn file_recovery_child() {
    let Ok(directory) = std::env::var("HB_TEST_FILE_RECOVERY_DIRECTORY") else {
        return;
    };
    let mut settings = HbConfig::default();
    settings.paths.data_dir = directory.clone();
    settings.database.engine = "surrealkv".into();
    let mut backend = FaultStorage::new();
    backend.inner = ObjectStoreStorage::local(Path::new(&directory).join("objects")).unwrap();
    let storage = Arc::new(backend);
    let db = DbClient::init(&settings).await.unwrap();
    let files = FileService::new(db.clone(), storage.clone(), config(6));
    storage.fail_put.store(true, Ordering::SeqCst);
    assert!(write(&files, "unknown", "123").await.is_err());
    storage.discard_put.store(true, Ordering::SeqCst);
    assert!(write(&files, "absent", "456").await.is_err());
    assert_eq!(count(&db).await, 2);
    assert!(matches!(
        write(&files, "full", "x").await,
        Err(HbError::PayloadTooLarge)
    ));
    // Simulate losing the process before any Rust or database cleanup runs.
    std::process::exit(0);
}

#[tokio::test]
async fn unknown_write_retains_reservation_and_restart_reconciles_observed_objects() {
    let directory = tempfile::tempdir().unwrap();
    let result = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "file_recovery_child", "--nocapture"])
        .env("HB_TEST_FILE_RECOVERY_DIRECTORY", directory.path())
        .output()
        .await
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut settings = HbConfig::default();
    settings.paths.data_dir = directory.path().to_string_lossy().into_owned();
    settings.database.engine = "surrealkv".into();
    let mut backend = FaultStorage::new();
    backend.inner = ObjectStoreStorage::local(directory.path().join("objects")).unwrap();
    let storage = Arc::new(backend);
    let db = DbClient::init(&settings).await.unwrap();
    let files = FileService::new(db.clone(), storage.clone(), config(6));
    files.reconcile().await.unwrap();
    assert_eq!(count(&db).await, 1); // Missing uncertain PUT stays charged.
    storage.fail_put.store(false, Ordering::SeqCst);
    storage.discard_put.store(false, Ordering::SeqCst);
    write(&files, "new", "123").await.unwrap();
    assert!(matches!(
        write(&files, "full", "x").await,
        Err(HbError::PayloadTooLarge)
    ));
}

#[tokio::test]
async fn parallel_hosts_share_quota_and_same_key_serialization() {
    let db = DbClient::memory().await.unwrap();
    let storage = Arc::new(ObjectStoreStorage::memory());
    let a = FileService::new(db.clone(), storage.clone(), config(8));
    let b = FileService::new(db.clone(), storage, config(8));
    let (one, two) = tokio::join!(write(&a, "a", "12345"), write(&b, "b", "67890"));
    assert_ne!(one.is_ok(), two.is_ok());
    let key = if one.is_ok() { "a" } else { "b" };
    a.call("files.remove", json!({"key":key})).await.unwrap();
    let (one, two) = tokio::join!(write(&a, "same", "1234"), write(&b, "same", "5678"));
    assert!(one.is_ok() && two.is_ok());
    assert_eq!(count(&db).await, 1);
}

#[tokio::test]
async fn file_bridge_checks_authority_and_transaction_before_input() {
    let db = DbClient::memory().await.unwrap();
    let mut settings = HbConfig::default();
    let storage = Arc::new(ObjectStoreStorage::memory());
    let create = |settings| {
        ApiHostFactory::new(
            db.clone(),
            Arc::new(settings),
            Arc::new(herta_mail::DisabledMailer),
            storage.clone(),
        )
        .create(HostContext::system())
    };
    let call = |operation: &str, arguments| HostCall {
        operation: operation.into(),
        arguments,
        auth_mode: AuthMode::System,
        transaction: None,
        remaining_ms: 1000,
        script: "files.js".into(),
    };
    let host = create(settings.clone());
    assert_eq!(
        host.call(call("files.write", Value::Null))
            .await
            .unwrap_err()
            .error_code(),
        "HB_CAPABILITY_DENIED"
    );
    settings.jsvm.files.enabled = true;
    let host = create(settings);
    let receipt = host
        .call(call("transaction.begin", Value::Null))
        .await
        .unwrap();
    assert_eq!(
        host.call(call("files.write", Value::Null))
            .await
            .unwrap_err()
            .error_code(),
        "HB_SIDE_EFFECT_IN_TRANSACTION"
    );
    let mut read = call("files.exists", json!({"key":"file"}));
    read.transaction = receipt["id"].as_str().map(str::to_owned);
    assert_eq!(host.call(read).await.unwrap(), false);
    host.finish(false).await.unwrap();
    host.call(call("files.write", json!({"key":"file","text":"body"})))
        .await
        .unwrap();
    assert!(!host.retry_safe());
    assert_eq!(
        host.call(call("files.readText", json!({"key":"file"})))
            .await
            .unwrap(),
        "body"
    );
}

#[tokio::test]
async fn administrator_resolution_requires_confirmed_absence_and_keeps_an_immutable_audit() {
    use herta_core::files::{FileResolution, FileResolve};
    for engine in ["memory", "surrealkv"] {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = HbConfig::default();
        settings.database.engine = engine.into();
        settings.paths.data_dir = directory.path().to_string_lossy().into_owned();
        let db = DbClient::init(&settings).await.unwrap();
        let storage = Arc::new(FaultStorage::new());
        let files = FileService::new(db.clone(), storage.clone(), config(6));
        let resolve = || FileResolve {
            resolution: FileResolution::NotWritten,
            note: "Backend confirmed write was rejected and no request remains in flight".into(),
        };
        storage.discard_put.store(true, Ordering::SeqCst);
        assert!(write(&files, "absent", "123").await.is_err());
        storage.discard_put.store(false, Ordering::SeqCst);
        storage.fail_put.store(true, Ordering::SeqCst);
        assert!(write(&files, "present", "456").await.is_err());
        let page = files.operations("", 1).await.unwrap();
        assert_eq!(page["items"].as_array().unwrap().len(), 1);
        let first = page["items"][0]["version"].as_str().unwrap();
        assert_eq!(page["nextCursor"], first);
        let next = files.operations(first, 1).await.unwrap();
        let second = next["items"][0]["version"].as_str().unwrap();
        assert_ne!(first, second);
        assert!(next["nextCursor"].is_null());
        assert!(
            files
                .operation(first)
                .await
                .unwrap()
                .get("object")
                .is_none()
        );
        storage.fail_head.store(true, Ordering::SeqCst);
        assert!(
            files
                .resolve(first, resolve(), "admin".into())
                .await
                .is_err()
        );
        storage.fail_head.store(false, Ordering::SeqCst);
        assert!(matches!(
            files.resolve(second, resolve(), "admin".into()).await,
            Err(HbError::Conflict(_))
        ));
        assert_eq!(count(&db).await, 2);
        assert!(matches!(
            write(&files, "full", "x").await,
            Err(HbError::PayloadTooLarge)
        ));
        let receipt = files
            .resolve(first, resolve(), "admin".into())
            .await
            .unwrap();
        assert_eq!(receipt["state"], "released");
        assert_eq!(receipt["resolvedBy"], "admin");
        assert_eq!(count(&db).await, 1);
        let restarted = FileService::new(db.clone(), storage.clone(), config(6));
        assert_eq!(
            restarted
                .resolve(first, resolve(), "other-admin".into())
                .await
                .unwrap(),
            receipt
        );
        assert_eq!(restarted.operation(first).await.unwrap(), receipt);
        storage.fail_put.store(false, Ordering::SeqCst);
        let live = write(&restarted, "new", "123").await.unwrap();
        assert!(matches!(
            restarted
                .resolve(live["version"].as_str().unwrap(), resolve(), "admin".into())
                .await,
            Err(HbError::Conflict(_))
        ));
        assert_eq!(
            restarted.operations("", 500).await.unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        for limit in [0, 501] {
            assert!(files.operations("", limit).await.is_err());
        }
    }
}

#[tokio::test]
async fn file_operation_endpoints_authenticate_before_releasing_a_reservation() {
    use herta_api::{ApiState, build_router};
    use salvo::{
        prelude::*,
        test::{ResponseExt, TestClient},
    };
    let directory = tempfile::tempdir().unwrap();
    let db = DbClient::memory().await.unwrap();
    let mut settings = HbConfig::default();
    settings.paths.data_dir = directory.path().to_string_lossy().into_owned();
    settings.auth.jwt_secret = Some("files-test-signing-secret-at-least-32-bytes".into());
    settings.auth.bootstrap_admin_email = Some("admin@example.com".into());
    settings.auth.bootstrap_admin_password = Some("correct horse battery staple".into());
    settings.jsvm.files = config(6);
    let storage = Arc::new(FaultStorage::new());
    let state = Arc::new(
        ApiState::new_with_storage(db.clone(), settings, storage.clone())
            .await
            .unwrap(),
    );
    let service = Service::new(build_router()).hoop(affix_state::inject(state.clone()));
    let files = FileService::new(db, storage.clone(), config(6));
    storage.discard_put.store(true, Ordering::SeqCst);
    assert!(write(&files, "absent", "123").await.is_err());
    let page = files.operations("", 1).await.unwrap();
    let version = page["items"][0]["version"].as_str().unwrap();
    let root = "http://localhost/api/admin/file-operations";
    for url in [root.to_owned(), format!("{root}/{version}")] {
        assert_eq!(
            TestClient::get(url).send(&service).await.status_code,
            Some(StatusCode::UNAUTHORIZED)
        );
    }
    let input = json!({"resolution":"not_written","note":"Backend confirmed no write will arrive"});
    let url = format!("{root}/{version}/resolve");
    assert_eq!(
        TestClient::post(&url)
            .json(&input)
            .send(&service)
            .await
            .status_code,
        Some(StatusCode::UNAUTHORIZED)
    );
    let mut login = TestClient::post("http://localhost/api/admin/auth/login")
        .json(&json!({"email":"admin@example.com","password":"correct horse battery staple"}))
        .send(&service)
        .await;
    let login: Value = login.take_json().await.unwrap();
    let token = login["data"]["accessToken"].as_str().unwrap();
    let mut response = TestClient::post(url)
        .bearer_auth(token)
        .json(&input)
        .send(&service)
        .await;
    assert_eq!(response.status_code, Some(StatusCode::OK));
    let receipt: Value = response.take_json().await.unwrap();
    assert_eq!(receipt["data"]["state"], "released");
    let mut response = TestClient::get(format!("{root}/{version}"))
        .bearer_auth(token)
        .send(&service)
        .await;
    let stored: Value = response.take_json().await.unwrap();
    assert_eq!(stored, receipt);
    let mut response = TestClient::get(root)
        .bearer_auth(token)
        .send(&service)
        .await;
    let page: Value = response.take_json().await.unwrap();
    assert_eq!(page["data"]["items"], json!([]));
}

async fn prepare_upload_cleanup(db: &DbClient, storage: &FaultStorage, state: &str) {
    use herta_core::extension::RecordUpload;
    use herta_db::{transaction::TransactionOwner, uploads};
    let schema = serde_json::from_value(
        json!({"name":"attachments","type":"base","schema_mode":"strict",
        "fields":[{"name":"file","type":"file"}],"rules":{}}),
    )
    .unwrap();
    let owner = TransactionOwner::new(db.clone(), None);
    let receipt = owner.begin().await.unwrap();
    let uploads = ["new.txt", "discarded.txt"].map(|filename| RecordUpload {
        collection: "attachments".into(),
        record_id: state.into(),
        field: "file".into(),
        filename: filename.into(),
        source: Default::default(),
    });
    uploads::track(
        db,
        &receipt.id,
        &schema,
        state,
        &json!({"file":"old.txt"}),
        &uploads,
    )
    .await
    .unwrap();
    for name in ["old.txt", "new.txt", "discarded.txt"] {
        storage
            .put_bytes(
                &uploads::storage_key("attachments", state, "file", name),
                Bytes::from_static(b"data"),
            )
            .await
            .unwrap();
    }
    let operation = receipt.id.clone();
    let record_id = state.to_owned();
    owner
        .execute(Some(receipt.id.clone()), true, move |session| {
            Box::pin(async move {
                uploads::save_references(
                    session,
                    &operation,
                    &schema,
                    &record_id,
                    &json!({"file":"new.txt"}),
                )
                .await
            })
        })
        .await
        .unwrap();
    if state == "committed" {
        owner.commit(&receipt.id).await.unwrap();
    } else {
        owner.finish().await.unwrap();
    }
    if state == "unknown" {
        // Lost cancel acknowledgement: no marker, but storage must wait for reconciliation.
        db.inner()
            .query("UPDATE type::record('_operations', $id) SET state = 'unknown'")
            .bind(("id", receipt.id))
            .await
            .unwrap()
            .check()
            .unwrap();
    }
}

async fn check_upload_cleanup(db: &DbClient, storage: &FaultStorage, recovered: bool) {
    for state in ["committed", "rolled_back", "unknown"] {
        for name in ["old.txt", "new.txt", "discarded.txt"] {
            let key = herta_db::uploads::storage_key("attachments", state, "file", name);
            let present = if state == "unknown" && !recovered {
                true
            } else if state == "committed" {
                name == "new.txt"
            } else {
                name == "old.txt"
            };
            assert_eq!(storage.head(&key).await.is_ok(), present, "{state}/{name}");
        }
    }
    assert_eq!(
        herta_db::uploads::entries(db, None, 0).await.unwrap().len(),
        if recovered { 0 } else { 3 }
    );
}

#[tokio::test]
async fn upload_compensation_preserves_committed_and_unknown_objects_after_delete_ack_loss() {
    for engine in ["memory", "surrealkv"] {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = HbConfig::default();
        settings.database.engine = engine.into();
        settings.paths.data_dir = directory.path().to_string_lossy().into_owned();
        let db = DbClient::init(&settings).await.unwrap();
        let storage = FaultStorage::new();
        for state in ["committed", "rolled_back", "unknown"] {
            prepare_upload_cleanup(&db, &storage, state).await;
        }
        storage.fail_delete.store(true, Ordering::SeqCst);
        assert!(
            herta_api::extensions::reconcile_uploads(&db, &storage, None)
                .await
                .is_err()
        );
        storage.fail_delete.store(false, Ordering::SeqCst);
        storage.fail_delete_after.store(true, Ordering::SeqCst);
        assert!(
            herta_api::extensions::reconcile_uploads(&db, &storage, None)
                .await
                .is_err()
        );
        storage.fail_delete_after.store(false, Ordering::SeqCst);
        herta_api::extensions::reconcile_uploads(&db, &storage, None)
            .await
            .unwrap();
        check_upload_cleanup(&db, &storage, false).await;
    }
}

#[tokio::test]
async fn record_and_collection_cleanup_child() {
    use herta_db::{SchemaManager, collections::CollectionService};
    let Ok(directory) = std::env::var("HB_TEST_RECORD_CLEANUP_DIRECTORY") else {
        return;
    };
    let mut settings = HbConfig::default();
    settings.database.engine = "surrealkv".into();
    settings.paths.data_dir = directory.clone();
    let db = DbClient::init(&settings).await.unwrap();
    let mut storage = FaultStorage::new();
    storage.inner = ObjectStoreStorage::local(Path::new(&directory).join("objects")).unwrap();
    for state in ["committed", "rolled_back", "unknown"] {
        prepare_upload_cleanup(&db, &storage, state).await;
    }
    let definition =
        json!({"name":"removed","type":"base","schema_mode":"strict","fields":[],"rules":{}});
    SchemaManager::new(&db)
        .create_collection(&serde_json::from_value(definition.clone()).unwrap())
        .await
        .unwrap();
    storage
        .put_bytes("records/removed/one/file/a.txt", Bytes::from_static(b"old"))
        .await
        .unwrap();
    CollectionService::apply(db.clone(), None, "delete".into(), json!("removed"), false)
        .await
        .unwrap();
    let docs = herta_api::docs::OpenApiCache::empty();
    storage.fail_delete.store(true, Ordering::SeqCst);
    assert!(
        herta_api::extensions::reconcile_collections(&db, &storage, &docs)
            .await
            .is_err()
    );
    storage.fail_delete.store(false, Ordering::SeqCst);
    storage.fail_delete_after.store(true, Ordering::SeqCst);
    assert!(
        herta_api::extensions::reconcile_collections(&db, &storage, &docs)
            .await
            .is_err()
    );
    assert!(matches!(
        CollectionService::new(&db)
            .prepare("create", definition, false)
            .await,
        Err(HbError::Conflict(_))
    ));
    assert!(
        herta_api::extensions::reconcile_uploads(&db, &storage, None)
            .await
            .is_err()
    );
    std::process::exit(0);
}

#[tokio::test]
async fn startup_recovers_upload_and_collection_cleanup_after_process_loss() {
    let directory = tempfile::tempdir().unwrap();
    let result = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "record_and_collection_cleanup_child",
            "--nocapture",
        ])
        .env("HB_TEST_RECORD_CLEANUP_DIRECTORY", directory.path())
        .output()
        .await
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut settings = HbConfig::default();
    settings.database.engine = "surrealkv".into();
    settings.paths.data_dir = directory.path().to_string_lossy().into_owned();
    let db = DbClient::init(&settings).await.unwrap();
    let mut storage = FaultStorage::new();
    storage.inner = ObjectStoreStorage::local(directory.path().join("objects")).unwrap();
    let storage = Arc::new(storage);
    let state = herta_api::ApiState::new_with_storage(db.clone(), settings, storage.clone())
        .await
        .unwrap();
    check_upload_cleanup(&db, &storage, true).await;
    assert!(matches!(
        storage.head("records/removed/one/file/a.txt").await,
        Err(HbError::NotFound)
    ));
    assert!(
        state.docs.read().await["paths"]
            .get("/api/collections/removed/records")
            .is_none()
    );
    let definition =
        json!({"name":"removed","type":"base","schema_mode":"strict","fields":[],"rules":{}});
    herta_db::collections::CollectionService::apply(db, None, "create".into(), definition, false)
        .await
        .unwrap();
}
