use herta_db::{ListParams, RecordManager};
use salvo::prelude::*;
use serde_json::{Value, json};

use crate::{
    response::{ApiFailure, ApiResponse, parse_error},
    router::ApiState,
};

#[handler]
pub async fn list(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let collection = path(req, "collection")?;
    let params = ListParams {
        page: req.query("page"),
        per_page: req.query("perPage"),
        sort: req.query("sort"),
        filter: req.query("filter"),
        expand: req.query("expand"),
    };
    let (records, total) = RecordManager::new(&state.db)
        .list(&collection, &params)
        .await?;
    res.render(Json(ApiResponse::with_meta(
        records,
        json!({"total": total, "page": params.page(), "perPage": params.per_page()}),
    )));
    Ok(())
}

#[handler]
pub async fn get(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let record = RecordManager::new(&state.db)
        .get(
            &path(req, "collection")?,
            &path(req, "id")?,
            req.query::<String>("expand").as_deref(),
        )
        .await?;
    res.render(Json(ApiResponse::ok(record)));
    Ok(())
}

#[handler]
pub async fn create(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let collection = path(req, "collection")?;
    let body: Value = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let record = RecordManager::new(&state.db)
        .create(&collection, body)
        .await?;
    res.status_code(StatusCode::CREATED);
    res.render(Json(ApiResponse::ok(record)));
    Ok(())
}

#[handler]
pub async fn update(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let collection = path(req, "collection")?;
    let id = path(req, "id")?;
    let body: Value = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let record = RecordManager::new(&state.db)
        .update(&collection, &id, body)
        .await?;
    res.render(Json(ApiResponse::ok(record)));
    Ok(())
}

#[handler]
pub async fn delete(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = state(depot)?;
    let record = RecordManager::new(&state.db)
        .delete(&path(req, "collection")?, &path(req, "id")?)
        .await?;
    res.render(Json(ApiResponse::ok(record)));
    Ok(())
}

fn state(depot: &Depot) -> Result<&ApiState, ApiFailure> {
    depot
        .get_typed::<ApiState>()
        .map_err(|_| ApiFailure(herta_core::HbError::Internal))
}

fn path(req: &Request, name: &str) -> Result<String, ApiFailure> {
    req.param::<String>(name)
        .ok_or_else(|| parse_error(format!("missing path parameter '{name}'")))
}
