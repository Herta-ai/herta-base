use salvo::prelude::*;

use crate::{response::ApiFailure, router::ApiState};

#[handler]
pub async fn openapi(depot: &mut Depot, res: &mut Response) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<ApiState>()
        .map_err(|_| ApiFailure(herta_core::HbError::Internal))?;
    res.render(Json(state.docs.read().await));
    Ok(())
}
