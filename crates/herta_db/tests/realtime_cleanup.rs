use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use futures_util::future::join_all;
use herta_db::{
    CollectionDef, CollectionType, DbClient, FieldDef, FieldType, RealtimeManager, RecordManager,
    RuleContext, SchemaManager, SchemaMode,
};
use serde_json::json;
use tokio::{
    sync::Barrier,
    time::{Duration, sleep},
};
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone, Default)]
struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLogs {
    type Writer = CapturedWriter;

    fn make_writer(&'a self) -> Self::Writer {
        CapturedWriter(Arc::clone(&self.0))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closing_during_writes_does_not_trigger_surrealdb_kill_warning() {
    let logs = CapturedLogs::default();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_ansi(false)
        .without_time()
        .with_writer(logs.clone())
        .finish()
        .init();

    let db = DbClient::memory().await.unwrap();
    SchemaManager::new(&db)
        .create_collection(&CollectionDef {
            name: "posts".into(),
            collection_type: CollectionType::Base,
            schema_mode: SchemaMode::Strict,
            fields: vec![FieldDef {
                name: "title".into(),
                field_type: FieldType::Text,
                required: true,
                options: None,
            }],
            indexes: vec![],
            rules: Default::default(),
        })
        .await
        .unwrap();

    const SUBSCRIPTIONS: usize = 12;
    let context = RuleContext {
        admin: true,
        ..Default::default()
    };
    let mut subscriptions = Vec::with_capacity(SUBSCRIPTIONS);
    for _ in 0..SUBSCRIPTIONS {
        subscriptions.push(
            RealtimeManager::new(&db)
                .subscribe("posts", None, &context)
                .await
                .unwrap(),
        );
    }

    let barrier = Arc::new(Barrier::new(SUBSCRIPTIONS + 1));
    let close_tasks = subscriptions.into_iter().map(|mut subscription| {
        let barrier = Arc::clone(&barrier);
        tokio::spawn(async move {
            barrier.wait().await;
            subscription.close().await.unwrap();
        })
    });
    let writer = {
        let barrier = Arc::clone(&barrier);
        let db = db.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            for index in 0..32 {
                RecordManager::new(&db)
                    .create("posts", json!({"title": format!("Post {index}")}))
                    .await
                    .unwrap();
                tokio::task::yield_now().await;
            }
        })
    };

    for result in join_all(close_tasks).await {
        result.unwrap();
    }
    writer.await.unwrap();
    sleep(Duration::from_millis(100)).await;

    let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    for warning in [
        "Failed to kill live query",
        "Failed to find live query",
        "Failed to find session",
    ] {
        assert!(
            !output.contains(warning),
            "SurrealDB emitted a live-query cleanup warning:\n{output}"
        );
    }
}
