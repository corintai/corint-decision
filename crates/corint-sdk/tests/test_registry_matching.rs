use corint_sdk::{DecisionEngineBuilder, DecisionRequest, RepositoryConfig};
use std::collections::HashMap;
use corint_core::Value;

#[tokio::test]
#[ignore = "Depends on repository files that may not be present"]
async fn test_registry_matching_simple_expression() {
    // Test that event.type == "test1" correctly triggers comprehensive_risk_assessment
    let engine = DecisionEngineBuilder::new()
        .with_repository(RepositoryConfig::file_system("../../repository"))
        .build()
        .await
        .expect("Failed to build engine");

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("test1".to_string()));
    event_data.insert("user_id".to_string(), Value::String("user123".to_string()));

    let request = DecisionRequest::new(event_data).with_trace();

    let response = engine.decide(request).await.expect("Decision failed");

    // Check that the correct pipeline was triggered
    assert!(response.trace.is_some(), "Trace should be present");
    let trace = response.trace.unwrap();
    let pipeline_id = trace.pipeline.as_ref().map(|p| &p.pipeline_id);

    assert_eq!(
        pipeline_id,
        Some(&"comprehensive_risk_assessment".to_string()),
        "Expected comprehensive_risk_assessment pipeline to be triggered"
    );
}

#[tokio::test]
#[ignore = "Depends on repository files that may not be present"]
async fn test_registry_matching_with_all_condition() {
    // Test that event.type == "transaction" AND event.source == "supabase" works
    let engine = DecisionEngineBuilder::new()
        .with_repository(RepositoryConfig::file_system("../../repository"))
        .build()
        .await
        .expect("Failed to build engine");

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("source".to_string(), Value::String("supabase".to_string()));

    let request = DecisionRequest::new(event_data).with_trace();

    let response = engine.decide(request).await.expect("Decision failed");

    // Check that the correct pipeline was triggered
    assert!(response.trace.is_some(), "Trace should be present");
    let trace = response.trace.unwrap();
    let pipeline_id = trace.pipeline.as_ref().map(|p| &p.pipeline_id);

    assert_eq!(
        pipeline_id,
        Some(&"supabase_transaction_pipeline".to_string()),
        "Expected supabase_transaction_pipeline to be triggered with all condition"
    );
}

#[tokio::test]
#[ignore = "Depends on repository files that may not be present"]
async fn test_registry_matching_fallback() {
    // Test that event.type == "transaction" (without source) triggers fraud_detection_pipeline
    let engine = DecisionEngineBuilder::new()
        .with_repository(RepositoryConfig::file_system("../../repository"))
        .build()
        .await
        .expect("Failed to build engine");

    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("user_id".to_string(), Value::String("user456".to_string()));

    let request = DecisionRequest::new(event_data).with_trace();

    let response = engine.decide(request).await.expect("Decision failed");

    // Check that the correct pipeline was triggered
    assert!(response.trace.is_some(), "Trace should be present");
    let trace = response.trace.unwrap();
    let pipeline_id = trace.pipeline.as_ref().map(|p| &p.pipeline_id);

    assert_eq!(
        pipeline_id,
        Some(&"fraud_detection_pipeline".to_string()),
        "Expected fraud_detection_pipeline to be triggered"
    );
}
