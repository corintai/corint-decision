//! Complete pipeline example
//!
//! This example demonstrates:
//! - Full SDK API with all features
//! - LLM integration for decision explanation
//! - Service calls for external data
//! - Metrics and observability
//! - Complete ruleset and pipeline execution

use corint_sdk::{
    DecisionEngineBuilder, DecisionRequest, Value,
    LLMConfig, LLMProvider, ServiceConfig, ServiceType,
};
use corint_runtime::observability::metrics::Metrics;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Complete Pipeline Example ===\n");

    // Configure LLM (using mock for this example)
    let llm_config = LLMConfig {
        provider: LLMProvider::Mock,
        api_key: "mock-key".to_string(),
        default_model: "gpt-4".to_string(),
        enable_cache: true,
    };

    // Configure external service (using mock for this example)
    let service_config = ServiceConfig {
        service_type: ServiceType::Mock,
        endpoint: "http://localhost:8080".to_string(),
    };

    // Build the decision engine with all features enabled
    let engine = DecisionEngineBuilder::new()
        .add_rule_file("examples/rules/complete_pipeline.yaml")
        .with_llm(llm_config)
        .with_service(service_config)
        .enable_metrics(true)
        .enable_tracing(true)
        .enable_semantic_analysis(true)
        .enable_constant_folding(true)
        .enable_dead_code_elimination(true)
        .build()
        .await?;

    println!("Complete pipeline engine initialized with:");
    println!("  - LLM integration (Mock)");
    println!("  - Service integration (Mock)");
    println!("  - Metrics enabled");
    println!("  - Tracing enabled");
    println!("  - All compiler optimizations enabled\n");

    // Create a comprehensive event with rich data
    let mut event_data = HashMap::new();

    // Transaction data
    event_data.insert("transaction_id".to_string(), Value::String("txn-12345".to_string()));
    event_data.insert("amount".to_string(), Value::Number(2500.0));
    event_data.insert("currency".to_string(), Value::String("USD".to_string()));

    // User data
    event_data.insert("user_id".to_string(), Value::String("user789".to_string()));
    event_data.insert("user_age_days".to_string(), Value::Number(45.0));
    event_data.insert("user_country".to_string(), Value::String("US".to_string()));
    event_data.insert("user_verified".to_string(), Value::Bool(true));

    // Device data
    event_data.insert("device_id".to_string(), Value::String("device-abc123".to_string()));
    event_data.insert("ip_address".to_string(), Value::String("192.168.1.1".to_string()));

    // Merchant data
    event_data.insert("merchant_id".to_string(), Value::String("merchant-xyz".to_string()));
    event_data.insert("merchant_category".to_string(), Value::String("electronics".to_string()));
    event_data.insert("merchant_country".to_string(), Value::String("CN".to_string()));

    let request = DecisionRequest::new(event_data)
        .with_metadata("request_id".to_string(), "req-pipeline-001".to_string())
        .with_metadata("source".to_string(), "api".to_string())
        .with_metadata("version".to_string(), "1.0".to_string());

    println!("Processing transaction:");
    println!("  Transaction ID: txn-12345");
    println!("  Amount: $2,500.00 USD");
    println!("  User: user789 (45 days old, verified)");
    println!("  Device: device-abc123");
    println!("  Merchant: merchant-xyz (electronics, CN)\n");

    // Execute the decision
    println!("Executing decision pipeline...\n");
    let start = std::time::Instant::now();
    let response = engine.decide(request).await?;
    let total_time = start.elapsed();

    // Display comprehensive results
    println!("=== Decision Results ===");
    println!("Action: {:?}", response.result.action);
    println!("Risk Score: {}", response.result.score);
    println!("Triggered Rules: {}", response.result.triggered_rules.len());

    if !response.result.triggered_rules.is_empty() {
        println!("\nTriggered Rules:");
        for rule in &response.result.triggered_rules {
            println!("  - {}", rule);
        }
    }

    if !response.result.explanation.is_empty() {
        println!("\nExplanation:");
        println!("  {}", response.result.explanation);
    }

    if !response.result.context.is_empty() {
        println!("\nContext Variables:");
        for (key, value) in &response.result.context {
            println!("  {}: {:?}", key, value);
        }
    }

    println!("\n=== Performance Metrics ===");
    println!("Processing Time: {}ms", response.processing_time_ms);
    println!("Total Time (including overhead): {}ms", total_time.as_millis());

    // Display engine metrics
    let metrics = engine.metrics();
    println!("\n=== Engine Metrics ===");
    println!("Total Executions: {}", metrics.counter("executions_total").get());

    if metrics.counter("llm_calls_success").get() > 0 {
        println!("Successful LLM Calls: {}", metrics.counter("llm_calls_success").get());
    }

    if metrics.counter("service_calls_success").get() > 0 {
        println!("Successful Service Calls: {}", metrics.counter("service_calls_success").get());
    }

    println!("\n=== Example Complete ===");
    println!("This example demonstrated:");
    println!("  ✓ Complete pipeline execution");
    println!("  ✓ Rich event data processing");
    println!("  ✓ LLM integration (mock)");
    println!("  ✓ Service integration (mock)");
    println!("  ✓ Metrics collection");
    println!("  ✓ Comprehensive result handling");

    Ok(())
}
