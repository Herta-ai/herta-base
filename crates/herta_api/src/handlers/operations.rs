use crate::{
    response::{ApiFailure, ApiResponse},
    router::SharedApiState,
};
use herta_core::HbError;
use salvo::prelude::*;

#[handler]
pub async fn get(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| ApiFailure(HbError::Internal))?;
    let id = req.param::<String>("id").ok_or(HbError::NotFound)?;
    // The alternate credential stays out of URLs, referrers and access logs.
    let credential = req
        .headers()
        .get("x-hb-operation-credential")
        .and_then(|value| value.to_str().ok());
    let identity = match super::auth::identity(req, state).await {
        Ok(identity) => Some(identity),
        Err(_) if credential.is_some() => None,
        Err(error) => return Err(error.into()),
    };
    let status = herta_db::transaction::operation_status(
        &state.db,
        &id,
        identity
            .as_ref()
            .and_then(herta_auth::AuthIdentity::record_id),
        credential,
    )
    .await?;
    res.headers_mut().insert(
        "cache-control",
        "no-store".parse().map_err(|_| HbError::Internal)?,
    );
    res.render(Json(ApiResponse::ok(status)));
    Ok(())
}
