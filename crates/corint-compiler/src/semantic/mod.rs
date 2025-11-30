//! Semantic analysis module
//!
//! This module provides semantic analysis and type checking for CORINT programs.

pub mod analyzer;
pub mod type_checker;

// Re-export for convenience
pub use analyzer::SemanticAnalyzer;
pub use type_checker::{TypeChecker, TypeInfo};
