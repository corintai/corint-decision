//! REST API implementation

use crate::error::ServerError;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
    response::{IntoResponse, Response},
    http::StatusCode,
};
use corint_core::Value;
use corint_runtime::observability::otel::OtelContext;
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

/// Application state with metrics
#[derive(Clone)]
pub struct AppStateWithMetrics {
    pub engine: Arc<DecisionEngine>,
    pub otel_ctx: Arc<OtelContext>,
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

/// Create REST API router with metrics endpoint
pub fn create_router_with_metrics(
    engine: Arc<DecisionEngine>,
    otel_ctx: Arc<OtelContext>,
) -> Router {
    let state = AppStateWithMetrics {
        engine: engine.clone(),
        otel_ctx,
    };

    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/decide", post(decide_with_metrics))
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
/// ```text
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
/// ```text
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

    #[test]
    fn test_json_to_value_null() {
        let json = serde_json::Value::Null;
        let value = json_to_value(json);
        assert!(matches!(value, Value::Null));
    }

    #[test]
    fn test_json_to_value_bool() {
        let json_true = serde_json::Value::Bool(true);
        let json_false = serde_json::Value::Bool(false);

        assert!(matches!(json_to_value(json_true), Value::Bool(true)));
        assert!(matches!(json_to_value(json_false), Value::Bool(false)));
    }

    #[test]
    fn test_json_to_value_number_integer() {
        let json = serde_json::json!(42);
        let value = json_to_value(json);

        if let Value::Number(n) = value {
            assert_eq!(n, 42.0);
        } else {
            panic!("Expected Number");
        }
    }

    #[test]
    fn test_json_to_value_number_float() {
        let json = serde_json::json!(3.14);
        let value = json_to_value(json);

        if let Value::Number(n) = value {
            assert!((n - 3.14).abs() < 0.001);
        } else {
            panic!("Expected Number");
        }
    }

    #[test]
    fn test_json_to_value_string() {
        let json = serde_json::json!("hello");
        let value = json_to_value(json);

        assert_eq!(value, Value::String("hello".to_string()));
    }

    #[test]
    fn test_json_to_value_array() {
        let json = serde_json::json!([1, 2, 3]);
        let value = json_to_value(json);

        if let Value::Array(arr) = value {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Value::Number(1.0));
            assert_eq!(arr[1], Value::Number(2.0));
            assert_eq!(arr[2], Value::Number(3.0));
        } else {
            panic!("Expected Array");
        }
    }

    #[test]
    fn test_json_to_value_nested_array() {
        let json = serde_json::json!([[1, 2], [3, 4]]);
        let value = json_to_value(json);

        if let Value::Array(outer) = value {
            assert_eq!(outer.len(), 2);
            if let Value::Array(inner) = &outer[0] {
                assert_eq!(inner.len(), 2);
                assert_eq!(inner[0], Value::Number(1.0));
            } else {
                panic!("Expected nested array");
            }
        } else {
            panic!("Expected Array");
        }
    }

    #[test]
    fn test_json_to_value_object() {
        let json = serde_json::json!({"key": "value"});
        let value = json_to_value(json);

        if let Value::Object(map) = value {
            assert_eq!(map.get("key"), Some(&Value::String("value".to_string())));
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_json_to_value_nested_object() {
        let json = serde_json::json!({
            "user": {
                "name": "Alice",
                "age": 30
            }
        });
        let value = json_to_value(json);

        if let Value::Object(outer) = value {
            if let Some(Value::Object(inner)) = outer.get("user") {
                assert_eq!(inner.get("name"), Some(&Value::String("Alice".to_string())));
                assert_eq!(inner.get("age"), Some(&Value::Number(30.0)));
            } else {
                panic!("Expected nested object");
            }
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_flatten_object_empty() {
        let event = Value::Object(HashMap::new());
        let mut result = HashMap::new();

        flatten_object("event", &event, &mut result);

        // Empty object should produce no flattened keys
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_flatten_object_non_object() {
        let event = Value::String("test".to_string());
        let mut result = HashMap::new();

        flatten_object("event", &event, &mut result);

        // Non-object should just store the value itself
        assert_eq!(result.get("event"), Some(&Value::String("test".to_string())));
    }

    #[test]
    fn test_flatten_object_array_value() {
        let mut obj = HashMap::new();
        obj.insert("tags".to_string(), Value::Array(vec![
            Value::String("fraud".to_string()),
            Value::String("suspicious".to_string()),
        ]));

        let event = Value::Object(obj);
        let mut result = HashMap::new();

        flatten_object("event", &event, &mut result);

        // Array should be stored as-is, not flattened
        if let Some(Value::Array(tags)) = result.get("event.tags") {
            assert_eq!(tags.len(), 2);
        } else {
            panic!("Expected array value");
        }
    }

    #[test]
    fn test_flatten_object_deep_nesting() {
        // Create 4 levels deep: event.a.b.c.d = "value"
        let mut level_d = HashMap::new();
        level_d.insert("d".to_string(), Value::String("value".to_string()));

        let mut level_c = HashMap::new();
        level_c.insert("c".to_string(), Value::Object(level_d));

        let mut level_b = HashMap::new();
        level_b.insert("b".to_string(), Value::Object(level_c));

        let mut level_a = HashMap::new();
        level_a.insert("a".to_string(), Value::Object(level_b));

        let event = Value::Object(level_a);
        let mut result = HashMap::new();

        flatten_object("event", &event, &mut result);

        // Check deeply nested value is accessible
        assert_eq!(result.get("event.a.b.c.d"), Some(&Value::String("value".to_string())));
    }

    #[test]
    fn test_health_response_fields() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            version: "1.0.0".to_string(),
        };

        assert_eq!(response.status, "healthy");
        assert_eq!(response.version, "1.0.0");
    }

    #[test]
    fn test_decide_response_payload_fields() {
        let response = DecideResponsePayload {
            request_id: "req_123".to_string(),
            pipeline_id: Some("pipeline_001".to_string()),
            action: Some("APPROVE".to_string()),
            score: 85,
            triggered_rules: vec!["rule1".to_string(), "rule2".to_string()],
            explanation: "Low risk transaction".to_string(),
            processing_time_ms: 42,
        };

        assert_eq!(response.request_id, "req_123");
        assert_eq!(response.pipeline_id, Some("pipeline_001".to_string()));
        assert_eq!(response.action, Some("APPROVE".to_string()));
        assert_eq!(response.score, 85);
        assert_eq!(response.triggered_rules.len(), 2);
        assert_eq!(response.explanation, "Low risk transaction");
        assert_eq!(response.processing_time_ms, 42);
    }

    #[test]
    fn test_decide_request_payload_empty() {
        let payload = DecideRequestPayload {
            event_data: HashMap::new(),
        };

        assert_eq!(payload.event_data.len(), 0);
    }

    #[test]
    fn test_json_to_value_mixed_types() {
        let json = serde_json::json!({
            "str": "text",
            "num": 123,
            "bool": true,
            "null": null,
            "arr": [1, 2, 3],
            "obj": {"nested": "value"}
        });

        let value = json_to_value(json);

        if let Value::Object(map) = value {
            assert!(matches!(map.get("str"), Some(Value::String(_))));
            assert!(matches!(map.get("num"), Some(Value::Number(_))));
            assert!(matches!(map.get("bool"), Some(Value::Bool(true))));
            assert!(matches!(map.get("null"), Some(Value::Null)));
            assert!(matches!(map.get("arr"), Some(Value::Array(_))));
            assert!(matches!(map.get("obj"), Some(Value::Object(_))));
        } else {
            panic!("Expected Object");
        }
    }
}

/// Metrics endpoint - returns Prometheus format metrics
async fn metrics(
    State(state): State<AppStateWithMetrics>,
) -> Response {
    match state.otel_ctx.metrics() {
        Ok(metrics_text) => {
            (
                StatusCode::OK,
                [("Content-Type", "text/plain; version=0.0.4")],
                metrics_text,
            ).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get metrics: {}", e),
            ).into_response()
        }
    }
}

/// Decision endpoint with metrics
#[axum::debug_handler]
async fn decide_with_metrics(
    State(state): State<AppStateWithMetrics>,
    Json(payload): Json<DecideRequestPayload>,
) -> Result<Json<DecideResponsePayload>, ServerError> {
    info!("Received decision request with {} event fields", payload.event_data.len());

    // Convert serde_json::Value to corint_core::Value
    let event_fields: HashMap<String, Value> = payload
        .event_data
        .into_iter()
        .map(|(k, v)| (k, json_to_value(v)))
        .collect();

    // Create event_data with dual structure (same as decide())
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

