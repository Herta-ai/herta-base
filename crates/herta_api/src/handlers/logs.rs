use herta_core::HbError;
use herta_db::{LogManager, LogQuery};
use salvo::prelude::*;
use serde_json::json;

use crate::{
    handlers::auth::require_admin,
    response::{ApiFailure, ApiResponse},
    router::ApiState,
};

/// List persisted server and request logs for the administrator console.
#[handler]
pub async fn list(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
) -> Result<(), ApiFailure> {
    let state = depot
        .get_typed::<ApiState>()
        .map_err(|_| ApiFailure(HbError::Internal))?;
    require_admin(req, state).await?;

    let params = parse_query(req)?;
    let (logs, total) = LogManager::new(&state.db).list(&params).await?;
    res.render(Json(ApiResponse::with_meta(
        logs,
        json!({
            "total": total,
            "page": params.page(),
            "perPage": params.per_page(),
        }),
    )));
    Ok(())
}

fn parse_query(req: &Request) -> Result<LogQuery, ApiFailure> {
    Ok(LogQuery {
        page: parse_number(req, "page")?,
        per_page: parse_number(req, "perPage")?,
        level: req.query::<String>("level"),
        log_type: req.query::<String>("logType"),
        keyword: req.query::<String>("q"),
        target: req.query::<String>("target"),
        path: req.query::<String>("path"),
        status_code: parse_number(req, "statusCode")?,
        from: req.query::<String>("from"),
        to: req.query::<String>("to"),
    })
}

fn parse_number<T>(req: &Request, name: &str) -> Result<Option<T>, ApiFailure>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    req.query::<String>(name)
        .map(|raw| {
            raw.parse::<T>().map_err(|error| {
                ApiFailure(HbError::validation(format!(
                    "{name} must be a valid number: {error}"
                )))
            })
        })
        .transpose()
}
