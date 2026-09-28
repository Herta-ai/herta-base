use super::*;
use async_trait::async_trait;
use herta_core::{
    HbError,
    extension::{HostCall, HostServices},
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::{Semaphore, mpsc};

struct Host {
    calls: mpsc::UnboundedSender<Value>,
    gate: Arc<Semaphore>,
    attempts: AtomicUsize,
    fail_first: AtomicBool,
    safe: AtomicBool,
}
impl HostFactory for Host {
    fn create(&self, context: HostContext) -> Arc<dyn HostServices> {
        assert_eq!(context.mode, AuthMode::System);
        assert!(context.admin);
        Arc::new(Attempt {
            host: self.calls.clone(),
            gate: self.gate.clone(),
            fail: self.fail_first.load(Ordering::SeqCst)
                && self.attempts.fetch_add(1, Ordering::SeqCst) == 0,
            safe: self.safe.load(Ordering::SeqCst),
        })
    }
}
struct Attempt {
    host: mpsc::UnboundedSender<Value>,
    gate: Arc<Semaphore>,
    fail: bool,
    safe: bool,
}
#[async_trait]
impl HostServices for Attempt {
    async fn call(&self, call: HostCall) -> HbResult<Value> {
        assert_eq!(call.operation, "http.send");
        assert_eq!(call.auth_mode, AuthMode::System);
        self.host
            .send(serde_json::from_str(call.arguments["body"].as_str().unwrap()).unwrap())
            .unwrap();
        self.gate.acquire().await.unwrap().forget();
        if self.fail {
            Err(HbError::Internal)
        } else {
            Ok(json!({"status":200,"ok":true,"headers":{},"body":"{}"}))
        }
    }
    fn retry_safe(&self) -> bool {
        self.safe
    }
    async fn finish(&self, _: bool) -> HbResult<()> {
        Ok(())
    }
}
fn script(version: &str) -> String {
    format!(
        r#"let counter=0; cronAdd('same-name','* * * * * *',async context=>{{
      if(!Object.isFrozen(context) || 'next' in context) throw new Error('mutable cron context');
      await $app.http.send({{url:'https://unused.invalid',method:'POST',body:JSON.stringify({{context,version:'{version}',counter:++counter}})}});
    }},{{retries:1,idempotent:true}});"#
    )
}
async fn fixture() -> (
    tempfile::TempDir,
    Driver,
    Arc<Host>,
    mpsc::UnboundedReceiver<Value>,
) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("main.js"), script("old")).unwrap();
    let config = JsvmConfig {
        enabled: true,
        pool_size: 1,
        ..Default::default()
    };
    let runtime = JsRuntime::load(directory.path(), config.clone())
        .await
        .unwrap();
    let (sender, receiver) = mpsc::unbounded_channel();
    let hosts = Arc::new(Host {
        calls: sender,
        gate: Arc::new(Semaphore::new(0)),
        attempts: AtomicUsize::new(0),
        fail_first: AtomicBool::new(true),
        safe: AtomicBool::new(true),
    });
    let mut driver = Driver::new(runtime, hosts.clone(), config, CancellationToken::new());
    driver.last_second = 0;
    (directory, driver, hosts, receiver)
}
async fn next(receiver: &mut mpsc::UnboundedReceiver<Value>) -> Value {
    timeout(Duration::from_secs(4), receiver.recv())
        .await
        .unwrap()
        .unwrap()
}
async fn drain(driver: &mut Driver) {
    timeout(Duration::from_secs(4), async {
        while let Some(task) = driver.tasks.join_next().await {
            task.unwrap();
        }
    })
    .await
    .unwrap();
}
fn time(second: u32) -> DateTime<Utc> {
    format!("2026-09-28T00:00:{second:02}Z").parse().unwrap()
}

#[tokio::test]
async fn reload_preserves_name_lock_snapshot_and_run_identity_across_retries() {
    let (directory, mut driver, hosts, mut calls) = fixture().await;
    driver.tick(time(0));
    let first = next(&mut calls).await;
    std::fs::write(directory.path().join("main.js"), script("new")).unwrap();
    driver.runtime.reload().await.unwrap();
    driver.tick(time(1));
    assert!(calls.try_recv().is_err());
    hosts.gate.add_permits(1);
    let retry = next(&mut calls).await;
    driver.tick(time(2));
    assert!(calls.try_recv().is_err());
    assert_eq!(first["version"], "old");
    assert_eq!(retry["version"], "old");
    assert_eq!(first["counter"], 1);
    assert_eq!(retry["counter"], 1);
    assert_eq!(first["context"]["runId"], retry["context"]["runId"]);
    assert_eq!(
        first["context"]["scheduledAt"],
        retry["context"]["scheduledAt"]
    );
    assert_ne!(first["context"]["attemptId"], retry["context"]["attemptId"]);
    hosts.gate.add_permits(1);
    drain(&mut driver).await;
    driver.tick(time(3));
    let fresh = next(&mut calls).await;
    assert_eq!(fresh["version"], "new");
    assert_ne!(first["context"]["runId"], fresh["context"]["runId"]);
    hosts.gate.add_permits(1);
    drain(&mut driver).await;
    driver.tick(time(3));
    driver.tick(time(1));
    assert!(
        driver.tasks.is_empty(),
        "clock reversal must not replay old occurrences"
    );
    driver.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_and_uncertain_side_effects_suppress_retries() {
    for stop in [false, true] {
        let (_directory, mut driver, hosts, mut calls) = fixture().await;
        hosts.safe.store(stop, Ordering::SeqCst);
        driver.tick(time(0));
        next(&mut calls).await;
        if stop {
            driver.stop.cancel();
        }
        hosts.gate.add_permits(1);
        drain(&mut driver).await;
        assert!(calls.try_recv().is_err());
        if stop {
            driver.tick(time(1));
            assert!(driver.tasks.is_empty());
        }
        driver.runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn invalid_calendar_and_retry_options_reject_the_candidate_snapshot() {
    let (directory, driver, _hosts, _calls) = fixture().await;
    let initial = driver.runtime.snapshot().hash.clone();
    for source in [
        "cronAdd('bad','* * * * *',()=>{});",
        "cronAdd('bad','* * * * * *',()=>{},{timezone:'not/IANA'});",
        "cronAdd('bad','* * * * * *',()=>{},{retries:1});",
        "cronAdd('bad','* * * * * *',()=>{},{idempotent:true,retries:4});",
        "cronAdd('bad','* * * * * *',()=>{},{maxRuntimeMs:30001});",
        "cronAdd('bad','* * * * * *',()=>{},{unexpected:true});",
    ] {
        std::fs::write(directory.path().join("main.js"), source).unwrap();
        assert!(driver.runtime.reload().await.is_err(), "{source}");
        assert_eq!(driver.runtime.snapshot().hash, initial);
    }
    driver.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn task_budget_replaces_regular_async_budget_and_drain_is_bounded() {
    let (directory, mut driver, hosts, mut calls) = fixture().await;
    std::fs::write(directory.path().join("main.js"), "cronAdd('budget','* * * * * *',async ()=>{await $app.http.send({url:'https://unused.invalid',body:'{}'})},{maxRuntimeMs:40});").unwrap();
    driver.runtime.reload().await.unwrap();
    driver.tick(time(0));
    next(&mut calls).await;
    drain(&mut driver).await;
    // The native operation keeps its owner until it settles; it wasn't retried.
    assert!(calls.try_recv().is_err());
    hosts.gate.add_permits(1);
    driver.runtime.shutdown().await.unwrap();
}
