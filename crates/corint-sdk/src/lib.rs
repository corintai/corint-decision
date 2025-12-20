//! CORINT Decision Engine SDK
//!
//! High-level API for building and executing decision engines.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use corint_sdk::{DecisionEngineBuilder, RepositoryConfig, DecisionRequest};
//!
//! // Create engine from file system repository
//! let engine = DecisionEngineBuilder::new()
//!     .with_repository(RepositoryConfig::file_system("repository"))
//!     .build()
//!     .await?;
//!
//! // Execute a decision
//! let request = DecisionRequest::new(event_data);
//! let response = engine.decide(request).await?;
//! ```

pub mod builder;
pub mod config;
pub mod decision_engine;
pub mod error;
pub mod validator;

// Re-export main types
pub use builder::DecisionEngineBuilder;
pub use config::{
    EngineConfig, LLMConfig, LLMProvider, ServiceConfig, ServiceType, StorageConfig, StorageType,
};
pub use decision_engine::{DecisionEngine, DecisionOptions, DecisionRequest, DecisionResponse};
pub use error::{Result, SdkError};

// Re-export validator types
pub use validator::{
    validate, validate_pipeline, validate_rule, validate_ruleset, Diagnostic, DiagnosticSeverity,
    DocumentMetadata, DslType, DslValidator, ValidationResult,
};

// Re-export repository types for unified configuration
pub use corint_repository::{
    ApiConfig, DataSourceConfig, FeatureDefinition, ListConfig, RepositoryConfig,
    RepositoryContent, RepositoryLoader, RepositorySource,
};

// Re-export commonly used types from dependencies
pub use corint_core::{ast::Action, Value};
pub use corint_runtime::{DecisionResult, MetricsCollector};
