//! Error types for the API layer

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// API-specific error types
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Authorization failed: {0}")]
    Authorization(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("Event processing error: {0}")]
    EventProcessing(#[from] ingest_events::error::EventError),

    #[error("Execution error: {0}")]
    Execution(#[from] ingest_execution::ExecutionError),

    #[error("State management error: {0}")]
    State(#[from] ingest_state::error::StateError),

    #[error("Configuration error: {0}")]
    Config(#[from] ingest_config::error::ConfigError),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

/// Error response format
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Authentication(_) => StatusCode::UNAUTHORIZED,
            ApiError::Authorization(_) => StatusCode::FORBIDDEN,
            ApiError::Validation(_) | ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::RateLimit => StatusCode::TOO_MANY_REQUESTS,
            ApiError::RateLimitExceeded(_) => StatusCode::TOO_MANY_REQUESTS,
            ApiError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get the error code as a string
    pub fn error_code(&self) -> &'static str {
        match self {
            ApiError::Authentication(_) => "AUTHENTICATION_FAILED",
            ApiError::Authorization(_) => "AUTHORIZATION_FAILED",
            ApiError::Validation(_) => "VALIDATION_ERROR",
            ApiError::NotFound(_) => "NOT_FOUND",
            ApiError::Conflict(_) => "CONFLICT",
            ApiError::RateLimit => "RATE_LIMIT_EXCEEDED",
            ApiError::RateLimitExceeded(_) => "RATE_LIMIT_EXCEEDED",
            ApiError::Internal(_) => "INTERNAL_ERROR",
            ApiError::EventProcessing(_) => "EVENT_PROCESSING_ERROR",
            ApiError::Execution(_) => "EXECUTION_ERROR",
            ApiError::State(_) => "STATE_ERROR",
            ApiError::Config(_) => "CONFIG_ERROR",
            ApiError::BadRequest(_) => "BAD_REQUEST",
            ApiError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
        }
    }

    /// Convert to ErrorResponse
    pub fn to_response(&self) -> ErrorResponse {
        ErrorResponse {
            error: self.error_code().to_string(),
            message: self.to_string(),
            code: self.status_code().as_u16(),
            details: None,
        }
    }

    /// Convert to ErrorResponse with details
    pub fn to_response_with_details(&self, details: serde_json::Value) -> ErrorResponse {
        ErrorResponse {
            error: self.error_code().to_string(),
            message: self.to_string(),
            code: self.status_code().as_u16(),
            details: Some(details),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let response = self.to_response();

        tracing::error!(
            error = %self,
            status = %status,
            "API error occurred"
        );

        (status, Json(response)).into_response()
    }
}

/// Result type alias for API operations
pub type Result<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_error_status_codes() {
        let fixtures = vec![
            (
                ApiError::Authentication("test".to_string()),
                StatusCode::UNAUTHORIZED,
            ),
            (
                ApiError::Authorization("test".to_string()),
                StatusCode::FORBIDDEN,
            ),
            (
                ApiError::Validation("test".to_string()),
                StatusCode::BAD_REQUEST,
            ),
            (
                ApiError::NotFound("test".to_string()),
                StatusCode::NOT_FOUND,
            ),
            (ApiError::Conflict("test".to_string()), StatusCode::CONFLICT),
            (ApiError::RateLimit, StatusCode::TOO_MANY_REQUESTS),
        ];

        for (error, expected_status) in fixtures {
            let actual = error.status_code();
            assert_eq!(actual, expected_status);
        }
    }

    #[test]
    fn test_error_codes() {
        let fixtures = vec![
            (
                ApiError::Authentication("test".to_string()),
                "AUTHENTICATION_FAILED",
            ),
            (
                ApiError::Authorization("test".to_string()),
                "AUTHORIZATION_FAILED",
            ),
            (ApiError::Validation("test".to_string()), "VALIDATION_ERROR"),
            (ApiError::NotFound("test".to_string()), "NOT_FOUND"),
            (ApiError::RateLimit, "RATE_LIMIT_EXCEEDED"),
        ];

        for (error, expected_code) in fixtures {
            let actual = error.error_code();
            assert_eq!(actual, expected_code);
        }
    }

    #[test]
    fn test_error_response_conversion() {
        let error = ApiError::Validation("Invalid input".to_string());
        let response = error.to_response();

        assert_eq!(response.error, "VALIDATION_ERROR");
        assert_eq!(response.message, "Validation error: Invalid input");
        assert_eq!(response.code, 400);
        assert_eq!(response.details, None);
    }

    #[test]
    fn test_error_response_with_details() {
        let error = ApiError::Validation("Invalid input".to_string());
        let details = serde_json::json!({"field": "email", "issue": "invalid format"});
        let response = error.to_response_with_details(details.clone());

        assert_eq!(response.error, "VALIDATION_ERROR");
        assert_eq!(response.details, Some(details));
    }
}
