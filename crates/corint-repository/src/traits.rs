//! Core trait definitions for the repository pattern
//!
//! This module defines three key traits:
//!
//! - [`Repository`]: Core read-only interface for loading artifacts
//! - [`CacheableRepository`]: Extension for cache management
//! - [`WritableRepository`]: Extension for write operations (CRUD)
//!
//! # Examples
//!
//! ## Basic Usage with File System
//!
//! ```no_run
//! use corint_repository::{Repository, FileSystemRepository};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let repo = FileSystemRepository::new("repository")?;
//!
//! // Load by file path
//! let (rule, _) = repo.load_rule("library/rules/velocity_check.yaml").await?;
//!
//! // Load by ID (searches recursively)
//! let (rule, _) = repo.load_rule("velocity_check").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Phase 3: Loading Templates and Inherited Rulesets
//!
//! ```no_run
//! use corint_repository::{Repository, FileSystemRepository};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let repo = FileSystemRepository::new("repository")?;
//!
//! // Load a decision template (Phase 3)
//! let (template, _) = repo.load_template("score_based_decision").await?;
//!
//! // Load a ruleset that uses inheritance (Phase 3)
//! let (ruleset, _) = repo.load_ruleset("payment_fraud_detection").await?;
//! if let Some(parent_id) = &ruleset.extends {
//!     // Compiler will automatically resolve inheritance
//!     let (parent, _) = repo.load_ruleset(parent_id).await?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Phase 4: Using PostgreSQL with Versioning
//!
//! ```no_run
//! # #[cfg(feature = "postgres")]
//! use corint_repository::{Repository, WritableRepository, PostgresRepository};
//!
//! # #[cfg(feature = "postgres")]
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut repo = PostgresRepository::new("postgresql://localhost/corint").await?;
//!
//! // Load rule (cached automatically)
//! let (rule, _) = repo.load_rule("velocity_check").await?;
//!
//! // Update rule (version automatically incremented)
//! let mut updated = rule.clone();
//! updated.score = 100;
//! repo.save_rule(&updated).await?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "postgres"))]
//! # fn main() {}
//! ```

use async_trait::async_trait;
use corint_core::ast::{DecisionTemplate, Pipeline, Rule, Ruleset};

use crate::{CacheStats, RepositoryResult};

/// Core repository trait for loading decision artifacts
///
/// This trait defines the interface for loading rules, rulesets, templates,
/// and pipelines from any storage backend.
///
/// # Implementation Notes
///
/// - All operations are async for non-blocking I/O
/// - Identifiers can be file paths or artifact IDs (implementation-specific)
/// - Implementations should return both the parsed artifact and raw content
/// - File system implementation searches recursively by ID
/// - Database implementation uses primary key lookup
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` for use across async tasks.
#[async_trait]
pub trait Repository: Send + Sync {
    /// Load a rule by path or ID
    ///
    /// # Arguments
    /// * `identifier` - Either a file path (e.g., "library/rules/fraud/fraud_farm.yaml")
    ///                  or a rule ID (e.g., "fraud_farm_pattern")
    ///
    /// # Returns
    /// A tuple of (Rule, raw_content_string)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use corint_repository::{Repository, FileSystemRepository};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let repo = FileSystemRepository::new("repository")?;
    ///
    /// // Load by path
    /// let (rule, content) = repo.load_rule("library/rules/velocity_check.yaml").await?;
    ///
    /// // Load by ID (searches directories)
    /// let (rule, content) = repo.load_rule("velocity_check").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Phase 3: Parameterized Rules
    ///
    /// Rules loaded from the repository may contain parameters:
    ///
    /// ```yaml
    /// rule:
    ///   id: velocity_check
    ///   params:
    ///     time_window_seconds: 3600
    ///     max_transactions: 10
    ///   when:
    ///     conditions:
    ///       - transaction_count(last_n_seconds: params.time_window_seconds) > params.max_transactions
    /// ```
    ///
    /// The compiler will inline these parameters at compile time.
    async fn load_rule(&self, identifier: &str) -> RepositoryResult<(Rule, String)>;

    /// Load a ruleset by path or ID
    ///
    /// # Arguments
    /// * `identifier` - Either a file path or ruleset ID
    ///
    /// # Returns
    /// A tuple of (Ruleset, raw_content_string)
    ///
    /// # Phase 3: Inheritance and Templates
    ///
    /// Rulesets may use inheritance:
    ///
    /// ```yaml
    /// ruleset:
    ///   id: payment_fraud_detection
    ///   extends: fraud_detection_base  # Inherits rules from parent
    ///   rules:
    ///     - high_amount_check  # Additional rules
    /// ```
    ///
    /// Or reference decision templates:
    ///
    /// ```yaml
    /// ruleset:
    ///   id: login_fraud_detection
    ///   rules:
    ///     - brute_force_check
    ///   decision_template: score_based_decision  # References template
    ///   template_params:
    ///     deny_threshold: 200
    ///     review_threshold: 100
    /// ```
    ///
    /// The compiler will resolve inheritance chains and instantiate templates.
    async fn load_ruleset(&self, identifier: &str) -> RepositoryResult<(Ruleset, String)>;

    /// Load a decision logic template by path or ID
    ///
    /// # Arguments
    /// * `identifier` - Either a file path or template ID
    ///
    /// # Returns
    /// A tuple of (DecisionTemplate, raw_content_string)
    ///
    /// # Phase 3: Decision Templates
    ///
    /// Templates provide reusable decision logic patterns:
    ///
    /// ```yaml
    /// decision_template:
    ///   id: score_based_decision
    ///   name: Score-Based Three-Tier Decision
    ///   params:
    ///     deny_threshold: integer
    ///     review_threshold: integer
    ///   logic:
    ///     - condition: total_score >= params.deny_threshold
    ///       action: deny
    ///     - condition: total_score >= params.review_threshold
    ///       action: review
    ///     - default: true
    ///       action: approve
    /// ```
    ///
    /// Templates are loaded by the compiler and instantiated with specific parameters.
    async fn load_template(&self, identifier: &str)
        -> RepositoryResult<(DecisionTemplate, String)>;

    /// Load a pipeline by path or ID
    ///
    /// # Arguments
    /// * `identifier` - Either a file path or pipeline ID
    ///
    /// # Returns
    /// A tuple of (Pipeline, raw_content_string)
    async fn load_pipeline(&self, identifier: &str) -> RepositoryResult<(Pipeline, String)>;

    /// Check if an artifact exists
    ///
    /// # Arguments
    /// * `identifier` - Either a file path or artifact ID
    async fn exists(&self, identifier: &str) -> RepositoryResult<bool>;

    /// List all available rules
    ///
    /// Returns a list of rule identifiers (paths or IDs)
    async fn list_rules(&self) -> RepositoryResult<Vec<String>>;

    /// List all available rulesets
    ///
    /// Returns a list of ruleset identifiers (paths or IDs)
    async fn list_rulesets(&self) -> RepositoryResult<Vec<String>>;

    /// List all available templates
    ///
    /// Returns a list of template identifiers (paths or IDs)
    async fn list_templates(&self) -> RepositoryResult<Vec<String>>;

    /// List all available pipelines
    ///
    /// Returns a list of pipeline identifiers (paths or IDs)
    async fn list_pipelines(&self) -> RepositoryResult<Vec<String>>;

    /// Load the registry configuration
    ///
    /// # Returns
    /// The raw registry YAML content as a string
    ///
    /// # Errors
    /// Returns an error if registry.yaml doesn't exist or can't be read
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use corint_repository::{Repository, FileSystemRepository};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let repo = FileSystemRepository::new("repository")?;
    ///
    /// // Load registry configuration
    /// let registry_content = repo.load_registry().await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn load_registry(&self) -> RepositoryResult<String>;
}

/// Extension trait for repositories that support caching
///
/// # Phase 4: Built-in Caching
///
/// Both [`FileSystemRepository`](crate::FileSystemRepository) and
/// [`PostgresRepository`](crate::PostgresRepository) implement this trait
/// with TTL-based caching.
///
/// # Examples
///
/// ```no_run
/// # use corint_repository::{Repository, CacheableRepository, FileSystemRepository};
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let mut repo = FileSystemRepository::new("repository")?;
///
/// // First load - cache miss
/// let (rule, _) = repo.load_rule("velocity_check").await?;
///
/// // Second load - cache hit
/// let (rule, _) = repo.load_rule("velocity_check").await?;
///
/// // Check cache statistics
/// let stats = repo.cache_stats();
/// println!("Cache hits: {}, misses: {}", stats.hits, stats.misses);
/// println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
///
/// // Clear cache for specific entry
/// repo.clear_cache_entry("velocity_check");
///
/// // Or clear all caches
/// repo.clear_cache();
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait CacheableRepository: Repository {
    /// Clear all caches
    fn clear_cache(&mut self);

    /// Clear cache entry for a specific identifier
    ///
    /// # Arguments
    /// * `identifier` - The identifier to clear from cache
    fn clear_cache_entry(&mut self, identifier: &str);

    /// Get cache statistics
    ///
    /// Returns information about cache hits, misses, and size
    fn cache_stats(&self) -> CacheStats;

    /// Enable or disable caching
    ///
    /// # Arguments
    /// * `enabled` - Whether caching should be enabled
    fn set_cache_enabled(&mut self, enabled: bool);

    /// Check if caching is enabled
    fn is_cache_enabled(&self) -> bool;
}

/// Extension trait for repositories that support write operations
///
/// Not all repositories need to support writes (e.g., read-only file system).
///
/// # Phase 4: PostgreSQL Write Support
///
/// Only [`PostgresRepository`](crate::PostgresRepository) currently implements
/// this trait. It provides full CRUD operations with automatic versioning.
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "postgres")]
/// # use corint_repository::{Repository, WritableRepository, PostgresRepository};
/// # #[cfg(feature = "postgres")]
/// # use corint_core::ast::{Rule, WhenBlock};
/// # #[cfg(feature = "postgres")]
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let mut repo = PostgresRepository::new("postgresql://localhost/corint").await?;
///
/// // Create new rule
/// let rule = Rule {
///     id: "new_rule".to_string(),
///     name: "New Rule".to_string(),
///     description: None,
///     params: None,
///     when: WhenBlock { event_type: None, conditions: vec![] },
///     score: 50,
///     metadata: None,
/// };
///
/// // Save (version 1)
/// repo.save_rule(&rule).await?;
///
/// // Update (version 2 - automatic)
/// let mut updated = rule.clone();
/// updated.score = 75;
/// repo.save_rule(&updated).await?;
///
/// // Delete
/// repo.delete_rule("new_rule").await?;
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "postgres"))]
/// # fn main() {}
/// ```
///
/// # Automatic Versioning
///
/// When using PostgreSQL, each save operation automatically increments the version:
///
/// - First `save_rule()` → version 1
/// - Second `save_rule()` → version 2
/// - And so on...
///
/// This enables audit trails and rollback capabilities.
#[async_trait]
pub trait WritableRepository: Repository {
    /// Save or update a rule
    ///
    /// # Arguments
    /// * `rule` - The rule to save
    async fn save_rule(&mut self, rule: &Rule) -> RepositoryResult<()>;

    /// Save or update a ruleset
    ///
    /// # Arguments
    /// * `ruleset` - The ruleset to save
    async fn save_ruleset(&mut self, ruleset: &Ruleset) -> RepositoryResult<()>;

    /// Save or update a template
    ///
    /// # Arguments
    /// * `template` - The template to save
    async fn save_template(&mut self, template: &DecisionTemplate) -> RepositoryResult<()>;

    /// Save or update a pipeline
    ///
    /// # Arguments
    /// * `pipeline` - The pipeline to save
    async fn save_pipeline(&mut self, pipeline: &Pipeline) -> RepositoryResult<()>;

    /// Delete a rule
    ///
    /// # Arguments
    /// * `identifier` - The rule identifier (path or ID)
    async fn delete_rule(&self, identifier: &str) -> RepositoryResult<()>;

    /// Delete a ruleset
    ///
    /// # Arguments
    /// * `identifier` - The ruleset identifier (path or ID)
    async fn delete_ruleset(&self, identifier: &str) -> RepositoryResult<()>;

    /// Delete a template
    ///
    /// # Arguments
    /// * `identifier` - The template identifier (path or ID)
    async fn delete_template(&self, identifier: &str) -> RepositoryResult<()>;

    /// Delete a pipeline
    ///
    /// # Arguments
    /// * `identifier` - The pipeline identifier (path or ID)
    async fn delete_pipeline(&self, identifier: &str) -> RepositoryResult<()>;
}
