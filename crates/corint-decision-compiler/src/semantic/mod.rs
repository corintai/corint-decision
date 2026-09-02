//! Semantic analysis module
//!
//! This module provides semantic analysis and type checking for CORINT programs.

pub mod analyzer;
pub mod pipeline_analyzer;
pub mod type_checker;

// Re-export for convenience
pub use analyzer::SemanticAnalyzer;
pub use pipeline_analyzer::analyze_new_pipeline;
pub use type_checker::{TypeChecker, TypeInfo};
