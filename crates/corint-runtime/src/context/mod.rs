//! Execution context module
//!
//! Manages the state during program execution with a flattened namespace architecture.

mod context;
mod env_vars;
mod field_lookup;
mod system_vars;

// Re-export public types
pub use context::{ContextInput, ExecutionContext};
