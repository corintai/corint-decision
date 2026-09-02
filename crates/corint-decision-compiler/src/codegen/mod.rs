//! Code generation module
//!
//! This module contains code generators that transform AST into IR.

pub mod expression_codegen;
pub mod pipeline_codegen;
pub mod rule_codegen;
pub mod ruleset_codegen;

// Re-export for convenience and backward compatibility
pub use expression_codegen::ExpressionCompiler;
pub use pipeline_codegen::PipelineCompiler;
pub use rule_codegen::RuleCompiler;
pub use ruleset_codegen::RulesetCompiler;
