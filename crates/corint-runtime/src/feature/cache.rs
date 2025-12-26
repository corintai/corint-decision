//! Feature cache management module
//!
//! This module provides caching functionality for feature execution:
//! - L1 (in-memory) cache with TTL-based expiration
//! - L2 (Redis) cache support (placeholder)
//! - Cache statistics tracking
//! - Cache key generation

use crate::feature::definition::FeatureDefinition;
use crate::feature::operator::{CacheBackend, CacheConfig, Operator};
use corint_core::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn};

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
pub(super) struct CacheEntry {
    pub(super) value: Value,
    pub(super) expires_at: u64, // Unix timestamp in seconds
}

impl CacheEntry {
    pub(super) fn new(value: Value, ttl: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            value,
            expires_at: now + ttl,
        }
    }

    pub(super) fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now >= self.expires_at
    }
}

/// Cache hit/miss statistics
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub compute_count: u64,
}

/// Cache manager for feature executor
pub(super) struct CacheManager {
    /// Local L1 cache (in-memory)
    local_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,

    /// Enable cache statistics
    enable_stats: bool,

    /// Cache hit/miss statistics
    stats: Arc<RwLock<CacheStats>>,
}

impl CacheManager {
    /// Create a new cache manager
    pub(super) fn new() -> Self {
        Self {
            local_cache: Arc::new(RwLock::new(HashMap::new())),
            enable_stats: false,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Enable cache statistics
    pub(super) fn with_stats(mut self) -> Self {
        self.enable_stats = true;
        self
    }

    /// Check if statistics are enabled
    pub(super) fn is_stats_enabled(&self) -> bool {
        self.enable_stats
    }

    /// Get statistics handle
    pub(super) fn stats(&self) -> &Arc<RwLock<CacheStats>> {
        &self.stats
    }

    /// Build cache key from feature name and context
    pub(super) fn build_cache_key(&self, feature_name: &str, context: &HashMap<String, Value>) -> String {
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
    pub(super) async fn get_from_l1_cache(&self, key: &str) -> Option<Value> {
        let cache = self.local_cache.read().await;
        if let Some(entry) = cache.get(key) {
            if !entry.is_expired() {
                return Some(entry.value.clone());
            }
        }
        None
    }

    /// Set value to L1 cache
    pub(super) async fn set_to_l1_cache(&self, key: &str, value: Value, ttl: u64) {
        let mut cache = self.local_cache.write().await;
        cache.insert(key.to_string(), CacheEntry::new(value, ttl));
    }

    /// Get value from L2 cache (Redis)
    pub(super) async fn get_from_l2_cache(&self, _key: &str) -> Option<Value> {
        // TODO: Implement Redis cache lookup
        // This would use the redis datasource to fetch cached values
        None
    }

    /// Set value to cache (L1 and optionally L2)
    pub(super) async fn set_to_cache(&self, key: &str, value: Value, config: &CacheConfig) {
        // Always set L1 cache
        self.set_to_l1_cache(key, value.clone(), config.ttl).await;

        // Set L2 cache if Redis backend
        if config.backend == CacheBackend::Redis {
            // TODO: Implement Redis cache write
        }
    }

    /// Clear L1 cache
    pub(super) async fn clear_cache(&self) {
        let mut cache = self.local_cache.write().await;
        cache.clear();
        info!("L1 cache cleared");
    }

    /// Get cache statistics
    pub(super) async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Print cache statistics
    pub(super) async fn print_stats(&self) {
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
        info!(
            "L1 Hits: {}, Misses: {}, Hit Rate: {:.2}%",
            stats.l1_hits, stats.l1_misses, l1_hit_rate
        );
        info!(
            "L2 Hits: {}, Misses: {}, Hit Rate: {:.2}%",
            stats.l2_hits, stats.l2_misses, l2_hit_rate
        );
        info!("Total Computations: {}", stats.compute_count);
    }

    /// Get cache configuration from feature (currently disabled)
    pub(super) fn get_cache_config<'a>(&self, _feature: &'a FeatureDefinition) -> Option<&'a CacheConfig> {
        // TODO: Implement cache configuration in new feature structure
        None
    }

    /// Get cache configuration from old Operator enum (deprecated, kept for tests)
    pub(super) fn get_cache_config_from_operator<'a>(&self, operator: &'a Operator) -> Option<&'a CacheConfig> {
        match operator {
            Operator::Count(op) => op.params.cache.as_ref(),
            Operator::Sum(op) => op.params.cache.as_ref(),
            Operator::Avg(op) => op.params.cache.as_ref(),
            Operator::Max(op) => op.params.cache.as_ref(),
            Operator::Min(op) => op.params.cache.as_ref(),
            Operator::CountDistinct(op) => op.params.cache.as_ref(),
            Operator::CrossDimensionCount(_) => None, // No cache support for this operator
            Operator::FirstSeen(_) => None,           // No cache support for this operator
            Operator::LastSeen(_) => None,            // No cache support for this operator
            Operator::TimeSince(_) => None,           // No cache support for this operator
            Operator::Velocity(op) => op.params.cache.as_ref(),
            _ => None,
        }
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}
