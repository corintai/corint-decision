use corint_sdk::{DecisionEngine, DecisionRequest};
use std::collections::HashMap;
use corint_core::Value;

#[test]
fn test_registry_matching_simple_expression() {
    // Test that event.type == "test1" correctly triggers comprehensive_risk_assessment
    let engine = DecisionEngine::from_file_system("../../repository").unwrap();

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("test1".to_string()));
    event_data.insert("user_id".to_string(), Value::String("user123".to_string()));

    let request = DecisionRequest {
        event_data,
        features: None,
        api: None,
        service: None,
        llm: None,
        variables: None,
        options: corint_sdk::DecisionOptions {
            enable_trace: true,
        },
    };

    let response = engine.decide(&request).unwrap();

    // Check that the correct pipeline was triggered
    assert!(response.trace.is_some(), "Trace should be present");
    let trace = response.trace.unwrap();
    let pipeline_id = trace.pipeline.as_ref().and_then(|p| p.pipeline_id.as_ref());

    assert_eq!(
        pipeline_id,
        Some(&"comprehensive_risk_assessment".to_string()),
        "Expected comprehensive_risk_assessment pipeline to be triggered"
    );
}

#[test]
fn test_registry_matching_with_all_condition() {
    // Test that event.type == "transaction" AND event.source == "supabase" works
    let engine = DecisionEngine::from_file_system("../../repository").unwrap();

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("source".to_string(), Value::String("supabase".to_string()));

    let request = DecisionRequest {
        event_data,
        features: None,
        api: None,
        service: None,
        llm: None,
        variables: None,
        options: corint_sdk::DecisionOptions {
            enable_trace: true,
        },
    };

    let response = engine.decide(&request).unwrap();

    // Check that the correct pipeline was triggered
    assert!(response.trace.is_some(), "Trace should be present");
    let trace = response.trace.unwrap();
    let pipeline_id = trace.pipeline.as_ref().and_then(|p| p.pipeline_id.as_ref());

    assert_eq!(
        pipeline_id,
        Some(&"supabase_transaction_pipeline".to_string()),
        "Expected supabase_transaction_pipeline to be triggered with all condition"
    );
}

#[test]
fn test_registry_matching_fallback() {
    // Test that event.type == "transaction" (without source) triggers fraud_detection_pipeline
    let engine = DecisionEngine::from_file_system("../../repository").unwrap();

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("user_id".to_string(), Value::String("user456".to_string()));

    let request = DecisionRequest {
        event_data,
        features: None,
        api: None,
        service: None,
        llm: None,
        variables: None,
        options: corint_sdk::DecisionOptions {
            enable_trace: true,
        },
    };

    let response = engine.decide(&request).unwrap();

    // Check that the correct pipeline was triggered
    assert!(response.trace.is_some(), "Trace should be present");
    let trace = response.trace.unwrap();
    let pipeline_id = trace.pipeline.as_ref().and_then(|p| p.pipeline_id.as_ref());

    assert_eq!(
        pipeline_id,
        Some(&"fraud_detection_pipeline".to_string()),
        "Expected fraud_detection_pipeline to be triggered"
    );
}
