use crate::{
    handlers::auth::require_admin,
    response::{ApiFailure, ApiResponse, parse_error},
    router::SharedApiState,
};
use herta_core::{HbError, outbox::OutboxResolve};
use salvo::prelude::*;

#[handler]
pub async fn list(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| ApiFailure(HbError::Internal))?;
    require_admin(req, state).await?;
    let limit = req
        .query::<String>("limit")
        .map(|v| {
            v.parse::<usize>()
                .map_err(|_| HbError::validation("invalid limit"))
        })
        .transpose()?
        .unwrap_or(100);
    let cursor = req.query::<String>("cursor").unwrap_or_default();
    let page = herta_db::outbox::list(&state.db, &cursor, limit).await?;
    res.render(Json(ApiResponse::ok(page)));
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
        .map_err(|_| ApiFailure(HbError::Internal))?;
    require_admin(req, state).await?;
    let id = req.param::<String>("id").ok_or(HbError::NotFound)?;
    let job = herta_db::outbox::get((&state.db).into(), &id).await?;
    res.render(Json(ApiResponse::ok(job.receipt)));
    Ok(())
}
#[handler]
pub async fn resolve(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| ApiFailure(HbError::Internal))?;
    let admin = require_admin(req, state).await?;
    let id = req.param::<String>("id").ok_or(HbError::NotFound)?;
    let input: OutboxResolve = req
        .parse_json_with_max_size(4096)
        .await
        .map_err(parse_error)?;
    let receipt = herta_db::outbox::resolve(
        &state.db,
        &id,
        input,
        admin.record_id().unwrap_or("_admins").into(),
    )
    .await?;
    res.render(Json(ApiResponse::ok(receipt)));
    Ok(())
}
