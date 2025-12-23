//! Server error types (matches API_REQUEST.md spec)

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

/// Generate a unique request ID
fn generate_request_id() -> String {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let random: u32 = rand::random::<u32>() & 0xFFFFFF;
    format!("req_{}_{:06x}", timestamp, random)
}

/// Server error type
#[derive(Error, Debug)]
pub enum ServerError {
    /// Decision engine error
    #[error("Engine error: {0}")]
    EngineError(
        #[from]
        #[source]
        corint_sdk::error::SdkError,
    ),

    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Validation failed
    #[error("Validation failed")]
    ValidationFailed(std::collections::HashMap<String, String>),

    /// Internal server error
    #[error("Internal error: {0}")]
    InternalError(
        #[from]
        #[source]
        anyhow::Error,
    ),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimitExceeded { retry_after: u32 },
}

/// Error response payload (matches API_REQUEST.md spec)
#[derive(Debug, Serialize)]
pub struct ErrorResponsePayload {
    /// Request ID for tracking
    pub request_id: String,

    /// HTTP status code
    pub status: u16,

    /// Error details
    pub error: ErrorDetails,
}

/// Error details
#[derive(Debug, Serialize)]
pub struct ErrorDetails {
    /// Error code
    pub code: String,

    /// Human-readable error message
    pub message: String,

    /// Additional error details (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,

    /// Seconds to wait before retrying (for rate limit errors)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u32>,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let request_id = generate_request_id();

        let (status, code, message, details, retry_after) = match &self {
            ServerError::EngineError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("An error occurred while processing your request: {}", e),
                Some(json!({ "hint": format!("Please contact support with request_id: {}", request_id) })),
                None,
            ),
            ServerError::InvalidRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                msg.clone(),
                None,
                None,
            ),
            ServerError::ValidationFailed(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_FAILED",
                "Request validation failed".to_string(),
                Some(json!(errors)),
                None,
            ),
            ServerError::InternalError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                format!("An unexpected error occurred: {}", e),
                Some(json!({ "hint": format!("Please contact support with request_id: {}", request_id) })),
                None,
            ),
            ServerError::NotFound(resource) => (
                StatusCode::NOT_FOUND,
                "RESOURCE_NOT_FOUND",
                format!("Resource not found: {}", resource),
                None,
                None,
            ),
            ServerError::RateLimitExceeded { retry_after } => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMIT_EXCEEDED",
                "Rate limit exceeded".to_string(),
                None,
                Some(*retry_after),
            ),
        };

        let body = Json(ErrorResponsePayload {
            request_id,
            status: status.as_u16(),
            error: ErrorDetails {
                code: code.to_string(),
                message,
                details,
                retry_after,
            },
        });

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_error_display() {
        let sdk_err = corint_sdk::error::SdkError::GenericError("compilation failed".to_string());
        let err = ServerError::EngineError(sdk_err);
        assert!(err.to_string().contains("Engine error"));
        assert!(err.to_string().contains("compilation failed"));
    }

    #[test]
    fn test_invalid_request_display() {
        let err = ServerError::InvalidRequest("missing field".to_string());
        assert_eq!(err.to_string(), "Invalid request: missing field");
    }

    #[test]
    fn test_internal_error_display() {
        let anyhow_err = anyhow::anyhow!("database connection failed");
        let err = ServerError::InternalError(anyhow_err);
        assert!(err.to_string().contains("Internal error"));
        assert!(err.to_string().contains("database connection failed"));
    }

    #[test]
    fn test_not_found_display() {
        let err = ServerError::NotFound("pipeline not found".to_string());
        assert_eq!(err.to_string(), "Not found: pipeline not found");
    }

    #[test]
    fn test_error_debug_format() {
        let sdk_err = corint_sdk::error::SdkError::NotInitialized;
        let err = ServerError::EngineError(sdk_err);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("EngineError"));
    }

    #[test]
    fn test_sdk_error_conversion() {
        let sdk_err = corint_sdk::error::SdkError::NotInitialized;
        let server_err: ServerError = sdk_err.into();
        assert!(server_err.to_string().contains("Engine error"));
        assert!(server_err.to_string().contains("Engine not initialized"));
    }

    #[test]
    fn test_anyhow_error_conversion() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let server_err: ServerError = anyhow_err.into();
        assert!(server_err.to_string().contains("Internal error"));
        assert!(server_err.to_string().contains("something went wrong"));
    }

    #[test]
    fn test_into_response_engine_error() {
        let sdk_err = corint_sdk::error::SdkError::NotInitialized;
        let err = ServerError::EngineError(sdk_err);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_into_response_invalid_request() {
        let err = ServerError::InvalidRequest("bad input".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_into_response_internal_error() {
        let anyhow_err = anyhow::anyhow!("crash");
        let err = ServerError::InternalError(anyhow_err);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_into_response_not_found() {
        let err = ServerError::NotFound("resource missing".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ServerError>();
    }

    #[test]
    fn test_validation_failed_response() {
        let mut errors = std::collections::HashMap::new();
        errors.insert("event.type".to_string(), "Field is required".to_string());
        let err = ServerError::ValidationFailed(errors);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn test_rate_limit_exceeded_response() {
        let err = ServerError::RateLimitExceeded { retry_after: 60 };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_generate_request_id_format() {
        let request_id = generate_request_id();
        assert!(request_id.starts_with("req_"));
        assert!(request_id.len() > 10);
    }

    #[test]
    fn test_error_response_payload_structure() {
        let payload = ErrorResponsePayload {
            request_id: "req_test".to_string(),
            status: 400,
            error: ErrorDetails {
                code: "INVALID_REQUEST".to_string(),
                message: "Bad request".to_string(),
                details: None,
                retry_after: None,
            },
        };
        assert_eq!(payload.status, 400);
        assert_eq!(payload.error.code, "INVALID_REQUEST");
    }
}
