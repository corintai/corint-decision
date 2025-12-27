//! Unified Data Source Client
//!
//! Provides a unified interface for accessing different data sources.

use super::cache::FeatureCache;
use super::config::{DataSourceConfig, DataSourceType};
use super::query::{Query, QueryResult};
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;


/// Unified data source client
pub struct DataSourceClient {
    /// Data source configuration
    config: DataSourceConfig,

    /// Feature cache
    cache: Arc<Mutex<FeatureCache>>,

    /// Underlying client implementation
    client: Box<dyn DataSourceImpl>,
}

impl DataSourceClient {
    /// Create a new data source client
    pub async fn new(config: DataSourceConfig) -> Result<Self> {
        let client: Box<dyn DataSourceImpl> = match &config.source_type {
            DataSourceType::FeatureStore(fs_config) => {
                Box::new(FeatureStoreClient::new(fs_config.clone()).await?)
            }
            DataSourceType::OLAP(olap_config) => {
                Box::new(OLAPClient::new(olap_config.clone()).await?)
            }
            DataSourceType::SQL(sql_config) => {
                Box::new(SQLClient::new(sql_config.clone(), config.pool_size).await?)
            }
        };

        Ok(Self {
            config,
            cache: Arc::new(Mutex::new(FeatureCache::new())),
            client,
        })
    }

    /// Execute a query
    pub async fn query(&self, query: Query) -> Result<QueryResult> {
        // Check cache first
        let cache_key = self.generate_cache_key(&query);
        if let Some(cached_value) = self.cache.lock().unwrap().get(&cache_key) {
            tracing::debug!("Cache hit for key: {}", cache_key);
            return Ok(QueryResult {
                rows: vec![cached_value.clone()],
                execution_time_ms: 0,
                source: self.config.name.clone(),
                from_cache: true,
            });
        }

        // Execute query
        let start = Instant::now();
        let result = self.client.execute(query.clone()).await?;
        let execution_time_ms = start.elapsed().as_millis() as u64;

        // Cache result if applicable
        if !result.rows.is_empty() {
            if let Some(row) = result.rows.first() {
                self.cache.lock().unwrap().set(
                    cache_key,
                    row.clone(),
                    std::time::Duration::from_secs(300),
                );
            }
        }

        Ok(QueryResult {
            rows: result.rows,
            execution_time_ms,
            source: self.config.name.clone(),
            from_cache: false,
        })
    }

    /// Get a feature from feature store
    pub async fn get_feature(&self, feature_name: &str, entity_key: &str) -> Result<Option<Value>> {
        let cache_key = format!("feature:{}:{}", feature_name, entity_key);

        // Check cache
        if let Some(cached) = self.cache.lock().unwrap().get(&cache_key) {
            return Ok(cached.get("value").cloned());
        }

        // Get from feature store
        if let Some(fs_client) = self.client.as_feature_store() {
            let value = fs_client.get_feature(feature_name, entity_key).await?;

            // Cache the result
            if let Some(ref val) = value {
                let mut row = HashMap::new();
                row.insert("value".to_string(), val.clone());
                self.cache
                    .lock()
                    .unwrap()
                    .set(cache_key, row, std::time::Duration::from_secs(300));
            }

            Ok(value)
        } else {
            Err(RuntimeError::RuntimeError(
                "Data source is not a feature store".to_string(),
            ))
        }
    }

    /// Generate cache key for a query
    fn generate_cache_key(&self, query: &Query) -> String {
        // Include all query parameters in cache key to avoid collisions
        let mut key_parts = vec![
            self.config.name.clone(),
            query.entity.clone(),
            serde_json::to_string(&query.filters).unwrap_or_default(),
        ];

        // Include time window in cache key
        if let Some(ref time_window) = query.time_window {
            key_parts.push(format!(
                "window:{}:{}",
                time_window.time_field,
                serde_json::to_string(&time_window.window_type).unwrap_or_default()
            ));
        }

        // Include aggregations in cache key (different aggregations should have different cache keys)
        if !query.aggregations.is_empty() {
            key_parts.push(format!(
                "agg:{}",
                serde_json::to_string(&query.aggregations).unwrap_or_default()
            ));
        }

        // Include group_by in cache key
        if !query.group_by.is_empty() {
            key_parts.push(format!("group_by:{}", query.group_by.join(",")));
        }

        // Include limit in cache key
        if let Some(limit) = query.limit {
            key_parts.push(format!("limit:{}", limit));
        }

        format!("query:{}", key_parts.join(":"))
    }

    /// Get data source name
    pub fn name(&self) -> &str {
        &self.config.name
    }
}

/// Trait for data source implementations
#[async_trait::async_trait]
pub(super) trait DataSourceImpl: Send + Sync {
    /// Execute a query
    async fn execute(&self, query: Query) -> Result<QueryResult>;

    /// Downcast to feature store client
    fn as_feature_store(&self) -> Option<&dyn FeatureStoreOps> {
        None
    }
}

/// Feature store operations
#[async_trait::async_trait]
pub(super) trait FeatureStoreOps: Send + Sync {
    /// Get a feature value
    async fn get_feature(&self, feature_name: &str, entity_key: &str) -> Result<Option<Value>>;
}

use super::feature_store::FeatureStoreClient;
use super::olap::OLAPClient;
use super::sql::SQLClient;

