//! Multi-Ruleset Example
//!
//! This example demonstrates multiple rulesets working together:
//! - Stage 1: Risk Detection Rules (5 rules calculate scores)
//! - Stage 2: First Ruleset (transaction_router)
//! - Stage 3: Second Ruleset (high_value_pipeline)
//! - Stage 4: Third Ruleset (standard_pipeline - final action wins)
//!
//! Key concepts:
//! - Multiple rulesets execute sequentially
//! - Rules execute first to accumulate scores
//! - Last ruleset's action becomes the final decision
//!
//! Limitations:
//! - No conditional routing (all rulesets always execute)
//! - True Pipeline DSL with routing is documented but not yet implemented

use corint_sdk::{DecisionEngineBuilder, DecisionRequest, Value, Action};
use corint_runtime::observability::metrics::Metrics;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing_subscriber::filter::LevelFilter::INFO.into()))
        .init();

    println!("=== Complete Payment Risk Control Example ===\n");
    println!("Multi-Ruleset Architecture (Sequential Execution):");
    println!("  Stage 1: Risk Detection Rules (5 rules)");
    println!("  Stage 2: First Ruleset (transaction_router)");
    println!("  Stage 3: Second Ruleset (high_value_pipeline)");
    println!("  Stage 4: Third Ruleset (standard_pipeline - final action)\n");
    println!("NOTE: All rulesets execute sequentially. The last one's action wins.\n");

    // Build the decision engine
    println!("Loading multi-stage pipeline...");
    let engine = DecisionEngineBuilder::new()
        .add_rule_file("examples/pipelines/complete_pipeline.yaml")
        .enable_metrics(true)
        .build()
        .await?;
    println!("✓ Pipeline initialized with 5 rules + 3 rulesets\n");

    // Test Case 1: Clean Standard Transaction
    println!("=== Test Case 1: Clean Standard Transaction ===");
    test_clean_standard(&engine).await?;

    // Test Case 2: Clean High-Value Transaction
    println!("\n=== Test Case 2: Clean High-Value Transaction ===");
    test_clean_high_value(&engine).await?;

    // Test Case 3: Card Testing (Critical - Blocked by Router)
    println!("\n=== Test Case 3: Card Testing Pattern ===");
    test_card_testing(&engine).await?;

    // Test Case 4: Standard Transaction with Medium Risk
    println!("\n=== Test Case 4: Standard Transaction + Medium Risk ===");
    test_standard_medium_risk(&engine).await?;

    // Test Case 5: High-Value with Single Risk Indicator
    println!("\n=== Test Case 5: High-Value + Single Risk ===");
    test_high_value_single_risk(&engine).await?;

    // Test Case 6: High-Value with Multiple Risks
    println!("\n=== Test Case 6: High-Value + Multiple Risks ===");
    test_high_value_multiple_risks(&engine).await?;

    // Display metrics
    display_metrics(&engine);

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  ✓ Multiple rulesets can work together sequentially");
    println!("  ✓ Rules execute first to calculate scores");
    println!("  ✓ Rulesets execute in order, last action wins");
    println!("  ✓ This demonstrates multi-ruleset coordination");
    println!("\nLimitations:");
    println!("  ✗ No conditional pipeline routing (all rulesets always execute)");
    println!("  ✗ True Pipeline DSL (with routing) is documented but not implemented");
    println!("  ✗ See docs/dsl/pipeline.md for the full pipeline specification");

    Ok(())
}

/// Test Case 1: Clean standard transaction (<= $1000)
async fn test_clean_standard(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    event_data.insert("event.type".to_string(), Value::String("payment".to_string()));
    event_data.insert("payment_amount".to_string(), Value::Number(250.0));  // Standard amount
    event_data.insert("country_code".to_string(), Value::String("US".to_string()));
    event_data.insert("payment_attempts_1h".to_string(), Value::Number(1.0));
    event_data.insert("unique_cards_1h".to_string(), Value::Number(1.0));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(5.0));
    event_data.insert("account_age_days".to_string(), Value::Number(180.0));
    event_data.insert("is_disposable_email".to_string(), Value::Bool(false));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Amount: $250 (Standard Pipeline)");
    println!("Expected: APPROVE (router approves, no risks)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    assert!(matches!(response.result.action, Some(Action::Approve)));

    Ok(())
}

/// Test Case 2: Clean high-value transaction (> $1000)
async fn test_clean_high_value(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    event_data.insert("event.type".to_string(), Value::String("payment".to_string()));
    event_data.insert("payment_amount".to_string(), Value::Number(1500.0));  // High-value
    event_data.insert("country_code".to_string(), Value::String("US".to_string()));
    event_data.insert("payment_attempts_1h".to_string(), Value::Number(1.0));
    event_data.insert("unique_cards_1h".to_string(), Value::Number(1.0));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(3.0));
    event_data.insert("account_age_days".to_string(), Value::Number(365.0));
    event_data.insert("is_disposable_email".to_string(), Value::Bool(false));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Amount: $1500 (High-Value Pipeline)");
    println!("Expected: APPROVE (clean high-value, all rulesets execute)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    // All rulesets execute sequentially, clean transaction approves
    assert!(matches!(response.result.action, Some(Action::Approve)));

    Ok(())
}

/// Test Case 3: Card testing pattern - blocked by router
async fn test_card_testing(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    event_data.insert("event.type".to_string(), Value::String("payment".to_string()));
    event_data.insert("payment_amount".to_string(), Value::Number(5.0));  // Small amount
    event_data.insert("payment_attempts_1h".to_string(), Value::Number(8.0));  // >= 5
    event_data.insert("unique_cards_1h".to_string(), Value::Number(5.0));  // >= 3
    event_data.insert("country_code".to_string(), Value::String("US".to_string()));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(15.0));
    event_data.insert("account_age_days".to_string(), Value::Number(100.0));
    event_data.insert("is_disposable_email".to_string(), Value::Bool(false));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Amount: $5, 8 attempts, 5 cards (Card Testing)");
    println!("Expected: DENY (router blocks immediately - score 80)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    assert!(matches!(response.result.action, Some(Action::Deny)));

    Ok(())
}

/// Test Case 4: Standard transaction with medium risk
async fn test_standard_medium_risk(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    event_data.insert("event.type".to_string(), Value::String("payment".to_string()));
    event_data.insert("payment_amount".to_string(), Value::Number(400.0));  // Standard
    event_data.insert("country_code".to_string(), Value::String("NG".to_string()));  // High-risk (30 pts)
    event_data.insert("is_disposable_email".to_string(), Value::Bool(true));  // Suspicious (35 pts)
    event_data.insert("payment_attempts_1h".to_string(), Value::Number(2.0));
    event_data.insert("unique_cards_1h".to_string(), Value::Number(1.0));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(8.0));
    event_data.insert("account_age_days".to_string(), Value::Number(90.0));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Amount: $400, Nigeria + disposable email");
    println!("Expected: CHALLENGE (standard pipeline: score 65, >= 60)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    // Standard pipeline executes last, score 65 >= 60 triggers Challenge
    assert!(matches!(response.result.action, Some(Action::Challenge)));

    Ok(())
}

/// Test Case 5: High-value with single risk indicator
async fn test_high_value_single_risk(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    event_data.insert("event.type".to_string(), Value::String("payment".to_string()));
    event_data.insert("payment_amount".to_string(), Value::Number(1200.0));  // High-value
    event_data.insert("country_code".to_string(), Value::String("ID".to_string()));  // Indonesia (30 pts)
    event_data.insert("payment_attempts_1h".to_string(), Value::Number(1.0));
    event_data.insert("unique_cards_1h".to_string(), Value::Number(1.0));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(5.0));
    event_data.insert("account_age_days".to_string(), Value::Number(200.0));
    event_data.insert("is_disposable_email".to_string(), Value::Bool(false));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Amount: $1200, Indonesia (single risk)");
    println!("Expected: APPROVE (score 30, below all thresholds)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    // Score 30 is below 40/60 thresholds in all pipelines, defaults to Approve
    assert!(matches!(response.result.action, Some(Action::Approve)));

    Ok(())
}

/// Test Case 6: High-value with multiple risks
async fn test_high_value_multiple_risks(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    event_data.insert("event.type".to_string(), Value::String("payment".to_string()));
    event_data.insert("payment_amount".to_string(), Value::Number(1800.0));  // High-value
    event_data.insert("account_age_days".to_string(), Value::Number(5.0));  // New account (60 pts - triggers)
    event_data.insert("country_code".to_string(), Value::String("NG".to_string()));  // High-risk (30 pts)
    event_data.insert("payment_attempts_1h".to_string(), Value::Number(2.0));
    event_data.insert("unique_cards_1h".to_string(), Value::Number(1.0));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(3.0));
    event_data.insert("is_disposable_email".to_string(), Value::Bool(false));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Amount: $1800, 5-day account + Nigeria");
    println!("Expected: CHALLENGE (standard pipeline: score 90, >= 60)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    // Standard pipeline: score 90 >= 60 triggers Challenge
    assert!(matches!(response.result.action, Some(Action::Challenge)));

    Ok(())
}

/// Display engine metrics
fn display_metrics(engine: &corint_sdk::DecisionEngine) {
    let metrics = engine.metrics();

    println!("\n=== Pipeline Metrics ===");
    println!("Total Executions: {}", metrics.counter("executions_total").get());
}
