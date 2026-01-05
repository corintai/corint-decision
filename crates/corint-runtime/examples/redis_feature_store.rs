//! Redis Feature Store Example
//!
//! This example demonstrates how to use Redis as a feature store datasource.
//!
//! Prerequisites:
//! 1. Start a local Redis server: `docker run -d -p 6379:6379 redis`
//! 2. Run this example with: `cargo run --example redis_feature_store --features redis`
//!
//! The example will:
//! - Connect to Redis
//! - Set some sample feature values
//! - Retrieve features using the DataSourceClient

use corint_runtime::datasource::{
    DataSourceClient, DataSourceConfig, DataSourceType, FeatureStoreConfig,
};
use corint_runtime::datasource::config::FeatureStoreProvider;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug logs
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== Redis Feature Store Example ===\n");

    // Step 1: Configure Redis datasource
    let config = DataSourceConfig {
        name: "redis_feature_store".to_string(),
        source_type: DataSourceType::FeatureStore(FeatureStoreConfig {
            provider: FeatureStoreProvider::Redis,
            connection_string: "redis://127.0.0.1:6379".to_string(),
            namespace: "user_features".to_string(),
            default_ttl: 3600, // 1 hour
            options: HashMap::new(),
        }),
        pool_size: 10,
        timeout_ms: 5000,
        pooling_enabled: true,
    };

    println!("Connecting to Redis at {}", "redis://127.0.0.1:6379");

    // Step 2: Create datasource client
    let client = DataSourceClient::new(config).await?;
    println!("✓ Successfully connected to Redis\n");

    // Step 3: Set some sample feature values
    println!("Setting sample features...");

    // Simulate setting features (in real scenario, features would be pre-computed and stored)
    // For this example, we'll use redis-cli to set values:
    println!("Please run the following Redis commands to set sample data:");
    println!("  redis-cli SET user_features:risk_score:user_123 85.5");
    println!("  redis-cli SET user_features:transaction_count:user_123 42");
    println!("  redis-cli SET user_features:account_age_days:user_123 365");
    println!("  redis-cli SET user_features:is_verified:user_123 true");
    println!();

    // Step 4: Retrieve features
    println!("Attempting to retrieve features from Redis...\n");

    let test_features = vec![
        ("risk_score", "user_123"),
        ("transaction_count", "user_123"),
        ("account_age_days", "user_123"),
        ("is_verified", "user_123"),
        ("non_existent_feature", "user_123"),
    ];

    for (feature_name, entity_key) in test_features {
        match client.get_feature(feature_name, entity_key).await {
            Ok(Some(value)) => {
                println!("✓ Feature '{:30}' for '{}': {:?}", feature_name, entity_key, value);
            }
            Ok(None) => {
                println!("○ Feature '{:30}' for '{}': Not found", feature_name, entity_key);
            }
            Err(e) => {
                println!("✗ Error fetching '{}' for '{}': {}", feature_name, entity_key, e);
            }
        }
    }

    println!("\n=== Example completed ===");
    println!("\nNote: If features are not found, make sure to:");
    println!("1. Redis is running on localhost:6379");
    println!("2. Set the sample data using the redis-cli commands shown above");

    Ok(())
}
