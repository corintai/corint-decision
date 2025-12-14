//! REST API implementation

use crate::error::ServerError;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
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

/// Decision request payload (supports Phase 5 multi-namespace format)
#[derive(Debug, Deserialize)]
pub struct DecideRequestPayload {
    /// Event data (required)
    pub event: HashMap<String, serde_json::Value>,

    /// Feature computation results (optional)
    #[serde(default)]
    pub features: Option<HashMap<String, serde_json::Value>>,

    /// External API results (optional)
    #[serde(default)]
    pub api: Option<HashMap<String, serde_json::Value>>,

    /// Service call results (optional)
    #[serde(default)]
    pub service: Option<HashMap<String, serde_json::Value>>,

    /// LLM analysis results (optional)
    #[serde(default)]
    pub llm: Option<HashMap<String, serde_json::Value>>,

    /// Variables (optional)
    #[serde(default)]
    pub vars: Option<HashMap<String, serde_json::Value>>,
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
    info!(
        "Received decision request with {} event fields",
        payload.event.len()
    );

    // Helper function to convert namespace
    let convert_namespace = |ns: HashMap<String, serde_json::Value>| -> HashMap<String, Value> {
        ns.into_iter()
            .map(|(k, v)| (k, json_to_value(v)))
            .collect()
    };

    // Convert event data (required)
    let event_data = convert_namespace(payload.event);

    // Create decision request with multi-namespace support
    let mut request = DecisionRequest::new(event_data);

    // Add optional namespaces if provided
    if let Some(features) = payload.features {
        request = request.with_features(convert_namespace(features));
    }
    if let Some(api) = payload.api {
        request = request.with_api(convert_namespace(api));
    }
    if let Some(service) = payload.service {
        request = request.with_service(convert_namespace(service));
    }
    if let Some(llm) = payload.llm {
        request = request.with_llm(convert_namespace(llm));
    }
    if let Some(vars) = payload.vars {
        request = request.with_vars(convert_namespace(vars));
    }

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
        serde_json::Value::Array(arr) => Value::Array(arr.into_iter().map(json_to_value).collect()),
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
        let json = serde_json::json!(3.5);
        let value = json_to_value(json);

        if let Value::Number(n) = value {
            assert!((n - 3.5).abs() < 0.001);
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
            event: HashMap::new(),
            features: None,
            api: None,
            service: None,
            llm: None,
            vars: None,
        };

        assert_eq!(payload.event.len(), 0);
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
async fn metrics(State(state): State<AppStateWithMetrics>) -> Response {
    match state.otel_ctx.metrics() {
        Ok(metrics_text) => (
            StatusCode::OK,
            [("Content-Type", "text/plain; version=0.0.4")],
            metrics_text,
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get metrics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get metrics: {}", e),
            )
                .into_response()
        }
    }
}

/// Decision endpoint with metrics
#[axum::debug_handler]
async fn decide_with_metrics(
    State(state): State<AppStateWithMetrics>,
    Json(payload): Json<DecideRequestPayload>,
) -> Result<Json<DecideResponsePayload>, ServerError> {
    info!(
        "Received decision request with {} event fields",
        payload.event.len()
    );

    // Helper function to convert namespace
    let convert_namespace = |ns: HashMap<String, serde_json::Value>| -> HashMap<String, Value> {
        ns.into_iter()
            .map(|(k, v)| (k, json_to_value(v)))
            .collect()
    };

    // Convert event data (required)
    let event_data = convert_namespace(payload.event);

    // Create decision request with multi-namespace support
    let mut request = DecisionRequest::new(event_data);

    // Add optional namespaces if provided
    if let Some(features) = payload.features {
        request = request.with_features(convert_namespace(features));
    }
    if let Some(api) = payload.api {
        request = request.with_api(convert_namespace(api));
    }
    if let Some(service) = payload.service {
        request = request.with_service(convert_namespace(service));
    }
    if let Some(llm) = payload.llm {
        request = request.with_llm(convert_namespace(llm));
    }
    if let Some(vars) = payload.vars {
        request = request.with_vars(convert_namespace(vars));
    }

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
