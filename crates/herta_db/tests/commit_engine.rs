//! Pause the real SurrealDB commit span, without adding a production failpoint.
//! Every probe runs in its own process so tracing and forced termination are isolated.
use futures_util::StreamExt;
use herta_core::HbConfig;
use herta_db::{
    DbClient,
    transaction::{OperationState, TransactionOwner, operation_status},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tracing::{Id, Subscriber, span::Attributes};
use tracing_subscriber::{
    Layer,
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
};

struct Gate {
    owner: OnceLock<TransactionOwner>,
    selected: Mutex<Option<Id>>,
    claimed: AtomicBool,
    after: bool,
    checkpoint: PathBuf,
    entered: tokio::sync::Notify,
    released: Mutex<bool>,
    release: Condvar,
}
impl Gate {
    fn pause(&self, id: &Id) {
        if self.selected.lock().unwrap().as_ref() != Some(id)
            || self.claimed.swap(true, Ordering::SeqCst)
        {
            return;
        }
        std::fs::write(&self.checkpoint, b"inside-surrealdb-core-commit").unwrap();
        self.entered.notify_one();
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.release.wait(released).unwrap();
        }
    }
    fn resume(&self) {
        *self.released.lock().unwrap() = true;
        self.release.notify_all();
    }
}
struct Probe(Arc<Gate>);
impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Probe {
    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, _: Context<'_, S>) {
        let metadata = attributes.metadata();
        if metadata.target() == "surrealdb::core::kvs::tx"
            && metadata.name() == "commit"
            && self
                .0
                .owner
                .get()
                .is_some_and(|owner| owner.interrupted_commit().is_some())
            && !self.0.claimed.load(Ordering::SeqCst)
        {
            *self.0.selected.lock().unwrap() = Some(id.clone());
        }
    }
    fn on_enter(&self, id: &Id, _: Context<'_, S>) {
        if !self.0.after {
            self.0.pause(id);
        }
    }
    fn on_close(&self, id: Id, _: Context<'_, S>) {
        if self.0.after {
            self.0.pause(&id);
        }
    }
}
fn config(path: &Path, engine: &str) -> HbConfig {
    let mut config = HbConfig::default();
    config.database.engine = engine.into();
    config.paths.data_dir = path.to_string_lossy().into_owned();
    config
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn commit_engine_child() {
    let Ok(path) = std::env::var("HB_TEST_COMMIT_ENGINE_DIR") else {
        return;
    };
    let path = PathBuf::from(path);
    let mode = std::env::var("HB_TEST_COMMIT_ENGINE_MODE").unwrap();
    let engine = std::env::var("HB_TEST_COMMIT_ENGINE_BACKEND").unwrap();
    let gate = Arc::new(Gate {
        owner: OnceLock::new(),
        selected: Mutex::new(None),
        claimed: AtomicBool::new(false),
        after: mode == "after",
        checkpoint: path.join("checkpoint"),
        entered: Default::default(),
        released: Mutex::new(false),
        release: Condvar::new(),
    });
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(Probe(gate.clone())),
    )
    .unwrap();
    let db = DbClient::init(&config(&path, &engine)).await.unwrap();
    db.inner()
        .query("DEFINE TABLE engine_probe SCHEMALESS")
        .await
        .unwrap()
        .check()
        .unwrap();
    let mut live = db
        .inner()
        .select::<Vec<Value>>("engine_probe")
        .live()
        .await
        .unwrap();
    let owner = TransactionOwner::new(db.clone(), Some("users:commit-probe".into()));
    let receipt = owner.begin().await.unwrap();
    owner
        .execute(Some(receipt.id.clone()), true, |session| {
            Box::pin(async move {
                session
                    .query(
                        "CREATE engine_probe:one SET value=1; CREATE engine_probe:two SET value=2",
                    )
                    .await
                    .map_err(|error| herta_core::HbError::Database(error.to_string()))?
                    .check()
                    .map_err(|error| herta_core::HbError::Database(error.to_string()))?;
                Ok(())
            })
        })
        .await
        .unwrap();
    std::fs::write(
        path.join("receipt.json"),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    assert!(gate.owner.set(owner.clone()).is_ok());
    let waiter = tokio::spawn({
        let owner = owner.clone();
        let id = receipt.id.clone();
        async move { owner.commit(&id).await }
    });
    tokio::time::timeout(Duration::from_secs(10), gate.entered.notified())
        .await
        .unwrap();
    if mode != "cancel" {
        std::future::pending::<()>().await;
        unreachable!();
    }
    owner.cancel();
    waiter.abort();
    let finish = tokio::spawn({
        let owner = owner.clone();
        async move { owner.finish().await }
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(
        !finish.is_finished(),
        "finish must wait behind the actual database commit"
    );
    gate.resume();
    tokio::time::timeout(Duration::from_secs(5), finish)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        operation_status(
            &db,
            &receipt.operation_id,
            None,
            Some(&receipt.check_credential)
        )
        .await
        .unwrap()
        .state,
        OperationState::Committed
    );
    let mut records = Vec::new();
    for _ in 0..2 {
        records.push(
            tokio::time::timeout(Duration::from_secs(2), live.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .data["value"]
                .as_u64()
                .unwrap(),
        );
    }
    records.sort();
    assert_eq!(records, [1, 2]);
    assert!(
        tokio::time::timeout(Duration::from_millis(30), live.next())
            .await
            .is_err(),
        "LIVE must deliver once per committed record"
    );
    std::fs::write(path.join("completed"), b"ok").unwrap();
}

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn spawn(path: &Path, engine: &str, mode: &str) -> Child {
    let log = std::fs::File::create(path.join("child.log")).unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "commit_engine_child", "--nocapture"])
        .env("HB_TEST_COMMIT_ENGINE_DIR", path)
        .env("HB_TEST_COMMIT_ENGINE_BACKEND", engine)
        .env("HB_TEST_COMMIT_ENGINE_MODE", mode)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Child(command.spawn().unwrap())
}
async fn checkpoint(child: &mut Child, path: &Path, file: &str) {
    let result = tokio::time::timeout(Duration::from_secs(20), async {
        while !path.join(file).exists() {
            if let Some(status) = child.0.try_wait().unwrap() {
                panic!(
                    "child exited {status}: {}",
                    std::fs::read_to_string(path.join("child.log")).unwrap()
                );
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        result.is_ok(),
        "checkpoint {file} timed out: {}",
        std::fs::read_to_string(path.join("child.log")).unwrap()
    );
}

#[tokio::test]
async fn cancellation_inside_engine_commit_preserves_atomicity_and_live_delivery() {
    for engine in ["memory", "surrealkv"] {
        let directory = tempfile::tempdir().unwrap();
        let mut child = spawn(directory.path(), engine, "cancel");
        checkpoint(&mut child, directory.path(), "completed").await;
        let status = child.0.wait().unwrap();
        assert!(
            status.success(),
            "{}",
            std::fs::read_to_string(directory.path().join("child.log")).unwrap()
        );
    }
}

#[tokio::test]
async fn process_loss_at_real_commit_boundaries_is_reconciled_after_surreal_kv_restart() {
    for (mode, expected) in [
        ("before", OperationState::RolledBack),
        ("after", OperationState::Committed),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut child = spawn(directory.path(), "surrealkv", mode);
        checkpoint(&mut child, directory.path(), "checkpoint").await;
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(directory.path().join("receipt.json")).unwrap())
                .unwrap();
        let db = DbClient::init(&config(directory.path(), "surrealkv"))
            .await
            .unwrap();
        let state = operation_status(
            &db,
            receipt["operationId"].as_str().unwrap(),
            None,
            receipt["checkCredential"].as_str(),
        )
        .await
        .unwrap();
        assert_eq!(state.state, expected, "{mode}");
        let mut response = db
            .inner()
            .query("SELECT `value` FROM engine_probe ORDER BY `value`")
            .await
            .unwrap()
            .check()
            .unwrap();
        let records: Vec<Value> = response.take(0).unwrap();
        assert_eq!(
            records,
            if expected == OperationState::Committed {
                vec![json!({"value":1}), json!({"value":2})]
            } else {
                vec![]
            }
        );
    }
}
