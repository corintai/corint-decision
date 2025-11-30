//! Main compiler
//!
//! Provides a unified interface for compiling different AST node types.

use corint_core::ast::Rule;
use corint_core::ir::Program;
use crate::error::Result;
use crate::rule::RuleCompiler;

/// The main CORINT compiler
pub struct Compiler {
    // Future: Add symbol table, optimization flags, etc.
}

impl Compiler {
    /// Create a new compiler instance
    pub fn new() -> Self {
        Self {}
    }

    /// Compile a rule into an IR program
    pub fn compile_rule(&self, rule: &Rule) -> Result<Program> {
        RuleCompiler::compile(rule)
    }

    // Future methods:
    // pub fn compile_ruleset(&self, ruleset: &Ruleset) -> Result<Program>
    // pub fn compile_pipeline(&self, pipeline: &Pipeline) -> Result<Program>
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::{Expression, Operator, WhenBlock};
    use corint_core::Value;

    #[test]
    fn test_compiler_compile_rule() {
        let compiler = Compiler::new();

        let when = WhenBlock::new().add_condition(Expression::binary(
            Expression::field_access(vec!["user".to_string(), "age".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(18.0)),
        ));

        let rule = Rule {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            when,
            score: 50,
        };

        let program = compiler.compile_rule(&rule).unwrap();

        assert!(!program.instructions.is_empty());
        assert_eq!(program.metadata.source_type, "rule");
    }

    #[test]
    fn test_compiler_default() {
        let compiler = Compiler::default();
        assert!(std::ptr::eq(&compiler as *const _, &compiler as *const _));
    }
}
