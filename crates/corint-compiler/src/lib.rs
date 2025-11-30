//! CORINT Compiler - AST to IR compiler
//!
//! This crate compiles CORINT AST into executable IR (Intermediate Representation).

pub mod error;
pub mod expression;
pub mod rule;
pub mod compiler;

// Re-export main types
pub use error::{CompileError, Result};
pub use compiler::Compiler;
