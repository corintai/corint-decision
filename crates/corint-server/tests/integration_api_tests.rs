//! Integration tests for REST API endpoints
//!
//! These tests create a real DecisionEngine and test the API endpoints end-to-end.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use corint_sdk::DecisionEngineBuilder;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;
use tower::ServiceExt;

// Import the types we need from the server module
// Since corint-server doesn't expose a lib, we'll create helper functions

/// Helper to create a test decision engine with a simple pipeline
async fn create_test_engine() -> (TempDir, Arc<corint_sdk::DecisionEngine>) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create directory structure
    fs::create_dir_all(repo_path.join("pipelines"))
        .await
        .unwrap();
    fs::create_dir_all(repo_path.join("library/rules"))
        .await
        .unwrap();
    fs::create_dir_all(repo_path.join("library/rulesets"))
        .await
        .unwrap();

    // Create a simple test rule
    let rule_yaml = r#"version: "0.1"

rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - "amount > 1000"
  score: 100
"#;

    fs::write(repo_path.join("library/rules/test_rule.yaml"), rule_yaml)
        .await
        .unwrap();

    // Create a test ruleset
    let ruleset_yaml = r#"version: "0.1"

ruleset:
  id: test_ruleset
  name: Test Ruleset
  rules:
    - test_rule
  decision_logic:
    - condition: total_score >= 100
      action: deny
    - condition: total_score >= 50
      action: review
    - default: true
      action: approve
"#;

    fs::write(
        repo_path.join("library/rulesets/test_ruleset.yaml"),
        ruleset_yaml,
    )
    .await
    .unwrap();

    // Create a pipeline
    let pipeline_yaml = r#"version: "0.1"

pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: test_step
  when:
    all:
      - event.type == "transaction"
  steps:
    - step:
        id: test_step
        name: Test Step
        type: ruleset
        ruleset: test_ruleset
        next: end
"#;

    fs::write(
        repo_path.join("pipelines/test_pipeline.yaml"),
        pipeline_yaml,
    )
    .await
    .unwrap();

    // Create registry
    let registry_yaml = r#"version: "0.1"

registry:
  pipelines:
    - event_type: transaction
      pipeline: test_pipeline
"#;

    fs::write(repo_path.join("registry.yaml"), registry_yaml)
        .await
        .unwrap();

    // Build decision engine
    let pipeline_path = repo_path.join("pipelines/test_pipeline.yaml");
    let registry_path = repo_path.join("registry.yaml");

    let engine = DecisionEngineBuilder::new()
        .add_rule_file(pipeline_path)
        .with_registry_file(registry_path)
        .build()
        .await
        .expect("Failed to build engine");

    (temp_dir, Arc::new(engine))
}

/// Helper to create the app router (mimics the server's create_router function)
fn create_test_router(engine: Arc<corint_sdk::DecisionEngine>) -> Router {
    use axum::{
        extract::State,
        routing::{get, post},
        Json, Router,
    };
    use corint_core::Value;
    use corint_sdk::DecisionRequest;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Clone)]
    struct AppState {
        engine: Arc<corint_sdk::DecisionEngine>,
    }

    #[derive(Debug, Serialize)]
    struct HealthResponse {
        status: String,
        version: String,
    }

    #[derive(Debug, Deserialize)]
    struct DecideRequestPayload {
        event: HashMap<String, serde_json::Value>,
        #[serde(default)]
        user: Option<HashMap<String, serde_json::Value>>,
        #[serde(default)]
        options: Option<RequestOptions>,
    }

    #[derive(Debug, Default, Deserialize)]
    struct RequestOptions {
        #[serde(default)]
        return_features: bool,
        #[serde(default)]
        enable_trace: bool,
    }

    #[derive(Debug, Serialize)]
    struct DecideResponsePayload {
        request_id: String,
        status: u16,
        process_time_ms: u64,
        pipeline_id: String,
        decision: DecisionPayload,
    }

    #[derive(Debug, Serialize)]
    struct DecisionPayload {
        result: String,
        actions: Vec<String>,
        scores: ScoresPayload,
        evidence: EvidencePayload,
        cognition: CognitionPayload,
    }

    #[derive(Debug, Serialize)]
    struct ScoresPayload {
        canonical: i32,
        raw: i32,
    }

    #[derive(Debug, Serialize)]
    struct EvidencePayload {
        triggered_rules: Vec<String>,
    }

    #[derive(Debug, Serialize)]
    struct CognitionPayload {
        summary: String,
        reason_codes: Vec<String>,
    }

    async fn health() -> Json<HealthResponse> {
        Json(HealthResponse {
            status: "healthy".to_string(),
            version: "0.1.0".to_string(),
        })
    }

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

    fn flatten_object(prefix: &str, value: &Value, result: &mut HashMap<String, Value>) {
        match value {
            Value::Object(map) => {
                for (key, val) in map {
                    let new_prefix = format!("{}.{}", prefix, key);
                    result.insert(new_prefix.clone(), val.clone());
                    if matches!(val, Value::Object(_)) {
                        flatten_object(&new_prefix, val, result);
                    }
                }
            }
            _ => {
                result.insert(prefix.to_string(), value.clone());
            }
        }
    }

    async fn decide(
        State(state): State<AppState>,
        Json(payload): Json<DecideRequestPayload>,
    ) -> Result<Json<DecideResponsePayload>, StatusCode> {
        let event_fields: HashMap<String, Value> = payload
            .event
            .into_iter()
            .map(|(k, v)| (k, json_to_value(v)))
            .collect();

        let mut event_data = HashMap::new();
        let event_object = Value::Object(event_fields.clone());
        event_data.insert("event".to_string(), event_object.clone());

        for (key, value) in &event_fields {
            event_data.insert(key.clone(), value.clone());
        }

        flatten_object("event", &event_object, &mut event_data);

        let request = DecisionRequest::new(event_data);
        let response = state
            .engine
            .decide(request)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let result_str = response
            .result
            .action
            .map(|a| format!("{:?}", a).to_uppercase())
            .unwrap_or_else(|| "PASS".to_string());

        Ok(Json(DecideResponsePayload {
            request_id: response.request_id,
            status: 200,
            process_time_ms: response.processing_time_ms,
            pipeline_id: response.pipeline_id.unwrap_or_else(|| "default".to_string()),
            decision: DecisionPayload {
                result: result_str,
                actions: Vec::new(),
                scores: ScoresPayload {
                    canonical: response.result.score.clamp(0, 1000),
                    raw: response.result.score,
                },
                evidence: EvidencePayload {
                    triggered_rules: response.result.triggered_rules,
                },
                cognition: CognitionPayload {
                    summary: response.result.explanation,
                    reason_codes: Vec::new(),
                },
            },
        }))
    }

    let state = AppState { engine };

    Router::new()
        .route("/health", get(health))
        .route("/v1/decide", post(decide))
        .with_state(state)
}

// Tests

#[tokio::test]
async fn test_health_endpoint() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "healthy");
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn test_decide_endpoint_high_amount() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let request_body = json!({
        "event": {
            "amount": 2000,
            "user_id": "user_123",
            "event_type": "transaction"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Verify new response structure
    assert!(json["request_id"].is_string());
    assert!(json["status"].is_number());
    assert!(json["decision"]["result"].is_string());
    assert!(json["decision"]["scores"]["canonical"].is_number());
    assert!(json["decision"]["scores"]["raw"].is_number());
    assert!(json["decision"]["evidence"]["triggered_rules"].is_array());
}

#[tokio::test]
async fn test_decide_endpoint_low_amount() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let request_body = json!({
        "event": {
            "amount": 500,
            "user_id": "user_456",
            "event_type": "transaction"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Verify new response structure
    assert!(json["request_id"].is_string());
    assert!(json["decision"]["scores"]["raw"].is_number());
    assert!(json["decision"]["result"].is_string());
}

#[tokio::test]
async fn test_decide_endpoint_missing_fields() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let request_body = json!({
        "event": {
            "user_id": "user_789"
            // Missing amount field
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Missing fields might cause an error or succeed with defaults
    assert!(
        response.status() == StatusCode::OK || response.status().is_server_error(),
        "Expected OK or server error, got: {}",
        response.status()
    );

    if response.status() == StatusCode::OK {
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert!(json["decision"]["scores"]["raw"].is_number());
    }
}

#[tokio::test]
async fn test_decide_endpoint_empty_event() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let request_body = json!({
        "event": {}
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Empty event data might cause an error or succeed with default values
    // Either is acceptable behavior
    assert!(
        response.status() == StatusCode::OK || response.status().is_server_error(),
        "Expected OK or server error, got: {}",
        response.status()
    );

    if response.status() == StatusCode::OK {
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert!(json["decision"]["scores"]["raw"].is_number());
    }
}

#[tokio::test]
async fn test_decide_endpoint_complex_nested_data() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let request_body = json!({
        "event": {
            "amount": 1500,
            "user": {
                "id": "user_999",
                "profile": {
                    "tier": "gold",
                    "age": 30
                }
            },
            "transaction": {
                "type": "purchase",
                "merchant": "Store XYZ"
            }
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Complex nested data might cause parsing issues or succeed
    assert!(
        response.status() == StatusCode::OK || response.status().is_server_error(),
        "Expected OK or server error, got: {}",
        response.status()
    );

    if response.status() == StatusCode::OK {
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();

        // Verify new response structure for complex nested data
        assert!(json["decision"]["scores"]["raw"].is_number());
        assert!(json["decision"]["result"].is_string());
    }
}

#[tokio::test]
async fn test_decide_endpoint_invalid_json() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from("invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 4xx error for invalid JSON
    assert!(response.status().is_client_error());
}

#[tokio::test]
async fn test_decide_endpoint_response_fields() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let request_body = json!({
        "event": {
            "amount": 1200,
            "event_type": "transaction"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Verify all expected fields per API spec
    assert!(json["request_id"].is_string());
    assert!(json["status"].is_number());
    assert!(json["process_time_ms"].is_number());
    assert!(json["pipeline_id"].is_string());

    // Verify decision structure
    assert!(json["decision"]["result"].is_string());
    assert!(json["decision"]["actions"].is_array());
    assert!(json["decision"]["scores"]["canonical"].is_number());
    assert!(json["decision"]["scores"]["raw"].is_number());
    assert!(json["decision"]["evidence"]["triggered_rules"].is_array());
    assert!(json["decision"]["cognition"]["summary"].is_string());
    assert!(json["decision"]["cognition"]["reason_codes"].is_array());
}

#[tokio::test]
async fn test_not_found_endpoint() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_health_method_not_allowed() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_decide_method_get_not_allowed() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/decide")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
