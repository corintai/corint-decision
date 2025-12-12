//! Server error types

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fmt;

/// Server error type
#[derive(Debug)]
pub enum ServerError {
    /// Decision engine error
    EngineError(String),
    
    /// Invalid request
    InvalidRequest(String),
    
    /// Internal server error
    InternalError(String),
    
    /// Not found
    NotFound(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::EngineError(msg) => write!(f, "Engine error: {}", msg),
            ServerError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            ServerError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            ServerError::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}

impl std::error::Error for ServerError {}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ServerError::EngineError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ServerError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ServerError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ServerError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

impl From<corint_sdk::error::SdkError> for ServerError {
    fn from(err: corint_sdk::error::SdkError) -> Self {
        ServerError::EngineError(err.to_string())
    }
}

impl From<anyhow::Error> for ServerError {
    fn from(err: anyhow::Error) -> Self {
        ServerError::InternalError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_error_display() {
        let err = ServerError::EngineError("compilation failed".to_string());
        assert_eq!(err.to_string(), "Engine error: compilation failed");
    }

    #[test]
    fn test_invalid_request_display() {
        let err = ServerError::InvalidRequest("missing field".to_string());
        assert_eq!(err.to_string(), "Invalid request: missing field");
    }

    #[test]
    fn test_internal_error_display() {
        let err = ServerError::InternalError("database connection failed".to_string());
        assert_eq!(err.to_string(), "Internal error: database connection failed");
    }

    #[test]
    fn test_not_found_display() {
        let err = ServerError::NotFound("pipeline not found".to_string());
        assert_eq!(err.to_string(), "Not found: pipeline not found");
    }

    #[test]
    fn test_error_debug_format() {
        let err = ServerError::EngineError("test".to_string());
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
        let err = ServerError::EngineError("test error".to_string());
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
        let err = ServerError::InternalError("crash".to_string());
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


