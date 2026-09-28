use herta_api::{ApiState, build_router, extensions::ApiHostFactory};
use herta_core::{
    HbConfig, HbError, HbResult, MailMessage, MailReceipt, MailStatus, Mailer,
    extension::{AuthMode, Extensions, HostCall, HostContext, HostFactory},
};
use herta_db::{CollectionDef, DbClient, RecordManager, SchemaManager};
use herta_jsvm::JsRuntime;
use herta_storage::ObjectStoreStorage;
use salvo::{
    prelude::*,
    test::{ResponseExt, TestClient},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Mailbox(Mutex<Vec<MailMessage>>);
#[async_trait]
impl Mailer for Mailbox {
    async fn send(&self, message: MailMessage) -> HbResult<MailReceipt> {
        self.0.lock().unwrap().push(message);
        Ok(MailReceipt {
            message_id: "<extension@example.test>".into(),
            status: MailStatus::Accepted,
        })
    }
}

async fn setup(source: &str) -> (tempfile::TempDir, Arc<JsRuntime>, Arc<ApiState>, Service) {
    let directory = tempfile::tempdir().unwrap();
    let mut config = HbConfig::default();
    config.paths.data_dir = directory.path().join("data").to_string_lossy().into_owned();
    config.database.engine = "memory".into();
    config.auth.jwt_secret = Some("extension-test-signing-secret-at-least-32-bytes".into());
    config.auth.bootstrap_admin_email = Some("admin@example.com".into());
    config.auth.bootstrap_admin_password = Some("correct horse battery staple".into());
    config.jsvm.enabled = true;
    config.jsvm.pool_size = 1;
    config.jsvm.files.enabled = true;
    config.jsvm.outbox.enabled = true;
    config.jsvm.mail.enabled = true;
    config.mail.driver = "smtp".into();
    config.jsvm.execution_timeout_ms = 250;
    std::fs::write(directory.path().join("main.js"), source).unwrap();
    let runtime = JsRuntime::load(directory.path(), config.jsvm.clone())
        .await
        .unwrap();
    let db = DbClient::memory().await.unwrap();
    for schema in [
        json!({"name":"posts","type":"base","schema_mode":"strict","fields":[
            {"name":"title","type":"text","required":true},{"name":"slug","type":"text","required":true}],
            "rules":{"list":true,"view":true,"create":"$request.body.title = 'input'","update":true,"delete":true}}),
        json!({"name":"audit","type":"base","schema_mode":"strict","fields":[{"name":"message","type":"text","required":true}],
            "rules":{"list":true,"view":true,"create":true}}),
        json!({"name":"assets","type":"base","schema_mode":"strict","fields":[
            {"name":"title","type":"text","required":true},{"name":"avatar","type":"file","options":{"maxSelect":1,"mimeTypes":["text/plain"],"extensions":["txt"]}}],
            "rules":{"list":true,"view":true,"create":"$request.body.mode != 'reject'","update":true,"delete":true}}),
    ] {
        SchemaManager::new(&db)
            .create_collection(&serde_json::from_value::<CollectionDef>(schema).unwrap())
            .await
            .unwrap();
    }
    let mut state = ApiState::new_with_services(
        db,
        config,
        Arc::new(ObjectStoreStorage::memory()),
        Arc::new(Mailbox::default()),
    )
    .await
    .unwrap();
    state.extensions = Some(Extensions {
        dispatcher: runtime.clone(),
        hosts: Arc::new(
            ApiHostFactory::new(
                state.db.clone(),
                state.config.clone(),
                state.mailer.clone(),
                state.storage.clone(),
            )
            .with_docs(state.docs.clone())
            .with_messages(state.messages.clone()),
        ),
    });
    let state = Arc::new(state);
    let service = Service::new(build_router())
        .hoop(affix_state::inject(state.clone()))
        .hoop(herta_api::handlers::extensions::dispatch);
    (directory, runtime, state, service)
}

#[tokio::test]
async fn native_custom_routes_enforce_guards_and_http_response_contract() {
    let (_directory, runtime, _state, service) = setup(r#"
      routerAdd('POST','/api/custom/echo/{id}',async e=>e.json(201,{
        id:e.request.pathValue('id'),body:await e.request.json(),text:await e.request.text(),bytes:[...await e.request.bytes()],q:e.request.query('q'),
        secret:e.request.header('authorization')
      }),$apis.bodyLimit(32));
      routerAdd('GET','/api/custom/text',e=>e.text(200,'你好'));
      routerAdd('GET','/api/custom/raw',e=>e.rawJson(200,{raw:true}));
      routerAdd('DELETE','/api/custom/empty',e=>e.noContent());
      routerAdd('GET','/api/custom/head',e=>e.text(200,'GET'));
      routerAdd('HEAD','/api/custom/head',e=>e.text(202,'HEAD'));
      routerAdd('GET','/api/custom/admin',()=>{throw new Error('JS must not execute')},$apis.requireAdmin());
      routerAdd('GET','/api/custom/auth',e=>e.json(200,e.auth),$apis.requireAuth());
      routerAdd('GET','/api/custom/rate',e=>e.json(200,{ok:true}),$apis.rateLimit({limit:1,windowMs:60000}));
    "#).await;
    let mut response = TestClient::post("http://localhost/api/custom/echo/a%20b?q=hello")
        .body(r#"{"hello":1}"#)
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 201);
    let value: Value = response.take_json().await.unwrap();
    assert_eq!(value["data"]["id"], "a b");
    assert_eq!(value["data"]["q"], "hello");
    assert_eq!(value["data"]["body"], json!({"hello":1}));
    assert_eq!(value["data"]["text"], r#"{"hello":1}"#);
    assert!(value["data"]["secret"].is_null());
    let response = TestClient::post("http://localhost/api/custom/echo/x")
        .body("x".repeat(33))
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 413);
    let response = TestClient::post("http://localhost/api/custom/echo/x")
        .body("bad json")
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 400);
    for path in ["text", "raw"] {
        let mut response = TestClient::get(format!("http://localhost/api/custom/{path}"))
            .send(&service)
            .await;
        assert_eq!(response.status_code.unwrap().as_u16(), 200);
        assert_eq!(
            response.take_string().await.unwrap(),
            if path == "text" {
                "你好"
            } else {
                r#"{"raw":true}"#
            }
        );
    }
    for (method, path, status) in [
        ("HEAD", "text", 200),
        ("HEAD", "head", 202),
        ("DELETE", "empty", 204),
        ("OPTIONS", "head", 204),
    ] {
        let url = format!("http://localhost/api/custom/{path}");
        let request = match method {
            "HEAD" => TestClient::head(url),
            "DELETE" => TestClient::delete(url),
            _ => TestClient::options(url),
        };
        let mut response = request.send(&service).await;
        assert_eq!(response.status_code.unwrap().as_u16(), status);
        assert!(response.take_bytes(None).await.unwrap().is_empty());
        if method == "OPTIONS" {
            assert_eq!(response.headers()["allow"], "GET, HEAD, OPTIONS");
        }
    }
    for path in ["admin", "auth"] {
        let response = TestClient::get(format!("http://localhost/api/custom/{path}"))
            .send(&service)
            .await;
        assert_eq!(response.status_code.unwrap().as_u16(), 401);
    }
    let response = TestClient::get("http://localhost/api/custom/text")
        .bearer_auth("invalid")
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 401);
    for status in [200, 429] {
        let response = TestClient::get("http://localhost/api/custom/rate")
            .send(&service)
            .await;
        assert_eq!(response.status_code.unwrap().as_u16(), status);
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn json_request_hooks_fill_required_fields_and_rollback_the_full_chain() {
    let (directory, runtime, state, service) = setup(r#"
      onRecordCreateRequest(async e=>{e.record.set('title','request-hook'); await e.next()},'posts');
      onRecordListRequest(async e=>{
        if('record' in e || e.records!==null)throw new Error('invalid list event before core');
        await e.next();
        if(!Array.isArray(e.records) || !Object.isFrozen(e.records))throw new Error('missing immutable results');
      },'posts');
      onRecordCreate(async e=>{
        e.record.set('slug','filled');
        await $app.save($app.newRecord('audit',{message:'inside'}));
        e.afterCommit(async e=>{await $app.save($app.newRecord('audit',{message:e.record.get('slug')}))});
        await e.next();
      },'posts');
    "#).await;
    let endpoint = "http://localhost/api/collections/posts/records";
    let mut response = TestClient::post(endpoint)
        .json(&json!({"title":"input"}))
        .send(&service)
        .await;
    let status = response.status_code.unwrap();
    let created: Value = response.take_json().await.unwrap();
    assert_eq!(status.as_u16(), 201, "{created}");
    assert_eq!(created["data"]["title"], "request-hook");
    assert_eq!(created["data"]["slug"], "filled");
    assert_eq!(
        RecordManager::new(&state.db)
            .list("audit", &Default::default())
            .await
            .unwrap()
            .1,
        2
    );
    let mut list = TestClient::get(endpoint).send(&service).await;
    let list: Value = list.take_json().await.unwrap();
    assert_eq!(list["meta"], json!({"total":1,"page":1,"perPage":30}));
    std::fs::write(
        directory.path().join("main.js"),
        r#"
      onRecordCreate(async e=>{
        e.record.set('slug','filled');await $app.save($app.newRecord('audit',{message:'rollback'}));
        await e.next();throw new Error('abort after write');
      },'posts');
    "#,
    )
    .unwrap();
    runtime.reload().await.unwrap();
    let response = TestClient::post(endpoint)
        .json(&json!({"title":"input"}))
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 500);
    assert_eq!(
        RecordManager::new(&state.db)
            .list("posts", &Default::default())
            .await
            .unwrap()
            .1,
        1
    );
    assert_eq!(
        RecordManager::new(&state.db)
            .list("audit", &Default::default())
            .await
            .unwrap()
            .1,
        2
    );
    std::fs::write(directory.path().join("main.js"),r#"
      onRecordCreate(async e=>{e.record.set('slug','filled');e.afterCommit(()=>{while(true){}});await e.next()},'posts');
    "#).unwrap();
    runtime.reload().await.unwrap();
    let response = TestClient::post(endpoint)
        .json(&json!({"title":"input"}))
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 201);
    assert_eq!(
        RecordManager::new(&state.db)
            .list("posts", &Default::default())
            .await
            .unwrap()
            .1,
        2
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn mail_bridge_checks_authority_adapter_transaction_then_input() {
    let db = DbClient::memory().await.unwrap();
    let mailbox = Arc::new(Mailbox::default());
    let mut config = HbConfig::default();
    config.mail.from_address = "sender@example.test".into();
    let call = |arguments| HostCall {
        operation: "mail.send".into(),
        arguments,
        auth_mode: AuthMode::System,
        remaining_ms: 1000,
        script: "test.js".into(),
        transaction: None,
    };
    let factory = |config| {
        ApiHostFactory::new(
            db.clone(),
            Arc::new(config),
            mailbox.clone(),
            Arc::new(ObjectStoreStorage::memory()),
        )
        .create(HostContext::system())
    };
    let host = factory(config.clone());
    assert_eq!(
        host.call(call(Value::Null)).await.unwrap_err().error_code(),
        "HB_CAPABILITY_DENIED"
    );
    config.jsvm.mail.enabled = true;
    let host = factory(config.clone());
    assert_eq!(
        host.call(call(Value::Null)).await.unwrap_err().error_code(),
        "HB_CAPABILITY_UNAVAILABLE"
    );
    config.mail.driver = "smtp".into();
    let host = factory(config);
    let mut begin = call(Value::Null);
    begin.operation = "transaction.begin".into();
    host.call(begin).await.unwrap();
    assert_eq!(
        host.call(call(Value::Null)).await.unwrap_err().error_code(),
        "HB_SIDE_EFFECT_IN_TRANSACTION"
    );
    assert!(mailbox.0.lock().unwrap().is_empty());
    host.finish(false).await.unwrap();
    assert!(matches!(
        host.call(call(Value::Null)).await,
        Err(HbError::Validation { .. })
    ));
    assert!(mailbox.0.lock().unwrap().is_empty());
    let receipt = host
        .call(call(
            json!({"to":[{"address":"reader@example.test"}],"subject":"hello","text":"body"}),
        ))
        .await
        .unwrap();
    assert_eq!(receipt["messageId"], "<extension@example.test>");
    assert!(
        !host.retry_safe(),
        "a failed cron must not repeat an accepted SMTP message"
    );
    assert_eq!(mailbox.0.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn http_bridge_checks_authority_and_native_transaction_before_parameters() {
    let db = DbClient::memory().await.unwrap();
    let factory = |config| {
        ApiHostFactory::new(
            db.clone(),
            Arc::new(config),
            Arc::new(Mailbox::default()),
            Arc::new(ObjectStoreStorage::memory()),
        )
        .create(HostContext::system())
    };
    let call = |arguments| HostCall {
        operation: "http.send".into(),
        arguments,
        auth_mode: AuthMode::System,
        remaining_ms: 1000,
        script: "http.js".into(),
        transaction: None,
    };
    let mut config = HbConfig::default();
    let host = factory(config.clone());
    assert_eq!(
        host.call(call(Value::Null)).await.unwrap_err().error_code(),
        "HB_CAPABILITY_DENIED"
    );
    config.jsvm.http.enabled = true;
    config.jsvm.http.allowlist = vec!["http://127.0.0.1".into()];
    let host = factory(config);
    let mut begin = call(Value::Null);
    begin.operation = "transaction.begin".into();
    host.call(begin).await.unwrap();
    // The untrusted call deliberately omits the transaction ID. Native state still wins.
    assert_eq!(
        host.call(call(Value::Null)).await.unwrap_err().error_code(),
        "HB_SIDE_EFFECT_IN_TRANSACTION"
    );
    host.finish(false).await.unwrap();
    assert_eq!(
        host.call(call(Value::Null)).await.unwrap_err().error_code(),
        "HB_VALIDATION_ERROR"
    );
    assert_eq!(
        host.call(call(json!({"url":"http://127.0.0.1"})))
            .await
            .unwrap_err()
            .error_code(),
        "HB_OUTBOUND_DENIED"
    );
}

#[tokio::test]
async fn multipart_hooks_fill_required_fields_and_settle_uploads_by_transaction_outcome() {
    let (_directory, runtime, state, service) = setup(
        r#"
      onRecordCreateRequest(async e=>{
        const body = await e.request.json();
        e.context.mode = body.mode;
        await e.next();
      },'assets');
      onRecordCreate(async e=>{
        e.record.set('title','filled');
        await $app.save($app.newRecord('audit',{message:'create:'+e.context.mode}));
        if (e.context.mode === 'discard') e.record.unset('avatar');
        await e.next();
        if (e.context.mode === 'abort') throw new Error('rollback');
      },'assets');
    "#,
    )
    .await;
    let body = |mode: &str| {
        format!(
            "--hb-upload\r\nContent-Disposition: form-data; name=\"data\"\r\n\r\n{{\"mode\":\"{mode}\"}}\r\n--hb-upload\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"hello.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--hb-upload--\r\n"
        )
    };
    // mode is request-only: request hooks remove it from the candidate before strict validation.
    let script = r#"
      onRecordCreateRequest(async e=>{e.context.mode=(await e.request.json()).mode;e.record.unset('mode');await e.next()},'assets');
      onRecordCreate(async e=>{
        e.record.set('title','filled');await $app.save($app.newRecord('audit',{message:'create:'+e.context.mode}));
        if(e.context.mode==='discard')e.record.unset('avatar');await e.next();
        if(e.context.mode==='abort')throw new Error('rollback');
      },'assets');
    "#;
    std::fs::write(_directory.path().join("main.js"), script).unwrap();
    runtime.reload().await.unwrap();
    let mut keep = None;
    for (mode, status) in [
        ("keep", 201),
        ("discard", 201),
        ("abort", 500),
        ("reject", 403),
    ] {
        let mut response = TestClient::post("http://localhost/api/collections/assets/records")
            .add_header(
                "content-type",
                "multipart/form-data; boundary=hb-upload",
                true,
            )
            .body(body(mode))
            .send(&service)
            .await;
        let actual = response.status_code.unwrap().as_u16();
        let value: Value = response.take_json().await.unwrap();
        assert_eq!(actual, status, "{mode}: {value}");
        if mode == "keep" {
            let key = herta_db::uploads::storage_key(
                "assets",
                value["data"]["id"].as_str().unwrap(),
                "avatar",
                value["data"]["avatar"].as_str().unwrap(),
            );
            assert_eq!(state.storage.head(&key).await.unwrap().size, 5);
            keep = Some((key, value["data"]["id"].as_str().unwrap().to_owned()));
        }
        if mode == "discard" {
            assert!(value["data"]["avatar"].is_null());
        }
        assert!(
            herta_db::uploads::entries(&state.db, None, 0)
                .await
                .unwrap()
                .is_empty(),
            "upload journal must settle for {mode}"
        );
    }
    assert_eq!(
        RecordManager::new(&state.db)
            .list("audit", &Default::default())
            .await
            .unwrap()
            .1,
        2
    );
    let (key, id) = keep.unwrap();
    let response = TestClient::delete(format!(
        "http://localhost/api/collections/assets/records/{id}"
    ))
    .send(&service)
    .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 200);
    assert!(matches!(
        state.storage.head(&key).await,
        Err(HbError::NotFound)
    ));
    runtime.shutdown().await.unwrap();
}

async fn members(state: &ApiState) {
    SchemaManager::new(&state.db)
        .create_collection(
            &serde_json::from_value::<CollectionDef>(json!({
                "name":"members","type":"auth","schema_mode":"strict",
                "fields":[{"name":"name","type":"text","required":true}],
                "rules":{"list":true,"view":true}
            }))
            .unwrap(),
        )
        .await
        .unwrap();
}

async fn auth_request(service: &Service, endpoint: &str, input: Value) -> (u16, Value) {
    let mut response = TestClient::post(format!("http://localhost/api/auth/members/{endpoint}"))
        .json(&input)
        .send(service)
        .await;
    (
        response.status_code.unwrap().as_u16(),
        response.take_json().await.unwrap(),
    )
}

async fn token_count(state: &ApiState) -> usize {
    let mut response = state
        .db
        .inner()
        .query("SELECT * FROM _auth_refresh_tokens WHERE collection = 'members'")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Value> = response.take(0).unwrap();
    rows.len()
}

#[tokio::test]
async fn auth_register_uses_both_chains_with_private_credentials_and_one_commit() {
    let (_directory, runtime, state, service) = setup(r#"
      function check(value) {
        const text=JSON.stringify(value);
        for(const secret of ['password','token_key','password_hash','refreshToken','accessToken','hunter2'])
          if(text.includes(secret))throw new Error('credential crossed bridge: '+secret);
      }
      onAuthRegister(async e=>{
        check(await e.request.json());check(await e.request.text());check(e.profile);
        if(e.request.header('authorization')!==null)throw new Error('authorization leaked');
        if(e.account!==null)throw new Error('account exists before creation');
        e.context.order=['auth'];
        e.profile.name='auth';
        try {e.profile.role='admin';throw new Error('protected write accepted')}catch(error){if(error.code!=='HB_VALIDATION_ERROR')throw error}
        e.afterCommit(async snapshot=>{
          check(snapshot);
          if(snapshot.profile.name!=='auth-record')throw new Error('wrong final profile');
          await $app.save($app.newRecord('audit',{message:'after-auth'}));
        });
        await e.next();
        try {e.profile.name='late';throw new Error('late write accepted')}catch(error){if(error.code!=='HB_VALIDATION_ERROR')throw error}
        e.context.order.push('auth-after');
        await $app.save($app.newRecord('audit',{message:e.context.order.join(',')}));
      },'members');
      onRecordCreate(async e=>{
        check(e.record.toJSON());
        if(e.context.order.join(',')!=='auth')throw new Error('wrong chain order');
        e.context.order.push('record');e.record.set('name',e.record.get('name')+'-record');
        e.afterCommit(async snapshot=>{check(snapshot);await $app.save($app.newRecord('audit',{message:'after-record'}))});
        await e.next();
        check(e.record.toJSON());
        try{e.record.set('name','late');throw new Error('late write accepted')}catch(error){if(error.code!=='HB_VALIDATION_ERROR')throw error}
        e.context.order.push('record-after');
      },'members');
    "#).await;
    members(&state).await;
    let (status, body) = auth_request(&service,"register",json!({"email":"member@example.com","password":"hunter2","role":"admin","passwordConfirm":"hunter2"})).await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(body["data"]["user"]["name"], "auth-record");
    assert_eq!(body["data"]["user"]["role"], "user");
    let access = body["data"]["accessToken"].as_str().unwrap();
    assert!(state.auth.authenticate(access).await.is_ok());
    assert_eq!(token_count(&state).await, 1);
    let (rows, total) = RecordManager::new(&state.db)
        .list("audit", &Default::default())
        .await
        .unwrap();
    assert_eq!(total, 3);
    let messages: Vec<_> = rows
        .iter()
        .map(|row| row["message"].as_str().unwrap())
        .collect();
    assert!(messages.contains(&"auth,record,record-after,auth-after"));
    assert!(messages.contains(&"after-auth"));
    assert!(messages.contains(&"after-record"));
    let (status, body) = auth_request(
        &service,
        "login",
        json!({"email":"member@example.com","password":"hunter2"}),
    )
    .await;
    assert_eq!(status, 200, "password must be hashed exactly once: {body}");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn auth_hook_rejection_rolls_back_account_tokens_and_caught_nested_failures() {
    let (directory, runtime, state, service) = setup("").await;
    members(&state).await;
    for (index, source, status, code) in [
        (
            0,
            "onAuthRegister(e=>{},'members');",
            409,
            "HB_HOOK_ABORTED",
        ),
        (
            1,
            "onRecordCreate(async e=>{await e.next();throw new Error('reject')},'members');",
            500,
            "HB_HOOK_ERROR",
        ),
        (
            2,
            "onAuthRegister(async e=>{try{await $app.save($app.newRecord('audit',{}))}catch{}await e.next()},'members');",
            409,
            "HB_HOOK_ABORTED",
        ),
        (
            3,
            "onAuthRegister(async e=>{await e.next();await e.next()},'members');",
            500,
            "HB_HOOK_ERROR",
        ),
        (
            4,
            "onRecordCreate(async e=>{await e.next();try{await $app.save(e.record)}catch(error){if(error.code!=='HB_HOOK_RECURSION')throw error}},'members');",
            409,
            "HB_HOOK_ABORTED",
        ),
    ] {
        std::fs::write(directory.path().join("main.js"), source).unwrap();
        runtime.reload().await.unwrap();
        let (actual,body)=auth_request(&service,"register",json!({"email":format!("reject{index}@example.com"),"password":"hunter2","name":"member"})).await;
        assert_eq!(actual, status, "{index}: {body}");
        assert_eq!(body["error"]["error"], code, "{body}");
        assert_eq!(
            RecordManager::new(&state.db)
                .list("members", &Default::default())
                .await
                .unwrap()
                .1,
            0
        );
        assert_eq!(token_count(&state).await, 0);
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn login_refresh_hooks_follow_verification_and_preserve_security_state() {
    let (directory, runtime, state, service) = setup("").await;
    members(&state).await;
    let (_, registered) = auth_request(
        &service,
        "register",
        json!({"email":"member@example.com","password":"hunter2","name":"member"}),
    )
    .await;
    let refresh = registered["data"]["refreshToken"].as_str().unwrap();
    std::fs::write(
        directory.path().join("main.js"),
        r#"
      onAuthLogin(async e=>{if(e.account.name!=='member')throw new Error('missing account');
        if(Object.keys(await e.request.json()).length)throw new Error('credentials leaked');
        throw new ForbiddenError('login veto')},'members');
      onTokenRefresh(async e=>{throw new ForbiddenError('refresh veto')},'members');
    "#,
    )
    .unwrap();
    runtime.reload().await.unwrap();
    let (status, body) = auth_request(
        &service,
        "login",
        json!({"email":"member@example.com","password":"incorrect"}),
    )
    .await;
    assert_eq!(status, 401, "{body}");
    let (status, body) = auth_request(
        &service,
        "login",
        json!({"email":"member@example.com","password":"hunter2"}),
    )
    .await;
    assert_eq!(status, 403, "{body}");
    let (status, body) = auth_request(&service, "refresh", json!({"refreshToken":refresh})).await;
    assert_eq!(status, 403, "{body}");
    assert_eq!(token_count(&state).await, 1);
    let (status, body) = auth_request(&service, "refresh", json!({"refreshToken":refresh})).await;
    assert_eq!(
        status, 401,
        "consumption and replay must survive veto: {body}"
    );
    assert!(
        state
            .auth
            .authenticate(registered["data"]["accessToken"].as_str().unwrap())
            .await
            .is_err()
    );
    std::fs::write(directory.path().join("main.js"),r#"
      onAuthLogin(async e=>{'use strict';try{e.account.role='admin';throw new Error('mutable account')}catch(error){if(!(error instanceof TypeError))throw error}await e.next()},'members');
      onTokenRefresh(async e=>{await e.next();e.afterCommit(()=>{while(true){}})},'members');
    "#).unwrap();
    runtime.reload().await.unwrap();
    let (status, body) = auth_request(
        &service,
        "login",
        json!({"email":"member@example.com","password":"hunter2"}),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, body) = auth_request(
        &service,
        "refresh",
        json!({"refreshToken":body["data"]["refreshToken"]}),
    )
    .await;
    assert_eq!(
        status, 200,
        "confirmed Auth success survives afterCommit interruption: {body}"
    );
    assert!(
        state
            .auth
            .authenticate(body["data"]["accessToken"].as_str().unwrap())
            .await
            .is_ok()
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn collection_events_share_version_checks_permissions_and_post_commit_work() {
    let (_directory,runtime,state,service)=setup(r#"
      onCollectionCreate(async e=>{
        if(e.originalCollection!==null)throw new Error('wrong original');
        if(Object.getPrototypeOf(e)!==Object.prototype)throw new Error('backing event exposed');
        e.collection.fields=[{name:'title',type:'text',required:true}];
        e.afterCommit(async s=>{await $app.save($app.newRecord('audit',{message:s.name+':'+s.collection.name}))});
        await e.next();
        try{e.collection.fields=[];throw new Error('late write accepted')}catch(error){if(error.code!=='HB_VALIDATION_ERROR')throw error}
        if(e.collection.name==='veto')throw new ForbiddenError('veto');
      });
      onCollectionUpdate(async e=>{
        const fields=e.collection.fields;fields.push({name:'added',type:'number'});e.collection.fields=fields;
        await e.next();
      });
      onCollectionDelete(async e=>{
        try{e.collection.rules={};throw new Error('mutable deletion')}catch(error){if(error.code!=='HB_VALIDATION_ERROR')throw error}
        await e.next();
      });
      routerAdd('POST','/api/custom/schema',async e=>e.json(200,await $app.collections.create({name:'forbidden',type:'base',schema_mode:'strict'})));
      routerAdd('POST','/api/custom/schema-save',async e=>{
        const old=await $app.collections.findByName('managed');
        const copy=await $app.collections.findByName('managed');copy.rules={list:true};
        const updated=await $app.collections.save(copy);
        let stale;try{await $app.collections.save(old)}catch(error){stale=error.code}
        const unchanged=await $app.collections.save(updated);
        return e.json(200,{stale,version:updated.version,unchanged:unchanged.version});
      },$apis.requireAdmin());
      onRecordCreate(async e=>{if(e.record.get('message')==='reject-nested')try{await $app.collections.create({name:'nested',type:'base',schema_mode:'strict'})}catch{}await e.next()},'audit');
    "#).await;
    let mut response = TestClient::post("http://localhost/api/admin/auth/login")
        .json(&json!({"email":"admin@example.com","password":"correct horse battery staple"}))
        .send(&service)
        .await;
    let body: Value = response.take_json().await.unwrap();
    let token = body["data"]["accessToken"].as_str().unwrap();
    for (name, status) in [("managed", 201), ("veto", 403)] {
        let mut response = TestClient::post("http://localhost/_/collections")
            .bearer_auth(token)
            .json(&json!({"name":name,"type":"base","schema_mode":"strict"}))
            .send(&service)
            .await;
        let actual = response.status_code.unwrap().as_u16();
        let body: Value = response.take_json().await.unwrap();
        assert_eq!(actual, status, "{body}");
        if name == "managed" {
            assert_eq!(body["data"]["fields"][0]["name"], "title");
            assert!(body["data"]["version"].is_string());
        }
    }
    assert!(
        SchemaManager::new(&state.db)
            .get_collection("veto")
            .await
            .is_err()
    );
    assert!(state.docs.read().await["paths"]["/api/collections/managed/records"].is_object());
    assert!(state.docs.read().await["paths"]["/api/collections/veto/records"].is_null());
    let mut response = TestClient::post("http://localhost/api/custom/schema")
        .send(&service)
        .await;
    let status = response.status_code.unwrap().as_u16();
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(
        status, 403,
        "request routes cannot inherit system hook authority: {body}"
    );
    assert_eq!(
        RecordManager::new(&state.db)
            .list("audit", &Default::default())
            .await
            .unwrap()
            .1,
        1,
        "only committed create runs afterCommit"
    );
    let mut response = TestClient::post("http://localhost/api/collections/audit/records")
        .bearer_auth(token)
        .json(&json!({"message":"reject-nested"}))
        .send(&service)
        .await;
    let actual = response.status_code.unwrap().as_u16();
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(
        actual, 409,
        "caught Schema attempt must poison Record transaction: {body}"
    );
    let mut response = TestClient::patch("http://localhost/_/collections/managed")
        .bearer_auth(token)
        .json(&json!({"rules":{"view":true}}))
        .send(&service)
        .await;
    let actual = response.status_code.unwrap().as_u16();
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(actual, 200, "{body}");
    assert_eq!(body["data"]["fields"][1]["name"], "added");
    // Reload removes mutation hooks before checking the no-op save contract.
    std::fs::write(
        _directory.path().join("main.js"),
        r#"
      routerAdd('POST','/api/custom/schema-save',async e=>{
        const old=await $app.collections.findByName('managed');
        const copy=await $app.collections.findByName('managed');copy.rules={list:true};
        const updated=await $app.collections.save(copy);
        let stale;try{await $app.collections.save(old)}catch(error){stale=error.code}
        const unchanged=await $app.collections.save(updated);
        return e.json(200,{stale,version:updated.version,unchanged:unchanged.version});
      },$apis.requireAdmin());
      onCollectionDelete(async e=>{await e.next()});
    "#,
    )
    .unwrap();
    runtime.reload().await.unwrap();
    let mut response = TestClient::post("http://localhost/api/custom/schema-save")
        .bearer_auth(token)
        .send(&service)
        .await;
    let status = response.status_code.unwrap().as_u16();
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["stale"], "HB_CONFLICT");
    assert_eq!(body["data"]["version"], body["data"]["unchanged"]);
    let upload = _directory.path().join("file.txt");
    std::fs::write(&upload, "old attachment").unwrap();
    state
        .storage
        .put_file("records/managed/old/file.txt", &upload)
        .await
        .unwrap();
    let response = TestClient::delete("http://localhost/_/collections/managed")
        .bearer_auth(token)
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 200);
    assert!(
        state
            .storage
            .head("records/managed/old/file.txt")
            .await
            .is_err()
    );
    assert!(state.docs.read().await["paths"]["/api/collections/managed/records"].is_null());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn strict_auth_profile_record_updates_preserve_host_fields() {
    let (_directory, runtime, state, service) = setup(
        r#"
      onRecordUpdate(async e=>{
        for(const field of ['password_hash','token_key','accessToken','refreshToken'])
          if(e.record.get(field)!==undefined)throw new Error('sensitive field leaked');
        e.record.set('name','hook profile');await e.next();
      },'members');
    "#,
    )
    .await;
    members(&state).await;
    let (status, registered) = auth_request(
        &service,
        "register",
        json!({"email":"profile@example.com","password":"hunter2","name":"old"}),
    )
    .await;
    assert_eq!(status, 201, "{registered}");
    let mut response = TestClient::post("http://localhost/api/admin/auth/login")
        .json(&json!({"email":"admin@example.com","password":"correct horse battery staple"}))
        .send(&service)
        .await;
    let admin: Value = response.take_json().await.unwrap();
    let url = format!(
        "http://localhost/api/collections/members/records/{}",
        registered["data"]["user"]["id"].as_str().unwrap()
    );
    let mut response = TestClient::patch(url)
        .bearer_auth(admin["data"]["accessToken"].as_str().unwrap())
        .json(&json!({"name":"input"}))
        .send(&service)
        .await;
    let status = response.status_code.unwrap().as_u16();
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["name"], "hook profile");
    assert_eq!(body["data"]["email"], "profile@example.com");
    let (status, body) = auth_request(
        &service,
        "login",
        json!({"email":"profile@example.com","password":"hunter2"}),
    )
    .await;
    assert_eq!(
        status, 200,
        "credentials must survive profile update: {body}"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shipped_example_bootstraps_and_demonstrates_atomic_rollback() {
    use herta_core::extension::Invocation;
    let (_directory, runtime, state, service) = setup(concat!(
        include_str!("../../../examples/js-runtime/main.js"),
        "\n",
        include_str!("../../../examples/js-runtime/services.js")
    ))
    .await;
    for name in ["bootstrap", "serve"] {
        runtime
            .invoke(
                Invocation {
                    name: name.into(),
                    payload: Value::Null,
                    auth_mode: AuthMode::System,
                    request_id: None,
                    registration: None,
                },
                state
                    .extensions
                    .as_ref()
                    .unwrap()
                    .hosts
                    .create(HostContext::system()),
                None,
            )
            .await
            .unwrap();
    }
    let mut response = TestClient::post("http://localhost/api/admin/auth/login")
        .json(&json!({"email":"admin@example.com","password":"correct horse battery staple"}))
        .send(&service)
        .await;
    let admin: Value = response.take_json().await.unwrap();
    for (fail, status) in [(false, 201), (true, 409)] {
        let mut response = TestClient::post("http://localhost/api/custom/demo-transaction")
            .bearer_auth(admin["data"]["accessToken"].as_str().unwrap())
            .json(&json!({"title":"Hello World","fail":fail}))
            .send(&service)
            .await;
        let actual = response.status_code.unwrap().as_u16();
        let body: Value = response.take_json().await.unwrap();
        assert_eq!(actual, status, "{body}");
    }
    assert_eq!(
        RecordManager::new(&state.db)
            .list("demo_posts", &Default::default())
            .await
            .unwrap()
            .1,
        1
    );
    assert_eq!(
        RecordManager::new(&state.db)
            .list("demo_audit", &Default::default())
            .await
            .unwrap()
            .1,
        1
    );
    let mut response = TestClient::post("http://localhost/api/auth/demo_members/register")
        .json(&json!({"email":"reader@example.com","password":"choose-a-password"}))
        .send(&service)
        .await;
    let actual = response.status_code.unwrap().as_u16();
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(actual, 201, "{body}");
    assert_eq!(body["data"]["user"]["name"], "New member");
    let token = admin["data"]["accessToken"].as_str().unwrap();
    let mut response = TestClient::post("http://localhost/api/custom/demo-file")
        .bearer_auth(token)
        .json(&json!({"report":"example"}))
        .send(&service)
        .await;
    let file: Value = response.take_json().await.unwrap();
    assert_eq!(response.status_code.unwrap().as_u16(), 201, "{file}");
    let mut response = TestClient::get("http://localhost/api/custom/demo-file")
        .bearer_auth(token)
        .send(&service)
        .await;
    assert_eq!(
        response.take_json::<Value>().await.unwrap(),
        json!({"report":"example"})
    );
    for (fail, status) in [(true, 400), (false, 202)] {
        let mut response = TestClient::post("http://localhost/api/custom/demo-outbox")
            .bearer_auth(token)
            .json(&json!({"address":"reader@example.com","key":"example","fail":fail}))
            .send(&service)
            .await;
        let body: Value = response.take_json().await.unwrap();
        assert_eq!(response.status_code.unwrap().as_u16(), status, "{body}");
    }
    assert_eq!(
        herta_db::outbox::list(&state.db, "", 100).await.unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    runtime
        .invoke(
            Invocation {
                name: "shutdown".into(),
                payload: Value::Null,
                auth_mode: AuthMode::System,
                request_id: None,
                registration: None,
            },
            state
                .extensions
                .as_ref()
                .unwrap()
                .hosts
                .create(HostContext::system()),
            None,
        )
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn javascript_files_roundtrip_binary_and_logical_response_after_commit() {
    let (_directory, runtime, _state, service) = setup(r#"
      routerAdd('POST','/api/custom/file',async e=>{
        await $app.transaction(async tx=>{
          try { await $app.files.write('denied','x'); throw new Error('transaction gate bypassed'); }
          catch(error) { if(error.code!=='HB_SIDE_EFFECT_IN_TRANSACTION')throw error; }
          tx.afterCommit(async()=>{
            await $app.files.write('report',new Uint8Array([0,255,128,65]),{contentType:'application/x-herta-test'});
          });
        });
        const bytes=await $app.files.readBytes('report');
        if(!(bytes instanceof Uint8Array) || bytes[1]!==255)throw new Error('invalid binary');
        return e.json(201,await $app.files.stat('report'));
      });
      routerAdd('GET','/api/custom/file',e=>e.file('report'));
    "#).await;
    let mut response = TestClient::post("http://localhost/api/custom/file")
        .send(&service)
        .await;
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(response.status_code.unwrap().as_u16(), 201, "{body}");
    assert_eq!(body["data"]["size"], 4);
    let mut response = TestClient::get("http://localhost/api/custom/file")
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 200);
    assert_eq!(
        response.headers()["content-type"],
        "application/x-herta-test"
    );
    assert_eq!(
        response.take_bytes(None).await.unwrap().as_ref(),
        &[0, 255, 128, 65]
    );
    let mut response = TestClient::head("http://localhost/api/custom/file")
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 200);
    assert!(response.take_bytes(None).await.unwrap().is_empty());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn binary_file_bridge_copies_views_without_json_expansion_or_aliasing() {
    let (_directory, runtime, _state, service) = setup(r#"
      routerAdd('POST','/api/custom/binary-buffer',async e=>{
        const storage=new Uint8Array(2*1024*1024+8); storage.fill(255);
        const view=storage.subarray(4,storage.length-4); view[0]=0; view[view.length-1]=128;
        const pending=$app.files.write('binary-buffer',view);
        view.fill(65);
        await pending;
        const read=await $app.files.readBytes('binary-buffer');
        if(!(read instanceof Uint8Array)||read.length!==2*1024*1024||read[0]!==0||read[1]!==255||read[read.length-1]!==128)
          throw new Error('binary view was expanded, aliased, or offset incorrectly');
        read[0]=99;
        const again=await $app.files.readBytes('binary-buffer');
        if(again[0]!==0)throw new Error('read aliases immutable storage');
        await $app.files.write('empty-buffer',new Uint8Array(0));
        if((await $app.files.readBytes('empty-buffer')).length!==0)throw new Error('empty binary lost');
        return e.json(200,{length:again.length});
      });
    "#).await;
    let mut response = TestClient::post("http://localhost/api/custom/binary-buffer")
        .send(&service)
        .await;
    let status = response.status_code.unwrap().as_u16();
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"]["length"], 2 * 1024 * 1024);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn binary_request_and_file_response_cross_the_worker_without_json_byte_arrays() {
    let (_directory, runtime, _state, service) = setup(r#"
      routerAdd('POST','/api/custom/binary-echo',async e=>{
        const bytes=await e.request.bytes();
        const text=await e.request.text();
        if(text.length!==bytes.length || text.charCodeAt(0)!==65533)throw new Error('invalid UTF-8 text semantics');
        try{await e.request.json();throw new Error('invalid UTF-8 parsed as JSON')}
        catch(error){if(error.code!=='HB_VALIDATION_ERROR')throw error;}
        const original=bytes[0]; bytes[0]=0;
        const second=await e.request.bytes();
        if(second[0]!==original)throw new Error('body cache aliases JS view');
        await $app.files.write('binary-echo',second);
        return e.file('binary-echo');
      });
    "#).await;
    let bytes = vec![255u8; 2 * 1024 * 1024];
    let mut response = TestClient::post("http://localhost/api/custom/binary-echo")
        .body(bytes.clone())
        .send(&service)
        .await;
    let status = response.status_code.unwrap().as_u16();
    let body = response.take_bytes(None).await.unwrap();
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    assert_eq!(body.as_ref(), bytes);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn javascript_outbox_rolls_back_with_records_and_admin_resolution_is_protected() {
    let (_directory,runtime,state,service)=setup(r#"
      routerAdd('POST','/api/custom/enqueue',async e=>{
        return e.json(201,await $app.transaction(async tx=>{
          await tx.save(tx.newRecord('audit',{message:'queued'}));
          const receipt=await $app.outbox.enqueue('mail.send',{to:[{address:'reader@example.com'}],subject:'hi',text:'secret body'},{idempotencyKey:'test'});
          if(e.request.query('abort'))throw new Error('rollback');
          return receipt;
        }));
      });
      routerAdd('POST','/api/custom/implicit',async e=>e.json(201,await $app.outbox.enqueue('mail.send',{to:[{address:'reader@example.com'}],subject:'hi',text:'secret body'},{idempotencyKey:'test'})));
    "#).await;
    let response = TestClient::post("http://localhost/api/custom/enqueue?abort=true")
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 500);
    assert_eq!(
        RecordManager::new(&state.db)
            .list("audit", &Default::default())
            .await
            .unwrap()
            .1,
        0
    );
    assert!(
        herta_db::outbox::claim(&state.db, 60)
            .await
            .unwrap()
            .is_none()
    );
    let mut response = TestClient::post("http://localhost/api/custom/enqueue")
        .send(&service)
        .await;
    let body: Value = response.take_json().await.unwrap();
    assert_eq!(response.status_code.unwrap().as_u16(), 201, "{body}");
    let id = body["data"]["jobId"].as_str().unwrap();
    let mut response = TestClient::post("http://localhost/api/custom/implicit")
        .send(&service)
        .await;
    assert_eq!(
        response.take_json::<Value>().await.unwrap()["data"]["jobId"],
        id
    );
    let mut job = herta_db::outbox::claim(&state.db, 60)
        .await
        .unwrap()
        .unwrap();
    herta_db::outbox::sending(&state.db, &mut job)
        .await
        .unwrap();
    state
        .db
        .inner()
        .query("UPDATE _extension_outbox SET leaseUntil=0")
        .await
        .unwrap()
        .check()
        .unwrap();
    herta_db::outbox::recover_expired(&state.db).await.unwrap();
    for path in [
        "api/admin/outbox".to_owned(),
        format!("api/admin/outbox/{id}"),
    ] {
        let response = TestClient::get(format!("http://localhost/{path}"))
            .send(&service)
            .await;
        assert_eq!(response.status_code.unwrap().as_u16(), 401);
    }
    let response = TestClient::post(format!("http://localhost/api/admin/outbox/{id}/resolve"))
        .json(&json!({"resolution":"accepted","note":"verified at receiver"}))
        .send(&service)
        .await;
    assert_eq!(response.status_code.unwrap().as_u16(), 401);
    let mut response = TestClient::post("http://localhost/api/admin/auth/login")
        .json(&json!({"email":"admin@example.com","password":"correct horse battery staple"}))
        .send(&service)
        .await;
    let login: Value = response.take_json().await.unwrap();
    let token = login["data"]["accessToken"].as_str().unwrap();
    let mut response = TestClient::get("http://localhost/api/admin/outbox")
        .bearer_auth(token)
        .send(&service)
        .await;
    let page: Value = response.take_json().await.unwrap();
    assert_eq!(page["data"]["items"][0]["state"], "unknown");
    assert!(!page.to_string().contains("secret body"));
    let mut response = TestClient::post(format!("http://localhost/api/admin/outbox/{id}/resolve"))
        .bearer_auth(token)
        .json(&json!({"resolution":"accepted","note":"verified at receiver"}))
        .send(&service)
        .await;
    assert_eq!(
        response.take_json::<Value>().await.unwrap()["data"]["state"],
        "accepted"
    );
    runtime.shutdown().await.unwrap();
}

struct DelayedCommitAck(Arc<dyn herta_core::extension::HostServices>);
#[async_trait]
impl herta_core::extension::HostServices for DelayedCommitAck {
    async fn call(&self, call: HostCall) -> HbResult<Value> {
        let committing = call.operation == "transaction.commit";
        let result = self.0.call(call).await;
        if committing && result.is_ok() {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        result
    }
    fn cancel(&self) {
        self.0.cancel();
    }
    fn interrupted_commit(&self) -> Option<HbError> {
        self.0.interrupted_commit()
    }
    async fn finish(&self, successful: bool) -> HbResult<()> {
        self.0.finish(successful).await
    }
}
#[tokio::test]
async fn timeout_after_native_commit_returns_a_verifiable_receipt_without_repeating_the_write() {
    let (_directory, runtime, state, _service) = setup(
        r#"
      routerAdd('POST','/api/custom/delayed-ack',async e=>{
        await $app.transaction(async tx=>{await tx.save(tx.newRecord('audit',{message:'once'}));});
        return e.noContent();
      });
    "#,
    )
    .await;
    let host = state
        .extensions
        .as_ref()
        .unwrap()
        .hosts
        .create(HostContext::system());
    let error = runtime
        .invoke(
            herta_core::extension::Invocation {
                name: "route".into(),
                payload: json!({"request":{"method":"POST","path":"/api/custom/delayed-ack"}}),
                auth_mode: AuthMode::System,
                request_id: None,
                registration: Some(0),
            },
            Arc::new(DelayedCommitAck(host)),
            Some(100),
        )
        .await
        .unwrap_err();
    assert_eq!(error.error_code(), "HB_COMMIT_UNKNOWN");
    let HbError::Extension(error) = error else {
        panic!()
    };
    let details = error.details.unwrap();
    let status = herta_db::transaction::operation_status(
        &state.db,
        details["operationId"].as_str().unwrap(),
        None,
        details["checkCredential"].as_str(),
    )
    .await
    .unwrap();
    assert_eq!(
        status.state,
        herta_db::transaction::OperationState::Committed
    );
    assert_eq!(
        RecordManager::new(&state.db)
            .list("audit", &Default::default())
            .await
            .unwrap()
            .1,
        1
    );
    runtime.shutdown().await.unwrap();
}
