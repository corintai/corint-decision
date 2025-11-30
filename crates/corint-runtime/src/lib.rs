//! CORINT Runtime - Execution engine for CORINT IR programs
//!
//! This crate provides the runtime execution engine that executes
//! compiled IR programs.

pub mod context;
pub mod engine;
pub mod error;
pub mod executor;
pub mod feature;
pub mod result;
pub mod storage;

// Re-export main types
pub use context::ExecutionContext;
pub use engine::PipelineExecutor;
pub use error::{RuntimeError, Result};
pub use executor::Executor;
pub use feature::FeatureExtractor;
pub use result::{DecisionResult, ExecutionResult};
pub use storage::{Event, EventFilter, InMemoryStorage, Storage, TimeRange};
