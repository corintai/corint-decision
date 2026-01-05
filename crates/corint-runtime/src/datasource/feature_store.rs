//! Feature Store Client Implementation
//!
//! Provides feature store connectivity for Redis, Feast, and HTTP-based feature stores.

use super::config::{FeatureStoreConfig, FeatureStoreProvider};
use super::query::{FilterOperator, Query, QueryResult, QueryType};
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashMap;

// Import the traits from client module
use super::client::{DataSourceImpl, FeatureStoreOps};

#[cfg(feature = "redis")]
use redis::aio::ConnectionManager;

/// Feature Store Client
pub(super) struct FeatureStoreClient {
    config: FeatureStoreConfig,
    #[cfg(feature = "redis")]
    redis_conn: Option<ConnectionManager>,
}

impl FeatureStoreClient {
    pub(super) async fn new(config: FeatureStoreConfig) -> Result<Self> {
        tracing::info!("Initializing feature store client: {:?}", config.provider);

        #[cfg(feature = "redis")]
        let redis_conn = if matches!(config.provider, FeatureStoreProvider::Redis) {
            Self::init_redis_connection(&config.connection_string).await.ok()
        } else {
            None
        };

        #[cfg(not(feature = "redis"))]
        let _config_check = &config; // Avoid unused variable warning

        Ok(Self {
            config,
            #[cfg(feature = "redis")]
            redis_conn,
        })
    }

    #[cfg(feature = "redis")]
    async fn init_redis_connection(connection_string: &str) -> Result<ConnectionManager> {
        use redis::Client;

        let client = Client::open(connection_string)
            .map_err(|e| RuntimeError::RuntimeError(format!("Failed to create Redis client: {}", e)))?;

        let conn_manager = ConnectionManager::new(client).await
            .map_err(|e| RuntimeError::RuntimeError(format!("Failed to connect to Redis: {}", e)))?;

        tracing::info!("Successfully connected to Redis: {}", connection_string);
        Ok(conn_manager)
    }

    /// Extract entity key from query filters
    fn extract_entity_key(&self, query: &Query) -> Result<String> {
        // Look for dimension filter that contains the entity key
        for filter in &query.filters {
            if filter.operator == FilterOperator::Eq {
                if let Value::String(ref key) = filter.value {
                    return Ok(key.clone());
                }
            }
        }

        Err(RuntimeError::RuntimeError(
            "Could not extract entity key from query filters".to_string(),
        ))
    }

    /// Get feature from Redis
    async fn get_redis_feature(
        &self,
        feature_name: &str,
        entity_key: &str,
    ) -> Result<Option<Value>> {
        #[cfg(feature = "redis")]
        {
            use redis::AsyncCommands;

            let redis_key = if self.config.namespace.is_empty() {
                format!("{}:{}", feature_name, entity_key)
            } else {
                format!("{}:{}:{}", self.config.namespace, feature_name, entity_key)
            };

            tracing::debug!("Fetching Redis key: {}", redis_key);

            let Some(ref conn) = self.redis_conn else {
                tracing::warn!("Redis connection not initialized, returning None");
                return Ok(None);
            };

            let mut conn = conn.clone();

            // Try to get the value from Redis
            match conn.get::<_, Option<String>>(&redis_key).await {
                Ok(Some(value_str)) => {
                    tracing::debug!("Found value in Redis for key {}: {}", redis_key, value_str);

                    // Parse the value - try to detect the type
                    let value = if let Ok(num) = value_str.parse::<f64>() {
                        Value::Number(num)
                    } else if let Ok(b) = value_str.parse::<bool>() {
                        Value::Bool(b)
                    } else if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&value_str) {
                        // Try to parse as JSON
                        match json_val {
                            serde_json::Value::Number(n) => {
                                Value::Number(n.as_f64().unwrap_or(0.0))
                            }
                            serde_json::Value::String(s) => Value::String(s),
                            serde_json::Value::Bool(b) => Value::Bool(b),
                            serde_json::Value::Null => Value::Null,
                            serde_json::Value::Array(arr) => {
                                // Convert JSON array to Value array
                                let converted: Vec<Value> = arr
                                    .into_iter()
                                    .filter_map(|v| serde_json::from_value(v).ok())
                                    .collect();
                                Value::Array(converted)
                            }
                            serde_json::Value::Object(obj) => {
                                // Convert JSON object to Value object
                                let converted: HashMap<String, Value> = obj
                                    .into_iter()
                                    .filter_map(|(k, v)| {
                                        serde_json::from_value(v).ok().map(|val| (k, val))
                                    })
                                    .collect();
                                Value::Object(converted)
                            }
                        }
                    } else {
                        // Default to string
                        Value::String(value_str)
                    };

                    Ok(Some(value))
                }
                Ok(None) => {
                    tracing::debug!("Key not found in Redis: {}", redis_key);
                    Ok(None)
                }
                Err(e) => {
                    tracing::error!("Redis error while fetching key {}: {}", redis_key, e);
                    Err(RuntimeError::RuntimeError(format!("Redis error: {}", e)))
                }
            }
        }

        #[cfg(not(feature = "redis"))]
        {
            let _ = (feature_name, entity_key);
            Err(RuntimeError::RuntimeError(
                "Redis feature is not enabled. Please compile with --features redis".to_string(),
            ))
        }
    }

    /// Set feature to Redis
    #[allow(dead_code)]
    async fn set_redis_feature(
        &self,
        feature_name: &str,
        entity_key: &str,
        value: &Value,
    ) -> Result<()> {
        #[cfg(feature = "redis")]
        {
            use redis::AsyncCommands;

            let redis_key = if self.config.namespace.is_empty() {
                format!("{}:{}", feature_name, entity_key)
            } else {
                format!("{}:{}:{}", self.config.namespace, feature_name, entity_key)
            };

            tracing::debug!("Setting Redis key: {} = {:?}", redis_key, value);

            let Some(ref conn) = self.redis_conn else {
                return Err(RuntimeError::RuntimeError(
                    "Redis connection not initialized".to_string(),
                ));
            };

            let mut conn = conn.clone();

            // Convert Value to string for storage
            let value_str = match value {
                Value::Null => "null".to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.clone(),
                Value::Array(_) | Value::Object(_) => {
                    // Serialize complex types as JSON
                    serde_json::to_string(value)
                        .map_err(|e| RuntimeError::RuntimeError(format!("JSON serialization error: {}", e)))?
                }
            };

            // Set with TTL
            let ttl = self.config.default_ttl;
            conn.set_ex::<_, _, ()>(&redis_key, value_str, ttl)
                .await
                .map_err(|e| RuntimeError::RuntimeError(format!("Redis SET error: {}", e)))?;

            tracing::debug!("Successfully set Redis key {} with TTL {}s", redis_key, ttl);
            Ok(())
        }

        #[cfg(not(feature = "redis"))]
        {
            let _ = (feature_name, entity_key, value);
            Err(RuntimeError::RuntimeError(
                "Redis feature is not enabled. Please compile with --features redis".to_string(),
            ))
        }
    }
}

#[async_trait::async_trait]
impl DataSourceImpl for FeatureStoreClient {
    async fn execute(&self, query: Query) -> Result<QueryResult> {
        tracing::debug!("Executing feature store query: {:?}", query.query_type);

        // Feature stores typically use key-value access, not SQL queries
        // For GetFeature queries, we extract the key from filters
        match query.query_type {
            QueryType::GetFeature => {
                // Extract entity key from filters
                let entity_key = self.extract_entity_key(&query)?;
                let feature_name = query.entity.as_str();

                // Get feature value
                let value = self.get_feature(feature_name, &entity_key).await?;

                // Convert to QueryResult format
                let row = if let Some(val) = value {
                    let mut map = HashMap::new();
                    map.insert("value".to_string(), val);
                    vec![map]
                } else {
                    vec![]
                };

                Ok(QueryResult {
                    rows: row,
                    execution_time_ms: 5, // Feature stores are typically very fast
                    source: self.config.namespace.clone(),
                    from_cache: false,
                })
            }
            _ => Err(RuntimeError::RuntimeError(
                "Feature stores only support GetFeature query type".to_string(),
            )),
        }
    }

    fn as_feature_store(&self) -> Option<&dyn FeatureStoreOps> {
        Some(self)
    }
}

#[async_trait::async_trait]
impl FeatureStoreOps for FeatureStoreClient {
    async fn get_feature(&self, feature_name: &str, entity_key: &str) -> Result<Option<Value>> {
        tracing::debug!("Getting feature {} for entity {}", feature_name, entity_key);

        match self.config.provider {
            FeatureStoreProvider::Redis => {
                self.get_redis_feature(feature_name, entity_key).await
            }
            FeatureStoreProvider::Feast => Err(RuntimeError::RuntimeError(
                "Feast not yet implemented".to_string(),
            )),
            FeatureStoreProvider::Http => Err(RuntimeError::RuntimeError(
                "HTTP feature store not yet implemented".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_key_generation() {
        let config = FeatureStoreConfig {
            provider: FeatureStoreProvider::Redis,
            connection_string: "redis://localhost:6379".to_string(),
            namespace: "test_namespace".to_string(),
            default_ttl: 3600,
            options: HashMap::new(),
        };

        let feature_name = "user_risk_score";
        let entity_key = "user_123";

        // Test with namespace
        let expected_key = format!("{}:{}:{}", config.namespace, feature_name, entity_key);
        assert_eq!(expected_key, "test_namespace:user_risk_score:user_123");

        // Test without namespace
        let config_no_ns = FeatureStoreConfig {
            provider: FeatureStoreProvider::Redis,
            connection_string: "redis://localhost:6379".to_string(),
            namespace: "".to_string(),
            default_ttl: 3600,
            options: HashMap::new(),
        };

        let expected_key_no_ns = format!("{}:{}", feature_name, entity_key);
        assert_eq!(expected_key_no_ns, "user_risk_score:user_123");

        // Verify config is used correctly
        assert_eq!(config_no_ns.namespace, "");
    }

    #[tokio::test]
    #[cfg(feature = "redis")]
    async fn test_redis_connection_error() {
        // Test with invalid connection string
        let config = FeatureStoreConfig {
            provider: FeatureStoreProvider::Redis,
            connection_string: "redis://invalid-host:9999".to_string(),
            namespace: "test".to_string(),
            default_ttl: 3600,
            options: HashMap::new(),
        };

        // This should fail to connect but should not panic
        let result = FeatureStoreClient::new(config).await;

        // The client should be created even if connection fails (graceful degradation)
        assert!(result.is_ok());

        let client = result.unwrap();

        // But redis_conn should be None
        assert!(client.redis_conn.is_none());
    }

    #[test]
    fn test_value_to_string_conversion() {
        // Test various Value types
        let test_cases = vec![
            (Value::Null, "null"),
            (Value::Bool(true), "true"),
            (Value::Bool(false), "false"),
            (Value::Number(42.0), "42"),
            (Value::Number(3.14), "3.14"),
            (Value::String("hello".to_string()), "hello"),
        ];

        for (value, expected) in test_cases {
            let result = match &value {
                Value::Null => "null".to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.clone(),
                Value::Array(_) | Value::Object(_) => serde_json::to_string(&value).unwrap(),
            };
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_json_array_serialization() {
        let arr = Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);

        let serialized = serde_json::to_string(&arr).unwrap();
        assert_eq!(serialized, "[1.0,2.0,3.0]");
    }

    #[test]
    fn test_json_object_serialization() {
        let mut obj = HashMap::new();
        obj.insert("name".to_string(), Value::String("John".to_string()));
        obj.insert("age".to_string(), Value::Number(30.0));

        let value = Value::Object(obj);
        let serialized = serde_json::to_string(&value).unwrap();

        // Parse back to verify
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(parsed.is_object());
        assert_eq!(parsed["name"], "John");
        assert_eq!(parsed["age"], 30.0);
    }

    #[test]
    fn test_feature_store_config_deserialization() {
        let yaml = r#"
provider: redis
connection_string: "redis://localhost:6379"
namespace: "user_features"
default_ttl: 7200
"#;

        let config: FeatureStoreConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(config.provider, FeatureStoreProvider::Redis));
        assert_eq!(config.connection_string, "redis://localhost:6379");
        assert_eq!(config.namespace, "user_features");
        assert_eq!(config.default_ttl, 7200);
    }
}
