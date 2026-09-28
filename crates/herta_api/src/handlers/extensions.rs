use crate::{
    extensions::request_context,
    response::{ApiFailure, parse_error},
    router::{ApiState, SharedApiState},
};
use herta_auth::AuthIdentity;
use herta_core::{
    HbError, HbResult, JsErrorKind,
    extension::{AuthMode, EventDispatcher, Invocation},
    host_buffer::HostReply,
    routes::{CustomRoute, NativeMiddleware, RateKey, normalize_path},
};
use salvo::http::Method;
use salvo::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Lives in ApiState, independent of registry revisions. Active keys are never evicted.
#[derive(Default)]
pub struct RouteLimits(Mutex<HashMap<String, Window>>);
struct Window {
    expires: Instant,
    hits: VecDeque<Instant>,
}
impl RouteLimits {
    fn check(&self, key: String, limit: usize, window: Duration, capacity: usize) -> HbResult<()> {
        let now = Instant::now();
        let mut keys = self.0.lock().map_err(|_| HbError::Internal)?;
        keys.retain(|_, entry| entry.expires > now);
        if !keys.contains_key(&key) && keys.len() >= capacity {
            return Err(HbError::RateLimited);
        }
        let entry = keys.entry(key).or_insert_with(|| Window {
            expires: now + window,
            hits: VecDeque::new(),
        });
        while entry
            .hits
            .front()
            .is_some_and(|time| now.duration_since(*time) >= window)
        {
            entry.hits.pop_front();
        }
        if entry.hits.len() >= limit {
            return Err(HbError::RateLimited);
        }
        entry.hits.push_back(now);
        entry.expires = entry.expires.max(now + window);
        Ok(())
    }
}

#[derive(Clone)]
pub struct RequestSnapshot(pub Arc<dyn EventDispatcher>);

/// Install as a Service hoop after the state injector, including for unmatched paths.
#[handler]
pub async fn dispatch(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    match route(req, depot, res).await {
        Ok(false) => {}
        Ok(true) => ctrl.skip_rest(),
        Err(error) => {
            error.write(req, depot, res).await;
            ctrl.skip_rest();
        }
    }
}

async fn route(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<bool, ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| ApiFailure(HbError::Internal))?
        .clone();
    let Some(extensions) = &state.extensions else {
        return Ok(false);
    };
    let path = normalize_path(req.uri().path())?;
    let snapshot = extensions.dispatcher.clone().pin();
    depot.insert_typed(RequestSnapshot(snapshot.clone()));
    let routes = snapshot
        .registrations()
        .iter()
        .filter(|item| item.kind == "route")
        .map(|item| CustomRoute::parse(item, &state.config.jsvm))
        .collect::<HbResult<Vec<_>>>()?;
    let matching: Vec<_> = routes
        .iter()
        .filter_map(|route| route.match_path(&path).map(|params| (route, params)))
        .collect();
    if matching.is_empty() {
        return Ok(false);
    }
    let identity = super::auth::identity(req, &state).await?;
    let mut allow: BTreeSet<String> = matching
        .iter()
        .map(|(route, _)| route.method.clone())
        .collect();
    if allow.contains("GET") {
        allow.insert("HEAD".into());
    }
    allow.insert("OPTIONS".into());
    let allow = allow.into_iter().collect::<Vec<_>>().join(", ");
    if req.method() == Method::OPTIONS {
        res.status_code(StatusCode::NO_CONTENT);
        res.headers_mut().insert(
            "allow",
            allow.parse().map_err(|_| ApiFailure(HbError::Internal))?,
        );
        return Ok(true);
    }
    let selected = matching
        .iter()
        .find(|(route, _)| route.method == req.method().as_str())
        .or_else(|| {
            (req.method() == Method::HEAD)
                .then(|| matching.iter().find(|(route, _)| route.method == "GET"))
                .flatten()
        });
    let Some((route, params)) = selected else {
        res.status_code(StatusCode::METHOD_NOT_ALLOWED);
        res.headers_mut().insert(
            "allow",
            allow.parse().map_err(|_| ApiFailure(HbError::Internal))?,
        );
        return Ok(true);
    };
    let mut limit = state.config.server.max_body_size;
    for (index, middleware) in route.middleware.iter().enumerate() {
        match middleware {
            NativeMiddleware::RequireAuth if matches!(identity, AuthIdentity::Anonymous) => {
                return Err(HbError::AuthRequired.into());
            }
            NativeMiddleware::RequireAdmin if !identity.is_admin() => {
                return Err(if matches!(identity, AuthIdentity::Anonymous) {
                    HbError::AuthRequired
                } else {
                    HbError::Forbidden
                }
                .into());
            }
            NativeMiddleware::BodyLimit { bytes } => {
                limit = limit.min(*bytes);
            }
            NativeMiddleware::RateLimit {
                limit,
                window_ms,
                key,
            } => {
                let ip = req
                    .remote_addr()
                    .ip()
                    .map_or_else(|| "unknown".into(), |ip| ip.to_string());
                let key = match key {
                    RateKey::Ip => ip,
                    RateKey::Auth => identity.record_id().map(str::to_owned).unwrap_or(ip),
                };
                state.extension_limits.check(
                    format!("{} {} {index} {key}", route.method, route.path),
                    *limit,
                    Duration::from_millis(*window_ms),
                    state.config.jsvm.max_rate_limit_keys,
                )?;
            }
            _ => {}
        }
    }
    let body = req
        .payload_with_max_size(limit)
        .await
        .map_err(parse_error)?
        .to_vec();
    let request = request_view(req, &path, params, &body);
    let request_body = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let value = snapshot
        .dispatch_frame(
            Invocation {
                name: "route".into(),
                payload: json!({"request":request,"auth":identity.as_rule_value()}),
                auth_mode: AuthMode::Request,
                request_id: Some(uuid::Uuid::now_v7().to_string()),
                registration: Some(route.id),
            },
            extensions
                .hosts
                .create(request_context(&identity, request_body)),
            Some(body),
        )
        .await?;
    render_frame(value, req, res, state.config.jsvm.max_response_bytes)?;
    Ok(true)
}

pub fn request_view(
    req: &Request,
    path: &str,
    params: &BTreeMap<String, String>,
    body: &[u8],
) -> Value {
    let headers: BTreeMap<_, _> = req
        .headers()
        .iter()
        .filter(|(name, _)| {
            !matches!(
                name.as_str(),
                "authorization" | "cookie" | "proxy-authorization" | "x-hb-operation-credential"
            )
        })
        .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str(), value)))
        .collect();
    let query: BTreeMap<_, _> = req.queries().iter().collect();
    json!({"method":req.method().as_str(),"path":path,"params":params,"query":query,"headers":headers,
        "validUtf8":std::str::from_utf8(body).is_ok()})
}

pub fn pinned(depot: &Depot, state: &ApiState) -> Option<Arc<dyn EventDispatcher>> {
    depot
        .get_typed::<RequestSnapshot>()
        .ok()
        .map(|value| value.0.clone())
        .or_else(|| {
            state
                .extensions
                .as_ref()
                .map(|extensions| extensions.dispatcher.clone().pin())
        })
}

#[derive(Deserialize)]
struct ExtensionResponse {
    status: u16,
    kind: String,
    body: Value,
    #[serde(default)]
    headers: BTreeMap<String, String>,
}

pub fn render_response(
    value: Value,
    req: &Request,
    res: &mut Response,
    maximum: usize,
) -> HbResult<()> {
    render_frame(value.into(), req, res, maximum)
}

pub fn render_frame(
    frame: HostReply,
    req: &Request,
    res: &mut Response,
    maximum: usize,
) -> HbResult<()> {
    let response: ExtensionResponse =
        serde_json::from_value(frame.value).map_err(|_| HbError::from(JsErrorKind::Hook))?;
    if frame.bytes.is_some() && response.kind != "bytes" {
        return Err(JsErrorKind::Hook.into());
    }
    let status =
        StatusCode::from_u16(response.status).map_err(|_| HbError::from(JsErrorKind::Hook))?;
    if response.status < 200
        || (response.status == 204 && response.kind != "empty")
        || response.status == 304
    {
        return Err(JsErrorKind::Hook.into());
    }
    let (body, content_type) = match response.kind.as_str() {
        "empty" if status == StatusCode::NO_CONTENT => (bytes::Bytes::new(), None),
        "json" => (
            bytes::Bytes::from(serde_json::to_vec(&response.body).map_err(|_| HbError::Internal)?),
            Some("application/json; charset=utf-8"),
        ),
        "text" | "html" => (
            bytes::Bytes::copy_from_slice(
                response.body.as_str().ok_or(HbError::Internal)?.as_bytes(),
            ),
            Some(if response.kind == "text" {
                "text/plain; charset=utf-8"
            } else {
                "text/html; charset=utf-8"
            }),
        ),
        "bytes" => (
            match frame.bytes {
                Some(bytes) => bytes::Bytes::from_owner(bytes),
                None => bytes::Bytes::from(
                    serde_json::from_value::<Vec<u8>>(response.body)
                        .map_err(|_| HbError::Internal)?,
                ),
            },
            Some("application/octet-stream"),
        ),
        _ => return Err(JsErrorKind::Hook.into()),
    };
    if body.len() > maximum {
        return Err(HbError::PayloadTooLarge);
    }
    let mut headers = salvo::http::HeaderMap::new();
    let mut header_bytes: usize = 0;
    for (name, value) in response.headers {
        let name: salvo::http::header::HeaderName = name
            .parse()
            .map_err(|_| HbError::validation("invalid response header"))?;
        if matches!(
            name.as_str(),
            "connection"
                | "keep-alive"
                | "proxy-authenticate"
                | "proxy-authorization"
                | "te"
                | "trailer"
                | "transfer-encoding"
                | "upgrade"
                | "content-length"
                | "set-cookie"
        ) {
            return Err(HbError::validation("protected response header"));
        }
        header_bytes = header_bytes
            .saturating_add(name.as_str().len())
            .saturating_add(value.len());
        if header_bytes > 8192 {
            return Err(HbError::PayloadTooLarge);
        }
        headers.insert(
            name,
            value
                .parse()
                .map_err(|_| HbError::validation("invalid response header"))?,
        );
    }
    if let Some(content_type) = content_type {
        headers
            .entry("content-type")
            .or_insert(salvo::http::HeaderValue::from_static(content_type));
    }
    res.status_code(status);
    res.headers_mut().extend(headers);
    if req.method() != Method::HEAD && status != StatusCode::NO_CONTENT {
        res.write_body(body).map_err(|_| HbError::Internal)?;
    }
    Ok(())
}
