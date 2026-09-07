use herta_core::{HbError, MailMessage};
use salvo::prelude::*;

use crate::{
    handlers::auth::require_admin,
    response::{ApiFailure, ApiResponse, parse_error},
    router::SharedApiState,
};

/// Send a message through the configured SMTP service (administrator only).
#[handler]
pub async fn send(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<SharedApiState>()
        .map_err(|_| ApiFailure(HbError::Internal))?;
    require_admin(req, state).await?;
    let message: MailMessage = req
        .parse_json_with_max_size(state.config.server.max_body_size)
        .await
        .map_err(parse_error)?;
    let receipt = state.mailer.send(message).await?;
    res.render(Json(ApiResponse::ok(receipt)));
    Ok(())
}
