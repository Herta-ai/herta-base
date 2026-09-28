use async_trait::async_trait;
use herta_core::{
    HbResult, JsvmConfig,
    extension::{AuthMode, EventDispatcher, HostCall, HostServices, Invocation, UnavailableHost},
    host_buffer::{HostBudget, HostBuffer, HostReply},
};
use herta_jsvm::JsRuntime;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

async fn load(script: &str) -> (tempfile::TempDir, Arc<JsRuntime>) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.js"), script).unwrap();
    let config = JsvmConfig {
        enabled: true,
        pool_size: 1,
        ..Default::default()
    };
    let runtime = JsRuntime::load(dir.path(), config).await.unwrap();
    (dir, runtime)
}
fn route() -> Invocation {
    Invocation {
        name: "route".into(),
        payload: json!({"request":{"method":"GET","path":"/api/custom"}}),
        auth_mode: AuthMode::Request,
        request_id: Some("test".into()),
        registration: Some(0),
    }
}

#[derive(Default)]
struct BinaryGate {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    budget: Mutex<Option<HostBudget>>,
}
#[async_trait]
impl HostServices for BinaryGate {
    async fn call(&self, _: HostCall) -> HbResult<Value> {
        panic!("binary call entered the JSON-only adapter")
    }
    async fn call_binary(
        &self,
        _: HostCall,
        bytes: Option<HostBuffer>,
        budget: HostBudget,
    ) -> HbResult<HostReply> {
        *self.budget.lock().unwrap() = Some(budget);
        self.entered.notify_one();
        self.release.notified().await;
        assert_eq!(bytes.as_ref().unwrap().as_ref().len(), 4096);
        Ok(Value::Null.into())
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn binary_buffers_stay_charged_through_cancelled_native_io() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("main.js"),
        r#"
      routerAdd('GET','/api/custom',async e=>{
        await $app.files.write('buffer',new Uint8Array(4096)); return e.noContent();
      });
    "#,
    )
    .unwrap();
    let runtime = JsRuntime::load(
        directory.path(),
        JsvmConfig {
            enabled: true,
            pool_size: 1,
            max_bridge_bytes: 8192,
            max_host_buffer_bytes: 8192,
            shutdown_timeout_ms: 30,
            execution_timeout_ms: 1000,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let host = Arc::new(BinaryGate::default());
    let result = runtime.invoke(route(), host.clone(), Some(100)).await;
    assert_eq!(result.unwrap_err().error_code(), "HB_HOOK_TIMEOUT");
    let budget = host
        .budget
        .lock()
        .unwrap()
        .clone()
        .expect("host accepted binary command");
    assert!(
        budget.reserve(8192).is_err(),
        "cancelled JS must not release native bytes"
    );
    host.release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), async {
        while budget.reserve(8192).is_err() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    runtime.shutdown().await.unwrap();
}

struct BinarySource;
struct LegacyFile;
#[async_trait]
impl HostServices for LegacyFile {
    async fn call(&self, call: HostCall) -> HbResult<Value> {
        assert_eq!(call.operation, "files.response");
        Ok(json!({"bytes":[0,255,128],"headers":{"content-type":"application/octet-stream"}}))
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        Ok(())
    }
}
#[tokio::test]
async fn json_only_hosts_and_dispatchers_keep_their_file_response_compatibility() {
    let (_directory, runtime) = load("routerAdd('GET','/api/custom',e=>e.file('legacy'));").await;
    let value = runtime
        .invoke(route(), Arc::new(LegacyFile), None)
        .await
        .unwrap();
    assert_eq!(value["kind"], "bytes");
    assert_eq!(value["body"], json!([0, 255, 128]));
    runtime.shutdown().await.unwrap();
}
#[async_trait]
impl HostServices for BinarySource {
    async fn call(&self, _: HostCall) -> HbResult<Value> {
        panic!("binary result entered JSON-only adapter")
    }
    async fn call_binary(
        &self,
        _: HostCall,
        _: Option<HostBuffer>,
        budget: HostBudget,
    ) -> HbResult<HostReply> {
        Ok(HostReply {
            value: Value::Null,
            bytes: Some(budget.allocate(1024 * 1024)?),
        })
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        Ok(())
    }
}
#[tokio::test]
async fn retained_binary_results_are_counted_by_the_javascript_heap_limit() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("main.js"),
        r#"
      routerAdd('GET','/api/custom',async e=>{
        const blocks=[]; for(;;)blocks.push(await $app.files.readBytes('buffer'));
      });
    "#,
    )
    .unwrap();
    let runtime = JsRuntime::load(
        directory.path(),
        JsvmConfig {
            enabled: true,
            pool_size: 1,
            execution_timeout_ms: 2000,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let result = runtime.invoke(route(), Arc::new(BinarySource), None).await;
    assert_eq!(result.unwrap_err().error_code(), "HB_HOOK_OOM");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn memory_stack_and_abandoned_promises_are_bounded() {
    for (script, code) in [
        (
            "routerAdd('GET','/api/custom',()=>{const blocks=[];for(;;) blocks.push(new Uint8Array(1024*1024));});",
            "HB_HOOK_OOM",
        ),
        (
            "routerAdd('GET','/api/custom',()=>{function recurse(){return 1+recurse()}return recurse()});",
            "HB_HOOK_ERROR",
        ),
        (
            "routerAdd('GET','/api/custom',e=>{Promise.reject(new Error('unhandled'));return e.noContent()});",
            "HB_HOOK_ERROR",
        ),
        (
            "routerAdd('GET','/api/custom',e=>{(async()=>{for(;;)await Promise.resolve()})();return e.noContent()});",
            "HB_HOOK_TIMEOUT",
        ),
    ] {
        let (_dir, runtime) = load(script).await;
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            runtime.invoke(route(), Arc::new(UnavailableHost), None),
        )
        .await
        .unwrap();
        assert_eq!(result.unwrap_err().error_code(), code, "{script}");
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn replay_is_checked_before_host_io_even_with_empty_manifest() {
    let (_dir, runtime) = load("routerAdd('GET','/api/custom',async e=>{await $app.findRecordsByFilter('posts');return e.noContent()});").await;
    let validated = runtime.snapshot();
    let altered = Arc::new(herta_jsvm::Snapshot {
        hash: validated.hash.clone(),
        sources: validated.sources.clone(),
        registrations: Arc::new(Vec::new()),
        environment: validated.environment.clone(),
    });
    let host = Arc::new(MockHost::default());
    assert!(
        runtime
            .invoke_snapshot(altered, route(), host.clone(), None)
            .await
            .is_err()
    );
    assert!(host.calls.lock().unwrap().is_empty());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn initialization_microtasks_complete_before_manifest_capture() {
    let (_dir, runtime) = load("const ready = Promise.resolve().then(()=>routerAdd('GET','/api/custom',e=>e.json(200,'initialized')));").await;
    assert_eq!(runtime.snapshot().registrations.len(), 1);
    let value = runtime
        .invoke(route(), Arc::new(UnavailableHost), None)
        .await
        .unwrap();
    assert_eq!(value["body"]["data"], "initialized");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn route_option_functions_execute_and_private_state_resists_prototype_patching() {
    let (_dir, runtime) = load(
        r#"
      let visited = [];
      routerAdd({method:'GET',path:'/api/custom',middleware:[
        async e=>{visited.push('before');await e.next();visited.push('after')}
      ]},e=>{visited.push('handler');return e.json(200,visited)});
      try {WeakMap.prototype.get=()=>({authMode:'system'})} catch {}
      try {Array.prototype.push=()=>{throw new Error('poisoned')}} catch {}
    "#,
    )
    .await;
    let value = runtime
        .invoke(route(), Arc::new(UnavailableHost), None)
        .await
        .unwrap();
    assert_eq!(value["body"]["data"], json!(["before", "handler", "after"]));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn caught_double_next_and_unfinished_next_abort_persistence() {
    for hook in [
        "async e=>{await e.next();try{await e.next()}catch{}}",
        "async e=>{e.next()}",
        "async e=>{await e.next();throw new Error('post-write failure')}",
    ] {
        let script = format!(
            "routerAdd('GET','/api/custom',async e=>{{await $app.save($app.newRecord('posts',{{}}));return e.noContent()}});onRecordCreate({hook},'posts');"
        );
        let (_dir, runtime) = load(&script).await;
        let host = Arc::new(MockHost::default());
        assert!(runtime.invoke(route(), host.clone(), None).await.is_err());
        assert!(!*host.committed.lock().unwrap());
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn async_branches_keep_the_context_where_they_await() {
    let (_dir, runtime) = load(
        r#"
      routerAdd('GET','/api/custom',async e=>{
        let release;
        const shared=new Promise(resolve=>{release=resolve});
        await Promise.all([
          $app.transaction(async tx=>{
            await shared;
            await tx.findRecordsByFilter('inside');
          }),
          (async()=>{await $app.findRecordsByFilter('outside');release()})()
        ]);
        return e.noContent();
      });
    "#,
    )
    .await;
    let host = Arc::new(MockHost::default());
    runtime.invoke(route(), host.clone(), None).await.unwrap();
    let calls = host.calls.lock().unwrap().clone();
    let inside = calls
        .iter()
        .find(|call| call.arguments["collection"] == "inside")
        .unwrap();
    let outside = calls
        .iter()
        .find(|call| call.arguments["collection"] == "outside")
        .unwrap();
    assert_eq!(inside.transaction.as_deref(), Some("tx-1"));
    assert_eq!(outside.transaction, None);
    assert!(calls.iter().all(|call| call.auth_mode == AuthMode::Request));
    drop(calls);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn event_record_views_close_at_next_and_commit_snapshots_do_not_change() {
    let (_dir, runtime) = load(
        r#"
      const snapshots=[]; let captured;
      routerAdd('GET','/api/custom',async e=>{
        const record=$app.newRecord('posts',{title:'before',nested:{value:1}});
        await $app.transaction(async tx=>{
          await tx.save(record);
          let readonly=false;try{captured.record.set('title','bad')}catch{readonly=true}
          if(!readonly)throw new Error('event reference became writable');
          record.set('title','later');
          tx.afterCommit(()=>{snapshots.push('tx')});
        });
        return e.json(200,snapshots);
      });
      onRecordCreate(async e=>{
        captured=e;
        const nested=e.record.get('nested');nested.value=999;
        if(e.record.get('nested').value!==1)throw new Error('nested value alias');
        e.record.set('title','committed');
        e.afterCommit(committed=>{
          snapshots.push(committed.record.get('title'));
          let readonly=false;try{committed.record.set('title','bad')}catch{readonly=true}
          if(!readonly)throw new Error('commit snapshot writable');
        });
        await e.next();
        let readonly=false;try{e.record.set('title','bad')}catch{readonly=true}
        if(!readonly)throw new Error('record writable after next');
      },'posts');
    "#,
    )
    .await;
    let value = runtime
        .invoke(route(), Arc::new(MockHost::default()), None)
        .await
        .unwrap();
    assert_eq!(value["body"]["data"], json!(["committed", "tx"]));
    runtime.shutdown().await.unwrap();
}

#[derive(Default)]
struct HangingHost {
    started: tokio::sync::Notify,
    cancelled: std::sync::atomic::AtomicBool,
    finished: std::sync::atomic::AtomicBool,
}

struct GatedHost {
    entered: tokio::sync::Semaphore,
    release: tokio::sync::Semaphore,
    calls: std::sync::atomic::AtomicUsize,
}
impl Default for GatedHost {
    fn default() -> Self {
        Self {
            entered: tokio::sync::Semaphore::new(0),
            release: tokio::sync::Semaphore::new(0),
            calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}
#[async_trait]
impl HostServices for GatedHost {
    async fn call(&self, _: HostCall) -> HbResult<Value> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.entered.add_permits(1);
        self.release.acquire().await.unwrap().forget();
        Ok(json!({"status":200,"body":"ok","headers":{}}))
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn host_channel_bounds_accepted_commands_and_keeps_cancelled_io_charged() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.js"), r#"
        routerAdd('GET','/api/custom', async e => {
            const results = await Promise.allSettled(Array.from({length:8}, () => $app.http.send({})));
            return e.json(200, results.map(r => r.status === 'fulfilled' ? 'ok' : r.reason.code));
        });
    "#).unwrap();
    let config = JsvmConfig {
        enabled: true,
        pool_size: 1,
        max_pending_host_calls: 2,
        shutdown_timeout_ms: 30,
        execution_timeout_ms: 1000,
        ..Default::default()
    };
    let runtime = JsRuntime::load(dir.path(), config).await.unwrap();
    let host = Arc::new(GatedHost::default());
    let invocation = tokio::spawn({
        let runtime = runtime.clone();
        let host = host.clone();
        async move { runtime.invoke(route(), host, None).await }
    });
    tokio::time::timeout(Duration::from_secs(2), host.entered.acquire_many(2))
        .await
        .unwrap()
        .unwrap()
        .forget();
    // Keep both accepted commands pending until all other calls have hit the channel limit.
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(host.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    host.release.add_permits(2);
    let output = invocation.await.unwrap().unwrap();
    let results = output["body"]["data"].as_array().unwrap();
    assert_eq!(results.iter().filter(|r| **r == "ok").count(), 2);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == "HB_PAYLOAD_TOO_LARGE")
            .count(),
        6
    );

    let blocked = Arc::new(GatedHost::default());
    assert_eq!(
        runtime
            .invoke(route(), blocked.clone(), Some(60))
            .await
            .unwrap_err()
            .error_code(),
        "HB_HOOK_TIMEOUT"
    );
    assert_eq!(
        runtime
            .invoke(route(), Arc::new(UnavailableHost), None)
            .await
            .unwrap_err()
            .error_code(),
        "HB_JS_BUSY"
    );
    blocked.release.add_permits(2);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let result = runtime
            .invoke(route(), Arc::new(UnavailableHost), None)
            .await;
        if result.is_ok() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    runtime.shutdown().await.unwrap();
}
#[async_trait]
impl HostServices for HangingHost {
    async fn call(&self, _: HostCall) -> HbResult<Value> {
        self.started.notify_one();
        while !self.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        Ok(Value::Null)
    }
    fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        self.finished
            .store(true, std::sync::atomic::Ordering::Release);
        Ok(())
    }
}

#[tokio::test]
async fn queue_deadlines_and_shutdown_cancel_inflight_host_io() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.js"),
        "routerAdd('GET','/api/custom',async e=>{await $app.http.send({});return e.noContent()});",
    )
    .unwrap();
    let config = JsvmConfig {
        enabled: true,
        pool_size: 1,
        queue_capacity: 1,
        queue_timeout_ms: 30,
        shutdown_timeout_ms: 30,
        ..Default::default()
    };
    let runtime = JsRuntime::load(dir.path(), config).await.unwrap();
    let host = Arc::new(HangingHost::default());
    let invocation = tokio::spawn({
        let runtime = runtime.clone();
        let host = host.clone();
        async move { runtime.invoke(route(), host, None).await }
    });
    host.started.notified().await;
    let queued = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            runtime
                .invoke(route(), Arc::new(UnavailableHost), None)
                .await
        }
    });
    tokio::task::yield_now().await;
    let rejected = runtime
        .invoke(route(), Arc::new(UnavailableHost), None)
        .await
        .unwrap_err();
    assert_eq!(rejected.error_code(), "HB_JS_BUSY");
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(100), queued)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err()
            .error_code(),
        "HB_JS_BUSY"
    );
    assert!(runtime.shutdown().await.is_err());
    assert!(
        tokio::time::timeout(Duration::from_millis(300), invocation)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(host.cancelled.load(std::sync::atomic::Ordering::Acquire));
    assert!(host.finished.load(std::sync::atomic::Ordering::Acquire));
    assert_eq!(
        runtime
            .invoke(route(), Arc::new(UnavailableHost), None)
            .await
            .unwrap_err()
            .error_code(),
        "HB_JS_BUSY"
    );
}

#[tokio::test]
async fn each_call_rebuilds_closures_without_sharing_global_state() {
    let (_dir, runtime) =
        load("let count=0; routerAdd('GET','/api/custom',e=>e.json(200,{count:++count}));").await;
    for _ in 0..3 {
        let value = runtime
            .invoke(route(), Arc::new(UnavailableHost), None)
            .await
            .unwrap();
        assert_eq!(value["body"]["data"]["count"], 1);
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_reload_preserves_old_registration_snapshot() {
    let (dir, runtime) = load("routerAdd('GET','/api/custom',e=>e.json(200,'old'));").await;
    let old = runtime.snapshot();
    let pinned = runtime.clone().pin();
    std::fs::write(
        dir.path().join("main.js"),
        "routerAdd('GET','/api/new',e=>e.json(200,'new')); throw new Error('bad');",
    )
    .unwrap();
    assert!(runtime.reload().await.is_err());
    assert_eq!(runtime.snapshot().hash, old.hash);
    assert_eq!(
        runtime
            .invoke(route(), Arc::new(UnavailableHost), None)
            .await
            .unwrap()["body"]["data"],
        "old"
    );
    std::fs::write(
        dir.path().join("main.js"),
        "routerAdd('GET','/api/custom',e=>e.json(200,'new'));",
    )
    .unwrap();
    runtime.reload().await.unwrap();
    assert_eq!(
        pinned
            .dispatch(route(), Arc::new(UnavailableHost))
            .await
            .unwrap()["body"]["data"],
        "old"
    );
    assert_eq!(
        runtime
            .invoke_snapshot(old, route(), Arc::new(UnavailableHost), None)
            .await
            .unwrap()["body"]["data"],
        "old"
    );
    assert_eq!(
        runtime
            .invoke(route(), Arc::new(UnavailableHost), None)
            .await
            .unwrap()["body"]["data"],
        "new"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn sync_and_promise_loops_are_interruptible_and_worker_recovers() {
    for script in [
        "routerAdd('GET','/api/custom',()=>{while(true){}});",
        "routerAdd('GET','/api/custom',async()=>{for(;;) await Promise.resolve();});",
        // This throws from a queued async reaction, exercising AsyncJobException
        // ownership as well as the native interrupt and context destruction.
        "routerAdd('GET','/api/custom',async()=>{await Promise.resolve();while(true){}});",
    ] {
        let (dir, runtime) = load(script).await;
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            runtime.invoke(route(), Arc::new(UnavailableHost), None),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert_eq!(error.error_code(), "HB_HOOK_TIMEOUT");
        std::fs::write(
            dir.path().join("main.js"),
            "routerAdd('GET','/api/custom',e=>e.json(200,'recovered'));",
        )
        .unwrap();
        runtime.reload().await.unwrap();
        assert_eq!(
            runtime
                .invoke(route(), Arc::new(UnavailableHost), None)
                .await
                .unwrap()["body"]["data"],
            "recovered"
        );
        runtime.shutdown().await.unwrap();
    }
}

#[derive(Default)]
struct MockHost {
    calls: Mutex<Vec<HostCall>>,
    committed: Mutex<bool>,
}
#[async_trait]
impl HostServices for MockHost {
    async fn call(&self, call: HostCall) -> HbResult<Value> {
        self.calls.lock().unwrap().push(call.clone());
        match call.operation.as_str() {
            "transaction.begin" => Ok(json!({"id":"tx-1"})),
            "transaction.commit" => {
                *self.committed.lock().unwrap() = true;
                Ok(Value::Null)
            }
            "transaction.rollback" | "transaction.markRollbackOnly" => Ok(Value::Null),
            "record.prepare" => Ok(
                json!({"candidate":call.arguments["candidate"],"original":null,"collection":{"name":call.arguments["collection"]}}),
            ),
            "record.write" => Ok(call.arguments["candidate"].clone()),
            "http.send" => {
                tokio::time::sleep(Duration::from_millis(180)).await;
                Ok(json!({"status":200,"body":"{}"}))
            }
            "record.list" => Ok(json!([])),
            _ => panic!("unexpected call {}", call.operation),
        }
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn async_io_wait_does_not_consume_javascript_active_budget() {
    let (_dir,runtime)=load("routerAdd('GET','/api/custom',async e=>{await $app.http.send({url:'https://example.com'});return e.json(200,'ok')});").await;
    let host = Arc::new(MockHost::default());
    assert_eq!(
        runtime.invoke(route(), host, None).await.unwrap()["body"]["data"],
        "ok"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn nested_save_uses_one_worker_and_inherits_request_identity_after_await() {
    let (_dir, runtime) = load(
        r#"
        routerAdd('GET','/api/custom',async e=>{
          const record=$app.newRecord('posts',{title:'hello'});
          await $app.save(record);
          return e.json(200,record);
        });
        onRecordCreate(async e=>{
          await Promise.resolve();
          const nested=$app.newRecord('audit',{message:'created'});
          await $app.save(nested);
          await e.next();
        }, 'posts');
    "#,
    )
    .await;
    let host = Arc::new(MockHost::default());
    let value = runtime.invoke(route(), host.clone(), None).await.unwrap();
    assert_eq!(value["body"]["data"]["title"], "hello");
    let calls = host.calls.lock().unwrap().clone();
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.operation == "transaction.begin")
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.operation == "record.write")
            .count(),
        2
    );
    assert!(calls.iter().all(|call| call.auth_mode == AuthMode::Request));
    assert!(
        calls
            .iter()
            .filter(|call| call.operation.starts_with("record."))
            .all(|call| call.transaction.as_deref() == Some("tx-1"))
    );
    drop(calls);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn persistence_hook_abort_prevents_commit() {
    let (_dir,runtime)=load(r#"
      routerAdd('GET','/api/custom',async e=>{await $app.save($app.newRecord('posts',{}));return e.json(200,'ok')});
      onRecordCreate(async e=>{},'posts');
    "#).await;
    let host = Arc::new(MockHost::default());
    assert_eq!(
        runtime
            .invoke(route(), host.clone(), None)
            .await
            .unwrap_err()
            .error_code(),
        "HB_HOOK_ABORTED"
    );
    assert!(!*host.committed.lock().unwrap());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn overlapping_routes_and_top_level_io_fail_loading() {
    for script in [
        "routerAdd('GET','/api/x/{id}',()=>{});routerAdd('GET','/api/x/latest',()=>{});",
        "routerAdd('GET','/api/{name}/{id}',()=>{});",
        "$app.findRecordsByFilter('posts');",
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("main.js"), script).unwrap();
        let config = JsvmConfig {
            enabled: true,
            ..Default::default()
        };
        assert!(JsRuntime::load(directory.path(), config).await.is_err());
    }
}

struct SlowFinish {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    done: tokio::sync::Notify,
}
#[async_trait]
impl HostServices for SlowFinish {
    async fn call(&self, _: HostCall) -> HbResult<Value> {
        Ok(Value::Null)
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        self.entered.notify_one();
        self.release.notified().await;
        self.done.notify_one();
        Ok(())
    }
}
#[tokio::test]
async fn stuck_settlement_is_bounded_retains_capacity_and_does_not_cancel_native_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("main.js"),
        "routerAdd('GET','/api/custom',e=>e.noContent());",
    )
    .unwrap();
    let config = JsvmConfig {
        enabled: true,
        pool_size: 1,
        shutdown_timeout_ms: 40,
        ..Default::default()
    };
    let runtime = JsRuntime::load(directory.path(), config).await.unwrap();
    let host = Arc::new(SlowFinish {
        entered: Default::default(),
        release: Default::default(),
        done: Default::default(),
    });
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        runtime.invoke(route(), host.clone(), None),
    )
    .await
    .unwrap();
    assert_eq!(result.unwrap_err().error_code(), "HB_HOOK_TIMEOUT");
    assert_eq!(
        runtime
            .invoke(route(), Arc::new(UnavailableHost), None)
            .await
            .unwrap_err()
            .error_code(),
        "HB_JS_BUSY"
    );
    host.release.notify_one();
    host.done.notified().await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if runtime
                .invoke(route(), Arc::new(UnavailableHost), None)
                .await
                .is_ok()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    runtime.shutdown().await.unwrap();
}
