use herta_db::collections::CollectionService;
use salvo::prelude::*;
use serde_json::{Value, json};

use crate::{
    handlers::auth::require_admin,
    response::{ApiFailure, ApiResponse, parse_error},
    router::SharedApiState,
};

#[handler]
pub async fn list(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| internal_state())?;
    require_admin(req, state).await?;
    let collections = CollectionService::new(&state.db).list().await?;
    res.render(Json(ApiResponse::ok(collections)));
    Ok(())
}

#[handler]
pub async fn create(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| internal_state())?;
    require_admin(req, state).await?;
    let definition: Value = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let created = mutate(req, depot, "create", definition, false).await?;
    res.status_code(StatusCode::CREATED);
    res.render(Json(ApiResponse::ok(created)));
    Ok(())
}

#[handler]
pub async fn get(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| internal_state())?;
    require_admin(req, state).await?;
    let name = path(req, "name")?;
    let collection = CollectionService::new(&state.db).get(&name).await?;
    res.render(Json(ApiResponse::ok(collection)));
    Ok(())
}

#[handler]
pub async fn update(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| internal_state())?;
    require_admin(req, state).await?;
    let name = path(req, "name")?;
    let mut patch: Value = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    patch
        .as_object_mut()
        .ok_or_else(|| ApiFailure(herta_core::HbError::validation("patch must be an object")))?
        .insert("name".into(), name.into());
    let updated = mutate(req, depot, "update", patch, true).await?;
    res.render(Json(ApiResponse::ok(updated)));
    Ok(())
}

#[handler]
pub async fn delete(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| internal_state())?;
    require_admin(req, state).await?;
    let name = path(req, "name")?;
    mutate(req, depot, "delete", name.clone().into(), false).await?;
    res.render(Json(ApiResponse::ok(
        json!({"name": name, "deleted": true}),
    )));
    Ok(())
}

async fn mutate(
    req: &Request,
    depot: &Depot,
    action: &str,
    value: Value,
    patch: bool,
) -> Result<Value, ApiFailure> {
    use herta_core::extension::{AuthMode, Invocation};
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| internal_state())?;
    let identity = require_admin(req, state).await?;
    if let Some(dispatcher) = super::extensions::pinned(depot, state) {
        let mut context = crate::extensions::request_context(&identity, value.clone());
        context.mode = AuthMode::System;
        return Ok(dispatcher
            .dispatch(
                Invocation {
                    name: format!("collection.{action}"),
                    payload: json!({"value":value,"patch":patch}),
                    auth_mode: AuthMode::System,
                    request_id: Some(uuid::Uuid::now_v7().to_string()),
                    registration: None,
                },
                state
                    .extensions
                    .as_ref()
                    .ok_or_else(internal_state)?
                    .hosts
                    .create(context),
            )
            .await?);
    }
    let result = CollectionService::apply(
        state.db.clone(),
        identity.record_id().map(str::to_owned),
        action.into(),
        value,
        patch,
    )
    .await?;
    // DDL already committed. Failed post-actions are retained for restart and
    // cannot turn a confirmed successful schema operation into an HTTP error.
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        crate::extensions::reconcile_collections(&state.db, state.storage.as_ref(), &state.docs),
    )
    .await
    {
        Ok(Ok(())) => {}
        outcome => tracing::error!(?outcome, "collection post-commit work remains pending"),
    }
    Ok(result)
}

fn path(req: &Request, name: &str) -> Result<String, ApiFailure> {
    req.param::<String>(name)
        .ok_or_else(|| parse_error(format!("missing path parameter '{name}'")))
}

fn internal_state() -> ApiFailure {
    ApiFailure(herta_core::HbError::Internal)
}
