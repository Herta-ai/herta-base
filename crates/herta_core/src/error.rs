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
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Request payload too large")]
    PayloadTooLarge,
    #[error("Database error: {0}")]
    Database(String),
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
            Self::NotFound | Self::CollectionNotFound(_) => 404,
            Self::Conflict(_) => 409,
            Self::PayloadTooLarge => 413,
            Self::Database(_) | Self::Internal => 500,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "HB_VALIDATION_ERROR",
            Self::InvalidFilter(_) => "HB_INVALID_FILTER",
            Self::InvalidSort(_) => "HB_INVALID_SORT",
            Self::NotFound => "HB_NOT_FOUND",
            Self::CollectionNotFound(_) => "HB_COLLECTION_NOT_FOUND",
            Self::Conflict(_) => "HB_CONFLICT",
            Self::PayloadTooLarge => "HB_PAYLOAD_TOO_LARGE",
            Self::Database(_) => "HB_DB_ERROR",
            Self::Internal => "HB_INTERNAL_ERROR",
        }
    }

    pub fn public_message(&self, dev_mode: bool) -> String {
        match self {
            Self::Database(message) if dev_mode => format!("Database error: {message}"),
            Self::Database(_) | Self::Internal => "Internal server error".into(),
            _ => self.to_string(),
        }
    }
}
