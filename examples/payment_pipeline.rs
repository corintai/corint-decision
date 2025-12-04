//! Payment Pipeline Example
//!
//! This example demonstrates the Pipeline DSL with conditional branch routing.
//! It shows how transactions are routed to different rulesets based on the payment amount:
//! - High-value transactions (> $1000) → high_value_rules (stricter thresholds)
//! - Standard transactions (≤ $1000) → standard_rules (balanced thresholds)

use corint_sdk::builder::DecisionEngineBuilder;
use corint_sdk::decision_engine::DecisionRequest;
use corint_sdk::Value;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("corint_runtime=debug".parse()?)
                .add_directive("corint_sdk=debug".parse()?)
        )
        .init();

    println!("{}", "=".repeat(80));
    println!("Payment Pipeline Example - Conditional Branch Routing");
    println!("{}", "=".repeat(80));
    println!();

    // Build the decision engine with payment_pipeline.yaml
    let engine = DecisionEngineBuilder::new()
        .add_rule_file("examples/rules/payment_pipeline.yaml")
        .build()
        .await?;

    println!("✓ Decision engine loaded successfully");
    println!();

    // Test Case 1: Standard transaction ($500) - should route to standard_rules
    println!("{}", "-".repeat(80));
    println!("Test Case 1: Standard Transaction ($500)");
    println!("{}", "-".repeat(80));

    let mut event_data = HashMap::new();
    event_data.insert("payment_amount".to_string(), Value::Number(500.0));
    event_data.insert("ip_address".to_string(), Value::String("8.8.8.8".to_string())); // US IP (Google DNS)
    event_data.insert("payment_attempts_1h".to_string(), Value::Number(2.0));
    event_data.insert("unique_cards_1h".to_string(), Value::Number(1.0));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(5.0));
    event_data.insert("account_age_days".to_string(), Value::Number(30.0));
    event_data.insert("is_disposable_email".to_string(), Value::Bool(false));

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("test_case".to_string(), "standard_clean".to_string());

    let response = engine.decide(request).await?;

    println!("Event Data:");
    println!("  payment_amount: $500");
    println!("  ip_address: 8.8.8.8 (US)");
    println!("  payment_attempts_1h: 2");
    println!("  account_age_days: 30");
    println!();
    println!("Pipeline Routing:");
    println!("  Expected Route: standard_rules (payment_amount <= 1000)");
    println!();
    println!("Decision Result:");
    println!("  Action: {:?}", response.result.action);
    println!("  Score: {}", response.result.score);
    println!("  Triggered Rules: {:?}", response.result.triggered_rules);
    println!("  Explanation: {}", response.result.explanation);
    println!("  Processing Time: {}ms", response.processing_time_ms);
    println!();

    // Test Case 2: High-value transaction ($5000) - should route to high_value_rules
    println!("{}", "-".repeat(80));
    println!("Test Case 2: High-Value Transaction ($5000) - Clean");
    println!("{}", "-".repeat(80));

    event_data.insert("payment_amount".to_string(), Value::Number(5000.0));

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("test_case".to_string(), "high_value_clean".to_string());

    let response = engine.decide(request).await?;

    println!("Event Data:");
    println!("  payment_amount: $5000");
    println!("  ip_address: 8.8.8.8 (US)");
    println!("  payment_attempts_1h: 2");
    println!("  account_age_days: 30");
    println!();
    println!("Pipeline Routing:");
    println!("  Expected Route: high_value_rules (payment_amount > 1000)");
    println!();
    println!("Decision Result:");
    println!("  Action: {:?}", response.result.action);
    println!("  Score: {}", response.result.score);
    println!("  Triggered Rules: {:?}", response.result.triggered_rules);
    println!("  Explanation: {}", response.result.explanation);
    println!("  Processing Time: {}ms", response.processing_time_ms);
    println!();

    // Test Case 3: High-value with risk country - should deny (high_value_rules is stricter)
    println!("{}", "-".repeat(80));
    println!("Test Case 3: High-Value Transaction ($5000) - High-Risk Country");
    println!("{}", "-".repeat(80));

    event_data.insert("payment_amount".to_string(), Value::Number(5000.0));
    event_data.insert("ip_address".to_string(), Value::String("41.58.10.75".to_string())); // Nigeria IP

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("test_case".to_string(), "high_value_risky".to_string());

    let response = engine.decide(request).await?;

    println!("Event Data:");
    println!("  payment_amount: $5000");
    println!("  ip_address: 41.58.10.75 (Nigeria)");
    println!("  payment_attempts_1h: 2");
    println!("  account_age_days: 30");
    println!();
    println!("Pipeline Routing:");
    println!("  Expected Route: high_value_rules (payment_amount > 1000)");
    println!();
    println!("Decision Result:");
    println!("  Action: {:?}", response.result.action);
    println!("  Score: {} (high_risk_country = +30)", response.result.score);
    println!("  Triggered Rules: {:?}", response.result.triggered_rules);
    println!("  Explanation: {}", response.result.explanation);
    println!("  Processing Time: {}ms", response.processing_time_ms);
    println!();
    println!("Note: high_value_rules requires 3DS for single risk indicator (triggered_count >= 1)");
    println!();

    // Test Case 4: Standard transaction with risk country - should review (standard_rules is more lenient)
    println!("{}", "-".repeat(80));
    println!("Test Case 4: Standard Transaction ($500) - High-Risk Country");
    println!("{}", "-".repeat(80));

    event_data.insert("payment_amount".to_string(), Value::Number(500.0));
    event_data.insert("ip_address".to_string(), Value::String("41.58.10.75".to_string())); // Nigeria IP

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("test_case".to_string(), "standard_risky".to_string());

    let response = engine.decide(request).await?;

    println!("Event Data:");
    println!("  payment_amount: $500");
    println!("  ip_address: 41.58.10.75 (Nigeria)");
    println!("  payment_attempts_1h: 2");
    println!("  account_age_days: 30");
    println!();
    println!("Pipeline Routing:");
    println!("  Expected Route: standard_rules (payment_amount <= 1000)");
    println!();
    println!("Decision Result:");
    println!("  Action: {:?}", response.result.action);
    println!("  Score: {} (high_risk_country = +30)", response.result.score);
    println!("  Triggered Rules: {:?}", response.result.triggered_rules);
    println!("  Explanation: {}", response.result.explanation);
    println!("  Processing Time: {}ms", response.processing_time_ms);
    println!();
    println!("Note: standard_rules only reviews when score >= 40 or triggered_count >= 3");
    println!();

    // Test Case 5: New account with high-value purchase - should deny
    println!("{}", "-".repeat(80));
    println!("Test Case 5: New Account ($5000 purchase)");
    println!("{}", "-".repeat(80));

    event_data.insert("payment_amount".to_string(), Value::Number(5000.0));
    event_data.insert("ip_address".to_string(), Value::String("8.8.8.8".to_string())); // US IP
    event_data.insert("account_age_days".to_string(), Value::Number(3.0));

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("test_case".to_string(), "new_account_high_value".to_string());

    let response = engine.decide(request).await?;

    println!("Event Data:");
    println!("  payment_amount: $5000");
    println!("  ip_address: 8.8.8.8 (US)");
    println!("  account_age_days: 3 (< 7 days)");
    println!();
    println!("Pipeline Routing:");
    println!("  Expected Route: high_value_rules (payment_amount > 1000)");
    println!();
    println!("Decision Result:");
    println!("  Action: {:?}", response.result.action);
    println!("  Score: {} (new_account_risk = +60)", response.result.score);
    println!("  Triggered Rules: {:?}", response.result.triggered_rules);
    println!("  Explanation: {}", response.result.explanation);
    println!("  Processing Time: {}ms", response.processing_time_ms);
    println!();
    println!("Note: high_value_rules denies new_account_risk pattern immediately");
    println!();

    // Test Case 6: Card testing pattern - should deny in both rulesets
    println!("{}", "-".repeat(80));
    println!("Test Case 6: Card Testing Pattern");
    println!("{}", "-".repeat(80));

    event_data.insert("payment_amount".to_string(), Value::Number(5.0));
    event_data.insert("payment_attempts_1h".to_string(), Value::Number(8.0));
    event_data.insert("unique_cards_1h".to_string(), Value::Number(5.0));
    event_data.insert("account_age_days".to_string(), Value::Number(30.0));

    let request = DecisionRequest::new(event_data.clone())
        .with_metadata("test_case".to_string(), "card_testing".to_string());

    let response = engine.decide(request).await?;

    println!("Event Data:");
    println!("  payment_amount: $5");
    println!("  payment_attempts_1h: 8");
    println!("  unique_cards_1h: 5");
    println!();
    println!("Pipeline Routing:");
    println!("  Expected Route: standard_rules (payment_amount <= 1000)");
    println!();
    println!("Decision Result:");
    println!("  Action: {:?}", response.result.action);
    println!("  Score: {} (card_testing = +80)", response.result.score);
    println!("  Triggered Rules: {:?}", response.result.triggered_rules);
    println!("  Explanation: {}", response.result.explanation);
    println!("  Processing Time: {}ms", response.processing_time_ms);
    println!();
    println!("Note: Both rulesets deny card_testing pattern as critical fraud");
    println!();

    println!("{}", "=".repeat(80));
    println!("Summary: Pipeline Conditional Routing Demonstrated");
    println!("{}", "=".repeat(80));
    println!();
    println!("The pipeline successfully routes transactions to different rulesets:");
    println!("  • payment_amount > 1000  → high_value_rules (stricter)");
    println!("  • payment_amount <= 1000 → standard_rules (more lenient)");
    println!();
    println!("This demonstrates TRUE pipeline DSL with branch-based conditional routing,");
    println!("not just metadata documentation!");
    println!();

    Ok(())
}
