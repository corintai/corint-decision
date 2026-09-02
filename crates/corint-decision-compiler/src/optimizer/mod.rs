//! Optimization module
//!
//! This module provides optimizations for CORINT programs.

pub mod constant_folding;
pub mod dead_code_elimination;

// Re-export for convenience
pub use constant_folding::ConstantFolder;
pub use dead_code_elimination::DeadCodeEliminator;
