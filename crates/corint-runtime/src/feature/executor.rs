//! Feature Executor Module
//!
//! This module implements the feature execution engine that:
//! - Executes feature operators against data sources
//! - Manages feature caching (L1 local, L2 Redis)
//! - Handles batch feature execution
//! - Manages feature dependencies

use crate::datasource::DataSourceClient;
use corint_core::Value;
use crate::feature::definition::FeatureDefinition;
use crate::feature::operator::{CacheBackend, CacheConfig, Operator};
use crate::context::ExecutionContext;
use anyhow::{Context as AnyhowContext, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Convert Value to String representation
fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(_) => "[array]".to_string(),
        Value::Object(_) => "{object}".to_string(),
    }
}

/// Cache entry with value and expiration time
#[derive(Debug, Clone)]
struct CacheEntry {
    value: Value,
    expires_at: u64, // Unix timestamp in seconds
}

impl CacheEntry {
    fn new(value: Value, ttl: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            value,
            expires_at: now + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now >= self.expires_at
    }
}

/// Feature executor that handles feature computation and caching
pub struct FeatureExecutor {
    /// Local L1 cache (in-memory)
    local_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,

    /// Data source clients for feature computation
    datasources: HashMap<String, Arc<DataSourceClient>>,

    /// Feature definitions registry
    features: HashMap<String, FeatureDefinition>,

    /// Enable cache statistics
    enable_stats: bool,

    /// Cache hit/miss statistics
    stats: Arc<RwLock<CacheStats>>,
}

#[derive(Debug, Default, Clone)]
struct CacheStats {
    l1_hits: u64,
    l1_misses: u64,
    l2_hits: u64,
    l2_misses: u64,
    compute_count: u64,
}

impl FeatureExecutor {
    /// Create a new feature executor
    pub fn new() -> Self {
        Self {
            local_cache: Arc::new(RwLock::new(HashMap::new())),
            datasources: HashMap::new(),
            features: HashMap::new(),
            enable_stats: false,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Enable cache statistics
    pub fn with_stats(mut self) -> Self {
        self.enable_stats = true;
        self
    }

    /// Add a data source client
    pub fn add_datasource(&mut self, name: impl Into<String>, client: DataSourceClient) {
        self.datasources.insert(name.into(), Arc::new(client));
    }

    /// Register a feature definition
    pub fn register_feature(&mut self, feature: FeatureDefinition) -> Result<()> {
        feature
            .validate()
            .map_err(|e| anyhow::anyhow!("Failed to validate feature '{}': {}", feature.name, e))?;

        self.features.insert(feature.name.clone(), feature);
        Ok(())
    }

    /// Register multiple features
    pub fn register_features(&mut self, features: Vec<FeatureDefinition>) -> Result<()> {
        for feature in features {
            self.register_feature(feature)?;
        }
        Ok(())
    }

    /// Check if a feature is registered
    pub fn has_feature(&self, feature_name: &str) -> bool {
        self.features.contains_key(feature_name)
    }

    /// Execute a single feature by name
    pub fn execute_feature<'a>(
        &'a self,
        feature_name: &'a str,
        context: &'a ExecutionContext,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            self.execute_feature_impl(feature_name, context).await
        })
    }

    /// Implementation of execute_feature (internal)
    async fn execute_feature_impl(
        &self,
        feature_name: &str,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let feature = self
            .features
            .get(feature_name)
            .with_context(|| format!("Feature '{}' not found", feature_name))?;

        if !feature.is_enabled() {
            return Ok(Value::Null);
        }

        // Build context map from ExecutionContext
        let context_map = context.event_data.clone();

        // Check dependencies first
        let mut dep_values = HashMap::new();
        for dep_name in &feature.dependencies {
            let dep_value = Box::pin(self.execute_feature_impl(dep_name, context)).await?;
            dep_values.insert(dep_name.clone(), dep_value);
        }

        // Try to get from cache
        if let Some(cache_config) = self.get_cache_config(&feature.operator) {
            let cache_key = self.build_cache_key(feature_name, &context_map);

            // L1 cache check
            if let Some(value) = self.get_from_l1_cache(&cache_key).await {
                if self.enable_stats {
                    self.stats.write().await.l1_hits += 1;
                }
                debug!("Feature '{}' L1 cache hit", feature_name);
                return Ok(value);
            }

            if self.enable_stats {
                self.stats.write().await.l1_misses += 1;
            }

            // L2 cache check (Redis)
            if cache_config.backend == CacheBackend::Redis {
                if let Some(value) = self.get_from_l2_cache(&cache_key).await {
                    if self.enable_stats {
                        self.stats.write().await.l2_hits += 1;
                    }
                    debug!("Feature '{}' L2 cache hit", feature_name);

                    // Populate L1 cache
                    self.set_to_l1_cache(&cache_key, value.clone(), cache_config.ttl)
                        .await;

                    return Ok(value);
                }

                if self.enable_stats {
                    self.stats.write().await.l2_misses += 1;
                }
            }

            // Compute feature
            let value = self
                .compute_feature(feature, &context_map, &dep_values)
                .await?;

            if self.enable_stats {
                self.stats.write().await.compute_count += 1;
            }

            // Store in cache
            self.set_to_cache(&cache_key, value.clone(), cache_config)
                .await;

            Ok(value)
        } else {
            // No caching, compute directly
            if self.enable_stats {
                self.stats.write().await.compute_count += 1;
            }

            self.compute_feature(feature, &context_map, &dep_values)
                .await
        }
    }

    /// Execute multiple features in batch
    pub async fn execute_features(
        &self,
        feature_names: &[String],
        context: &ExecutionContext,
    ) -> Result<HashMap<String, Value>> {
        let mut results = HashMap::new();

        // Sort features by dependency order
        let sorted_features = self.sort_by_dependencies(feature_names)?;

        for feature_name in sorted_features {
            let value = self.execute_feature_impl(&feature_name, context).await?;
            results.insert(feature_name, value);
        }

        Ok(results)
    }

    /// Execute all registered features
    pub async fn execute_all(&self, context: &ExecutionContext) -> Result<HashMap<String, Value>> {
        let feature_names: Vec<String> = self.features.keys().cloned().collect();
        self.execute_features(&feature_names, context).await
    }

    /// Compute a feature value (no caching)
    async fn compute_feature(
        &self,
        feature: &FeatureDefinition,
        context: &HashMap<String, Value>,
        _dependencies: &HashMap<String, Value>,
    ) -> Result<Value> {
        debug!("Computing feature '{}'", feature.name);

        // Determine data source
        let datasource_name = self.get_datasource_name(&feature.operator);
        let datasource = self
            .datasources
            .get(&datasource_name)
            .with_context(|| format!("Data source '{}' not found", datasource_name))?;

        // Execute operator
        self.execute_operator(&feature.operator, datasource, context)
            .await
    }

    /// Execute an operator against a data source
    async fn execute_operator(
        &self,
        operator: &Operator,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let result = match operator {
            Operator::Count(op) => op.execute(datasource, context).await,
            Operator::Sum(op) => op.execute(datasource, context).await,
            Operator::Avg(op) => op.execute(datasource, context).await,
            Operator::Max(op) => op.execute(datasource, context).await,
            Operator::Min(op) => op.execute(datasource, context).await,
            Operator::CountDistinct(op) => op.execute(datasource, context).await,
            Operator::CrossDimensionCount(op) => op.execute(datasource, context).await,
            Operator::FirstSeen(op) => op.execute(datasource, context).await,
            Operator::LastSeen(op) => op.execute(datasource, context).await,
            Operator::TimeSince(op) => op.execute(datasource, context).await,
            Operator::Velocity(op) => op.execute(datasource, context).await,
            Operator::FeatureStoreLookup(op) => op.execute(datasource, context).await,
            Operator::ProfileLookup(op) => op.execute(datasource, context).await,
            Operator::Expression(op) => op.execute(context).await,
        };

        result.map_err(|e| anyhow::anyhow!("Operator execution failed: {}", e))
    }

    /// Get data source name from operator
    fn get_datasource_name(&self, operator: &Operator) -> String {
        match operator {
            // Operators with explicit datasource field
            Operator::FeatureStoreLookup(op) => op.datasource.clone(),
            Operator::ProfileLookup(op) => op.datasource.clone(),

            // Operators with OperatorParams (check params.datasource)
            Operator::Count(op) => op.params.datasource.clone().unwrap_or_else(|| "default".to_string()),
            Operator::Sum(op) => op.params.datasource.clone().unwrap_or_else(|| "default".to_string()),
            Operator::Avg(op) => op.params.datasource.clone().unwrap_or_else(|| "default".to_string()),
            Operator::Max(op) => op.params.datasource.clone().unwrap_or_else(|| "default".to_string()),
            Operator::Min(op) => op.params.datasource.clone().unwrap_or_else(|| "default".to_string()),
            Operator::CountDistinct(op) => op.params.datasource.clone().unwrap_or_else(|| "default".to_string()),
            Operator::Velocity(op) => op.params.datasource.clone().unwrap_or_else(|| "default".to_string()),

            // Other operators use default
            _ => "default".to_string(),
        }
    }

    /// Get cache configuration from operator
    fn get_cache_config<'a>(&self, operator: &'a Operator) -> Option<&'a CacheConfig> {
        match operator {
            Operator::Count(op) => op.params.cache.as_ref(),
            Operator::Sum(op) => op.params.cache.as_ref(),
            Operator::Avg(op) => op.params.cache.as_ref(),
            Operator::Max(op) => op.params.cache.as_ref(),
            Operator::Min(op) => op.params.cache.as_ref(),
            Operator::CountDistinct(op) => op.params.cache.as_ref(),
            Operator::CrossDimensionCount(_) => None, // No cache support for this operator
            Operator::FirstSeen(_) => None, // No cache support for this operator
            Operator::LastSeen(_) => None, // No cache support for this operator
            Operator::TimeSince(_) => None, // No cache support for this operator
            Operator::Velocity(op) => op.params.cache.as_ref(),
            _ => None,
        }
    }

    /// Build cache key from feature name and context
    fn build_cache_key(&self, feature_name: &str, context: &HashMap<String, Value>) -> String {
        // Extract key dimension values from context
        let mut key_parts = vec![feature_name.to_string()];

        // Add relevant context values (user_id, device_id, ip_address, etc.)
        for key in &["user_id", "device_id", "ip_address", "merchant_id"] {
            if let Some(value) = context.get(*key) {
                let value_str = value_to_string(value);
                key_parts.push(format!("{}:{}", key, value_str));
            }
        }

        key_parts.join(":")
    }

    /// Get value from L1 cache
    async fn get_from_l1_cache(&self, key: &str) -> Option<Value> {
        let cache = self.local_cache.read().await;
        if let Some(entry) = cache.get(key) {
            if !entry.is_expired() {
                return Some(entry.value.clone());
            }
        }
        None
    }

    /// Set value to L1 cache
    async fn set_to_l1_cache(&self, key: &str, value: Value, ttl: u64) {
        let mut cache = self.local_cache.write().await;
        cache.insert(key.to_string(), CacheEntry::new(value, ttl));
    }

    /// Get value from L2 cache (Redis)
    async fn get_from_l2_cache(&self, _key: &str) -> Option<Value> {
        // TODO: Implement Redis cache lookup
        // This would use the redis datasource to fetch cached values
        None
    }

    /// Set value to cache (L1 and optionally L2)
    async fn set_to_cache(&self, key: &str, value: Value, config: &CacheConfig) {
        // Always set L1 cache
        self.set_to_l1_cache(key, value.clone(), config.ttl).await;

        // Set L2 cache if Redis backend
        if config.backend == CacheBackend::Redis {
            // TODO: Implement Redis cache write
        }
    }

    /// Sort features by dependency order (topological sort)
    fn sort_by_dependencies(&self, feature_names: &[String]) -> Result<Vec<String>> {
        let mut sorted = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut visiting = std::collections::HashSet::new();

        for name in feature_names {
            self.visit_feature(name, &mut sorted, &mut visited, &mut visiting)?;
        }

        Ok(sorted)
    }

    /// Visit a feature in dependency graph (for topological sort)
    fn visit_feature(
        &self,
        name: &str,
        sorted: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
        visiting: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        if visited.contains(name) {
            return Ok(());
        }

        if visiting.contains(name) {
            return Err(anyhow::anyhow!("Circular dependency detected: {}", name));
        }

        visiting.insert(name.to_string());

        if let Some(feature) = self.features.get(name) {
            for dep in &feature.dependencies {
                self.visit_feature(dep, sorted, visited, visiting)?;
            }
        }

        visiting.remove(name);
        visited.insert(name.to_string());
        sorted.push(name.to_string());

        Ok(())
    }

    /// Clear L1 cache
    pub async fn clear_cache(&self) {
        let mut cache = self.local_cache.write().await;
        cache.clear();
        info!("L1 cache cleared");
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Print cache statistics
    pub async fn print_stats(&self) {
        if !self.enable_stats {
            warn!("Statistics not enabled");
            return;
        }

        let stats = self.stats.read().await;
        let total = stats.l1_hits + stats.l1_misses;
        let l1_hit_rate = if total > 0 {
            (stats.l1_hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let l2_total = stats.l2_hits + stats.l2_misses;
        let l2_hit_rate = if l2_total > 0 {
            (stats.l2_hits as f64 / l2_total as f64) * 100.0
        } else {
            0.0
        };

        info!("=== Feature Executor Cache Statistics ===");
        info!("L1 Hits: {}, Misses: {}, Hit Rate: {:.2}%", stats.l1_hits, stats.l1_misses, l1_hit_rate);
        info!("L2 Hits: {}, Misses: {}, Hit Rate: {:.2}%", stats.l2_hits, stats.l2_misses, l2_hit_rate);
        info!("Total Computations: {}", stats.compute_count);
    }
}

impl Default for FeatureExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_expiration() {
        let entry = CacheEntry::new(Value::Integer(42), 1); // 1 second TTL
        assert!(!entry.is_expired());

        std::thread::sleep(Duration::from_secs(2));
        assert!(entry.is_expired());
    }

    #[test]
    fn test_cache_key_building() {
        let executor = FeatureExecutor::new();
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("user123".to_string()));
        context.insert("device_id".to_string(), Value::String("device456".to_string()));

        let key = executor.build_cache_key("login_count_24h", &context);
        assert!(key.contains("login_count_24h"));
        assert!(key.contains("user_id:user123"));
        assert!(key.contains("device_id:device456"));
    }
}
