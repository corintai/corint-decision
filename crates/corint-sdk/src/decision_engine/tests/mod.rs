//! Unit tests for DecisionEngine

use super::*;
use crate::config::EngineConfig;
use corint_core::Value;
use std::collections::HashMap;

#[test]
fn test_decision_request() {
    let mut event_data = HashMap::new();
    event_data.insert("user_id".to_string(), Value::String("123".to_string()));

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("request_id".to_string(), "req-123".to_string());

    assert_eq!(request.event_data.len(), 1);
    assert_eq!(request.metadata.len(), 1);
}

#[tokio::test]
async fn test_engine_creation() {
    let config = EngineConfig::new();

    let engine = DecisionEngine::new(config).await;
    assert!(engine.is_ok());
}

#[tokio::test]
async fn test_ruleset_execution() {
    use crate::builder::DecisionEngineBuilder;
    use corint_core::ast::Signal;

    // Create a temporary YAML file with pipeline
    let yaml_content = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  when:
    event.type: test
  steps:
  - include:
      ruleset: test_execution

---

ruleset:
  id: test_execution
  name: Test Execution
  rules: []
  conclusion:
  - when: amount > 100
    signal: review
  - default: true
    signal: approve
"#;
    let temp_file = "/tmp/test_ruleset_exec.yaml";
    std::fs::write(temp_file, yaml_content).unwrap();

    let engine = DecisionEngineBuilder::new()
        .add_rule_file(temp_file)
        .build()
        .await
        .unwrap();

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("test".to_string()));
    event_data.insert("amount".to_string(), Value::Number(150.0));

    let request = DecisionRequest::new(event_data);
    let response = engine.decide(request).await.unwrap();

    println!("Test ruleset execution:");
    println!("  Action: {:?}", response.result.signal);
    println!("  Score: {}", response.result.score);

    // Should be Review because 150 > 100
    assert!(response.result.signal.is_some());
    assert!(matches!(response.result.signal, Some(Signal::Review)));
}

#[tokio::test]
async fn test_fraud_detection_ruleset() {
    use crate::builder::DecisionEngineBuilder;
    use corint_core::ast::Signal;

    // Create a temporary YAML file matching fraud_detection.yaml
    let yaml_content = r#"
pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline
  when:
    event.type: transaction
  steps:
  - include:
      ruleset: fraud_detection

---

ruleset:
  id: fraud_detection
  name: Fraud Detection Ruleset
  description: Simple fraud detection based on transaction amount
  rules: []
  conclusion:
  - when: transaction_amount > 10000
    signal: decline
    reason: Extremely high value transaction
    terminate: true
  - when: transaction_amount > 1000
    signal: review
    reason: High value transaction
  - when: transaction_amount > 100
    signal: review
    reason: Elevated transaction amount
  - default: true
    signal: approve
"#;
    let temp_file = "/tmp/test_fraud_detection.yaml";
    std::fs::write(temp_file, yaml_content).unwrap();

    let engine = DecisionEngineBuilder::new()
        .add_rule_file(temp_file)
        .build()
        .await
        .unwrap();

    // Test Case 1: Normal transaction (50.0) - should approve
    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("transaction_amount".to_string(), Value::Number(50.0));
    let request = DecisionRequest::new(event_data.clone());
    let response = engine.decide(request).await.unwrap();
    println!("Test Case 1 (50.0):");
    println!("  Action: {:?}", response.result.signal);
    println!("  Score: {}", response.result.score);
    println!("  Triggered Rules: {:?}", response.result.triggered_rules);
    assert!(
        matches!(response.result.signal, Some(Signal::Approve)),
        "Expected Some(Approve) but got {:?}",
        response.result.signal
    );

    // Test Case 2: High value (5000.0) - should review
    event_data.insert("transaction_amount".to_string(), Value::Number(5000.0));
    let request = DecisionRequest::new(event_data.clone());
    let response = engine.decide(request).await.unwrap();
    println!("Test Case 2 (5000.0): Action: {:?}", response.result.signal);
    assert!(matches!(response.result.signal, Some(Signal::Review)));

    // Test Case 3: Very high value (15000.0) - should decline
    event_data.insert("transaction_amount".to_string(), Value::Number(15000.0));
    let request = DecisionRequest::new(event_data);
    let response = engine.decide(request).await.unwrap();
    println!(
        "Test Case 3 (15000.0): Action: {:?}",
        response.result.signal
    );
    assert!(matches!(response.result.signal, Some(Signal::Decline)));
}

#[tokio::test]
async fn test_decide_request_id_generation() {
    use crate::builder::DecisionEngineBuilder;

    let yaml_content = r#"
pipeline:
  id: simple_pipeline
  name: Simple Pipeline
  when:
    event.type: test
  steps:
  - include:
      ruleset: simple_ruleset

---

ruleset:
  id: simple_ruleset
  name: Simple Ruleset
  rules: []
  conclusion:
  - default: true
    signal: approve
"#;
    let temp_file = "/tmp/test_request_id.yaml";
    std::fs::write(temp_file, yaml_content).unwrap();

    let engine = DecisionEngineBuilder::new()
        .add_rule_file(temp_file)
        .build()
        .await
        .unwrap();

    // Test Case 1: Auto-generated request ID
    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("test".to_string()));

    let request = DecisionRequest::new(event_data.clone());
    let response = engine.decide(request).await.unwrap();

    println!("Auto-generated request_id: {}", response.request_id);
    assert!(response.request_id.starts_with("req_"));
    assert!(response.request_id.len() > 10);

    // Test Case 2: Custom request ID
    let request = DecisionRequest::new(event_data)
        .with_metadata("request_id".to_string(), "custom_req_123".to_string());
    let response = engine.decide(request).await.unwrap();

    println!("Custom request_id: {}", response.request_id);
    assert_eq!(response.request_id, "custom_req_123");
}

#[tokio::test]
async fn test_decide_metadata_handling() {
    use crate::builder::DecisionEngineBuilder;

    let yaml_content = r#"
pipeline:
  id: metadata_pipeline
  name: Metadata Pipeline
  when:
    event.type: test
  steps:
  - include:
      ruleset: metadata_ruleset

---

ruleset:
  id: metadata_ruleset
  name: Metadata Ruleset
  rules: []
  conclusion:
  - default: true
    signal: approve
"#;
    let temp_file = "/tmp/test_metadata.yaml";
    std::fs::write(temp_file, yaml_content).unwrap();

    let engine = DecisionEngineBuilder::new()
        .add_rule_file(temp_file)
        .build()
        .await
        .unwrap();

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("test".to_string()));

    let request = DecisionRequest::new(event_data)
        .with_metadata("event_id".to_string(), "evt_123".to_string())
        .with_metadata("source".to_string(), "mobile_app".to_string())
        .with_metadata("user_agent".to_string(), "iOS/14.5".to_string());

    let response = engine.decide(request).await.unwrap();

    // Verify metadata is preserved in response
    assert_eq!(
        response.metadata.get("event_id"),
        Some(&"evt_123".to_string())
    );
    assert_eq!(
        response.metadata.get("source"),
        Some(&"mobile_app".to_string())
    );
    assert_eq!(
        response.metadata.get("user_agent"),
        Some(&"iOS/14.5".to_string())
    );
    // Note: request_id may also be added to metadata, so length could be 3 or 4
    assert!(response.metadata.len() >= 3);

    println!("Metadata preserved: {:?}", response.metadata);
}

#[tokio::test]
async fn test_decide_processing_time() {
    use crate::builder::DecisionEngineBuilder;

    let yaml_content = r#"
pipeline:
  id: timing_pipeline
  name: Timing Pipeline
  when:
    event.type: test
  steps:
  - include:
      ruleset: timing_ruleset

---

ruleset:
  id: timing_ruleset
  name: Timing Ruleset
  rules: []
  conclusion:
  - when: value > 100
    signal: review
  - default: true
    signal: approve
"#;
    let temp_file = "/tmp/test_timing.yaml";
    std::fs::write(temp_file, yaml_content).unwrap();

    let engine = DecisionEngineBuilder::new()
        .add_rule_file(temp_file)
        .build()
        .await
        .unwrap();

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("test".to_string()));
    event_data.insert("value".to_string(), Value::Number(150.0));

    let request = DecisionRequest::new(event_data);
    let response = engine.decide(request).await.unwrap();

    // Verify processing time is recorded and reasonable
    // Note: May be 0 for very fast operations, which is acceptable
    assert!(response.processing_time_ms < 1000); // Should complete in less than 1 second

    println!("Processing time: {}ms", response.processing_time_ms);
}

#[tokio::test]
async fn test_decide_with_missing_fields() {
    use crate::builder::DecisionEngineBuilder;
    use corint_core::ast::Signal;

    let yaml_content = r#"
pipeline:
  id: missing_field_pipeline
  name: Missing Field Pipeline
  when:
    event.type: test
  steps:
  - include:
      ruleset: missing_field_ruleset

---

ruleset:
  id: missing_field_ruleset
  name: Missing Field Ruleset
  rules: []
  conclusion:
  - when: optional_field > 100
    signal: review
  - default: true
    signal: approve
"#;
    let temp_file = "/tmp/test_missing_field.yaml";
    std::fs::write(temp_file, yaml_content).unwrap();

    let engine = DecisionEngineBuilder::new()
        .add_rule_file(temp_file)
        .build()
        .await
        .unwrap();

    // Test with missing optional_field - should use default action
    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("test".to_string()));
    // Note: optional_field is not provided

    let request = DecisionRequest::new(event_data);
    let response = engine.decide(request).await.unwrap();

    println!("Missing field test:");
    println!("  Action: {:?}", response.result.signal);

    // Should approve because condition fails (Null > 100 is false)
    assert!(matches!(response.result.signal, Some(Signal::Approve)));
}

#[tokio::test]
async fn test_decide_with_content_api() {
    use crate::builder::DecisionEngineBuilder;
    use corint_core::ast::Signal;

    // Simulate loading from repository/API
    let rule_content = r#"
pipeline:
  id: content_pipeline
  name: Content Pipeline
  when:
    event.type: api_test
  steps:
  - include:
      ruleset: content_ruleset

---

ruleset:
  id: content_ruleset
  name: Content Ruleset
  rules: []
  conclusion:
  - when: risk_score > 50
    signal: decline
  - default: true
    signal: approve
"#;

    let engine = DecisionEngineBuilder::new()
        .add_rule_content("content_pipeline", rule_content)
        .build()
        .await
        .unwrap();

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("api_test".to_string()));
    event_data.insert("risk_score".to_string(), Value::Number(75.0));

    let request = DecisionRequest::new(event_data);
    let response = engine.decide(request).await.unwrap();

    println!("Content API test:");
    println!("  Action: {:?}", response.result.signal);

    assert!(matches!(response.result.signal, Some(Signal::Decline)));
}

#[tokio::test]
async fn test_builder_config_options() {
    use crate::builder::DecisionEngineBuilder;

    let yaml_content = r#"
pipeline:
  id: config_test_pipeline
  name: Config Test Pipeline
  when:
    event.type: test
  steps:
  - include:
      ruleset: config_test_ruleset

---

ruleset:
  id: config_test_ruleset
  name: Config Test Ruleset
  rules: []
  conclusion:
  - default: true
    signal: approve
"#;
    let temp_file = "/tmp/test_config.yaml";
    std::fs::write(temp_file, yaml_content).unwrap();

    // Test with various configuration options
    let engine = DecisionEngineBuilder::new()
        .add_rule_file(temp_file)
        .enable_metrics(true)
        .enable_tracing(false)
        .enable_semantic_analysis(true)
        .enable_constant_folding(true)
        .enable_dead_code_elimination(true)
        .build()
        .await
        .unwrap();

    // Verify configuration is applied
    let config = engine.config();
    assert!(config.enable_metrics);
    assert!(!config.enable_tracing);
    assert!(config.compiler_options.enable_semantic_analysis);
    assert!(config.compiler_options.enable_constant_folding);
    assert!(config.compiler_options.enable_dead_code_elimination);

    // Test execution still works
    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("test".to_string()));

    let request = DecisionRequest::new(event_data);
    let response = engine.decide(request).await.unwrap();

    assert!(response.request_id.starts_with("req_"));
    println!(
        "Config test passed with request_id: {}",
        response.request_id
    );
}

#[tokio::test]
async fn test_rules_in_ruleset() {
    use crate::builder::DecisionEngineBuilder;
    use corint_core::ast::Signal;

    // Test that rules defined in the same file are correctly loaded and executed
    let yaml_content = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  when:
    event.type: test
  steps:
  - include:
      ruleset: test_ruleset

---

rule:
  id: high_risk
  name: High Risk Rule
  when:
    conditions:
    - event.risk_level > 80
  score: 100

---

ruleset:
  id: test_ruleset
  rules:
  - high_risk
  conclusion:
  - when: total_score >= 100
    signal: decline
  - default: true
    signal: approve
"#;
    let temp_file = "/tmp/test_rules_in_ruleset.yaml";
    std::fs::write(temp_file, yaml_content).unwrap();

    let engine = DecisionEngineBuilder::new()
        .add_rule_file(temp_file)
        .build()
        .await
        .unwrap();

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("test".to_string()));
    event_data.insert("risk_level".to_string(), Value::Number(90.0));

    let request = DecisionRequest::new(event_data).with_trace();
    let response = engine.decide(request).await.unwrap();

    println!("Score: {}", response.result.score);
    println!("Triggered rules: {:?}", response.result.triggered_rules);
    println!("Action: {:?}", response.result.signal);

    assert_eq!(response.result.score, 100, "Rule should add 100 points");
    assert!(
        response.result.triggered_rules.contains(&"high_risk".to_string()),
        "high_risk rule should be triggered"
    );
    assert!(matches!(response.result.signal, Some(Signal::Decline)));
}
