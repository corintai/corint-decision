//! Data models for the repository layer

use std::time::{Duration, Instant};

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of entries in cache
    pub size: usize,
    /// Total memory used (approximate, in bytes)
    pub memory_bytes: usize,
}

impl CacheStats {
    /// Calculate cache hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// A cached artifact with TTL support
#[derive(Debug, Clone)]
pub(crate) struct CachedArtifact<T> {
    /// The cached data
    pub data: T,
    /// The raw content string
    pub content: String,
    /// When this entry was cached
    pub cached_at: Instant,
    /// Time-to-live duration
    pub ttl: Duration,
}

impl<T> CachedArtifact<T> {
    /// Create a new cached artifact
    pub fn new(data: T, content: String, ttl: Duration) -> Self {
        Self {
            data,
            content,
            cached_at: Instant::now(),
            ttl,
        }
    }

    /// Check if this cached entry has expired
    pub fn is_expired(&self) -> bool {
        self.cached_at.elapsed() > self.ttl
    }

    /// Get the age of this cached entry
    #[allow(dead_code)]
    pub fn age(&self) -> Duration {
        self.cached_at.elapsed()
    }
}

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Default time-to-live for cache entries
    pub default_ttl: Duration,
    /// Maximum number of entries to keep in cache
    pub max_entries: Option<usize>,
    /// Maximum memory usage (in bytes)
    pub max_memory_bytes: Option<usize>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_ttl: Duration::from_secs(300), // 5 minutes
            max_entries: Some(1000),
            max_memory_bytes: Some(100 * 1024 * 1024), // 100 MB
        }
    }
}

impl CacheConfig {
    /// Create a new cache configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable caching
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Set the default TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// Set the maximum number of entries
    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = Some(max);
        self
    }

    /// Remove the entry limit
    pub fn unlimited_entries(mut self) -> Self {
        self.max_entries = None;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_stats_hit_rate() {
        let stats = CacheStats {
            hits: 80,
            misses: 20,
            size: 100,
            memory_bytes: 1024,
        };

        assert_eq!(stats.hit_rate(), 0.8);
    }

    #[test]
    fn test_cache_stats_zero_total() {
        let stats = CacheStats {
            hits: 0,
            misses: 0,
            size: 0,
            memory_bytes: 0,
        };

        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_default() {
        let stats = CacheStats::default();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.size, 0);
        assert_eq!(stats.memory_bytes, 0);
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cached_artifact_creation() {
        let data = "test_data".to_string();
        let content = "test_content".to_string();
        let ttl = Duration::from_secs(60);

        let artifact = CachedArtifact::new(data.clone(), content.clone(), ttl);

        assert_eq!(artifact.data, data);
        assert_eq!(artifact.content, content);
        assert_eq!(artifact.ttl, ttl);
        assert!(!artifact.is_expired());
    }

    #[test]
    fn test_cached_artifact_age() {
        let data = "test".to_string();
        let content = "content".to_string();
        let ttl = Duration::from_secs(60);

        let artifact = CachedArtifact::new(data, content, ttl);
        let age = artifact.age();

        // Age should be very small (just created)
        assert!(age < Duration::from_millis(100));
    }

    #[test]
    fn test_cached_artifact_not_expired() {
        let data = "test".to_string();
        let content = "content".to_string();
        let ttl = Duration::from_secs(60);

        let artifact = CachedArtifact::new(data, content, ttl);

        assert!(!artifact.is_expired());
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();

        assert!(config.enabled);
        assert_eq!(config.default_ttl, Duration::from_secs(300));
        assert_eq!(config.max_entries, Some(1000));
        assert_eq!(config.max_memory_bytes, Some(100 * 1024 * 1024));
    }

    #[test]
    fn test_cache_config_new() {
        let config = CacheConfig::new();

        assert!(config.enabled);
        assert_eq!(config.default_ttl, Duration::from_secs(300));
    }

    #[test]
    fn test_cache_config_disabled() {
        let config = CacheConfig::disabled();

        assert!(!config.enabled);
        assert_eq!(config.default_ttl, Duration::from_secs(300));
    }

    #[test]
    fn test_cache_config_with_ttl() {
        let ttl = Duration::from_secs(600);
        let config = CacheConfig::new().with_ttl(ttl);

        assert_eq!(config.default_ttl, ttl);
    }

    #[test]
    fn test_cache_config_with_max_entries() {
        let config = CacheConfig::new().with_max_entries(500);

        assert_eq!(config.max_entries, Some(500));
    }

    #[test]
    fn test_cache_config_unlimited_entries() {
        let config = CacheConfig::new().unlimited_entries();

        assert_eq!(config.max_entries, None);
    }

    #[test]
    fn test_cache_config_chaining() {
        let config = CacheConfig::new()
            .with_ttl(Duration::from_secs(120))
            .with_max_entries(2000);

        assert!(config.enabled);
        assert_eq!(config.default_ttl, Duration::from_secs(120));
        assert_eq!(config.max_entries, Some(2000));
    }

    #[test]
    fn test_cache_stats_100_percent_hit_rate() {
        let stats = CacheStats {
            hits: 100,
            misses: 0,
            size: 50,
            memory_bytes: 2048,
        };

        assert_eq!(stats.hit_rate(), 1.0);
    }

    #[test]
    fn test_cache_stats_0_percent_hit_rate() {
        let stats = CacheStats {
            hits: 0,
            misses: 100,
            size: 50,
            memory_bytes: 2048,
        };

        assert_eq!(stats.hit_rate(), 0.0);
    }
}
