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
    /// Request ID (for tracking and correlation)
    pub request_id: String,

    /// Pipeline ID that processed this request
    pub pipeline_id: Option<String>,

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
    let event_fields: HashMap<String, Value> = payload
        .event_data
        .into_iter()
        .map(|(k, v)| (k, json_to_value(v)))
        .collect();

    // Create event_data with dual structure:
    // 1. Original nested structure under "event" key (semantic clarity)
    // 2. Completely flattened structure with dot notation (fast lookup)
    let mut event_data = HashMap::new();
    
    // Store original nested structure
    let event_object = Value::Object(event_fields.clone());
    event_data.insert("event".to_string(), event_object.clone());
    
    // Store top-level fields for backward compatibility
    for (key, value) in &event_fields {
        event_data.insert(key.clone(), value.clone());
    }
    
    // Flatten the entire event object recursively
    flatten_object("event", &event_object, &mut event_data);

    // Create decision request
    let request = DecisionRequest::new(event_data);

    // Execute decision
    let response = state.engine.decide(request).await?;

    // Convert action to string
    let action_str = response.result.action.map(|a| format!("{:?}", a));

    Ok(Json(DecideResponsePayload {
        request_id: response.request_id,
        pipeline_id: response.pipeline_id,
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

/// Recursively flatten nested objects into dot-notation keys
/// 
/// Example:
/// ```
/// event = {
///   user: {
///     id: "123",
///     profile: {
///       tier: "gold"
///     }
///   },
///   amount: 1000
/// }
/// ```
/// 
/// Produces:
/// ```
/// event.user.id = "123"
/// event.user.profile.tier = "gold"
/// event.amount = 1000
/// ```
fn flatten_object(prefix: &str, value: &Value, result: &mut HashMap<String, Value>) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let new_prefix = format!("{}.{}", prefix, key);
                // Store this level
                result.insert(new_prefix.clone(), val.clone());
                // Recursively flatten if it's an object
                if matches!(val, Value::Object(_)) {
                    flatten_object(&new_prefix, val, result);
                }
            }
        }
        _ => {
            // For non-object values, just store as-is
            result.insert(prefix.to_string(), value.clone());
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

    #[test]
    fn test_flatten_object_simple() {
        let mut obj = HashMap::new();
        obj.insert("user_id".to_string(), Value::String("user_001".to_string()));
        obj.insert("amount".to_string(), Value::Number(1000.0));
        
        let event = Value::Object(obj);
        let mut result = HashMap::new();
        
        flatten_object("event", &event, &mut result);
        
        assert_eq!(result.get("event.user_id"), Some(&Value::String("user_001".to_string())));
        assert_eq!(result.get("event.amount"), Some(&Value::Number(1000.0)));
    }

    #[test]
    fn test_flatten_object_nested() {
        // Create nested structure: event.user.profile.tier = "gold"
        let mut profile = HashMap::new();
        profile.insert("tier".to_string(), Value::String("gold".to_string()));
        profile.insert("age".to_string(), Value::Number(30.0));
        
        let mut user = HashMap::new();
        user.insert("id".to_string(), Value::String("user_001".to_string()));
        user.insert("profile".to_string(), Value::Object(profile));
        
        let mut event_obj = HashMap::new();
        event_obj.insert("user".to_string(), Value::Object(user));
        event_obj.insert("amount".to_string(), Value::Number(1000.0));
        
        let event = Value::Object(event_obj);
        let mut result = HashMap::new();
        
        flatten_object("event", &event, &mut result);
        
        // Check all levels are flattened
        assert!(result.contains_key("event.user"));
        assert_eq!(result.get("event.user.id"), Some(&Value::String("user_001".to_string())));
        assert!(result.contains_key("event.user.profile"));
        assert_eq!(result.get("event.user.profile.tier"), Some(&Value::String("gold".to_string())));
        assert_eq!(result.get("event.user.profile.age"), Some(&Value::Number(30.0)));
        assert_eq!(result.get("event.amount"), Some(&Value::Number(1000.0)));
    }
}

