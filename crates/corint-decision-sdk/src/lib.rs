//! CORINT Decision Engine SDK
//!
//! High-level developer API for building and executing decision engines.
//!
//! The implementation lives in `corint-decision-engine`. This crate exposes only the
//! application-facing types intended for SDK consumers; engine internals remain
//! available from `corint-decision-engine` for server and infrastructure adapters.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use corint_decision_sdk::{DecisionEngineBuilder, RepositoryConfig, DecisionRequest};
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

pub use corint_decision_engine::{
    config::CompilerOptions,
    validate, validate_pipeline, validate_rule, validate_ruleset, DecisionEngine,
    DecisionEngineBuilder, DecisionOptions, DecisionRequest, DecisionResponse, DecisionResult,
    Diagnostic, DiagnosticSeverity, DocumentMetadata, DslType, DslValidator, EngineConfig,
    ExecutionTrace, LLMConfig, LLMProvider, MetricsCollector, RepositoryConfig, Result,
    EngineError, ScoreNormalizer, ServiceConfig, ServiceType, Signal, StorageConfig, StorageType,
    ValidationResult, Value,
};
