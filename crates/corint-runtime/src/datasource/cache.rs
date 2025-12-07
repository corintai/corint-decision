//! Feature Caching Layer
//!
//! Caching strategies for feature computation results.

use corint_core::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Cache strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStrategy {
    /// Enable caching
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Time-to-live in seconds
    #[serde(default = "default_ttl")]
    pub ttl: u64,

    /// Cache key template (supports {field} substitution)
    pub key_template: Option<String>,

    /// Strategy type
    #[serde(default)]
    pub strategy: CacheStrategyType,
}

/// Cache strategy types
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheStrategyType {
    /// Lazy loading - cache on read
    #[default]
    Lazy,

    /// Eager loading - pre-fetch
    Eager,

    /// Refresh - update in background
    Refresh,
}

/// Cached result wrapper
#[derive(Debug, Clone)]
pub struct CachedResult {
    /// The cached value
    pub value: HashMap<String, Value>,

    /// When the cache entry was created
    pub cached_at: SystemTime,

    /// TTL duration
    pub ttl: Duration,
}

impl CachedResult {
    /// Create a new cached result
    pub fn new(value: HashMap<String, Value>, ttl: Duration) -> Self {
        Self {
            value,
            cached_at: SystemTime::now(),
            ttl,
        }
    }

    /// Check if the cache entry is still valid
    pub fn is_valid(&self) -> bool {
        match self.cached_at.elapsed() {
            Ok(elapsed) => elapsed < self.ttl,
            Err(_) => false,
        }
    }

    /// Get remaining TTL
    pub fn remaining_ttl(&self) -> Option<Duration> {
        self.cached_at
            .elapsed()
            .ok()
            .and_then(|elapsed| self.ttl.checked_sub(elapsed))
    }
}

/// In-memory cache for feature results
pub struct FeatureCache {
    cache: HashMap<String, CachedResult>,
}

impl FeatureCache {
    /// Create a new feature cache
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Get a cached value
    pub fn get(&self, key: &str) -> Option<&HashMap<String, Value>> {
        self.cache.get(key).and_then(|entry| {
            if entry.is_valid() {
                Some(&entry.value)
            } else {
                None
            }
        })
    }

    /// Set a cached value
    pub fn set(&mut self, key: String, value: HashMap<String, Value>, ttl: Duration) {
        self.cache.insert(key, CachedResult::new(value, ttl));
    }

    /// Remove expired entries
    pub fn cleanup(&mut self) {
        self.cache.retain(|_, entry| entry.is_valid());
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get cache size
    pub fn size(&self) -> usize {
        self.cache.len()
    }
}

impl Default for FeatureCache {
    fn default() -> Self {
        Self::new()
    }
}

fn default_true() -> bool {
    true
}

fn default_ttl() -> u64 {
    300 // 5 minutes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_validity() {
        let mut cache = FeatureCache::new();

        let mut value = HashMap::new();
        value.insert("count".to_string(), Value::Number(42.0));

        cache.set("test_key".to_string(), value.clone(), Duration::from_secs(1));

        // Should be valid immediately
        assert!(cache.get("test_key").is_some());

        // Wait for expiration
        std::thread::sleep(Duration::from_secs(2));

        // Should be expired
        assert!(cache.get("test_key").is_none());
    }

    #[test]
    fn test_cache_cleanup() {
        let mut cache = FeatureCache::new();

        let mut value = HashMap::new();
        value.insert("count".to_string(), Value::Number(1.0));

        // Add expired entry
        cache.set("expired".to_string(), value.clone(), Duration::from_secs(0));

        // Add valid entry
        value.insert("count".to_string(), Value::Number(2.0));
        cache.set("valid".to_string(), value, Duration::from_secs(60));

        std::thread::sleep(Duration::from_millis(100));

        assert_eq!(cache.size(), 2);
        cache.cleanup();
        assert_eq!(cache.size(), 1);
        assert!(cache.get("valid").is_some());
    }
}
