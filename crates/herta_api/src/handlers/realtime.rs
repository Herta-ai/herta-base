use std::{
    convert::Infallible,
    future::pending,
    pin::Pin,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{SecondsFormat, Utc};
use futures_util::stream;
use herta_auth::AuthIdentity;
use herta_core::{HbError, HbResult};
use herta_db::{DbClient, RealtimeAction, RealtimeManager, RealtimeSubscription};
use salvo::{
    http::header::{HeaderName, HeaderValue},
    prelude::*,
    sse::{self, SseEvent},
};
use serde_json::{Value, json};
use tokio::time::{Instant, Interval, Sleep, interval_at, sleep};
use uuid::Uuid;

use crate::{
    handlers::auth::rule_context,
    response::{ApiFailure, error_value, parse_error},
    router::{ApiState, RealtimePermit},
};

#[handler]
pub async fn subscribe(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let collection = req
        .param::<String>("collection")
        .ok_or_else(|| parse_error("missing path parameter 'collection'"))?;
    let filter = req.query::<String>("filter");
    let (identity, expires_at) = authenticate(req, state).await?;
    let ip = req
        .remote_addr()
        .ip()
        .map_or_else(|| "unknown".into(), |ip| ip.to_string());
    let permit = state.realtime.try_acquire(ip)?;
    let subscription = RealtimeManager::new(&state.db)
        .subscribe(
            &collection,
            filter.as_deref(),
            &rule_context(&identity, Value::Null),
        )
        .await?;

    let heartbeat = Duration::from_secs(state.config.realtime.heartbeat_seconds);
    let stream_state = StreamState {
        subscription_id: Uuid::now_v7().to_string(),
        collection,
        subscription,
        _db: state.db.clone(),
        _permit: permit,
        heartbeat: interval_at(Instant::now() + heartbeat, heartbeat),
        expires: expires_at.map(expiry_sleep),
        phase: Phase::Connected,
        dev_mode: state.config.server.dev_mode,
    };
    let events = stream::unfold(stream_state, next_event);
    res.headers_mut().insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    sse::stream(res, events);
    Ok(())
}

async fn authenticate(req: &Request, state: &ApiState) -> HbResult<(AuthIdentity, Option<u64>)> {
    let token = if let Some(header) = req.headers().get("authorization") {
        let header = header.to_str().map_err(|_| HbError::AuthRequired)?;
        Some(
            header
                .strip_prefix("Bearer ")
                .filter(|token| !token.is_empty() && !token.contains(char::is_whitespace))
                .ok_or(HbError::AuthRequired)?
                .to_owned(),
        )
    } else {
        req.query::<String>("token")
    };
    let Some(token) = token else {
        return Ok((AuthIdentity::Anonymous, None));
    };
    let authentication = state.auth.authenticate_with_expiry(&token).await?;
    Ok((authentication.identity, Some(authentication.expires_at)))
}

struct StreamState {
    subscription_id: String,
    collection: String,
    subscription: RealtimeSubscription,
    _db: DbClient,
    _permit: RealtimePermit,
    heartbeat: Interval,
    expires: Option<Pin<Box<Sleep>>>,
    phase: Phase,
    dev_mode: bool,
}

enum Phase {
    Connected,
    Streaming,
    Done,
}

async fn next_event(mut state: StreamState) -> Option<(Result<SseEvent, Infallible>, StreamState)> {
    match state.phase {
        Phase::Connected => {
            state.phase = Phase::Streaming;
            let event = event(
                "connected",
                json!({
                    "subscriptionId": state.subscription_id,
                    "collection": state.collection,
                    "timestamp": timestamp()
                }),
            );
            return Some((Ok(event), state));
        }
        Phase::Done => return None,
        Phase::Streaming => {}
    }

    let outcome = {
        let subscription = &mut state.subscription;
        let heartbeat = &mut state.heartbeat;
        let expires = &mut state.expires;
        tokio::select! {
            biased;
            _ = wait_for_expiry(expires) => Outcome::Expired,
            notification = subscription.next() => Outcome::Notification(notification),
            _ = heartbeat.tick() => Outcome::Heartbeat,
        }
    };
    let event = match outcome {
        Outcome::Expired => {
            state.phase = Phase::Done;
            event("error", error_value(&HbError::TokenExpired, state.dev_mode))
        }
        Outcome::Heartbeat => event("ping", json!({"timestamp": timestamp()})),
        Outcome::Notification(Ok(Some(notification))) => {
            let action = match notification.action {
                RealtimeAction::Create => "create",
                RealtimeAction::Update => "update",
                RealtimeAction::Delete => "delete",
            };
            let record_id = notification
                .record
                .get("id")
                .cloned()
                .unwrap_or(Value::Null);
            event(
                action,
                json!({
                    "id": record_id,
                    "action": action,
                    "record": notification.record,
                    "timestamp": timestamp()
                }),
            )
            .id(Uuid::now_v7().to_string())
        }
        Outcome::Notification(Ok(None)) => return None,
        Outcome::Notification(Err(error)) => {
            state.phase = Phase::Done;
            event("error", error_value(&error, state.dev_mode))
        }
    };
    Some((Ok(event), state))
}

enum Outcome {
    Expired,
    Heartbeat,
    Notification(HbResult<Option<herta_db::RealtimeEvent>>),
}

async fn wait_for_expiry(expires: &mut Option<Pin<Box<Sleep>>>) {
    match expires {
        Some(expires) => expires.as_mut().await,
        None => pending().await,
    }
}

fn expiry_sleep(expires_at: u64) -> Pin<Box<Sleep>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Box::pin(sleep(Duration::from_secs(expires_at.saturating_sub(now))))
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn event(name: &'static str, data: Value) -> SseEvent {
    SseEvent::default()
        .name(name)
        .json(data)
        .expect("serde_json::Value always serializes")
}

fn state(depot: &Depot) -> Result<&ApiState, ApiFailure> {
    depot
        .get_typed::<ApiState>()
        .map_err(|_| ApiFailure(HbError::Internal))
}
