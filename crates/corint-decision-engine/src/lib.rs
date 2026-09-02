//! CORINT Decision Engine application layer.
//!
//! This crate owns the orchestration needed to turn a repository of CORINT DSL
//! documents into a runnable decision engine. Transport adapters should depend
//! on this crate (directly or through `corint-decision-sdk`) rather than assembling the
//! parser, compiler, runtime, and repository layers themselves.

pub mod builder;
pub mod config;
pub mod decision_engine;
pub mod error;
pub mod score;
pub mod validator;

pub use builder::DecisionEngineBuilder;
pub use config::{
    EngineConfig, LLMConfig, LLMProvider, ServiceConfig, ServiceType, StorageConfig, StorageType,
};
pub use decision_engine::{DecisionEngine, DecisionOptions, DecisionRequest, DecisionResponse};
pub use error::{EngineError, Result};
pub use score::ScoreNormalizer;
pub use validator::{
    validate, validate_pipeline, validate_rule, validate_ruleset, Diagnostic, DiagnosticSeverity,
    DocumentMetadata, DslType, DslValidator, ValidationResult,
};

// Repository configuration is part of the engine's public construction API.
pub use corint_decision_repository::{
    ApiConfig, DataSourceConfig, FeatureDefinition, ListConfig, RepositoryConfig,
    RepositoryContent, RepositoryLoader, RepositorySource,
};

// Re-export the stable domain and runtime types that transport adapters need.
pub use corint_decision_model::{ast::Signal, Value};
pub use corint_decision_runtime::datasource::config::FeatureStoreProvider;
pub use corint_decision_runtime::{
    datasource::{
        DataSourceConfig as RuntimeDataSourceConfig, DataSourceType, FeatureStoreConfig,
        OLAPConfig, OLAPProvider, SQLConfig, SQLProvider,
    },
    DecisionResult, ExecutionTrace, MetricsCollector,
};
