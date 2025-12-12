//! Server error types

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Server error type
#[derive(Error, Debug)]
pub enum ServerError {
    /// Decision engine error
    #[error("Engine error: {0}")]
    EngineError(#[from] #[source] corint_sdk::error::SdkError),

    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    InternalError(#[from] #[source] anyhow::Error),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ServerError::EngineError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ServerError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ServerError::InternalError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ServerError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
        }));

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
}


