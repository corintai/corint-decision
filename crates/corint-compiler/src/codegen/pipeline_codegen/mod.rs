//! Pipeline code generation module
//!
//! Compiles Pipeline AST with DAG structure into IR programs.

pub(super) mod compiler;
mod condition_compiler;
mod instruction_gen;
mod metadata_builder;
mod validator;

// Re-export public types
pub use compiler::PipelineCompiler;
