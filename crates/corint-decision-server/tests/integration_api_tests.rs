//! Integration tests for REST API endpoints
//!
//! These tests create a real DecisionEngine and test the API endpoints end-to-end.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use corint_decision_engine::DecisionEngineBuilder;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;
use tower::ServiceExt;

const DECISION_TOKEN: &str = "integration-decision-token-1234567890";

/// Helper to create a test decision engine with a simple pipeline
async fn create_test_engine() -> (TempDir, Arc<corint_decision_engine::DecisionEngine>) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create directory structure
    fs::create_dir_all(repo_path.join("pipelines"))
        .await
        .unwrap();
    fs::create_dir_all(repo_path.join("rules")).await.unwrap();
    fs::create_dir_all(repo_path.join("rulesets"))
        .await
        .unwrap();

    // Create a simple test rule
    let rule_yaml = r#"version: "0.1"

rule:
  id: test_rule
  name: Test Rule
  when:
    all:
      - "event.amount > 1000"
  score: 100
"#;

    fs::write(repo_path.join("rules/test_rule.yaml"), rule_yaml)
        .await
        .unwrap();

    // Create a test ruleset
    let ruleset_yaml = r#"version: "0.1"
import:
  rules: [rules/test_rule.yaml]
---
ruleset:
  id: test_ruleset
  name: Test Ruleset
  rules:
    - test_rule
  conclusion:
    - when: total_score >= 100
      signal: decline
    - when: total_score >= 50
      signal: review
    - default: true
      signal: approve
"#;

    fs::write(repo_path.join("rulesets/test_ruleset.yaml"), ruleset_yaml)
        .await
        .unwrap();

    // Create a pipeline
    let pipeline_yaml = r#"version: "0.1"
import:
  rulesets: [rulesets/test_ruleset.yaml]
---
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
  decision:
    - when: results.test_ruleset.score >= 100
      result: decline
    - default: true
      result: approve
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
  - when: event.type == "transaction"
    pipeline: test_pipeline
"#;

    fs::write(repo_path.join("registry.yaml"), registry_yaml)
        .await
        .unwrap();

    // Build decision engine
    let registry_path = repo_path.join("registry.yaml");

    let engine = DecisionEngineBuilder::new()
        .with_repository(corint_decision_engine::RepositoryConfig::file_system(
            repo_path.to_string_lossy(),
        ))
        .with_registry_file(registry_path)
        .build()
        .await
        .expect("Failed to build engine");

    (temp_dir, Arc::new(engine))
}

/// Test the production router, including authorization and response conversion.
fn create_test_router(engine: Arc<corint_decision_engine::DecisionEngine>) -> Router {
    let manager = Arc::new(corint_decision_server::snapshot::EngineManager::new(engine).unwrap());
    let access = corint_decision_server::access::AccessPolicy::new(
        DECISION_TOKEN,
        "integration-publisher-token-1234567890",
        "test",
    )
    .unwrap();
    corint_decision_server::api::create_router(manager, access)
}

// Tests

#[tokio::test]
async fn test_health_endpoint() {
    let (_temp, engine) = create_test_engine().await;
    let app = create_test_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
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
            "type": "transaction"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
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

    assert_eq!(json["decision"]["result"], "decline");
    assert_eq!(json["decision"]["scores"]["raw"], 100);
    assert_eq!(
        json["decision"]["evidence"]["triggered_rules"],
        json!(["test_rule"])
    );

    // Verify response structure
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
            "type": "transaction"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
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

    assert_eq!(json["decision"]["result"], "approve");
    assert_eq!(json["decision"]["scores"]["raw"], 0);
    assert_eq!(json["decision"]["evidence"]["triggered_rules"], json!([]));

    // Verify response structure
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
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    {
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
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    {
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
            "type": "transaction",
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
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
                .method("POST")
                .uri("/v1/decide")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    {
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
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
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
            "type": "transaction"
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
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
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
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
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
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
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
                .method("GET")
                .uri("/v1/decide")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
