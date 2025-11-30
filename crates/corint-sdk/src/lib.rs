//! CORINT Decision Engine SDK
//!
//! High-level API for building and executing decision engines.

pub mod config;
pub mod decision_engine;
pub mod error;
pub mod builder;

// Re-export main types
pub use config::{EngineConfig, StorageConfig, LLMConfig, ServiceConfig};
pub use decision_engine::{DecisionEngine, DecisionRequest, DecisionResponse};
pub use error::{SdkError, Result};
pub use builder::DecisionEngineBuilder;

// Re-export commonly used types from dependencies
pub use corint_core::{Value, ast::Action};
pub use corint_runtime::{DecisionResult, MetricsCollector};
