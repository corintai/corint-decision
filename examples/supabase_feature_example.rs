//! Supabase Feature Calculation Example
//!
//! This example demonstrates how to use Supabase PostgreSQL database
//! for feature calculation in CORINT Decision Engine.
//!
//! ## Prerequisites
//!
//! 1. Ensure Supabase database is set up with the schema:
//!    ```bash
//!    psql 'postgresql://postgres.hfsbvqfkwbslcuvokmis:XPRozJ3DZox5KUnh@aws-1-ap-southeast-1.pooler.supabase.com:5432/postgres?sslmode=require' -f docs/schema/postgres-schema.sql
//!    ```
//!
//! 2. Insert sample data (optional):
//!    ```bash
//!    psql 'postgresql://postgres.hfsbvqfkwbslcuvokmis:XPRozJ3DZox5KUnh@aws-1-ap-southeast-1.pooler.supabase.com:5432/postgres?sslmode=require' -f docs/schema/postgres-examples.sql
//!    ```
//!
//! ## Run
//! ```bash
//! cargo run --example supabase_feature_example --features sqlx
//! ```

use corint_runtime::datasource::{DataSourceClient, DataSourceConfig};
use corint_runtime::feature::{FeatureExecutor, FeatureRegistry};
use corint_runtime::context::ExecutionContext;
use corint_sdk::Value;
use std::collections::HashMap;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("corint_runtime=info".parse()?)
        )
        .init();

    println!("{}", "=".repeat(80));
    println!("Supabase Feature Calculation Example");
    println!("{}", "=".repeat(80));
    println!();

    // Step 1: Load Supabase configuration from YAML file
    println!("{}", "-".repeat(80));
    println!("Step 1: Loading Supabase configuration");
    println!("{}", "-".repeat(80));
    
    let config_path = "examples/configs/datasources/supabase_events.yaml";
    let config_content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file {}: {}", config_path, e))?;
    
    let supabase_config: DataSourceConfig = serde_yaml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;
    
    println!("✓ Loaded Supabase configuration:");
    println!("  Name: {}", supabase_config.name);
    println!("  Type: {:?}", supabase_config.source_type);
    println!("  Connection: Session Pooler (IPv4-compatible)");
    println!();

    // Step 2: Create Supabase data source client
    println!("{}", "-".repeat(80));
    println!("Step 2: Creating Supabase data source client");
    println!("{}", "-".repeat(80));
    
    let supabase_client = DataSourceClient::new(supabase_config).await
        .map_err(|e| format!("Failed to create Supabase client: {}", e))?;
    
    println!("✓ Supabase client created successfully");
    println!();

    // Step 3: Initialize FeatureExecutor
    println!("{}", "-".repeat(80));
    println!("Step 3: Initializing FeatureExecutor");
    println!("{}", "-".repeat(80));
    
    let mut executor = FeatureExecutor::new().with_stats();
    
    // Register Supabase as the default data source
    // Note: We use the same client for both names since DataSourceClient doesn't implement Clone
    executor.add_datasource("supabase_events", supabase_client);
    
    // For default, we need to create another client instance
    let supabase_config_default: DataSourceConfig = serde_yaml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config for default: {}", e))?;
    let supabase_client_default = DataSourceClient::new(supabase_config_default).await
        .map_err(|e| format!("Failed to create default Supabase client: {}", e))?;
    executor.add_datasource("default", supabase_client_default);
    
    println!("✓ Registered data sources:");
    println!("  - supabase_events (Supabase PostgreSQL)");
    println!("  - default -> supabase_events");
    println!();

    // Step 4: Load and register features
    println!("{}", "-".repeat(80));
    println!("Step 4: Loading features from YAML");
    println!("{}", "-".repeat(80));
    
    let mut registry = FeatureRegistry::new();
    
    // Load features that use Supabase
    registry.load_from_file("examples/configs/features/user_features.yaml")?;
    registry.load_from_file("examples/configs/features/device_features.yaml")?;
    
    registry.print_summary();
    
    // Register features to executor
    for feature in registry.all_features() {
        executor.register_feature(feature.clone())?;
    }
    
    println!("✓ Registered {} features", registry.count());
    println!();

    // Step 5: Create execution context
    println!("{}", "-".repeat(80));
    println!("Step 5: Creating execution context");
    println!("{}", "-".repeat(80));
    
    // Create event data with event.* prefix support
    // Template variables like {event.user_id} need to be accessible
    let mut event_data = HashMap::new();
    
    // Store both direct keys and event.* prefixed keys for template resolution
    let user_id = Value::String("user_001".to_string());
    let device_id = Value::String("device_001".to_string());
    let ip_address = Value::String("203.0.113.1".to_string());
    let event_type = Value::String("login".to_string());
    let amount = Value::Number(1500.0);
    
    // Direct keys (for backward compatibility)
    event_data.insert("user_id".to_string(), user_id.clone());
    event_data.insert("device_id".to_string(), device_id.clone());
    event_data.insert("ip_address".to_string(), ip_address.clone());
    event_data.insert("event_type".to_string(), event_type.clone());
    event_data.insert("amount".to_string(), amount.clone());
    
    // event.* prefixed keys (for template variables like {event.user_id})
    event_data.insert("event.user_id".to_string(), user_id);
    event_data.insert("event.device_id".to_string(), device_id);
    event_data.insert("event.ip_address".to_string(), ip_address);
    event_data.insert("event.event_type".to_string(), event_type);
    event_data.insert("event.amount".to_string(), amount);
    
    // Clone event_data for context (context takes ownership)
    let context = ExecutionContext::new(event_data.clone());
    
    println!("✓ Execution context created");
    println!("  user_id: user_001");
    println!("  device_id: device_001");
    println!("  ip_address: 203.0.113.1");
    println!("  event_type: login");
    println!("  Note: Template variables {{event.user_id}}, {{event.device_id}} are supported");
    println!();

    // Step 6: Execute features (using 7-day window for testing)
    println!("{}", "=".repeat(80));
    println!("Step 6: Executing features with Supabase (7-day window)");
    println!("{}", "=".repeat(80));
    println!();

    // Example 1: Unique devices count (7 days)
    println!("Feature: unique_devices_7d");
    println!("Description: Count of unique devices in last 7 days");
    match executor.execute_feature("unique_devices_7d", &context).await {
        Ok(value) => {
            println!("  ✓ Result: {:?}", value);
        }
        Err(e) => {
            println!("  ✗ Error: {}", e);
        }
    }
    println!();

    // Example 2: Transaction sum (7 days)
    println!("Feature: transaction_sum_7d");
    println!("Description: Total transaction amount in last 7 days");
    match executor.execute_feature("transaction_sum_7d", &context).await {
        Ok(value) => {
            println!("  ✓ Result: {:?}", value);
        }
        Err(e) => {
            println!("  ✗ Error: {}", e);
        }
    }
    println!();

    // Example 3: Max transaction (7 days)
    println!("Feature: max_transaction_7d");
    println!("Description: Maximum transaction amount in last 7 days");
    match executor.execute_feature("max_transaction_7d", &context).await {
        Ok(value) => {
            println!("  ✓ Result: {:?}", value);
        }
        Err(e) => {
            println!("  ✗ Error: {}", e);
        }
    }
    println!();

    // Example 4: Transaction count (24 hours - may be 0 if data is older)
    println!("Feature: transaction_count_24h");
    println!("Description: Number of transactions in last 24 hours");
    match executor.execute_feature("transaction_count_24h", &context).await {
        Ok(value) => {
            println!("  ✓ Result: {:?}", value);
            if value == Value::Number(0.0) {
                println!("  Note: Result is 0 because data timestamps are older than 24 hours");
            }
        }
        Err(e) => {
            println!("  ✗ Error: {}", e);
        }
    }
    println!();

    // Step 7: Execute complete pipeline with ruleset
    println!("{}", "=".repeat(80));
    println!("Step 7: Executing Complete Pipeline with Ruleset");
    println!("{}", "=".repeat(80));
    println!();
    
    println!("Pipeline Overview:");
    println!("  1. Rules execute and reference features using features. prefix (e.g., features.transaction_sum_7d > 5000)");
    println!("  2. Features are automatically calculated from Supabase when rules access them");
    println!("  3. Feature values are cached in event_data for subsequent accesses");
    println!("  4. Ruleset evaluates rule conditions using feature values");
    println!("  5. Decision logic determines action based on rule scores");
    println!();

    // Add event.type for rule matching
    event_data.insert("event.type".to_string(), Value::String("transaction".to_string()));

    // Initialize DecisionEngine with ruleset and FeatureExecutor
    println!("Initializing DecisionEngine with ruleset and FeatureExecutor...");
    use corint_sdk::{DecisionEngineBuilder, DecisionRequest};
    use std::sync::Arc;
    
    // Wrap FeatureExecutor in Arc for sharing
    let feature_executor_arc = Arc::new(executor);
    
    let decision_engine = DecisionEngineBuilder::new()
        .add_rule_file("examples/pipelines/supabase_feature_ruleset.yaml")
        .enable_metrics(true)
        .with_feature_executor(feature_executor_arc.clone())
        .build()
        .await
        .map_err(|e| format!("Failed to build DecisionEngine: {}", e))?;
    
    println!("✓ DecisionEngine initialized");
    println!("  Ruleset: supabase_risk_assessment");
    println!("  Rules: high_transaction_volume, multiple_devices, high_value_transaction, rapid_transactions, suspicious_device_pattern");
    println!("  FeatureExecutor: Integrated (features will be calculated on-demand)");
    println!();
    println!("Note: Features will be calculated from Supabase automatically when rules reference them");
    println!();

    // Execute decision pipeline
    // Features will be calculated automatically when rules access them via LoadField instruction
    println!("Executing decision pipeline...");
    println!("  Rules will reference features, triggering automatic calculation from Supabase");
    println!();

    // Create event_data WITHOUT pre-calculated features
    // Features will be calculated on-demand during rule execution
    let decision_request = DecisionRequest::new(event_data);
    let decision_response = decision_engine.decide(decision_request).await
        .map_err(|e| format!("Failed to execute decision: {}", e))?;

    println!("✓ Decision completed");
    println!();
    println!("Decision Result:");
    println!("  Action: {:?}", decision_response.result.action);
    println!("  Score: {}", decision_response.result.score);
    println!("  Triggered Rules: {:?}", decision_response.result.triggered_rules);
    if !decision_response.result.explanation.is_empty() {
        println!("  Reason: {}", decision_response.result.explanation);
    }
    println!("  Processing Time: {} ms", decision_response.processing_time_ms);
    println!();
    println!("Note: Features were calculated from Supabase during rule execution");
    println!();

    // Test Case 2: High-risk scenario with actual Supabase data
    // To trigger rules, we would need to insert high-risk events into Supabase
    // For this example, we'll use a different user_id that might have different feature values
    println!("{}", "=".repeat(80));
    println!("Test Case 2: Testing with Different User (user_003)");
    println!("{}", "=".repeat(80));
    println!();
    println!("Note: Features will be calculated from Supabase when rules reference them");
    println!();
    
    // Create event_data for a different user (without pre-calculated features)
    let mut test_event_data = HashMap::new();
    test_event_data.insert("user_id".to_string(), Value::String("user_003".to_string()));
    test_event_data.insert("device_id".to_string(), Value::String("device_003".to_string()));
    test_event_data.insert("ip_address".to_string(), Value::String("203.0.113.3".to_string()));
    test_event_data.insert("event.type".to_string(), Value::String("transaction".to_string()));
    
    // Add event.* prefixed keys
    test_event_data.insert("event.user_id".to_string(), Value::String("user_003".to_string()));
    test_event_data.insert("event.device_id".to_string(), Value::String("device_003".to_string()));
    test_event_data.insert("event.ip_address".to_string(), Value::String("203.0.113.3".to_string()));
    test_event_data.insert("event.event_type".to_string(), Value::String("transaction".to_string()));
    
    println!("Event Data:");
    println!("  user_id: user_003");
    println!("  device_id: device_003");
    println!("  event.type: transaction");
    println!();
    println!("Executing decision pipeline...");
    println!("  Features will be calculated from Supabase when rules reference them");
    println!();
    
    let test_request = DecisionRequest::new(test_event_data);
    let test_response = decision_engine.decide(test_request).await
        .map_err(|e| format!("Failed to execute decision: {}", e))?;
    
    println!("✓ Decision completed");
    println!();
    println!("Decision Result:");
    println!("  Action: {:?}", test_response.result.action);
    println!("  Score: {}", test_response.result.score);
    println!("  Triggered Rules: {:?}", test_response.result.triggered_rules);
    if !test_response.result.explanation.is_empty() {
        println!("  Reason: {}", test_response.result.explanation);
    }
    println!("  Processing Time: {} ms", test_response.processing_time_ms);
    println!();

    // Step 8: Display statistics (if available)
    println!("{}", "=".repeat(80));
    println!("Execution Statistics");
    println!("{}", "=".repeat(80));
    println!("  Note: Statistics available when using .with_stats()");
    println!();

    println!("{}", "=".repeat(80));
    println!("✓ Example completed successfully!");
    println!("{}", "=".repeat(80));
    println!();
    println!("Summary:");
    println!("  ✓ Features calculated from Supabase PostgreSQL (on-demand during rule execution)");
    println!("  ✓ Rules evaluated using feature values");
    println!("  ✓ Risk decision made based on ruleset logic");
    println!("  ✓ Test Case 1 (user_001): {:?} - Score: {}", 
             decision_response.result.action, decision_response.result.score);
    println!("  ✓ Test Case 2 (user_003): {:?} - Score: {} ({} rules triggered)", 
             test_response.result.action, test_response.result.score, 
             test_response.result.triggered_rules.len());
    println!();
    println!("Pipeline Flow:");
    println!("  1. Rules execute and reference features using features. prefix (e.g., features.transaction_sum_7d > 5000)");
    println!("  2. PipelineExecutor's LoadField instruction checks if field exists in event_data");
    println!("  3. If not found, checks if it's a registered feature name");
    println!("  4. If it's a feature, FeatureExecutor calculates it from Supabase on-demand");
    println!("  5. Feature value is cached in event_data for subsequent accesses");
    println!("  6. Ruleset evaluates rule conditions using feature values");
    println!("  7. Decision logic determines action based on rule scores");
    println!();
    println!("✓ Features are now calculated lazily during rule execution from Supabase!");
    println!();

    Ok(())
}

