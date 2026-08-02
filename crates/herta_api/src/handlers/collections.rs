use herta_db::{CollectionDef, SchemaManager, UpdateCollectionRequest};
use salvo::prelude::*;
use serde_json::json;

use crate::{
    response::{ApiFailure, ApiResponse, parse_error},
    router::ApiState,
};

#[handler]
pub async fn list(depot: &mut Depot, res: &mut Response) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<ApiState>()
        .map_err(|_| internal_state())?;
    let collections = SchemaManager::new(&state.db).list_collections().await?;
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
        .get_typed::<ApiState>()
        .map_err(|_| internal_state())?;
    let definition: CollectionDef = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let created = SchemaManager::new(&state.db)
        .create_collection(&definition)
        .await?;
    state.docs.refresh(state).await?;
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
        .get_typed::<ApiState>()
        .map_err(|_| internal_state())?;
    let name = path(req, "name")?;
    let collection = SchemaManager::new(&state.db).get_collection(&name).await?;
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
        .get_typed::<ApiState>()
        .map_err(|_| internal_state())?;
    let name = path(req, "name")?;
    let patch: UpdateCollectionRequest = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let updated = SchemaManager::new(&state.db)
        .update_collection(&name, &patch)
        .await?;
    state.docs.refresh(state).await?;
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
        .get_typed::<ApiState>()
        .map_err(|_| internal_state())?;
    let name = path(req, "name")?;
    SchemaManager::new(&state.db)
        .delete_collection(&name)
        .await?;
    state.docs.refresh(state).await?;
    res.render(Json(ApiResponse::ok(
        json!({"name": name, "deleted": true}),
    )));
    Ok(())
}

fn path(req: &Request, name: &str) -> Result<String, ApiFailure> {
    req.param::<String>(name)
        .ok_or_else(|| parse_error(format!("missing path parameter '{name}'")))
}

fn internal_state() -> ApiFailure {
    ApiFailure(herta_core::HbError::Internal)
}
