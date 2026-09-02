//! Custom extractors and middleware
//!
//! Provides custom request extractors with better error handling.

use axum::{
    extract::{rejection::JsonRejection, FromRequest, Request},
    http::StatusCode,
    Json,
};
use serde_json::json;

/// Custom JSON extractor with better error messages
pub struct JsonExtractor<T>(pub T);

#[axum::async_trait]
impl<S, T> FromRequest<S> for JsonExtractor<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => {
                let error_message = match rejection {
                    JsonRejection::JsonDataError(err) => {
                        format!("Invalid JSON data: {}", err)
                    }
                    JsonRejection::JsonSyntaxError(err) => {
                        format!("JSON syntax error: {}", err)
                    }
                    JsonRejection::MissingJsonContentType(_) => {
                        "Missing 'Content-Type: application/json' header".to_string()
                    }
                    _ => format!("Failed to parse JSON: {}", rejection),
                };

                Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": error_message,
                        "status": 400,
                    })),
                ))
            }
        }
    }
}
