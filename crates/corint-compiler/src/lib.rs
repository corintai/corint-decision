//! CORINT Compiler - AST to IR compiler
//!
//! This crate compiles CORINT AST into executable IR (Intermediate Representation).

pub mod error;
pub mod compiler;
pub mod codegen;
pub mod semantic;
pub mod optimizer;
pub mod import_resolver;

// Re-export main types
pub use error::{CompileError, Result};
pub use compiler::{Compiler, CompilerOptions};

// Re-export import resolver types
pub use import_resolver::{ImportResolver, ResolvedDocument};

// Re-export codegen types for backward compatibility
pub use codegen::ExpressionCompiler;
pub use codegen::RuleCompiler;
pub use codegen::RulesetCompiler;
pub use codegen::PipelineCompiler;

// Re-export semantic types
pub use semantic::{SemanticAnalyzer, TypeChecker, TypeInfo};

// Re-export optimizer types
pub use optimizer::{ConstantFolder, DeadCodeEliminator};
