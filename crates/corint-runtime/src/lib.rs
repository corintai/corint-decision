//! CORINT Runtime - Execution engine for CORINT IR programs
//!
//! This crate provides the runtime execution engine that executes
//! compiled IR programs.

pub mod context;
pub mod engine;
pub mod error;
pub mod executor;
pub mod feature;
pub mod llm;
pub mod observability;
pub mod result;
pub mod service;
pub mod storage;

// Re-export main types
pub use context::ExecutionContext;
pub use engine::PipelineExecutor;
pub use error::{RuntimeError, Result};
pub use executor::Executor;
pub use feature::FeatureExtractor;
pub use llm::{LLMClient, LLMRequest, LLMResponse};
pub use observability::{Metrics, MetricsCollector};
pub use result::{DecisionResult, ExecutionResult};
pub use service::{ServiceClient, ServiceRequest, ServiceResponse};
pub use storage::{Event, EventFilter, InMemoryStorage, Storage, TimeRange};
