//! Simple rule execution example
//!
//! This example demonstrates:
//! - Creating a DecisionEngine with a simple rule
//! - Executing a decision request
//! - Handling the decision response

use corint_sdk::{DecisionEngineBuilder, DecisionRequest, Value};
use corint_runtime::observability::metrics::Metrics;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Simple Rule Example ===\n");

    // Build the decision engine with a rule file
    let engine = DecisionEngineBuilder::new()
        .add_rule_file("examples/pipelines/simple_rule.yaml")
        .enable_metrics(true)
        .enable_semantic_analysis(true)
        .build()
        .await?;

    println!("Decision engine initialized successfully\n");

    // Create a decision request with event data
    let mut event_data = HashMap::new();
    event_data.insert("event_type".to_string(), Value::String("transaction3".to_string()));
    event_data.insert("amount".to_string(), Value::Number(150.0));
    event_data.insert("user_id".to_string(), Value::String("user123".to_string()));
    event_data.insert("country".to_string(), Value::String("US".to_string()));

    let request = DecisionRequest::new(event_data)
        .with_metadata("request_id".to_string(), "req-001".to_string());

    println!("Event data:");
    println!("  amount: 150.0");
    println!("  user_id: user123");
    println!("  country: US\n");

    // Execute the decision
    let response = engine.decide(request).await?;

    // Display the results
    println!("Decision Results:");
    println!("  Request ID: {}", response.request_id);
    println!("  Action: {:?}", response.result.action);
    println!("  Score: {}", response.result.score);
    println!("  Triggered Rules: {:?}", response.result.triggered_rules);
    println!("  Processing Time: {}ms", response.processing_time_ms);

    if !response.result.explanation.is_empty() {
        println!("  Explanation: {}", response.result.explanation);
    }

    // Display metrics
    let metrics = engine.metrics();
    println!("\nMetrics:");
    println!("  Total Executions: {}", metrics.counter("executions_total").get());

    Ok(())
}
