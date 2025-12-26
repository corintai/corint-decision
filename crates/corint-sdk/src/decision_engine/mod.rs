//! DecisionEngine - Main API for executing decisions
//!
//! This module provides the core `DecisionEngine` that executes risk decision
//! pipelines based on event data.
//!
//! # Architecture
//!
//! The module is organized into:
//! - `types`: Request/Response types (DecisionRequest, DecisionResponse, DecisionOptions)
//! - `engine`: Core DecisionEngine implementation
//! - `when_evaluator`: When block and condition evaluation logic
//! - `trace_builder`: Execution trace construction utilities
//! - `compiler_helper`: Rule compilation and loading utilities
//! - `tests`: Unit tests (test-only)

mod types;
mod when_evaluator;
mod trace_builder;
mod compiler_helper;
mod engine;

// Re-export public types
pub use types::{DecisionOptions, DecisionRequest, DecisionResponse};
pub use engine::DecisionEngine;

// Tests module (only compiled in test mode)
#[cfg(test)]
mod tests;
