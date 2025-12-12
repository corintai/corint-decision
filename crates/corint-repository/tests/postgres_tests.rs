//! Integration tests for PostgresRepository
//!
//! These tests require a PostgreSQL database to be running.
//! Set the DATABASE_URL environment variable to run these tests:
//!
//! ```bash
//! export DATABASE_URL="postgresql://localhost/corint_test"
//! cargo test --package corint-repository --features postgres
//! ```

#[cfg(feature = "postgres")]
mod postgres_tests {
    use corint_core::ast::{Expression, Rule, WhenBlock};
    use corint_repository::{PostgresRepository, Repository, WritableRepository, CacheableRepository};
    use sqlx::postgres::PgPool;

    /// Get database URL from environment or use default test database
    fn get_database_url() -> String {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/corint_test".to_string())
    }

    /// Create test database and run migrations
    async fn setup_test_db() -> PgPool {
        let db_url = get_database_url();

        // Connect to database
        let pool = PgPool::connect(&db_url)
            .await
            .expect("Failed to connect to test database. Make sure PostgreSQL is running and DATABASE_URL is set correctly.");

        // Clean up existing test data
        sqlx::query("TRUNCATE TABLE rules, rulesets, templates, pipelines CASCADE")
            .execute(&pool)
            .await
            .ok(); // Ignore error if tables don't exist yet

        pool
    }

    /// Create a test rule
    fn create_test_rule(id: &str) -> Rule {
        use corint_core::ast::Operator;
        use corint_core::types::Value;

        Rule {
            id: id.to_string(),
            name: format!("Test Rule {}", id),
            description: Some("A test rule".to_string()),
            params: None,
            when: WhenBlock {
                event_type: None,
                conditions: vec![Expression::Binary {
                    op: Operator::Gt,
                    left: Box::new(Expression::FieldAccess(vec!["amount".to_string()])),
                    right: Box::new(Expression::Literal(Value::Number(1000.0))),
                }],
            },
            score: 50,
            metadata: None,
        }
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_save_and_load_rule() {
        let pool = setup_test_db().await;
        let mut repo = PostgresRepository::with_pool(pool);

        let rule = create_test_rule("test_rule_1");

        // Save the rule
        repo.save_rule(&rule)
            .await
            .expect("Failed to save rule");

        // Load the rule back
        let (loaded_rule, _content) = repo
            .load_rule("test_rule_1")
            .await
            .expect("Failed to load rule");

        assert_eq!(loaded_rule.id, rule.id);
        assert_eq!(loaded_rule.name, rule.name);
        assert_eq!(loaded_rule.score, rule.score);
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_rule_versioning() {
        let pool = setup_test_db().await;
        let mut repo = PostgresRepository::with_pool(pool);

        // Create and save initial version
        let mut rule = create_test_rule("test_rule_version");
        repo.save_rule(&rule).await.expect("Failed to save rule");

        // Update the rule
        rule.score = 75;
        repo.save_rule(&rule).await.expect("Failed to update rule");

        // Load the rule - should get the latest version
        let (loaded_rule, _content) = repo
            .load_rule("test_rule_version")
            .await
            .expect("Failed to load rule");

        assert_eq!(loaded_rule.score, 75);
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_rule_not_found() {
        let pool = setup_test_db().await;
        let repo = PostgresRepository::with_pool(pool);

        let result = repo.load_rule("nonexistent_rule").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_exists() {
        let pool = setup_test_db().await;
        let mut repo = PostgresRepository::with_pool(pool);

        let rule = create_test_rule("test_exists_rule");
        repo.save_rule(&rule).await.expect("Failed to save rule");

        let exists = repo
            .exists("test_exists_rule")
            .await
            .expect("Failed to check existence");

        assert!(exists);

        let not_exists = repo
            .exists("nonexistent_rule")
            .await
            .expect("Failed to check existence");

        assert!(!not_exists);
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_delete_rule() {
        let pool = setup_test_db().await;
        let mut repo = PostgresRepository::with_pool(pool);

        let rule = create_test_rule("test_delete_rule");
        repo.save_rule(&rule).await.expect("Failed to save rule");

        // Verify it exists
        let exists = repo.exists("test_delete_rule").await.unwrap();
        assert!(exists);

        // Delete it
        repo.delete_rule("test_delete_rule")
            .await
            .expect("Failed to delete rule");

        // Verify it's gone
        let exists = repo.exists("test_delete_rule").await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_list_rules() {
        let pool = setup_test_db().await;
        let mut repo = PostgresRepository::with_pool(pool);

        // Save multiple rules
        for i in 1..=3 {
            let rule = create_test_rule(&format!("list_test_rule_{}", i));
            repo.save_rule(&rule).await.expect("Failed to save rule");
        }

        let rules = repo.list_rules().await.expect("Failed to list rules");

        assert!(rules.len() >= 3);
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_cache_behavior() {
        let pool = setup_test_db().await;
        let mut repo = PostgresRepository::with_pool(pool);

        let rule = create_test_rule("test_cache_rule");
        repo.save_rule(&rule).await.expect("Failed to save rule");

        // Clear cache to start fresh
        repo.clear_cache();

        // First load - cache miss
        let stats_before = repo.cache_stats();
        let (rule1, _) = repo
            .load_rule("test_cache_rule")
            .await
            .expect("Failed to load rule");

        // Second load - should be from cache
        let (rule2, _) = repo
            .load_rule("test_cache_rule")
            .await
            .expect("Failed to load rule");

        let stats_after = repo.cache_stats();

        assert_eq!(rule1.id, rule2.id);

        // Should have at least one hit (from the second load)
        assert!(stats_after.hits > stats_before.hits);

        // Cache should have entries
        assert!(stats_after.size > 0);
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_cache_clear_entry() {
        let pool = setup_test_db().await;
        let mut repo = PostgresRepository::with_pool(pool);

        let rule = create_test_rule("test_cache_clear_entry");
        repo.save_rule(&rule).await.expect("Failed to save rule");

        // Load to populate cache
        repo.load_rule("test_cache_clear_entry")
            .await
            .expect("Failed to load rule");

        // Clear this specific entry
        repo.clear_cache_entry("test_cache_clear_entry");

        // Give the async task time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Next load should be a cache miss (though we can't directly verify this
        // without more detailed stats tracking)
        let (_rule, _) = repo
            .load_rule("test_cache_clear_entry")
            .await
            .expect("Failed to load rule");
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_cache_enable_disable() {
        let pool = setup_test_db().await;
        let mut repo = PostgresRepository::with_pool(pool);

        assert!(repo.is_cache_enabled());

        repo.set_cache_enabled(false);
        assert!(!repo.is_cache_enabled());

        repo.set_cache_enabled(true);
        assert!(repo.is_cache_enabled());
    }
}
