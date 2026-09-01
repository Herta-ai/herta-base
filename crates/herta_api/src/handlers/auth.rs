use herta_auth::{AuthIdentity, Credentials, RefreshRequest};
use herta_core::{HbError, HbResult};
use herta_db::RuleContext;
use salvo::prelude::*;
use serde_json::Value;

use crate::{
    response::{ApiFailure, ApiResponse, parse_error},
    router::{ApiState, SharedApiState},
};

#[handler]
pub async fn register_default(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    register(req, depot, res, "_users").await
}

#[handler]
pub async fn register_collection(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let collection = path(req, "collection")?;
    register(req, depot, res, &collection).await
}

async fn register(
    req: &mut Request,
    depot: &Depot,
    res: &mut Response,
    collection: &str,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    state.auth.check_register_rate(&client_key(req))?;
    let body: Credentials = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let auth = state.auth.register(collection, body).await?;
    res.status_code(StatusCode::CREATED);
    res.render(Json(ApiResponse::ok(auth)));
    Ok(())
}

#[handler]
pub async fn login_default(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    login(req, depot, res, "_users", false).await
}

#[handler]
pub async fn login_collection(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let collection = path(req, "collection")?;
    login(req, depot, res, &collection, false).await
}

#[handler]
pub async fn login_admin(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    login(req, depot, res, "_admins", true).await
}

async fn login(
    req: &mut Request,
    depot: &Depot,
    res: &mut Response,
    collection: &str,
    admin: bool,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    state.auth.check_login_rate(&client_key(req))?;
    let body: Credentials = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let auth = state.auth.login(collection, body, admin).await?;
    res.render(Json(ApiResponse::ok(auth)));
    Ok(())
}

#[handler]
pub async fn refresh_default(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    refresh(req, depot, res, false, Some("_users")).await
}

#[handler]
pub async fn refresh_collection(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let collection = path(req, "collection")?;
    refresh(req, depot, res, false, Some(&collection)).await
}

#[handler]
pub async fn refresh_admin(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    refresh(req, depot, res, true, Some("_admins")).await
}

async fn refresh(
    req: &mut Request,
    depot: &Depot,
    res: &mut Response,
    admin: bool,
    collection: Option<&str>,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    state.auth.check_refresh_rate(&client_key(req))?;
    let body: RefreshRequest = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let auth = state
        .auth
        .refresh(&body.refresh_token, admin, collection)
        .await?;
    res.render(Json(ApiResponse::ok(auth)));
    Ok(())
}

#[handler]
pub async fn me_default(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    me_user(req, depot, res, "_users").await
}

#[handler]
pub async fn me_collection(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let collection = path(req, "collection")?;
    me_user(req, depot, res, &collection).await
}

async fn me_user(
    req: &mut Request,
    depot: &Depot,
    res: &mut Response,
    collection: &str,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let identity = identity(req, state).await?;
    if !matches!(&identity, AuthIdentity::User { collection: token_collection, .. } if token_collection == collection)
    {
        return Err(ApiFailure(HbError::Forbidden));
    }
    res.render(Json(ApiResponse::ok(state.auth.me(&identity).await?)));
    Ok(())
}

#[handler]
pub async fn me_admin(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let identity = require_admin(req, state).await?;
    res.render(Json(ApiResponse::ok(state.auth.me(&identity).await?)));
    Ok(())
}

pub async fn identity(req: &Request, state: &ApiState) -> HbResult<AuthIdentity> {
    let Some(header) = req.headers().get("authorization") else {
        return Ok(AuthIdentity::Anonymous);
    };
    let header = header.to_str().map_err(|_| HbError::Unauthorized)?;
    let Some(token) = header.strip_prefix("Bearer ") else {
        return Err(HbError::Unauthorized);
    };
    if token.is_empty() || token.contains(char::is_whitespace) {
        return Err(HbError::Unauthorized);
    }
    state.auth.authenticate(token).await
}

pub async fn require_admin(req: &Request, state: &ApiState) -> HbResult<AuthIdentity> {
    let identity = identity(req, state).await?;
    if identity.is_admin() {
        Ok(identity)
    } else if matches!(identity, AuthIdentity::Anonymous) {
        Err(HbError::AuthRequired)
    } else {
        Err(HbError::Forbidden)
    }
}

pub fn rule_context(identity: &AuthIdentity, body: Value) -> RuleContext {
    RuleContext {
        admin: identity.is_admin(),
        auth: identity.as_rule_value(),
        auth_record: identity
            .record_id()
            .and_then(|id| herta_db::record::parse_record_id(id).ok()),
        request_body: body,
    }
}

fn client_key(req: &Request) -> String {
    req.remote_addr()
        .ip()
        .map_or_else(|| "unknown".into(), |ip| ip.to_string())
}

fn state(depot: &Depot) -> Result<&ApiState, ApiFailure> {
    depot
        .get_typed::<SharedApiState>()
        .map(AsRef::as_ref)
        .map_err(|_| ApiFailure(HbError::Internal))
}

fn path(req: &Request, name: &str) -> Result<String, ApiFailure> {
    req.param::<String>(name)
        .ok_or_else(|| parse_error(format!("missing path parameter '{name}'")))
}
