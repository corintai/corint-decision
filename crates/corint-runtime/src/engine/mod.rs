//! Execution engine module
//!
//! Provides executors for running IR programs.

pub mod pipeline_executor;

// Re-export for convenience
pub use pipeline_executor::PipelineExecutor;
