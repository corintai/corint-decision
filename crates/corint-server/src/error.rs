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


