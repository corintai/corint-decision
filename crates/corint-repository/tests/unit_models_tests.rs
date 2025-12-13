//! Unit tests for repository models (CacheStats, CachedArtifact, CacheConfig)

use corint_repository::{CacheConfig, CacheStats};
use std::time::Duration;

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
fn test_cache_stats_hit_rate_zero_requests() {
    let stats = CacheStats {
        hits: 0,
        misses: 0,
        size: 0,
        memory_bytes: 0,
    };

    assert_eq!(stats.hit_rate(), 0.0);
}

#[test]
fn test_cache_stats_hit_rate_all_hits() {
    let stats = CacheStats {
        hits: 100,
        misses: 0,
        size: 50,
        memory_bytes: 10000,
    };

    assert_eq!(stats.hit_rate(), 1.0);
}

#[test]
fn test_cache_stats_hit_rate_all_misses() {
    let stats = CacheStats {
        hits: 0,
        misses: 100,
        size: 50,
        memory_bytes: 10000,
    };

    assert_eq!(stats.hit_rate(), 0.0);
}

#[test]
fn test_cache_stats_hit_rate_mixed() {
    let stats = CacheStats {
        hits: 75,
        misses: 25,
        size: 50,
        memory_bytes: 10000,
    };

    assert_eq!(stats.hit_rate(), 0.75);
}

#[test]
fn test_cache_stats_hit_rate_precision() {
    let stats = CacheStats {
        hits: 2,
        misses: 3,
        size: 2,
        memory_bytes: 1000,
    };

    // 2/5 = 0.4
    assert!((stats.hit_rate() - 0.4).abs() < 0.0001);
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
}

#[test]
fn test_cache_config_with_ttl() {
    let config = CacheConfig::new().with_ttl(Duration::from_secs(60));

    assert_eq!(config.default_ttl, Duration::from_secs(60));
    assert!(config.enabled);
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
fn test_cache_config_builder_chaining() {
    let config = CacheConfig::new()
        .with_ttl(Duration::from_secs(120))
        .with_max_entries(250)
        .unlimited_entries();

    assert!(config.enabled);
    assert_eq!(config.default_ttl, Duration::from_secs(120));
    assert_eq!(config.max_entries, None);
}

#[test]
fn test_cache_config_disabled_with_custom_ttl() {
    let config = CacheConfig::disabled().with_ttl(Duration::from_secs(10));

    assert!(!config.enabled);
    assert_eq!(config.default_ttl, Duration::from_secs(10));
}

// Test CachedArtifact through public API
#[tokio::test]
async fn test_cached_artifact_expiration() {
    use corint_repository::{CacheableRepository, FileSystemRepository, Repository};
    use tempfile::TempDir;
    use tokio::fs;

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create directory and test file
    fs::create_dir_all(repo_path.join("library/rules"))
        .await
        .unwrap();

    let rule_content = r#"version: "0.1"

rule:
  id: ttl_test_rule
  name: TTL Test Rule
  when:
    conditions:
      - amount > 100
  score: 10
"#;
    fs::write(
        repo_path.join("library/rules/ttl_test_rule.yaml"),
        rule_content,
    )
    .await
    .unwrap();

    // Create repo with 1-second TTL
    let config = CacheConfig::new().with_ttl(Duration::from_millis(100));
    let repo = FileSystemRepository::with_cache_config(repo_path, config).unwrap();

    // Load rule - should cache it
    let _ = repo.load_rule("ttl_test_rule").await.unwrap();

    // Load again immediately - should be cache hit
    let stats_before = repo.cache_stats();
    let _ = repo.load_rule("ttl_test_rule").await.unwrap();
    let stats_after = repo.cache_stats();

    assert!(
        stats_after.hits > stats_before.hits,
        "Second load should be cache hit"
    );

    // Wait for cache to expire
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Load again - should be cache miss due to expiration
    let stats_before = repo.cache_stats();
    let _ = repo.load_rule("ttl_test_rule").await.unwrap();
    let stats_after = repo.cache_stats();

    assert!(
        stats_after.misses > stats_before.misses,
        "Load after TTL should be cache miss"
    );
}

#[tokio::test]
async fn test_cache_disabled() {
    use corint_repository::{CacheableRepository, FileSystemRepository, Repository};
    use tempfile::TempDir;
    use tokio::fs;

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create directory and test file
    fs::create_dir_all(repo_path.join("library/rules"))
        .await
        .unwrap();

    let rule_content = r#"version: "0.1"

rule:
  id: no_cache_rule
  name: No Cache Rule
  when:
    conditions:
      - amount > 100
  score: 10
"#;
    fs::write(
        repo_path.join("library/rules/no_cache_rule.yaml"),
        rule_content,
    )
    .await
    .unwrap();

    // Create repo with caching disabled
    let config = CacheConfig::disabled();
    let repo = FileSystemRepository::with_cache_config(repo_path, config).unwrap();

    assert!(!repo.is_cache_enabled());

    // Load rule twice
    let _ = repo.load_rule("no_cache_rule").await.unwrap();
    let _ = repo.load_rule("no_cache_rule").await.unwrap();

    // With caching disabled, both should be misses
    let stats = repo.cache_stats();
    assert_eq!(stats.hits, 0, "Cache disabled should have no hits");
}
