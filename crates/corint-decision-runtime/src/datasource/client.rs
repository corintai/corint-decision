//! Unified Data Source Client
//!
//! Provides a unified interface for accessing different data sources.

use super::cache::FeatureCache;
use super::config::{DataSourceConfig, DataSourceType};
use super::query::{Query, QueryResult};
use crate::error::{Result, RuntimeError};
use corint_decision_model::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Unified data source client
pub struct DataSourceClient {
    /// Data source configuration
    config: DataSourceConfig,

    /// Feature cache
    cache: Arc<Mutex<FeatureCache>>,
    query_cache: Mutex<HashMap<String, (Instant, QueryResult)>>,

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
            query_cache: Mutex::new(HashMap::new()),
            client,
        })
    }

    /// Execute a query
    pub async fn query(&self, query: Query) -> Result<QueryResult> {
        let ttl = Duration::from_secs(self.config.query_cache_ttl_secs);
        let cache_key = self.generate_cache_key(&query);
        if !ttl.is_zero() {
            if let Some((created, result)) = self.query_cache.lock().unwrap().get(&cache_key) {
                if created.elapsed() < ttl {
                    let mut result = result.clone();
                    result.from_cache = true;
                    result.execution_time_ms = 0;
                    return Ok(result);
                }
            }
        }
        let start = Instant::now();
        let mut result = self.client.execute(query).await?;
        result.execution_time_ms = start.elapsed().as_millis() as u64;
        result.source = self.config.name.clone();
        result.from_cache = false;
        if !ttl.is_zero() {
            let mut cache = self.query_cache.lock().unwrap();
            cache.retain(|_, (created, _)| created.elapsed() < ttl);
            // Bound memory for high-cardinality entity keys.
            if cache.len() >= 4096 {
                cache.clear();
            }
            cache.insert(cache_key, (Instant::now(), result.clone()));
        }
        Ok(result)
    }

    /// Explicitly invalidate cached query results after operator-owned writes.
    pub fn clear_query_cache(&self) {
        self.query_cache.lock().unwrap().clear();
    }

    /// Admission check shared by feature registration and query construction.
    pub fn validate_aggregation(&self, method: &str) -> Result<()> {
        if !matches!(
            method,
            "count"
                | "sum"
                | "avg"
                | "min"
                | "max"
                | "distinct"
                | "median"
                | "stddev"
                | "percentile"
        ) {
            return Err(RuntimeError::InvalidOperation(format!(
                "Unsupported aggregation method: {method}"
            )));
        }
        if let DataSourceType::SQL(config) = &self.config.source_type {
            use super::config::SQLProvider;
            if matches!(config.provider, SQLProvider::MySQL)
                || (matches!(config.provider, SQLProvider::SQLite)
                    && matches!(method, "median" | "stddev" | "percentile"))
            {
                return Err(RuntimeError::InvalidOperation(format!(
                    "Unsupported aggregation '{method}' for {:?}",
                    config.provider
                )));
            }
        }
        Ok(())
    }

    /// Get a feature from feature store
    pub async fn get_feature(&self, feature_name: &str, entity_key: &str) -> Result<Option<Value>> {
        let cache_key = format!("feature:{}:{}", feature_name, entity_key);
        let ttl = match &self.config.source_type {
            DataSourceType::FeatureStore(config) => config.default_ttl,
            _ => 0,
        };

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
                    .set(cache_key, row, Duration::from_secs(ttl));
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
        // Includes query_type, filters, windows, grouping, aliases and limits.
        serde_json::to_string(query).expect("Query is serializable")
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
