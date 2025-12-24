//! PostgreSQL database repository implementation

use async_trait::async_trait;
use corint_core::ast::{Pipeline, Rule, Ruleset};
use corint_parser::{PipelineParser, RuleParser, RulesetParser};
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use crate::{error::RepositoryError, models::*, traits::*, CacheStats, RepositoryResult};

/// PostgreSQL database repository
///
/// Stores and loads artifacts from a PostgreSQL database with caching support.
pub struct PostgresRepository {
    /// Database connection pool
    pool: PgPool,
    /// Cache for rules
    rule_cache: Arc<RwLock<HashMap<String, CachedArtifact<Rule>>>>,
    /// Cache for rulesets
    ruleset_cache: Arc<RwLock<HashMap<String, CachedArtifact<Ruleset>>>>,
    /// Cache for pipelines
    pipeline_cache: Arc<RwLock<HashMap<String, CachedArtifact<Pipeline>>>>,
    /// Cache configuration
    cache_config: Arc<Mutex<CacheConfig>>,
    /// Cache statistics
    stats: Arc<Mutex<CacheStats>>,
}

impl PostgresRepository {
    /// Create a new PostgreSQL repository
    ///
    /// # Arguments
    /// * `database_url` - PostgreSQL connection string (e.g., "postgresql://user:pass@localhost/db")
    ///
    /// # Example
    /// ```no_run
    /// use corint_repository::PostgresRepository;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let repo = PostgresRepository::new("postgresql://localhost/corint").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(database_url: &str) -> RepositoryResult<Self> {
        let pool = PgPool::connect(database_url).await?;

        Ok(Self {
            pool,
            rule_cache: Arc::new(RwLock::new(HashMap::new())),
            ruleset_cache: Arc::new(RwLock::new(HashMap::new())),
            pipeline_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_config: Arc::new(Mutex::new(CacheConfig::default())),
            stats: Arc::new(Mutex::new(CacheStats::default())),
        })
    }

    /// Create a new PostgreSQL repository with an existing pool
    pub fn with_pool(pool: PgPool) -> Self {
        Self {
            pool,
            rule_cache: Arc::new(RwLock::new(HashMap::new())),
            ruleset_cache: Arc::new(RwLock::new(HashMap::new())),
            pipeline_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_config: Arc::new(Mutex::new(CacheConfig::default())),
            stats: Arc::new(Mutex::new(CacheStats::default())),
        }
    }

    /// Create a new PostgreSQL repository with custom cache configuration
    pub async fn with_cache_config(
        database_url: &str,
        config: CacheConfig,
    ) -> RepositoryResult<Self> {
        let mut repo = Self::new(database_url).await?;
        repo.cache_config = Arc::new(Mutex::new(config));
        Ok(repo)
    }

    /// Check cache and potentially load from cache
    async fn check_cache<T: Clone>(
        &self,
        cache: &Arc<RwLock<HashMap<String, CachedArtifact<T>>>>,
        identifier: &str,
    ) -> Option<(T, String)> {
        let enabled = self.cache_config.lock().unwrap().enabled;
        if !enabled {
            return None;
        }

        let cache_read = cache.read().await;
        if let Some(cached) = cache_read.get(identifier) {
            if !cached.is_expired() {
                // Cache hit
                self.stats.lock().unwrap().hits += 1;
                return Some((cached.data.clone(), cached.content.clone()));
            }
        }

        // Cache miss
        self.stats.lock().unwrap().misses += 1;
        None
    }

    /// Store in cache
    async fn store_in_cache<T: Clone>(
        &self,
        cache: &Arc<RwLock<HashMap<String, CachedArtifact<T>>>>,
        identifier: &str,
        data: T,
        content: String,
    ) {
        let (enabled, ttl) = {
            let config = self.cache_config.lock().unwrap();
            (config.enabled, config.default_ttl)
        };

        if !enabled {
            return;
        }

        let cached = CachedArtifact::new(data, content, ttl);
        let mut cache_write = cache.write().await;
        cache_write.insert(identifier.to_string(), cached);

        // Update stats
        self.stats.lock().unwrap().size = cache_write.len();
    }
}

#[async_trait]
impl Repository for PostgresRepository {
    async fn load_rule(&self, identifier: &str) -> RepositoryResult<(Rule, String)> {
        // Check cache first
        if let Some(cached) = self.check_cache(&self.rule_cache, identifier).await {
            return Ok(cached);
        }

        // Query database by ID or path
        let row = sqlx::query(
            r#"
            SELECT id, content, version, updated_at
            FROM rules
            WHERE id = $1 OR path = $1
            ORDER BY version DESC
            LIMIT 1
            "#,
        )
        .bind(identifier)
        .fetch_optional(&self.pool)
        .await?;

        let row = row.ok_or_else(|| RepositoryError::NotFound {
            path: identifier.to_string(),
        })?;

        let content: String = row.try_get("content")?;

        // Parse the YAML content
        let doc = RuleParser::parse_with_imports(&content)?;

        // Store in cache
        self.store_in_cache(
            &self.rule_cache,
            identifier,
            doc.definition.clone(),
            content.clone(),
        )
        .await;

        Ok((doc.definition, content))
    }

    async fn load_ruleset(&self, identifier: &str) -> RepositoryResult<(Ruleset, String)> {
        // Check cache first
        if let Some(cached) = self.check_cache(&self.ruleset_cache, identifier).await {
            return Ok(cached);
        }

        // Query database
        let row = sqlx::query(
            r#"
            SELECT id, content, version, updated_at
            FROM rulesets
            WHERE id = $1 OR path = $1
            ORDER BY version DESC
            LIMIT 1
            "#,
        )
        .bind(identifier)
        .fetch_optional(&self.pool)
        .await?;

        let row = row.ok_or_else(|| RepositoryError::NotFound {
            path: identifier.to_string(),
        })?;

        let content: String = row.try_get("content")?;

        // Parse the YAML content
        let doc = RulesetParser::parse_with_imports(&content)?;

        // Store in cache
        self.store_in_cache(
            &self.ruleset_cache,
            identifier,
            doc.definition.clone(),
            content.clone(),
        )
        .await;

        Ok((doc.definition, content))
    }

    async fn load_pipeline(&self, identifier: &str) -> RepositoryResult<(Pipeline, String)> {
        // Check cache first
        if let Some(cached) = self.check_cache(&self.pipeline_cache, identifier).await {
            return Ok(cached);
        }

        // Query database
        let row = sqlx::query(
            r#"
            SELECT id, content, version, updated_at
            FROM pipelines
            WHERE id = $1 OR path = $1
            ORDER BY version DESC
            LIMIT 1
            "#,
        )
        .bind(identifier)
        .fetch_optional(&self.pool)
        .await?;

        let row = row.ok_or_else(|| RepositoryError::NotFound {
            path: identifier.to_string(),
        })?;

        let content: String = row.try_get("content")?;

        // Parse the YAML content
        let doc = PipelineParser::parse_with_imports(&content)?;

        // Store in cache
        self.store_in_cache(
            &self.pipeline_cache,
            identifier,
            doc.definition.clone(),
            content.clone(),
        )
        .await;

        Ok((doc.definition, content))
    }

    async fn exists(&self, identifier: &str) -> RepositoryResult<bool> {
        // Check all tables for the identifier
        let rule_exists: bool = sqlx::query(
            r#"SELECT EXISTS(SELECT 1 FROM rules WHERE id = $1 OR path = $1) as exists"#,
        )
        .bind(identifier)
        .fetch_one(&self.pool)
        .await?
        .try_get("exists")?;

        if rule_exists {
            return Ok(true);
        }

        let ruleset_exists: bool = sqlx::query(
            r#"SELECT EXISTS(SELECT 1 FROM rulesets WHERE id = $1 OR path = $1) as exists"#,
        )
        .bind(identifier)
        .fetch_one(&self.pool)
        .await?
        .try_get("exists")?;

        if ruleset_exists {
            return Ok(true);
        }

        let pipeline_exists: bool = sqlx::query(
            r#"SELECT EXISTS(SELECT 1 FROM pipelines WHERE id = $1 OR path = $1) as exists"#,
        )
        .bind(identifier)
        .fetch_one(&self.pool)
        .await?
        .try_get("exists")?;

        Ok(pipeline_exists)
    }

    async fn list_rules(&self) -> RepositoryResult<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT COALESCE(path, id) as identifier
            FROM rules
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| r.try_get::<Option<String>, _>("identifier").ok().flatten())
            .collect())
    }

    async fn list_rulesets(&self) -> RepositoryResult<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT COALESCE(path, id) as identifier
            FROM rulesets
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| r.try_get::<Option<String>, _>("identifier").ok().flatten())
            .collect())
    }

    async fn list_pipelines(&self) -> RepositoryResult<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT COALESCE(path, id) as identifier
            FROM pipelines
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| r.try_get::<Option<String>, _>("identifier").ok().flatten())
            .collect())
    }

    async fn load_registry(&self) -> RepositoryResult<String> {
        // PostgreSQL repository doesn't support registry yet
        // Registry is typically stored in filesystem
        Err(RepositoryError::Other(
            "Registry is not supported in PostgreSQL repository. Use FileSystem repository for registry configuration.".to_string()
        ))
    }
}

#[async_trait]
impl CacheableRepository for PostgresRepository {
    fn clear_cache(&mut self) {
        let rule_cache = Arc::clone(&self.rule_cache);
        let ruleset_cache = Arc::clone(&self.ruleset_cache);
        let pipeline_cache = Arc::clone(&self.pipeline_cache);

        tokio::spawn(async move {
            rule_cache.write().await.clear();
            ruleset_cache.write().await.clear();
            pipeline_cache.write().await.clear();
        });

        self.stats.lock().unwrap().size = 0;
    }

    fn clear_cache_entry(&mut self, identifier: &str) {
        let rule_cache = Arc::clone(&self.rule_cache);
        let ruleset_cache = Arc::clone(&self.ruleset_cache);
        let pipeline_cache = Arc::clone(&self.pipeline_cache);
        let id = identifier.to_string();

        tokio::spawn(async move {
            rule_cache.write().await.remove(&id);
            ruleset_cache.write().await.remove(&id);
            pipeline_cache.write().await.remove(&id);
        });
    }

    fn cache_stats(&self) -> CacheStats {
        self.stats.lock().unwrap().clone()
    }

    fn set_cache_enabled(&mut self, enabled: bool) {
        self.cache_config.lock().unwrap().enabled = enabled;
    }

    fn is_cache_enabled(&self) -> bool {
        self.cache_config.lock().unwrap().enabled
    }
}

#[async_trait]
impl WritableRepository for PostgresRepository {
    async fn save_rule(&mut self, rule: &Rule) -> RepositoryResult<()> {
        let content = serde_yaml::to_string(rule)
            .map_err(|e| RepositoryError::Other(format!("Failed to serialize rule: {}", e)))?;

        sqlx::query(
            r#"
            INSERT INTO rules (id, content, version, updated_at)
            VALUES ($1, $2, 1, NOW())
            ON CONFLICT (id) DO UPDATE
            SET content = $2, version = rules.version + 1, updated_at = NOW()
            "#,
        )
        .bind(&rule.id)
        .bind(&content)
        .execute(&self.pool)
        .await?;

        // Clear cache for this rule
        self.clear_cache_entry(&rule.id);

        Ok(())
    }

    async fn save_ruleset(&mut self, ruleset: &Ruleset) -> RepositoryResult<()> {
        let content = serde_yaml::to_string(ruleset)
            .map_err(|e| RepositoryError::Other(format!("Failed to serialize ruleset: {}", e)))?;

        sqlx::query(
            r#"
            INSERT INTO rulesets (id, content, version, extends, updated_at)
            VALUES ($1, $2, 1, $3, NOW())
            ON CONFLICT (id) DO UPDATE
            SET content = $2, version = rulesets.version + 1, extends = $3, updated_at = NOW()
            "#,
        )
        .bind(&ruleset.id)
        .bind(&content)
        .bind(&ruleset.extends)
        .execute(&self.pool)
        .await?;

        // Clear cache for this ruleset
        self.clear_cache_entry(&ruleset.id);

        Ok(())
    }

    async fn save_pipeline(&mut self, pipeline: &Pipeline) -> RepositoryResult<()> {
        let content = serde_yaml::to_string(pipeline)
            .map_err(|e| RepositoryError::Other(format!("Failed to serialize pipeline: {}", e)))?;

        let pipeline_id = pipeline
            .id
            .as_ref()
            .ok_or_else(|| RepositoryError::Other("Pipeline must have an id".to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO pipelines (id, content, version, updated_at)
            VALUES ($1, $2, 1, NOW())
            ON CONFLICT (id) DO UPDATE
            SET content = $2, version = pipelines.version + 1, updated_at = NOW()
            "#,
        )
        .bind(pipeline_id)
        .bind(&content)
        .execute(&self.pool)
        .await?;

        // Clear cache for this pipeline
        self.clear_cache_entry(pipeline_id);

        Ok(())
    }

    async fn delete_rule(&self, identifier: &str) -> RepositoryResult<()> {
        sqlx::query(r#"DELETE FROM rules WHERE id = $1 OR path = $1"#)
            .bind(identifier)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_ruleset(&self, identifier: &str) -> RepositoryResult<()> {
        sqlx::query(r#"DELETE FROM rulesets WHERE id = $1 OR path = $1"#)
            .bind(identifier)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_pipeline(&self, identifier: &str) -> RepositoryResult<()> {
        sqlx::query(r#"DELETE FROM pipelines WHERE id = $1 OR path = $1"#)
            .bind(identifier)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
