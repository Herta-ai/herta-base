use herta_db::{ListParams, RecordManager};
use salvo::prelude::*;
use serde_json::{Value, json};

use crate::{
    handlers::auth::{identity, rule_context},
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
    let identity = identity(req, state).await?;
    let collection = path(req, "collection")?;
    let params = ListParams {
        page: req.query("page"),
        per_page: req.query("perPage"),
        sort: req.query("sort"),
        filter: req.query("filter"),
        expand: req.query("expand"),
    };
    let (records, total) = RecordManager::new(&state.db)
        .list_authorized(&collection, &params, &rule_context(&identity, Value::Null))
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
    let identity = identity(req, state).await?;
    let record = RecordManager::new(&state.db)
        .get_authorized(
            &path(req, "collection")?,
            &path(req, "id")?,
            req.query::<String>("expand").as_deref(),
            &rule_context(&identity, Value::Null),
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
    let identity = identity(req, state).await?;
    let collection = path(req, "collection")?;
    let body: Value = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let record = RecordManager::new(&state.db)
        .create_authorized(&collection, body.clone(), &rule_context(&identity, body))
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
    let identity = identity(req, state).await?;
    let collection = path(req, "collection")?;
    let id = path(req, "id")?;
    let body: Value = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let record = RecordManager::new(&state.db)
        .update_authorized(
            &collection,
            &id,
            body.clone(),
            &rule_context(&identity, body),
        )
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
    let identity = identity(req, state).await?;
    let record = RecordManager::new(&state.db)
        .delete_authorized(
            &path(req, "collection")?,
            &path(req, "id")?,
            &rule_context(&identity, Value::Null),
        )
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
