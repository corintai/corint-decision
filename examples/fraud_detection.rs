//! Fraud detection example
//!
//! This example demonstrates:
//! - Real-world fraud detection scenario
//! - Multiple transaction checks
//! - Risk scoring and action determination

use corint_sdk::{DecisionEngineBuilder, DecisionRequest, Value};
use corint_runtime::observability::metrics::Metrics;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Fraud Detection Example ===\n");

    // Build the decision engine with fraud detection rules
    let engine = DecisionEngineBuilder::new()
        .add_rule_file("examples/rules/fraud_detection.yaml")
        .enable_metrics(true)
        .enable_tracing(true)
        .build()
        .await?;

    println!("Fraud detection engine initialized\n");

    // Test Case 1: Normal transaction
    println!("--- Test Case 1: Normal Transaction ---");
    let mut event_data = HashMap::new();
    event_data.insert("transaction_amount".to_string(), Value::Number(50.0));
    event_data.insert("user_id".to_string(), Value::String("user456".to_string()));
    event_data.insert("merchant_country".to_string(), Value::String("US".to_string()));
    event_data.insert("user_country".to_string(), Value::String("US".to_string()));
    event_data.insert("device_fingerprint".to_string(), Value::String("known_device_123".to_string()));

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("request_id".to_string(), "txn-001".to_string());

    let response = engine.decide(request).await?;
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Processing Time: {}ms\n", response.processing_time_ms);

    // Test Case 2: High-value transaction
    println!("--- Test Case 2: High-Value Transaction ---");
    event_data.insert("transaction_amount".to_string(), Value::Number(5000.0));

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("request_id".to_string(), "txn-002".to_string());

    let response = engine.decide(request).await?;
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);
    println!("Processing Time: {}ms\n", response.processing_time_ms);

    // Test Case 3: Cross-border transaction
    println!("--- Test Case 3: Cross-Border Transaction ---");
    event_data.insert("transaction_amount".to_string(), Value::Number(200.0));
    event_data.insert("merchant_country".to_string(), Value::String("NG".to_string()));

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("request_id".to_string(), "txn-003".to_string());

    let response = engine.decide(request).await?;
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);
    println!("Processing Time: {}ms\n", response.processing_time_ms);

    // Test Case 4: Unknown device + high value
    println!("--- Test Case 4: Unknown Device + High Value ---");
    event_data.insert("transaction_amount".to_string(), Value::Number(1000.0));
    event_data.insert("device_fingerprint".to_string(), Value::String("unknown_device_999".to_string()));
    event_data.insert("merchant_country".to_string(), Value::String("US".to_string()));

    let request = DecisionRequest::new(event_data)
        .with_metadata("request_id".to_string(), "txn-004".to_string());

    let response = engine.decide(request).await?;
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);
    println!("Processing Time: {}ms\n", response.processing_time_ms);

    // Display overall metrics
    let metrics = engine.metrics();
    println!("=== Overall Metrics ===");
    println!("Total Executions: {}", metrics.counter("executions_total").get());

    Ok(())
}
