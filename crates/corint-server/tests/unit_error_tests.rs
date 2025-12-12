//! Unit tests for ServerError types

use axum::{
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;

// Re-create the ServerError enum for testing since it's not exported as a library
#[derive(Debug)]
pub enum ServerError {
    EngineError(String),
    InvalidRequest(String),
    InternalError(String),
    NotFound(String),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            ServerError::EngineError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ServerError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ServerError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ServerError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };

        let body = axum::Json(json!({
            "error": error_message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

#[test]
fn test_engine_error_display() {
    let error = ServerError::EngineError("Pipeline compilation failed".to_string());
    assert_eq!(error.to_string(), "Engine error: Pipeline compilation failed");
}

#[test]
fn test_invalid_request_display() {
    let error = ServerError::InvalidRequest("Missing required field: event_data".to_string());
    assert_eq!(error.to_string(), "Invalid request: Missing required field: event_data");
}

#[test]
fn test_internal_error_display() {
    let error = ServerError::InternalError("Database connection lost".to_string());
    assert_eq!(error.to_string(), "Internal error: Database connection lost");
}

#[test]
fn test_not_found_display() {
    let error = ServerError::NotFound("Pipeline not found".to_string());
    assert_eq!(error.to_string(), "Not found: Pipeline not found");
}

#[tokio::test]
async fn test_engine_error_response() {
    let error = ServerError::EngineError("Test error".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_invalid_request_response() {
    let error = ServerError::InvalidRequest("Bad payload".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_internal_error_response() {
    let error = ServerError::InternalError("Something went wrong".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_not_found_response() {
    let error = ServerError::NotFound("Resource missing".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn test_error_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ServerError>();
}

#[test]
fn test_error_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<ServerError>();
}

#[test]
fn test_error_debug_format() {
    let error = ServerError::EngineError("test".to_string());
    let debug = format!("{:?}", error);
    assert!(debug.contains("EngineError"));
    assert!(debug.contains("test"));
}
