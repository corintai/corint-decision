//! Unit tests for FileSystemRepository implementation
//!
//! These tests focus on path resolution, caching behavior, and edge cases.

use corint_repository::{CacheableRepository, FileSystemRepository, Repository};
use tempfile::TempDir;
use tokio::fs;

/// Helper function to create a test repository with sample files
async fn create_test_repo() -> (TempDir, FileSystemRepository) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create directory structure
    fs::create_dir_all(repo_path.join("library/rules/fraud")).await.unwrap();
    fs::create_dir_all(repo_path.join("library/rules/kyc")).await.unwrap();
    fs::create_dir_all(repo_path.join("library/rulesets")).await.unwrap();
    fs::create_dir_all(repo_path.join("library/templates")).await.unwrap();
    fs::create_dir_all(repo_path.join("pipelines")).await.unwrap();

    // Create test rules
    let fraud_rule = r#"version: "0.1"

rule:
  id: fraud_check
  name: Fraud Check Rule
  when:
    conditions:
      - amount > 1000
  score: 50
"#;
    fs::write(
        repo_path.join("library/rules/fraud/fraud_check.yaml"),
        fraud_rule,
    )
    .await
    .unwrap();

    let kyc_rule = r#"version: "0.1"

rule:
  id: kyc_verification
  name: KYC Verification
  when:
    conditions:
      - verified == false
  score: 100
"#;
    fs::write(
        repo_path.join("library/rules/kyc/kyc_verification.yaml"),
        kyc_rule,
    )
    .await
    .unwrap();

    // Create test ruleset
    let ruleset = r#"version: "0.1"

ruleset:
  id: test_ruleset
  name: Test Ruleset
  rules:
    - fraud_check
    - kyc_verification
  decision_logic:
    - condition: total_score >= 100
      action: deny
    - default: true
      action: approve
"#;
    fs::write(
        repo_path.join("library/rulesets/test_ruleset.yaml"),
        ruleset,
    )
    .await
    .unwrap();

    // Create test template
    let template = r#"version: "0.1"

template:
  id: score_template
  name: Score-Based Template
  params:
    threshold: 100
  decision_logic:
    - condition: total_score >= params.threshold
      action: deny
    - default: true
      action: approve
"#;
    fs::write(
        repo_path.join("library/templates/score_template.yaml"),
        template,
    )
    .await
    .unwrap();

    // Create test pipeline
    let pipeline = r#"version: "0.1"

pipeline:
  id: test_pipeline
  name: Test Pipeline
  stages:
    - ruleset: test_ruleset
"#;
    fs::write(
        repo_path.join("pipelines/test_pipeline.yaml"),
        pipeline,
    )
    .await
    .unwrap();

    // Create registry
    let registry = r#"version: "0.1"

registry:
  pipelines:
    - event_type: transaction
      pipeline: test_pipeline
"#;
    fs::write(repo_path.join("registry.yaml"), registry).await.unwrap();

    let repo = FileSystemRepository::new(repo_path).unwrap();
    (temp_dir, repo)
}

// ==================================================
// Path Resolution Tests
// ==================================================

#[tokio::test]
async fn test_load_rule_by_full_path() {
    let (_temp, repo) = create_test_repo().await;

    let (rule, content) = repo
        .load_rule("library/rules/fraud/fraud_check.yaml")
        .await
        .expect("Failed to load rule by path");

    assert_eq!(rule.id, "fraud_check");
    assert_eq!(rule.name, "Fraud Check Rule");
    assert!(content.contains("fraud_check"));
}

#[tokio::test]
async fn test_load_rule_by_id() {
    let (_temp, repo) = create_test_repo().await;

    let (rule, _) = repo
        .load_rule("fraud_check")
        .await
        .expect("Failed to load rule by ID");

    assert_eq!(rule.id, "fraud_check");
}

#[tokio::test]
async fn test_load_rule_by_id_searches_subdirectories() {
    let (_temp, repo) = create_test_repo().await;

    // kyc_verification is in library/rules/kyc/ subdirectory
    let (rule, _) = repo
        .load_rule("kyc_verification")
        .await
        .expect("Failed to find rule in subdirectory");

    assert_eq!(rule.id, "kyc_verification");
}

#[tokio::test]
async fn test_load_ruleset_by_path() {
    let (_temp, repo) = create_test_repo().await;

    let (ruleset, content) = repo
        .load_ruleset("library/rulesets/test_ruleset.yaml")
        .await
        .expect("Failed to load ruleset");

    assert_eq!(ruleset.id, "test_ruleset");
    assert!(content.contains("test_ruleset"));
}

#[tokio::test]
async fn test_load_ruleset_by_id() {
    let (_temp, repo) = create_test_repo().await;

    let (ruleset, _) = repo
        .load_ruleset("test_ruleset")
        .await
        .expect("Failed to load ruleset by ID");

    assert_eq!(ruleset.id, "test_ruleset");
    assert_eq!(ruleset.rules.len(), 2);
}

#[tokio::test]
async fn test_load_template_by_path() {
    let (_temp, repo) = create_test_repo().await;

    let (template, _) = repo
        .load_template("library/templates/score_template.yaml")
        .await
        .expect("Failed to load template");

    assert_eq!(template.id, "score_template");
}

#[tokio::test]
async fn test_load_template_by_id() {
    let (_temp, repo) = create_test_repo().await;

    let (template, _) = repo
        .load_template("score_template")
        .await
        .expect("Failed to load template by ID");

    assert_eq!(template.id, "score_template");
}

#[tokio::test]
async fn test_load_pipeline_by_path() {
    let (_temp, repo) = create_test_repo().await;

    let (pipeline, _) = repo
        .load_pipeline("pipelines/test_pipeline.yaml")
        .await
        .expect("Failed to load pipeline");

    assert_eq!(pipeline.id, Some("test_pipeline".to_string()));
}

#[tokio::test]
async fn test_load_pipeline_by_id() {
    let (_temp, repo) = create_test_repo().await;

    let (pipeline, _) = repo
        .load_pipeline("test_pipeline")
        .await
        .expect("Failed to load pipeline by ID");

    assert_eq!(pipeline.id, Some("test_pipeline".to_string()));
}

#[tokio::test]
async fn test_load_registry() {
    let (_temp, repo) = create_test_repo().await;

    let content = repo.load_registry().await.expect("Failed to load registry");

    assert!(content.contains("registry"));
    assert!(content.contains("test_pipeline"));
}

// ==================================================
// Caching Tests
// ==================================================

#[tokio::test]
async fn test_cache_hit_on_second_load() {
    let (_temp, repo) = create_test_repo().await;

    // First load - cache miss
    let stats_before = repo.cache_stats();
    let (rule1, _) = repo
        .load_rule("fraud_check")
        .await
        .expect("Failed to load rule");
    let stats_after_first = repo.cache_stats();

    assert_eq!(stats_after_first.misses, stats_before.misses + 1);

    // Second load - cache hit
    let (rule2, _) = repo
        .load_rule("fraud_check")
        .await
        .expect("Failed to load rule");
    let stats_after_second = repo.cache_stats();

    assert_eq!(rule1.id, rule2.id);
    assert_eq!(stats_after_second.hits, stats_after_first.hits + 1);
}

#[tokio::test]
async fn test_cache_separate_for_different_identifiers() {
    let (_temp, repo) = create_test_repo().await;

    // Load same rule by different identifiers (path vs ID)
    let _ = repo
        .load_rule("library/rules/fraud/fraud_check.yaml")
        .await
        .unwrap();
    let _ = repo.load_rule("fraud_check").await.unwrap();

    let stats = repo.cache_stats();

    // Both should be cache misses since identifiers are different
    assert_eq!(stats.misses, 2);
    assert_eq!(stats.hits, 0);
}

#[tokio::test]
async fn test_clear_cache() {
    let (_temp, mut repo) = create_test_repo().await;

    // Load to populate cache
    let _ = repo.load_rule("fraud_check").await.unwrap();
    let _ = repo.load_ruleset("test_ruleset").await.unwrap();

    let stats_before_clear = repo.cache_stats();
    assert!(stats_before_clear.size > 0 || stats_before_clear.misses > 0);

    // Clear cache
    repo.clear_cache();

    // Give async task time to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let stats_after_clear = repo.cache_stats();
    assert_eq!(stats_after_clear.size, 0);
}

#[tokio::test]
async fn test_clear_cache_entry() {
    let (_temp, mut repo) = create_test_repo().await;

    // Load two rules
    let _ = repo.load_rule("fraud_check").await.unwrap();
    let _ = repo.load_rule("kyc_verification").await.unwrap();

    // Clear specific entry
    repo.clear_cache_entry("fraud_check");

    // Give async task time to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Load both again
    let stats_before = repo.cache_stats();
    let _ = repo.load_rule("fraud_check").await.unwrap(); // Should be miss
    let _ = repo.load_rule("kyc_verification").await.unwrap(); // Should be hit

    let stats_after = repo.cache_stats();

    // We should have at least one hit (from kyc_verification)
    assert!(stats_after.hits > stats_before.hits);
}

#[tokio::test]
async fn test_cache_enable_disable() {
    let (_temp, mut repo) = create_test_repo().await;

    assert!(repo.is_cache_enabled());

    // Disable cache
    repo.set_cache_enabled(false);
    assert!(!repo.is_cache_enabled());

    // Load twice with cache disabled
    let _ = repo.load_rule("fraud_check").await.unwrap();
    let _ = repo.load_rule("fraud_check").await.unwrap();

    let stats = repo.cache_stats();
    assert_eq!(stats.hits, 0, "With cache disabled, there should be no hits");

    // Re-enable cache
    repo.set_cache_enabled(true);
    assert!(repo.is_cache_enabled());

    // Now cache should work
    let _ = repo.load_rule("kyc_verification").await.unwrap();
    let _ = repo.load_rule("kyc_verification").await.unwrap();

    let stats_after_enable = repo.cache_stats();
    assert!(stats_after_enable.hits > 0, "After enabling, cache should work");
}

// ==================================================
// List Operations Tests
// ==================================================

#[tokio::test]
async fn test_list_rules() {
    let (_temp, repo) = create_test_repo().await;

    let rules = repo.list_rules().await.expect("Failed to list rules");

    assert_eq!(rules.len(), 2);
    assert!(rules.iter().any(|r| r.contains("fraud_check")));
    assert!(rules.iter().any(|r| r.contains("kyc_verification")));
}

#[tokio::test]
async fn test_list_rulesets() {
    let (_temp, repo) = create_test_repo().await;

    let rulesets = repo.list_rulesets().await.expect("Failed to list rulesets");

    assert_eq!(rulesets.len(), 1);
    assert!(rulesets.iter().any(|r| r.contains("test_ruleset")));
}

#[tokio::test]
async fn test_list_templates() {
    let (_temp, repo) = create_test_repo().await;

    let templates = repo.list_templates().await.expect("Failed to list templates");

    assert_eq!(templates.len(), 1);
    assert!(templates.iter().any(|t| t.contains("score_template")));
}

#[tokio::test]
async fn test_list_pipelines() {
    let (_temp, repo) = create_test_repo().await;

    let pipelines = repo.list_pipelines().await.expect("Failed to list pipelines");

    assert_eq!(pipelines.len(), 1);
    assert!(pipelines.iter().any(|p| p.contains("test_pipeline")));
}

#[tokio::test]
async fn test_list_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create only the root directory, no subdirectories
    let repo = FileSystemRepository::new(repo_path).unwrap();

    let rules = repo.list_rules().await.expect("Failed to list rules");
    assert_eq!(rules.len(), 0);

    let rulesets = repo.list_rulesets().await.expect("Failed to list rulesets");
    assert_eq!(rulesets.len(), 0);
}

// ==================================================
// Exists Tests
// ==================================================

#[tokio::test]
async fn test_exists_true_for_file() {
    let (_temp, repo) = create_test_repo().await;

    let exists = repo
        .exists("library/rules/fraud/fraud_check.yaml")
        .await
        .expect("Failed to check existence");

    assert!(exists);
}

#[tokio::test]
async fn test_exists_false_for_nonexistent() {
    let (_temp, repo) = create_test_repo().await;

    let exists = repo
        .exists("library/rules/nonexistent.yaml")
        .await
        .expect("Failed to check existence");

    assert!(!exists);
}

// ==================================================
// Concurrent Access Tests
// ==================================================

#[tokio::test]
async fn test_concurrent_loads() {
    let (_temp, repo) = create_test_repo().await;

    let repo = std::sync::Arc::new(repo);

    // Spawn multiple tasks loading the same rule
    let mut handles = vec![];
    for _ in 0..10 {
        let repo_clone = repo.clone();
        let handle = tokio::spawn(async move {
            repo_clone.load_rule("fraud_check").await
        });
        handles.push(handle);
    }

    // Wait for all tasks
    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(_)) = handle.await {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 10, "All concurrent loads should succeed");
}

#[tokio::test]
async fn test_concurrent_different_artifacts() {
    let (_temp, repo) = create_test_repo().await;

    let repo = std::sync::Arc::new(repo);

    // Load different artifacts concurrently
    let repo1 = repo.clone();
    let h1 = tokio::spawn(async move { repo1.load_rule("fraud_check").await });

    let repo2 = repo.clone();
    let h2 = tokio::spawn(async move { repo2.load_ruleset("test_ruleset").await });

    let repo3 = repo.clone();
    let h3 = tokio::spawn(async move { repo3.load_pipeline("test_pipeline").await });

    let r1 = h1.await.unwrap();
    let r2 = h2.await.unwrap();
    let r3 = h3.await.unwrap();

    assert!(r1.is_ok());
    assert!(r2.is_ok());
    assert!(r3.is_ok());
}

// ==================================================
// Edge Cases
// ==================================================

#[tokio::test]
async fn test_empty_repository() {
    let temp_dir = TempDir::new().unwrap();
    let repo = FileSystemRepository::new(temp_dir.path()).unwrap();

    let result = repo.load_rule("any_rule").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_load_with_special_characters_in_id() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    fs::create_dir_all(repo_path.join("library/rules")).await.unwrap();

    let rule = r#"version: "0.1"

rule:
  id: rule_with_underscore_123
  name: Special Rule
  when:
    conditions:
      - "amount > 0"
  score: 10
"#;
    fs::write(
        repo_path.join("library/rules/rule_with_underscore_123.yaml"),
        rule,
    )
    .await
    .unwrap();

    let repo = FileSystemRepository::new(repo_path).unwrap();

    let (loaded_rule, _) = repo
        .load_rule("rule_with_underscore_123")
        .await
        .expect("Failed to load rule with special characters");

    assert_eq!(loaded_rule.id, "rule_with_underscore_123");
}

#[tokio::test]
async fn test_yml_extension_support() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    fs::create_dir_all(repo_path.join("library/rules")).await.unwrap();

    let rule = r#"version: "0.1"

rule:
  id: yml_rule
  name: YML Rule
  when:
    conditions:
      - "amount > 0"
  score: 10
"#;
    // Use .yml extension instead of .yaml
    fs::write(repo_path.join("library/rules/yml_rule.yml"), rule)
        .await
        .unwrap();

    let repo = FileSystemRepository::new(repo_path).unwrap();

    let rules = repo.list_rules().await.expect("Failed to list rules");
    assert_eq!(rules.len(), 1);
    assert!(rules[0].ends_with(".yml"));
}
