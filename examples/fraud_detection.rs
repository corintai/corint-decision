//! Realistic Fraud Detection Example
//!
//! This example demonstrates production-grade fraud detection with:
//! - 6 realistic fraud patterns (fraud farms, account takeover, velocity abuse, etc.)
//! - Multi-feature risk assessment
//! - Score accumulation from multiple rules
//! - Comprehensive test scenarios

use corint_sdk::{DecisionEngineBuilder, DecisionRequest, Value, Action};
use corint_runtime::observability::metrics::Metrics;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber to enable logging
    // Use RUST_LOG environment variable to control log level
    // Example: RUST_LOG=debug cargo run --example fraud_detection
    //          RUST_LOG=trace cargo run --example fraud_detection
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing_subscriber::filter::LevelFilter::INFO.into()))
        .init();

    println!("=== Realistic Fraud Detection System ===\n");
    println!("This example demonstrates 6 production fraud patterns:");
    println!("  1. Fraud Farm Detection (IP/Device association)");
    println!("  2. Account Takeover (device + location + failed logins)");
    println!("  3. Velocity Abuse (transaction frequency spikes)");
    println!("  4. Amount Outlier (statistical anomaly detection)");
    println!("  5. Suspicious Geography (impossible travel, high-risk locations)");
    println!("  6. New User High-Risk (fake accounts, first-party fraud)\n");

    // Build the decision engine
    println!("Loading fraud detection ruleset...");
    let engine = DecisionEngineBuilder::new()
        .add_rule_file("examples/rules/fraud_detection.yaml")
        .enable_metrics(true)
        .build()
        .await?;
    println!("✓ Engine initialized with 6 fraud detection rules\n");

    // Test Case 1: Clean Transaction
    println!("=== Test Case 1: Clean Transaction ===");
    test_clean_transaction(&engine).await?;

    // Test Case 2: Fraud Farm Detected
    println!("\n=== Test Case 2: Fraud Farm Detection ===");
    test_fraud_farm(&engine).await?;

    // Test Case 3: Account Takeover
    println!("\n=== Test Case 3: Account Takeover ===");
    test_account_takeover(&engine).await?;

    // Test Case 4: Velocity Abuse
    println!("\n=== Test Case 4: Velocity Abuse ===");
    test_velocity_abuse(&engine).await?;

    // Test Case 5: Amount Outlier
    println!("\n=== Test Case 5: Amount Outlier ===");
    test_amount_outlier(&engine).await?;

    // Test Case 6: Multiple Patterns (Critical Risk)
    println!("\n=== Test Case 6: Multiple Fraud Patterns (Critical) ===");
    test_multiple_patterns(&engine).await?;

    // Test Case 7: New User Suspicious
    println!("\n=== Test Case 7: New User Suspicious Behavior ===");
    test_new_user_fraud(&engine).await?;

    // Display metrics
    display_metrics(&engine);

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  ✓ Each rule detects a specific fraud pattern");
    println!("  ✓ Scores accumulate from multiple triggered rules");
    println!("  ✓ Decision thresholds: 200=Critical, 150=VeryHigh, 100=High, 60=Review, 30=Monitor");
    println!("  ✓ Real-world features: IP association, velocity, outliers, geography, behavior");

    Ok(())
}

/// Test Case 1: Normal, clean transaction
async fn test_clean_transaction(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    // Clean signals - no risk indicators
    event_data.insert("event_type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("transaction_amount".to_string(), Value::Number(150.0));
    event_data.insert("ip_device_count".to_string(), Value::Number(2.0));
    event_data.insert("ip_user_count".to_string(), Value::Number(1.0));
    event_data.insert("new_device_indicator".to_string(), Value::Bool(false));
    event_data.insert("failed_login_count_1h".to_string(), Value::Number(0.0));
    event_data.insert("country_changed".to_string(), Value::Bool(false));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(3.0));
    event_data.insert("velocity_ratio".to_string(), Value::Number(1.0));
    event_data.insert("amount_zscore".to_string(), Value::Number(0.5));
    event_data.insert("is_amount_outlier".to_string(), Value::Bool(false));
    event_data.insert("geo_distance_km".to_string(), Value::Number(0.0));
    event_data.insert("hours_since_last_login".to_string(), Value::Number(24.0));
    event_data.insert("is_off_hours".to_string(), Value::Bool(false));
    event_data.insert("country_risk_level".to_string(), Value::String("low".to_string()));
    event_data.insert("user_age_days".to_string(), Value::Number(365.0));
    event_data.insert("new_payment_method".to_string(), Value::Bool(false));
    event_data.insert("user_verified".to_string(), Value::Bool(true));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Scenario: Normal user, small amount, verified, no anomalies");
    println!("Expected: APPROVE (no rules triggered, score = 0)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    assert!(matches!(response.result.action, Some(Action::Approve)));

    Ok(())
}

/// Test Case 2: Fraud Farm - High IP/Device association
async fn test_fraud_farm(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    // Fraud farm signals (primary indicators for this test)
    event_data.insert("event_type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("ip_device_count".to_string(), Value::Number(15.0));  // > 10
    event_data.insert("ip_user_count".to_string(), Value::Number(8.0));     // > 5
    event_data.insert("transaction_amount".to_string(), Value::Number(500.0));

    // Other rule fields (set to non-triggering values)
    event_data.insert("new_device_indicator".to_string(), Value::Bool(false));
    event_data.insert("failed_login_count_1h".to_string(), Value::Number(0.0));
    event_data.insert("country_changed".to_string(), Value::Bool(false));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(12.0));
    event_data.insert("velocity_ratio".to_string(), Value::Number(1.5));
    event_data.insert("amount_zscore".to_string(), Value::Number(0.8));
    event_data.insert("is_amount_outlier".to_string(), Value::Bool(false));
    event_data.insert("geo_distance_km".to_string(), Value::Number(0.0));
    event_data.insert("hours_since_last_login".to_string(), Value::Number(12.0));
    event_data.insert("is_off_hours".to_string(), Value::Bool(false));
    event_data.insert("country_risk_level".to_string(), Value::String("low".to_string()));
    event_data.insert("user_age_days".to_string(), Value::Number(100.0));
    event_data.insert("new_payment_method".to_string(), Value::Bool(false));
    event_data.insert("user_verified".to_string(), Value::Bool(true));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Scenario: 15 devices and 8 users from same IP (fraud farm pattern)");
    println!("Expected: DENY (fraud_farm_pattern triggered, score = 100)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    assert!(matches!(response.result.action, Some(Action::Deny)));

    Ok(())
}

/// Test Case 3: Account Takeover - New device + failed logins + location change
async fn test_account_takeover(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    // Account takeover signals (primary indicators for this test)
    event_data.insert("event_type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("new_device_indicator".to_string(), Value::Bool(true));
    event_data.insert("failed_login_count_1h".to_string(), Value::Number(5.0));  // >= 3
    event_data.insert("country_changed".to_string(), Value::Bool(true));
    event_data.insert("transaction_amount".to_string(), Value::Number(1000.0));

    // Other rule fields (set to non-triggering values)
    event_data.insert("ip_device_count".to_string(), Value::Number(3.0));
    event_data.insert("ip_user_count".to_string(), Value::Number(2.0));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(8.0));
    event_data.insert("velocity_ratio".to_string(), Value::Number(1.2));
    event_data.insert("amount_zscore".to_string(), Value::Number(1.0));
    event_data.insert("is_amount_outlier".to_string(), Value::Bool(false));
    event_data.insert("geo_distance_km".to_string(), Value::Number(100.0));
    event_data.insert("hours_since_last_login".to_string(), Value::Number(10.0));
    event_data.insert("is_off_hours".to_string(), Value::Bool(false));
    event_data.insert("country_risk_level".to_string(), Value::String("low".to_string()));
    event_data.insert("user_age_days".to_string(), Value::Number(200.0));
    event_data.insert("new_payment_method".to_string(), Value::Bool(false));
    event_data.insert("user_verified".to_string(), Value::Bool(true));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Scenario: New device + 5 failed logins + country changed");
    println!("Expected: REVIEW (account_takeover_pattern, score = 85, 60 <= score < 100)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    assert!(matches!(response.result.action, Some(Action::Review)));

    Ok(())
}

/// Test Case 4: Velocity Abuse - High transaction frequency
async fn test_velocity_abuse(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    // Velocity abuse signals (primary indicators for this test)
    event_data.insert("event_type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(25.0));  // > 20
    event_data.insert("velocity_ratio".to_string(), Value::Number(8.0));          // > 5.0
    event_data.insert("transaction_amount".to_string(), Value::Number(200.0));

    // Other rule fields (set to non-triggering values)
    event_data.insert("ip_device_count".to_string(), Value::Number(2.0));
    event_data.insert("ip_user_count".to_string(), Value::Number(1.0));
    event_data.insert("new_device_indicator".to_string(), Value::Bool(false));
    event_data.insert("failed_login_count_1h".to_string(), Value::Number(0.0));
    event_data.insert("country_changed".to_string(), Value::Bool(false));
    event_data.insert("amount_zscore".to_string(), Value::Number(0.5));
    event_data.insert("is_amount_outlier".to_string(), Value::Bool(false));
    event_data.insert("geo_distance_km".to_string(), Value::Number(0.0));
    event_data.insert("hours_since_last_login".to_string(), Value::Number(5.0));
    event_data.insert("is_off_hours".to_string(), Value::Bool(false));
    event_data.insert("country_risk_level".to_string(), Value::String("low".to_string()));
    event_data.insert("user_age_days".to_string(), Value::Number(180.0));
    event_data.insert("new_payment_method".to_string(), Value::Bool(false));
    event_data.insert("user_verified".to_string(), Value::Bool(true));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Scenario: 25 transactions in 24h, 8x velocity spike");
    println!("Expected: REVIEW (velocity_abuse_pattern, score = 70, 60 <= score < 100)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    assert!(matches!(response.result.action, Some(Action::Review)));

    Ok(())
}

/// Test Case 5: Amount Outlier - Statistical anomaly
async fn test_amount_outlier(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    // Amount outlier signals (primary indicators for this test)
    event_data.insert("event_type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("amount_zscore".to_string(), Value::Number(4.5));           // > 3.0
    event_data.insert("is_amount_outlier".to_string(), Value::Bool(true));
    event_data.insert("transaction_amount".to_string(), Value::Number(8000.0));   // > 5000

    // Other rule fields (set to non-triggering values)
    event_data.insert("ip_device_count".to_string(), Value::Number(2.0));
    event_data.insert("ip_user_count".to_string(), Value::Number(1.0));
    event_data.insert("new_device_indicator".to_string(), Value::Bool(false));
    event_data.insert("failed_login_count_1h".to_string(), Value::Number(0.0));
    event_data.insert("country_changed".to_string(), Value::Bool(false));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(10.0));
    event_data.insert("velocity_ratio".to_string(), Value::Number(1.0));
    event_data.insert("geo_distance_km".to_string(), Value::Number(0.0));
    event_data.insert("hours_since_last_login".to_string(), Value::Number(12.0));
    event_data.insert("is_off_hours".to_string(), Value::Bool(false));
    event_data.insert("country_risk_level".to_string(), Value::String("low".to_string()));
    event_data.insert("user_age_days".to_string(), Value::Number(300.0));
    event_data.insert("new_payment_method".to_string(), Value::Bool(false));
    event_data.insert("user_verified".to_string(), Value::Bool(true));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Scenario: Amount Z-score 4.5, $8000 (outlier + high amount)");
    println!("Expected: REVIEW (amount_outlier_pattern, score = 75, 60 <= score < 100)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    assert!(matches!(response.result.action, Some(Action::Review)));

    Ok(())
}

/// Test Case 6: Multiple Patterns - Critical risk
async fn test_multiple_patterns(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    // Multiple fraud signals combined
    event_data.insert("event_type".to_string(), Value::String("transaction".to_string()));

    // Fraud farm signals
    event_data.insert("ip_device_count".to_string(), Value::Number(12.0));
    event_data.insert("ip_user_count".to_string(), Value::Number(6.0));

    // Velocity abuse signals
    event_data.insert("transaction_count_24h".to_string(), Value::Number(30.0));
    event_data.insert("velocity_ratio".to_string(), Value::Number(10.0));

    // Amount outlier signals
    event_data.insert("amount_zscore".to_string(), Value::Number(5.0));
    event_data.insert("is_amount_outlier".to_string(), Value::Bool(true));
    event_data.insert("transaction_amount".to_string(), Value::Number(15000.0));

    // Other rule fields (set to non-triggering values)
    event_data.insert("new_device_indicator".to_string(), Value::Bool(false));
    event_data.insert("failed_login_count_1h".to_string(), Value::Number(0.0));
    event_data.insert("country_changed".to_string(), Value::Bool(false));
    event_data.insert("geo_distance_km".to_string(), Value::Number(0.0));
    event_data.insert("hours_since_last_login".to_string(), Value::Number(5.0));
    event_data.insert("is_off_hours".to_string(), Value::Bool(false));
    event_data.insert("country_risk_level".to_string(), Value::String("low".to_string()));
    event_data.insert("user_age_days".to_string(), Value::Number(500.0));
    event_data.insert("new_payment_method".to_string(), Value::Bool(false));
    event_data.insert("user_verified".to_string(), Value::Bool(true));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Scenario: Fraud farm + velocity abuse + amount outlier");
    println!("Expected: DENY CRITICAL (multiple patterns, score = 245 >= 200)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    assert!(matches!(response.result.action, Some(Action::Deny)));

    Ok(())
}

/// Test Case 7: New User Fraud
async fn test_new_user_fraud(engine: &corint_sdk::DecisionEngine) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_data = HashMap::new();

    // New user high-risk signals (primary indicators for this test)
    event_data.insert("event_type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("user_age_days".to_string(), Value::Number(3.0));          // < 7
    event_data.insert("transaction_amount".to_string(), Value::Number(2000.0));  // > 1000
    event_data.insert("new_payment_method".to_string(), Value::Bool(true));
    event_data.insert("user_verified".to_string(), Value::Bool(false));

    // Other rule fields (set to non-triggering values)
    event_data.insert("ip_device_count".to_string(), Value::Number(1.0));
    event_data.insert("ip_user_count".to_string(), Value::Number(1.0));
    event_data.insert("new_device_indicator".to_string(), Value::Bool(true));  // Expected for new user
    event_data.insert("failed_login_count_1h".to_string(), Value::Number(0.0));
    event_data.insert("country_changed".to_string(), Value::Bool(false));
    event_data.insert("transaction_count_24h".to_string(), Value::Number(2.0));
    event_data.insert("velocity_ratio".to_string(), Value::Number(1.0));
    event_data.insert("amount_zscore".to_string(), Value::Number(2.0));
    event_data.insert("is_amount_outlier".to_string(), Value::Bool(false));
    event_data.insert("geo_distance_km".to_string(), Value::Number(0.0));
    event_data.insert("hours_since_last_login".to_string(), Value::Number(1.0));
    event_data.insert("is_off_hours".to_string(), Value::Bool(false));
    event_data.insert("country_risk_level".to_string(), Value::String("low".to_string()));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Scenario: 3-day old account, $2000, new payment, unverified");
    println!("Expected: REVIEW (new_user_high_risk, score = 50, 30 <= score < 60)");
    println!("Result: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);

    assert!(matches!(response.result.action, Some(Action::Review)));

    Ok(())
}

/// Display engine metrics
fn display_metrics(engine: &corint_sdk::DecisionEngine) {
    let metrics = engine.metrics();

    println!("\n=== Overall Metrics ===");
    println!("Total Executions: {}", metrics.counter("executions_total").get());

    if metrics.counter("rule_evaluations").get() > 0 {
        println!("Rule Evaluations: {}", metrics.counter("rule_evaluations").get());
    }
}
