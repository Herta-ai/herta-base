use herta_core::HbError;
use salvo::prelude::*;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: Option<T>,
    pub meta: Option<Value>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: u16,
    pub message: String,
    pub error: String,
    pub details: Option<Value>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            data: Some(data),
            meta: None,
            error: None,
        }
    }

    pub fn with_meta(data: T, meta: Value) -> Self {
        Self {
            data: Some(data),
            meta: Some(meta),
            error: None,
        }
    }
}

#[derive(Debug)]
pub struct ApiFailure(pub HbError);

impl From<HbError> for ApiFailure {
    fn from(value: HbError) -> Self {
        Self(value)
    }
}

#[async_trait]
impl Writer for ApiFailure {
    async fn write(self, _req: &mut Request, depot: &mut Depot, res: &mut Response) {
        let dev_mode = depot
            .get_typed::<crate::router::ApiState>()
            .map(|state| state.config.server.dev_mode)
            .unwrap_or(false);
        let status =
            StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let details = match &self.0 {
            HbError::Validation { details, .. } => details.clone(),
            _ => None,
        };
        let body = ApiResponse::<Value> {
            data: None,
            meta: None,
            error: Some(ApiError {
                code: self.0.status_code(),
                message: self.0.public_message(dev_mode),
                error: self.0.error_code().into(),
                details,
            }),
        };
        res.status_code(status);
        res.render(Json(body));
    }
}

pub fn parse_error(error: impl std::fmt::Display) -> ApiFailure {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("size")
        && message.to_ascii_lowercase().contains("limit")
    {
        ApiFailure(HbError::PayloadTooLarge)
    } else {
        ApiFailure(HbError::validation(message))
    }
}
