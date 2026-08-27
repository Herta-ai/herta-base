use serde_json::Value;
use thiserror::Error;

pub type HbResult<T> = Result<T, HbError>;

#[derive(Debug, Error)]
pub enum HbError {
    #[error("Validation failed: {message}")]
    Validation {
        message: String,
        details: Option<Value>,
    },
    #[error("Invalid filter expression: {0}")]
    InvalidFilter(String),
    #[error("Invalid sort parameter: {0}")]
    InvalidSort(String),
    #[error("Record not found")]
    NotFound,
    #[error("Record not found")]
    RecordNotFound,
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Request payload too large")]
    PayloadTooLarge,
    #[error("Unsupported media type: {0}")]
    UnsupportedMediaType(String),
    #[error("Requested byte range cannot be satisfied")]
    RangeNotSatisfiable,
    #[error("Authentication required")]
    AuthRequired,
    #[error("Authentication failed")]
    Unauthorized,
    #[error("Authentication token has expired")]
    TokenExpired,
    #[error("Access is forbidden")]
    Forbidden,
    #[error("Rate limit exceeded")]
    RateLimited,
    #[error("Account is temporarily locked")]
    AccountLocked,
    #[error("Database error: {0}")]
    Database(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Internal error")]
    Internal,
}

impl HbError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
            details: None,
        }
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::Validation { .. } | Self::InvalidFilter(_) | Self::InvalidSort(_) => 400,
            Self::NotFound | Self::RecordNotFound | Self::CollectionNotFound(_) => 404,
            Self::Conflict(_) => 409,
            Self::PayloadTooLarge => 413,
            Self::UnsupportedMediaType(_) => 415,
            Self::RangeNotSatisfiable => 416,
            Self::AuthRequired | Self::Unauthorized | Self::TokenExpired => 401,
            Self::Forbidden => 403,
            Self::RateLimited => 429,
            Self::AccountLocked => 423,
            Self::Database(_) | Self::Storage(_) | Self::Internal => 500,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "HB_VALIDATION_ERROR",
            Self::InvalidFilter(_) => "HB_INVALID_FILTER",
            Self::InvalidSort(_) => "HB_INVALID_SORT",
            Self::NotFound => "HB_NOT_FOUND",
            Self::RecordNotFound => "HB_RECORD_NOT_FOUND",
            Self::CollectionNotFound(_) => "HB_COLLECTION_NOT_FOUND",
            Self::Conflict(_) => "HB_CONFLICT",
            Self::PayloadTooLarge => "HB_PAYLOAD_TOO_LARGE",
            Self::UnsupportedMediaType(_) => "HB_UNSUPPORTED_MEDIA_TYPE",
            Self::RangeNotSatisfiable => "HB_RANGE_NOT_SATISFIABLE",
            Self::AuthRequired => "HB_AUTH_REQUIRED",
            Self::Unauthorized => "HB_UNAUTHORIZED",
            Self::TokenExpired => "HB_TOKEN_EXPIRED",
            Self::Forbidden => "HB_FORBIDDEN",
            Self::RateLimited => "HB_RATE_LIMITED",
            Self::AccountLocked => "HB_ACCOUNT_LOCKED",
            Self::Database(_) => "HB_DB_ERROR",
            Self::Storage(_) => "HB_STORAGE_ERROR",
            Self::Internal => "HB_INTERNAL_ERROR",
        }
    }

    pub fn public_message(&self, dev_mode: bool) -> String {
        match self {
            Self::Database(message) if dev_mode => format!("Database error: {message}"),
            Self::Storage(message) if dev_mode => format!("Storage error: {message}"),
            Self::Database(_) | Self::Storage(_) | Self::Internal => "Internal server error".into(),
            _ => self.to_string(),
        }
    }
}
