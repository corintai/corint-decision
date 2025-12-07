//! Data Source Configuration
//!
//! Configuration structures for different types of data sources.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    /// Data source name (unique identifier)
    pub name: String,

    /// Data source type
    #[serde(flatten)]
    pub source_type: DataSourceType,

    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,

    /// Connection timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Enable connection pooling
    #[serde(default = "default_true")]
    pub pooling_enabled: bool,
}

/// Data source type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DataSourceType {
    #[serde(rename = "feature_store")]
    FeatureStore(FeatureStoreConfig),

    #[serde(rename = "olap")]
    OLAP(OLAPConfig),

    #[serde(rename = "sql")]
    SQL(SQLConfig),
}

/// Feature Store configuration (Redis, Feast, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStoreConfig {
    /// Feature store provider
    pub provider: FeatureStoreProvider,

    /// Connection string
    pub connection_string: String,

    /// Feature namespace/prefix
    #[serde(default)]
    pub namespace: String,

    /// Default TTL for cached features (seconds)
    #[serde(default = "default_feature_ttl")]
    pub default_ttl: u64,

    /// Additional configuration
    #[serde(default)]
    pub options: HashMap<String, String>,
}

/// Feature store provider types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureStoreProvider {
    /// Redis-based feature store
    Redis,

    /// Feast feature store
    Feast,

    /// Custom HTTP-based feature store
    Http,
}

/// OLAP Database configuration (ClickHouse, Druid, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLAPConfig {
    /// OLAP database type
    pub provider: OLAPProvider,

    /// Connection string
    pub connection_string: String,

    /// Database name
    pub database: String,

    /// Default table/collection for events
    #[serde(default = "default_events_table")]
    pub events_table: String,

    /// Additional configuration
    #[serde(default)]
    pub options: HashMap<String, String>,
}

/// OLAP database providers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OLAPProvider {
    /// ClickHouse - Columnar OLAP database
    ClickHouse,

    /// Apache Druid - Real-time OLAP
    Druid,

    /// TimescaleDB (PostgreSQL extension for time-series)
    TimescaleDB,

    /// InfluxDB - Time-series database
    InfluxDB,
}

/// SQL Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SQLConfig {
    /// SQL database type
    pub provider: SQLProvider,

    /// Connection string
    pub connection_string: String,

    /// Database name
    pub database: String,

    /// Default table for events
    #[serde(default = "default_events_table")]
    pub events_table: String,

    /// Additional configuration
    #[serde(default)]
    pub options: HashMap<String, String>,
}

/// SQL database providers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SQLProvider {
    /// PostgreSQL
    PostgreSQL,

    /// MySQL
    MySQL,

    /// SQLite (for testing)
    SQLite,
}

// Default value functions
fn default_pool_size() -> u32 {
    10
}

fn default_timeout() -> u64 {
    5000 // 5 seconds
}

fn default_true() -> bool {
    true
}

fn default_feature_ttl() -> u64 {
    3600 // 1 hour
}

fn default_events_table() -> String {
    "events".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_redis_config() {
        let yaml = r#"
name: my_feature_store
type: feature_store
provider: redis
connection_string: "redis://localhost:6379"
namespace: "user_features"
default_ttl: 3600
"#;

        let config: DataSourceConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "my_feature_store");

        match config.source_type {
            DataSourceType::FeatureStore(fs_config) => {
                assert_eq!(fs_config.namespace, "user_features");
                assert!(matches!(fs_config.provider, FeatureStoreProvider::Redis));
            }
            _ => panic!("Expected FeatureStore type"),
        }
    }

    #[test]
    fn test_parse_clickhouse_config() {
        let yaml = r#"
name: clickhouse_events
type: olap
provider: clickhouse
connection_string: "http://localhost:8123"
database: "risk_events"
events_table: "user_events"
"#;

        let config: DataSourceConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "clickhouse_events");

        match config.source_type {
            DataSourceType::OLAP(olap_config) => {
                assert_eq!(olap_config.database, "risk_events");
                assert!(matches!(olap_config.provider, OLAPProvider::ClickHouse));
            }
            _ => panic!("Expected OLAP type"),
        }
    }
}
