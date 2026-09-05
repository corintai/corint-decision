//! Execution context module
//!
//! Manages the state during program execution with a flattened namespace architecture.

mod env_vars;
#[path = "context.rs"]
mod execution;
mod field_lookup;
mod system_vars;

// Re-export public types
pub use execution::{ContextInput, ExecutionContext};
