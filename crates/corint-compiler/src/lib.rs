//! CORINT Compiler - AST to IR compiler
//!
//! This crate compiles CORINT AST into executable IR (Intermediate Representation).

pub mod codegen;
pub mod compiler;
pub mod error;
pub mod import_resolver;
pub mod optimizer;
pub mod semantic;

// Re-export main types
pub use compiler::{Compiler, CompilerOptions};
pub use error::{CompileError, Result};

// Re-export import resolver types
pub use import_resolver::{ImportResolver, ResolvedDocument};

// Re-export codegen types for backward compatibility
pub use codegen::ExpressionCompiler;
pub use codegen::PipelineCompiler;
pub use codegen::RuleCompiler;
pub use codegen::RulesetCompiler;

// Re-export semantic types
pub use semantic::{SemanticAnalyzer, TypeChecker, TypeInfo};

// Re-export optimizer types
pub use optimizer::{ConstantFolder, DeadCodeEliminator};
