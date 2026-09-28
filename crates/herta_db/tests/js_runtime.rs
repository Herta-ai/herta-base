use async_trait::async_trait;
use herta_core::{
    HbConfig, HbResult,
    extension::{AuthMode, HostCall, HostServices, Invocation},
};
use herta_db::{
    CollectionDef, DbClient, RecordManager, RuleContext, SchemaManager, commands::DatabaseCommands,
};
use herta_jsvm::JsRuntime;
use serde_json::{Value, json};
use std::sync::Arc;

struct Host(DatabaseCommands);
#[async_trait]
impl HostServices for Host {
    async fn call(&self, call: HostCall) -> HbResult<Value> {
        self.0.call(call).await
    }
    fn cancel(&self) {
        self.0.owner.cancel();
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        self.0.owner.finish().await
    }
}

async fn exercise(engine: &str) {
    let directory = tempfile::tempdir().unwrap();
    let mut config = HbConfig::default();
    config.database.engine = engine.into();
    config.paths.data_dir = directory.path().to_string_lossy().into_owned();
    config.jsvm.enabled = true;
    config.jsvm.pool_size = 1;
    let db = DbClient::init(&config).await.unwrap();
    for schema in [
        json!({"name":"posts","type":"base","schema_mode":"strict",
        "fields":[{"name":"title","type":"text","required":true},{"name":"slug","type":"text","required":true}],
        "rules":{"list":true,"view":true,"create":"$record.title = 'hook' AND $request.body.title = 'input'","update":true}}),
        json!({"name":"audit","type":"base","schema_mode":"strict",
            "fields":[{"name":"message","type":"text","required":true}],
            "rules":{"list":true,"view":true,"create":true}}),
    ] {
        let schema: CollectionDef = serde_json::from_value(schema).unwrap();
        SchemaManager::new(&db)
            .create_collection(&schema)
            .await
            .unwrap();
    }
    let script = directory.path().join("main.js");
    std::fs::write(&script, r#"
      onRecordCreate(async e=>{
        await Promise.resolve();
        e.record.set('title','hook');e.record.set('slug','filled');
        await $app.save($app.newRecord('audit',{message:'inside'}));
        e.afterCommit(async committed=>{
          await $app.save($app.newRecord('audit',{message:'committed:'+committed.record.get('slug')}));
        });
        await e.next();
      },'posts');
    "#).unwrap();
    let runtime = JsRuntime::load(directory.path(), config.jsvm)
        .await
        .unwrap();
    let host = || {
        Arc::new(Host(DatabaseCommands::new(
            db.clone(),
            RuleContext {
                request_body: json!({"title":"input"}),
                ..Default::default()
            },
            AuthMode::Request,
            None,
            true,
        )))
    };
    let invocation = || Invocation {
        name: "record.create".into(),
        payload: json!({"collection":"posts",
        "input":{"title":"input"},"id":uuid::Uuid::now_v7().to_string(),"requestBody":{"title":"input"}}),
        auth_mode: AuthMode::Request,
        request_id: Some("integration".into()),
        registration: None,
    };
    let created = runtime.invoke(invocation(), host(), None).await.unwrap();
    assert_eq!(created["title"], "hook");
    assert_eq!(created["slug"], "filled");
    let records = RecordManager::new(&db);
    assert_eq!(
        records
            .list("posts", &Default::default())
            .await
            .unwrap()
            .0
            .len(),
        1
    );
    let (audit, _) = records.list("audit", &Default::default()).await.unwrap();
    assert_eq!(audit.len(), 2);
    assert!(audit.iter().any(|row| row["message"] == "committed:filled"));

    // A hook catches an inner required-field failure and then writes the parent.
    // Rust's owner still refuses to commit any portion of the invocation.
    std::fs::write(
        &script,
        r#"
      onRecordCreate(async e=>{
        try {await $app.save($app.newRecord('audit',{}))} catch {}
        e.record.set('title','hook');e.record.set('slug','filled');
        await e.next();
      },'posts');
    "#,
    )
    .unwrap();
    runtime.reload().await.unwrap();
    let error = runtime
        .invoke(invocation(), host(), None)
        .await
        .unwrap_err();
    assert_eq!(error.error_code(), "HB_HOOK_ABORTED");
    assert_eq!(
        records
            .list("posts", &Default::default())
            .await
            .unwrap()
            .0
            .len(),
        1
    );
    assert_eq!(
        records
            .list("audit", &Default::default())
            .await
            .unwrap()
            .0
            .len(),
        2
    );

    // Required unset is validated against the complete candidate, and request
    // mode does not gain authority from the default system-mode record hook.
    std::fs::write(&script, r#"
      routerAdd('GET','/api/custom',async e=>{
        const records=await $app.findRecordsByFilter('posts','slug = $slug','title',1,0,{slug:'filled'});
        records[0].unset('slug');
        await $app.save(records[0]);return e.noContent();
      });
    "#).unwrap();
    runtime.reload().await.unwrap();
    let error = runtime
        .invoke(
            Invocation {
                name: "route".into(),
                payload: json!({}),
                auth_mode: AuthMode::Request,
                request_id: None,
                registration: Some(0),
            },
            host(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(error.error_code(), "HB_VALIDATION_ERROR");
    assert_eq!(
        records
            .get("posts", created["id"].as_str().unwrap(), None)
            .await
            .unwrap()["slug"],
        "filled"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn memory_js_hooks_share_transactions_and_preserve_rules() {
    exercise("memory").await;
}

#[tokio::test]
async fn surrealkv_js_hooks_share_transactions_and_preserve_rules() {
    exercise("surrealkv").await;
}
