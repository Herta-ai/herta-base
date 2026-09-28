use crate::{
    files::FileService,
    handlers::auth::require_admin,
    response::{ApiFailure, ApiResponse, parse_error},
    router::SharedApiState,
};
use herta_core::{HbError, files::FileResolve};
use salvo::prelude::*;

fn service(state: &SharedApiState) -> FileService {
    FileService::new(
        state.db.clone(),
        state.storage.clone(),
        state.config.jsvm.files.clone(),
    )
}

#[handler]
pub async fn list(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| HbError::Internal)?;
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
    res.render(Json(ApiResponse::ok(
        service(state).operations(&cursor, limit).await?,
    )));
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
        .map_err(|_| HbError::Internal)?;
    require_admin(req, state).await?;
    let version = req.param::<String>("version").ok_or(HbError::NotFound)?;
    res.render(Json(ApiResponse::ok(
        service(state).operation(&version).await?,
    )));
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
        .map_err(|_| HbError::Internal)?;
    let admin = require_admin(req, state).await?;
    let version = req.param::<String>("version").ok_or(HbError::NotFound)?;
    let input: FileResolve = req
        .parse_json_with_max_size(4096)
        .await
        .map_err(parse_error)?;
    let receipt = service(state)
        .resolve(
            &version,
            input,
            admin.record_id().ok_or(HbError::Unauthorized)?.into(),
        )
        .await?;
    res.render(Json(ApiResponse::ok(receipt)));
    Ok(())
}
