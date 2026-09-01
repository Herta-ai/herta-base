use herta_auth::AuthIdentity;
use herta_db::{LogEntry, LogSender, LogType};
use salvo::prelude::*;

use crate::handlers::auth::identity;
use crate::router::SharedApiState;

/// Persists request metadata after the downstream handler has produced a response.
pub struct RequestLogger {
    sender: LogSender,
}

// Reading the log stream must not create another log-stream event.
fn should_skip_request_log(path: &str) -> bool {
    matches!(path, "/api/admin/logs" | "/api/realtime/_logs")
}

impl RequestLogger {
    pub fn new(sender: LogSender) -> Self {
        Self { sender }
    }
}

#[handler]
impl RequestLogger {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        if should_skip_request_log(&path) {
            ctrl.call_next(req, depot, res).await;
            return;
        }
        let remote_ip = req.remote_addr().ip().map(|ip| ip.to_string());
        let referer = header_value(req, "referer");
        let user_agent = header_value(req, "user-agent");
        let auth = match depot.get_typed::<SharedApiState>() {
            Ok(state) => identity(req, state).await.unwrap_or_default(),
            Err(_) => AuthIdentity::Anonymous,
        };
        let (auth_type, user_id, user_collection) = identity_fields(&auth);

        ctrl.call_next(req, depot, res).await;

        let status_code = res.status_code.unwrap_or(StatusCode::OK).as_u16();
        let level = match status_code {
            500..=599 => "error",
            400..=499 => "warn",
            _ => "info",
        };
        let entry = LogEntry {
            log_type: LogType::Request,
            level: level.into(),
            message: format!("{method} {path} -> {status_code}"),
            target: "herta_api::request".into(),
            method: Some(method),
            path: Some(path),
            status_code: Some(status_code),
            referer,
            remote_ip,
            user_agent,
            auth_type: Some(auth_type.into()),
            user_id,
            user_collection,
        };
        let _ = self.sender.try_send(entry);
    }
}

fn header_value(req: &Request, name: &str) -> Option<String> {
    req.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn identity_fields(identity: &AuthIdentity) -> (&'static str, Option<String>, Option<String>) {
    match identity {
        AuthIdentity::Anonymous => ("anonymous", None, None),
        AuthIdentity::User { id, collection, .. } => {
            ("user", Some(id.clone()), Some(collection.clone()))
        }
        AuthIdentity::Admin { id, .. } => ("admin", Some(id.clone()), Some("_admins".into())),
    }
}
