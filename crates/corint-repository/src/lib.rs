//! Repository abstraction layer for CORINT Decision Engine
//!
//! This crate provides a unified interface for loading rules, rulesets,
//! and pipelines from different storage backends (file system, database, etc.).
//!
//! # Features
//!
//! - **File System Repository**: Load from YAML files on disk
//! - **PostgreSQL Repository**: Database-backed storage with versioning (Phase 4)
//! - **Caching**: Built-in TTL-based caching for performance
//! - **Async API**: Non-blocking I/O operations with Tokio
//! - **Inheritance**: Support for ruleset inheritance chains
//!
//! # Quick Start
//!
//! ## File System (Development)
//!
//! ```no_run
//! use corint_repository::{Repository, FileSystemRepository};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create file system repository
//!     let repo = FileSystemRepository::new("repository")?;
//!
//!     // Load a rule by path
//!     let (rule, content) = repo.load_rule("library/rules/fraud/fraud_farm.yaml").await?;
//!     println!("Loaded rule: {}", rule.id);
//!
//!     // Or load by ID (searches recursively)
//!     let (rule, content) = repo.load_rule("fraud_farm").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## PostgreSQL (Production)
//!
//! ```no_run
//! # #[cfg(feature = "postgres")]
//! use corint_repository::{Repository, WritableRepository, PostgresRepository};
//!
//! # #[cfg(feature = "postgres")]
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Connect to PostgreSQL
//!     let mut repo = PostgresRepository::new("postgresql://localhost/corint").await?;
//!
//!     // Load a rule (cached automatically)
//!     let (rule, _) = repo.load_rule("fraud_farm").await?;
//!
//!     // Update rule (version increments automatically)
//!     let mut updated = rule.clone();
//!     updated.score = 100;
//!     repo.save_rule(&updated).await?;
//!
//!     Ok(())
//! }
//! # #[cfg(not(feature = "postgres"))]
//! # fn main() {}
//! ```
//!
//! # Resolving Inheritance Chains
//!
//! ```no_run
//! use corint_repository::{Repository, FileSystemRepository};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let repo = FileSystemRepository::new("repository")?;
//!
//!     // Load ruleset that extends another
//!     let (ruleset, _) = repo.load_ruleset("payment_fraud_detection").await?;
//!
//!     // Check if it has a parent
//!     if let Some(parent_id) = &ruleset.extends {
//!         // Compiler will walk inheritance chain automatically
//!         let (parent, _) = repo.load_ruleset(parent_id).await?;
//!         println!("Extends: {}", parent.id);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Phase 4: Caching and Versioning
//!
//! ## Cache Statistics
//!
//! ```no_run
//! use corint_repository::{Repository, CacheableRepository, FileSystemRepository};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut repo = FileSystemRepository::new("repository")?;
//!
//!     // Load twice (first is cache miss, second is hit)
//!     let _ = repo.load_rule("velocity_check").await?;
//!     let _ = repo.load_rule("velocity_check").await?;
//!
//!     // Check cache performance
//!     let stats = repo.cache_stats();
//!     println!("Cache hit rate: {:.2}%", stats.hit_rate() * 100.0);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Automatic Versioning
//!
//! ```no_run
//! # #[cfg(feature = "postgres")]
//! use corint_repository::{WritableRepository, PostgresRepository};
//!
//! # #[cfg(feature = "postgres")]
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut repo = PostgresRepository::new("postgresql://localhost/corint").await?;
//!
//!     // First save: version 1
//!     // Second save: version 2 (automatic increment)
//!     // Each save creates a new version for audit trail
//!
//!     Ok(())
//! }
//! # #[cfg(not(feature = "postgres"))]
//! # fn main() {}
//! ```
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────┐
//! │        Application Layer               │
//! │  (Compiler, Runtime, Server)           │
//! └──────────────┬─────────────────────────┘
//!                │ Repository Trait
//!                ↓
//! ┌────────────────────────────────────────┐
//! │    Repository Abstraction Layer        │
//! │  - Repository (read operations)        │
//! │  - CacheableRepository (cache mgmt)    │
//! │  - WritableRepository (CRUD)           │
//! └──────────────┬─────────────────────────┘
//!                │
//!       ┌────────┴────────┐
//!       ↓                 ↓
//! ┌──────────────┐  ┌──────────────────┐
//! │ FileSystem   │  │  PostgreSQL      │
//! │ Repository   │  │  Repository      │
//! │ - YAML files │  │  - Versioning    │
//! │ - Caching    │  │  - Audit log     │
//! └──────────────┘  └──────────────────┘
//! ```
//!
//! For more information, see:
//! - [crates/corint-repository/README.md](https://github.com/corint/corint-decision/blob/main/crates/corint-repository/README.md) - Complete repository API documentation
//! - [examples/database_repository.rs](https://github.com/corint/corint-decision/blob/main/examples/database_repository.rs) - PostgreSQL example
//! - [QUICK_START_PHASE3.md](https://github.com/corint/corint-decision/blob/main/QUICK_START_PHASE3.md) - Phase 3 features guide

pub mod config;
pub mod content;
pub mod error;
pub mod file_system;
pub mod loader;
pub mod models;
pub mod traits;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "api")]
pub mod api;

// Re-exports - Configuration
pub use config::{ConfigError, RepositoryConfig, RepositorySource};

// Re-exports - Content
pub use content::{
    ApiConfig, ApiEndpoint, DataSourceConfig, FeatureCache, FeatureDefinition, FeatureFilter,
    ListConfig, PoolConfig, RepositoryContent, TimeWindow,
};

// Re-exports - Loader
pub use loader::RepositoryLoader;

// Re-exports - Error
pub use error::{RepositoryError, RepositoryResult};

// Re-exports - Repositories
pub use file_system::FileSystemRepository;
pub use models::*;
pub use traits::*;

#[cfg(feature = "postgres")]
pub use postgres::PostgresRepository;

#[cfg(feature = "api")]
pub use api::ApiRepository;
