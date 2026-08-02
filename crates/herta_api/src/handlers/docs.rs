use salvo::prelude::*;

use crate::{handlers::auth::require_admin, response::ApiFailure, router::ApiState};

#[handler]
pub async fn openapi(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<ApiState>()
        .map_err(|_| ApiFailure(herta_core::HbError::Internal))?;
    require_admin(req, state).await?;
    res.render(Json(state.docs.read().await));
    Ok(())
}
