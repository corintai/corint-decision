//! Integration tests for FileSystemRepository

use corint_repository::{FileSystemRepository, Repository};
use tempfile::TempDir;
use tokio::fs;

async fn create_test_repo() -> (TempDir, FileSystemRepository) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create directory structure
    fs::create_dir_all(repo_path.join("library/rules/fraud"))
        .await
        .unwrap();
    fs::create_dir_all(repo_path.join("library/rulesets"))
        .await
        .unwrap();
    fs::create_dir_all(repo_path.join("pipelines"))
        .await
        .unwrap();

    // Create a test rule
    let rule_content = r#"version: "0.1"

rule:
  id: test_fraud_rule
  name: Test Fraud Rule
  description: A simple test rule

  when:
    conditions:
      - amount > 1000

  score: 50
"#;
    fs::write(
        repo_path.join("library/rules/fraud/test_fraud_rule.yaml"),
        rule_content,
    )
    .await
    .unwrap();

    // Create a test ruleset
    let ruleset_content = r#"version: "0.1"

ruleset:
  id: test_ruleset
  name: Test Ruleset

  rules:
    - test_fraud_rule

  decision_logic:
    - condition: total_score >= 100
      action: deny
    - default: true
      action: approve
"#;
    fs::write(
        repo_path.join("library/rulesets/test_ruleset.yaml"),
        ruleset_content,
    )
    .await
    .unwrap();

    let repo = FileSystemRepository::new(repo_path).unwrap();
    (temp_dir, repo)
}

#[tokio::test]
async fn test_load_rule_by_path() {
    let (_temp, repo) = create_test_repo().await;

    let (rule, content) = repo
        .load_rule("library/rules/fraud/test_fraud_rule.yaml")
        .await
        .expect("Failed to load rule");

    assert_eq!(rule.id, "test_fraud_rule");
    assert_eq!(rule.name, "Test Fraud Rule");
    assert_eq!(rule.score, 50);
    assert!(content.contains("test_fraud_rule"));
}

#[tokio::test]
async fn test_load_rule_by_id() {
    let (_temp, repo) = create_test_repo().await;

    let (rule, _) = repo
        .load_rule("test_fraud_rule")
        .await
        .expect("Failed to load rule by ID");

    assert_eq!(rule.id, "test_fraud_rule");
}

#[tokio::test]
async fn test_load_ruleset_by_path() {
    let (_temp, repo) = create_test_repo().await;

    let (ruleset, content) = repo
        .load_ruleset("library/rulesets/test_ruleset.yaml")
        .await
        .expect("Failed to load ruleset");

    assert_eq!(ruleset.id, "test_ruleset");
    assert_eq!(ruleset.rules.len(), 1);
    assert_eq!(ruleset.rules[0], "test_fraud_rule");
    assert!(content.contains("test_ruleset"));
}

#[tokio::test]
async fn test_cache_behavior() {
    let (_temp, repo) = create_test_repo().await;

    // First load - cache miss
    let start1 = std::time::Instant::now();
    let (rule1, _) = repo
        .load_rule("library/rules/fraud/test_fraud_rule.yaml")
        .await
        .expect("Failed to load rule");
    let duration1 = start1.elapsed();

    // Second load - should be from cache
    let start2 = std::time::Instant::now();
    let (rule2, _) = repo
        .load_rule("library/rules/fraud/test_fraud_rule.yaml")
        .await
        .expect("Failed to load rule");
    let duration2 = start2.elapsed();

    assert_eq!(rule1.id, rule2.id);

    // Cache hit should be faster (though not guaranteed in all environments)
    println!("First load: {:?}, Second load: {:?}", duration1, duration2);

    // At minimum, verify they both succeeded
    assert_eq!(rule1.id, "test_fraud_rule");
    assert_eq!(rule2.id, "test_fraud_rule");
}

#[tokio::test]
async fn test_list_rules() {
    let (_temp, repo) = create_test_repo().await;

    let rules = repo.list_rules().await.expect("Failed to list rules");

    assert!(!rules.is_empty());
    assert!(rules.iter().any(|r| r.contains("test_fraud_rule.yaml")));
}

#[tokio::test]
async fn test_rule_not_found() {
    let (_temp, repo) = create_test_repo().await;

    let result = repo.load_rule("nonexistent_rule.yaml").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_exists() {
    let (_temp, repo) = create_test_repo().await;

    let exists = repo
        .exists("library/rules/fraud/test_fraud_rule.yaml")
        .await
        .expect("Failed to check existence");

    assert!(exists);

    let not_exists = repo
        .exists("library/rules/fraud/nonexistent.yaml")
        .await
        .expect("Failed to check existence");

    assert!(!not_exists);
}
