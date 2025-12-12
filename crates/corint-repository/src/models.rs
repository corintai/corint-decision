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
