use super::realtime::{event, timestamp};
use crate::{
    messages::MessageSubscription,
    response::{ApiFailure, error_value},
    router::{RealtimePermit, SharedApiState},
};
use herta_core::{HbError, JsErrorKind};
use salvo::{
    prelude::*,
    sse::{self, SseEvent},
};
use serde_json::json;
use std::{convert::Infallible, time::Duration};

#[handler]
pub async fn subscribe(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| HbError::Internal)?;
    if !state.config.jsvm.enabled || !state.config.jsvm.realtime.enabled {
        return Err(HbError::from(JsErrorKind::Denied).into());
    }
    let token = super::realtime::token(req)?.ok_or(HbError::AuthRequired)?;
    let topic = req
        .query::<String>("topic")
        .ok_or_else(|| HbError::validation("missing message topic"))?;
    let ip = req
        .remote_addr()
        .ip()
        .map_or_else(|| "unknown".into(), |ip| ip.to_string());
    let permit = state.realtime.try_acquire(ip)?;
    let subscription = state.messages.subscribe(topic, token).await?;
    let heartbeat = Duration::from_secs(state.config.realtime.heartbeat_seconds);
    let stream = StreamState {
        subscription,
        _permit: permit,
        heartbeat: tokio::time::interval_at(tokio::time::Instant::now() + heartbeat, heartbeat),
        phase: 0,
        dev: state.config.server.dev_mode,
    };
    res.headers_mut()
        .insert("x-accel-buffering", "no".parse().unwrap());
    res.headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    sse::stream(res, futures_util::stream::unfold(stream, next));
    Ok(())
}
struct StreamState {
    subscription: MessageSubscription,
    _permit: RealtimePermit,
    heartbeat: tokio::time::Interval,
    phase: u8,
    dev: bool,
}
async fn next(mut state: StreamState) -> Option<(Result<SseEvent, Infallible>, StreamState)> {
    if state.phase == 2 {
        return None;
    }
    if state.phase == 0 {
        state.phase = 1;
        let connected = event(
            "connected",
            json!({"connectionId":state.subscription.id(),"topic":state.subscription.topic(),"timestamp":timestamp()}),
        );
        return Some((Ok(connected), state));
    }
    let result = tokio::select! {
        biased;
        message = state.subscription.next() => match message {
            Ok(Some(message)) => Ok(Some(event("message", serde_json::to_value(message.as_ref()).expect("message JSON")).id(message.id.clone()))),
            Ok(None) => return None,
            Err(error) => Err(error),
        },
        _ = state.heartbeat.tick() => state.subscription.check().await.map(|_| Some(event("ping", json!({"timestamp":timestamp()})))),
    };
    let event = match result {
        Ok(event) => event?,
        Err(error) => {
            state.phase = 2;
            event("error", error_value(&error, state.dev))
        }
    };
    Some((Ok(event), state))
}
