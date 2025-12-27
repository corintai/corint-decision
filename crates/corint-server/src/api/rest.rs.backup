//! REST API implementation

use crate::error::ServerError;
use axum::{
    extract::{rejection::JsonRejection, FromRequest, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use corint_core::Value;
use corint_runtime::observability::otel::OtelContext;
use corint_sdk::{DecisionEngine, DecisionRequest, ScoreNormalizer};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<RwLock<DecisionEngine>>,
}

/// Application state with metrics
#[derive(Clone)]
pub struct AppStateWithMetrics {
    pub engine: Arc<RwLock<DecisionEngine>>,
    pub otel_ctx: Arc<OtelContext>,
}

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

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Decision request payload (matches API_REQUEST.md spec)
#[derive(Debug, Deserialize)]
pub struct DecideRequestPayload {
    /// Event data (required)
    pub event: HashMap<String, serde_json::Value>,

    /// User profile/context (optional)
    #[serde(default)]
    pub user: Option<HashMap<String, serde_json::Value>>,

    /// Optional configuration
    #[serde(default)]
    pub options: Option<RequestOptions>,

    // Legacy/internal namespaces (for backward compatibility)
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

/// Request options
#[derive(Debug, Default, Deserialize)]
pub struct RequestOptions {
    /// Whether to return computed feature values
    #[serde(default)]
    pub return_features: bool,

    /// Whether to return detailed execution trace
    #[serde(default)]
    pub enable_trace: bool,

    /// Whether to process asynchronously
    #[serde(default, rename = "async")]
    pub async_mode: bool,
}

/// Decision response payload (matches API_REQUEST.md spec)
#[derive(Debug, Serialize)]
pub struct DecideResponsePayload {
    /// Request ID (for tracking and correlation)
    pub request_id: String,

    /// HTTP status code
    pub status: u16,

    /// Processing time in milliseconds
    pub process_time_ms: u64,

    /// Pipeline ID that processed this request
    pub pipeline_id: String,

    /// Decision result
    pub decision: DecisionPayload,

    /// Computed features (only present if options.return_features = true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, serde_json::Value>>,

    /// Detailed execution trace (only present if options.enable_trace = true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<corint_runtime::ExecutionTrace>,
}

/// Decision payload (nested in response)
#[derive(Debug, Serialize)]
pub struct DecisionPayload {
    /// Decision result: "ALLOW", "DENY", "REVIEW", "HOLD", "PASS"
    pub result: String,

    /// Actions to take
    pub actions: Vec<String>,

    /// Risk scores
    pub scores: ScoresPayload,

    /// Evidence
    pub evidence: EvidencePayload,

    /// Cognition (explanation)
    pub cognition: CognitionPayload,
}

/// Scores payload
#[derive(Debug, Serialize)]
pub struct ScoresPayload {
    /// Normalized risk score (0-1000)
    pub canonical: i32,

    /// Raw aggregated score from rules
    pub raw: i32,

    /// Confidence level (0-1) - optional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// Evidence payload
#[derive(Debug, Serialize)]
pub struct EvidencePayload {
    /// Array of triggered rule IDs
    pub triggered_rules: Vec<String>,
}

/// Cognition payload
#[derive(Debug, Serialize)]
pub struct CognitionPayload {
    /// Human-readable explanation
    pub summary: String,

    /// Machine-readable reason codes
    pub reason_codes: Vec<String>,
}

/// Create REST API router
pub fn create_router(engine: Arc<DecisionEngine>) -> Router {
    let state = AppState {
        engine: Arc::new(RwLock::new(
            Arc::try_unwrap(engine).unwrap_or_else(|_arc| {
                // This should not happen during normal initialization
                // If it does, log a warning and create a minimal engine
                tracing::warn!("Arc<DecisionEngine> has multiple references during router creation");
                // We can't clone DecisionEngine, so we panic with a clear message
                panic!("Cannot create router: DecisionEngine Arc has multiple references. This is a programming error.");
            })
        )),
    };

    Router::new()
        .route("/health", get(health))
        .route("/v1/decide", post(decide))
        .route("/v1/repo/reload", post(reload_repository))  // Changed from GET to POST
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
        engine: Arc::new(RwLock::new(
            Arc::try_unwrap(engine).unwrap_or_else(|_arc| {
                tracing::warn!("Arc<DecisionEngine> has multiple references during router creation");
                panic!("Cannot create router: DecisionEngine Arc has multiple references. This is a programming error.");
            })
        )),
        otel_ctx,
    };

    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/decide", post(decide_with_metrics))
        .route("/v1/repo/reload", post(reload_repository_with_metrics))  // Changed from GET to POST
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
    JsonExtractor(payload): JsonExtractor<DecideRequestPayload>,
) -> Result<Json<DecideResponsePayload>, ServerError> {
    let options = payload.options.unwrap_or_default();

    info!(
        "Received decision request with {} event fields, enable_trace={}",
        payload.event.len(),
        options.enable_trace
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

    // Add user namespace if provided
    if let Some(user) = payload.user {
        request = request.with_vars(convert_namespace(user));
    }

    // Add optional namespaces if provided (legacy/internal)
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

    // Enable tracing if requested
    if options.enable_trace {
        request = request.with_trace();
    }

    // Execute decision (acquire read lock - allows concurrent reads)
    let engine = state.engine.read().await;
    let response = engine.decide(request).await?;
    drop(engine); // Release lock as soon as possible

    // Convert action to decision result string
    let result_str = response
        .result
        .signal
        .map(|a| format!("{:?}", a).to_uppercase())
        .unwrap_or_else(|| "PASS".to_string());

    // Build the response
    Ok(Json(DecideResponsePayload {
        request_id: response.request_id,
        status: 200,
        process_time_ms: response.processing_time_ms,
        pipeline_id: response.pipeline_id.unwrap_or_else(|| "default".to_string()),
        decision: DecisionPayload {
            result: result_str,
            actions: Vec::new(), // TODO: Extract actions from context if available
            scores: ScoresPayload {
                canonical: normalize_score(response.result.score),
                raw: response.result.score,
                confidence: None,
            },
            evidence: EvidencePayload {
                triggered_rules: response.result.triggered_rules,
            },
            cognition: CognitionPayload {
                summary: response.result.explanation.clone(),
                reason_codes: extract_reason_codes(&response.result.explanation),
            },
        },
        features: if options.return_features {
            Some(
                response
                    .result
                    .context
                    .into_iter()
                    .map(|(k, v)| (k, value_to_json(v)))
                    .collect(),
            )
        } else {
            None
        },
        trace: response.trace,
    }))
}

/// Normalize raw score to canonical 0-1000 range using sigmoid/logistic function
///
/// Uses the SDK's ScoreNormalizer with default parameters for industry-standard
/// score normalization that provides smooth S-curve mapping.
fn normalize_score(raw: i32) -> i32 {
    // Use sigmoid normalization for smooth, production-grade score mapping
    ScoreNormalizer::default().normalize(raw)
}

/// Extract reason codes from explanation string
fn extract_reason_codes(explanation: &str) -> Vec<String> {
    // Simple extraction: look for common patterns
    let mut codes = Vec::new();

    // Extract codes from explanation (this is a basic implementation)
    // In a real system, these would come from the decision result
    if explanation.to_lowercase().contains("email") && explanation.to_lowercase().contains("not verified") {
        codes.push("EMAIL_NOT_VERIFIED".to_string());
    }
    if explanation.to_lowercase().contains("phone") && explanation.to_lowercase().contains("not verified") {
        codes.push("PHONE_NOT_VERIFIED".to_string());
    }
    if explanation.to_lowercase().contains("new account") || explanation.to_lowercase().contains("account_age") {
        codes.push("NEW_ACCOUNT".to_string());
    }
    if explanation.to_lowercase().contains("high") && explanation.to_lowercase().contains("amount") {
        codes.push("HIGH_TRANSACTION_AMOUNT".to_string());
    }
    if explanation.to_lowercase().contains("low risk") {
        codes.push("LOW_RISK".to_string());
    }

    codes
}

/// Convert corint_core::Value to serde_json::Value
fn value_to_json(v: Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(b),
        Value::Number(n) => serde_json::json!(n),
        Value::String(s) => serde_json::Value::String(s),
        Value::Array(arr) => serde_json::Value::Array(arr.into_iter().map(value_to_json).collect()),
        Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .into_iter()
                .map(|(k, v)| (k, value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
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
            status: 200,
            process_time_ms: 42,
            pipeline_id: "pipeline_001".to_string(),
            decision: DecisionPayload {
                result: "ALLOW".to_string(),
                actions: vec![],
                scores: ScoresPayload {
                    canonical: 85,
                    raw: 85,
                    confidence: None,
                },
                evidence: EvidencePayload {
                    triggered_rules: vec!["rule1".to_string(), "rule2".to_string()],
                },
                cognition: CognitionPayload {
                    summary: "Low risk transaction".to_string(),
                    reason_codes: vec!["LOW_RISK".to_string()],
                },
            },
            features: None,
            trace: None,
        };

        assert_eq!(response.request_id, "req_123");
        assert_eq!(response.status, 200);
        assert_eq!(response.pipeline_id, "pipeline_001");
        assert_eq!(response.decision.result, "ALLOW");
        assert_eq!(response.decision.scores.canonical, 85);
        assert_eq!(response.decision.evidence.triggered_rules.len(), 2);
        assert_eq!(response.decision.cognition.summary, "Low risk transaction");
        assert_eq!(response.process_time_ms, 42);
        assert!(response.trace.is_none());
    }

    #[test]
    fn test_decide_request_payload_empty() {
        let payload = DecideRequestPayload {
            event: HashMap::new(),
            user: None,
            options: None,
            features: None,
            api: None,
            service: None,
            llm: None,
            vars: None,
        };

        assert_eq!(payload.event.len(), 0);
        assert!(payload.options.is_none());
    }

    #[test]
    fn test_request_options_defaults() {
        let options = RequestOptions::default();
        assert!(!options.return_features);
        assert!(!options.enable_trace);
        assert!(!options.async_mode);
    }

    #[test]
    fn test_normalize_score() {
        // Negative scores become 0
        assert_eq!(normalize_score(-100), 0);
        assert_eq!(normalize_score(-1), 0);

        // Sigmoid normalization provides smooth S-curve
        // Center point (500) should map to ~500
        let center = normalize_score(500);
        assert!(center >= 495 && center <= 505, "Center: {}", center);

        // Low scores should be compressed
        let low = normalize_score(100);
        assert!(low > 0 && low < 200, "Low: {}", low);

        // High scores should be compressed
        let high = normalize_score(1000);
        assert!(high > 700 && high < 1000, "High: {}", high);

        // Very high scores should saturate near 1000
        let very_high = normalize_score(5000);
        assert!(very_high >= 900 && very_high <= 1000, "Very high: {}", very_high);

        // Scores should increase monotonically
        assert!(normalize_score(300) < normalize_score(500));
        assert!(normalize_score(500) < normalize_score(700));
    }

    #[test]
    fn test_extract_reason_codes() {
        let codes = extract_reason_codes("Low risk transaction");
        assert!(codes.contains(&"LOW_RISK".to_string()));

        let codes2 = extract_reason_codes("Email not verified, high transaction amount");
        assert!(codes2.contains(&"EMAIL_NOT_VERIFIED".to_string()));
        assert!(codes2.contains(&"HIGH_TRANSACTION_AMOUNT".to_string()));
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
    JsonExtractor(payload): JsonExtractor<DecideRequestPayload>,
) -> Result<Json<DecideResponsePayload>, ServerError> {
    let options = payload.options.unwrap_or_default();

    info!(
        "Received decision request with {} event fields, enable_trace={}",
        payload.event.len(),
        options.enable_trace
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

    // Add user namespace if provided
    if let Some(user) = payload.user {
        request = request.with_vars(convert_namespace(user));
    }

    // Add optional namespaces if provided (legacy/internal)
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

    // Enable tracing if requested
    if options.enable_trace {
        request = request.with_trace();
    }

    // Execute decision (acquire read lock - allows concurrent reads)
    let engine = state.engine.read().await;
    let response = engine.decide(request).await?;
    drop(engine); // Release lock as soon as possible

    // Convert action to decision result string
    let result_str = response
        .result
        .signal
        .map(|a| format!("{:?}", a).to_uppercase())
        .unwrap_or_else(|| "PASS".to_string());

    // Build the response
    Ok(Json(DecideResponsePayload {
        request_id: response.request_id,
        status: 200,
        process_time_ms: response.processing_time_ms,
        pipeline_id: response.pipeline_id.unwrap_or_else(|| "default".to_string()),
        decision: DecisionPayload {
            result: result_str,
            actions: Vec::new(),
            scores: ScoresPayload {
                canonical: normalize_score(response.result.score),
                raw: response.result.score,
                confidence: None,
            },
            evidence: EvidencePayload {
                triggered_rules: response.result.triggered_rules,
            },
            cognition: CognitionPayload {
                summary: response.result.explanation.clone(),
                reason_codes: extract_reason_codes(&response.result.explanation),
            },
        },
        features: if options.return_features {
            Some(
                response
                    .result
                    .context
                    .into_iter()
                    .map(|(k, v)| (k, value_to_json(v)))
                    .collect(),
            )
        } else {
            None
        },
        trace: response.trace,
    }))
}

/// Reload repository endpoint response
#[derive(Debug, Serialize)]
pub struct ReloadResponse {
    pub success: bool,
    pub message: String,
}

/// Reload repository endpoint
async fn reload_repository(State(state): State<AppState>) -> Result<Json<ReloadResponse>, ServerError> {
    info!("Received repository reload request");

    // Reload engine using SDK's reload method (acquire write lock - exclusive access)
    {
        let mut engine = state.engine.write().await;
        engine.reload().await.map_err(|e| {
            error!("Failed to reload repository: {}", e);
            ServerError::InternalError(anyhow::anyhow!("Failed to reload repository: {}", e))
        })?;
    }

    info!("Repository reloaded successfully");
    Ok(Json(ReloadResponse {
        success: true,
        message: "Repository reloaded successfully".to_string(),
    }))
}

/// Reload repository endpoint with metrics
async fn reload_repository_with_metrics(
    State(state): State<AppStateWithMetrics>,
) -> Result<Json<ReloadResponse>, ServerError> {
    info!("Received repository reload request");

    // Reload engine using SDK's reload method (acquire write lock - exclusive access)
    {
        let mut engine = state.engine.write().await;
        engine.reload().await.map_err(|e| {
            error!("Failed to reload repository: {}", e);
            ServerError::InternalError(anyhow::anyhow!("Failed to reload repository: {}", e))
        })?;
    }

    info!("Repository reloaded successfully");
    Ok(Json(ReloadResponse {
        success: true,
        message: "Repository reloaded successfully".to_string(),
    }))
}
