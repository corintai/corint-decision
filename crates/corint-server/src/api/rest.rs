//! REST API implementation

use crate::error::ServerError;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use corint_core::Value;
use corint_sdk::{DecisionEngine, DecisionRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<DecisionEngine>,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Decision request payload
#[derive(Debug, Deserialize)]
pub struct DecideRequestPayload {
    /// Event data
    pub event_data: HashMap<String, serde_json::Value>,
}

/// Decision response payload
#[derive(Debug, Serialize)]
pub struct DecideResponsePayload {
    /// Action
    pub action: Option<String>,
    
    /// Score
    pub score: i32,
    
    /// Triggered rules
    pub triggered_rules: Vec<String>,
    
    /// Explanation
    pub explanation: String,
    
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// Create REST API router
pub fn create_router(engine: Arc<DecisionEngine>) -> Router {
    let state = AppState { engine };

    Router::new()
        .route("/health", get(health))
        .route("/v1/decide", post(decide))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

/// Health check endpoint
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Decision endpoint
#[axum::debug_handler]
async fn decide(
    State(state): State<AppState>,
    Json(payload): Json<DecideRequestPayload>,
) -> Result<Json<DecideResponsePayload>, ServerError> {
    info!("Received decision request with {} event fields", payload.event_data.len());

    // Convert serde_json::Value to corint_core::Value
    let event_data: HashMap<String, Value> = payload
        .event_data
        .into_iter()
        .map(|(k, v)| (k, json_to_value(v)))
        .collect();

    // Create decision request
    let request = DecisionRequest::new(event_data);

    // Execute decision
    let response = state.engine.decide(request).await?;

    // Convert action to string
    let action_str = response.result.action.map(|a| format!("{:?}", a));

    Ok(Json(DecideResponsePayload {
        action: action_str,
        score: response.result.score,
        triggered_rules: response.result.triggered_rules,
        explanation: response.result.explanation,
        processing_time_ms: response.processing_time_ms,
    }))
}


/// Convert serde_json::Value to corint_core::Value
fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Number(i as f64)
            } else if let Some(f) = n.as_f64() {
                Value::Number(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(arr) => {
            Value::Array(arr.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let map = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect();
            Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_to_value_conversion() {
        let json = serde_json::json!({
            "string": "test",
            "number": 42,
            "bool": true,
            "null": null
        });

        let value = json_to_value(json);
        
        if let Value::Object(map) = value {
            assert!(matches!(map.get("string"), Some(Value::String(_))));
            assert!(matches!(map.get("number"), Some(Value::Number(_))));
            assert!(matches!(map.get("bool"), Some(Value::Bool(true))));
            assert!(matches!(map.get("null"), Some(Value::Null)));
        } else {
            panic!("Expected Object");
        }
    }
}

