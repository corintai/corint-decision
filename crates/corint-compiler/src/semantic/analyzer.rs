//! Semantic analyzer
//!
//! Performs semantic analysis on AST nodes to detect errors before compilation.

use corint_core::ast::{Expression, Rule, Ruleset, Pipeline};
use crate::error::{CompileError, Result};
use std::collections::HashSet;

/// Semantic analyzer
pub struct SemanticAnalyzer {
    /// Variables that are defined in the current scope
    defined_variables: HashSet<String>,
}

impl SemanticAnalyzer {
    /// Create a new semantic analyzer
    pub fn new() -> Self {
        Self {
            defined_variables: HashSet::new(),
        }
    }

    /// Analyze a rule
    pub fn analyze_rule(&mut self, rule: &Rule) -> Result<()> {
        // Check for empty rule ID
        if rule.id.is_empty() {
            return Err(CompileError::InvalidExpression(
                "Rule ID cannot be empty".to_string(),
            ));
        }

        // Check for empty name
        if rule.name.is_empty() {
            return Err(CompileError::InvalidExpression(
                "Rule name cannot be empty".to_string(),
            ));
        }

        // Analyze all conditions
        for condition in &rule.when.conditions {
            self.analyze_expression(condition)?;
        }

        Ok(())
    }

    /// Analyze a ruleset
    pub fn analyze_ruleset(&mut self, ruleset: &Ruleset) -> Result<()> {
        // Check for empty ruleset ID
        if ruleset.id.is_empty() {
            return Err(CompileError::InvalidExpression(
                "Ruleset ID cannot be empty".to_string(),
            ));
        }

        // Check for duplicate rule references
        let mut seen_rules = HashSet::new();
        for rule_id in &ruleset.rules {
            if !seen_rules.insert(rule_id) {
                return Err(CompileError::InvalidExpression(format!(
                    "Duplicate rule reference: {}",
                    rule_id
                )));
            }
        }

        // Analyze decision logic
        for decision_rule in &ruleset.decision_logic {
            if let Some(condition) = &decision_rule.condition {
                self.analyze_expression(condition)?;
            }
        }

        Ok(())
    }

    /// Analyze a pipeline
    pub fn analyze_pipeline(&mut self, _pipeline: &Pipeline) -> Result<()> {
        // TODO: Analyze pipeline steps
        // - Check for circular dependencies
        // - Validate step references
        // - Check data flow
        Ok(())
    }

    /// Analyze an expression
    fn analyze_expression(&mut self, expr: &Expression) -> Result<()> {
        match expr {
            Expression::Literal(_) => {
                // Literals are always valid
                Ok(())
            }

            Expression::FieldAccess(path) => {
                // Field access is valid if path is not empty
                if path.is_empty() {
                    return Err(CompileError::InvalidExpression(
                        "Field path cannot be empty".to_string(),
                    ));
                }
                Ok(())
            }

            Expression::Binary { left, right, .. } => {
                // Recursively analyze both operands
                self.analyze_expression(left)?;
                self.analyze_expression(right)?;
                Ok(())
            }

            Expression::Unary { operand, .. } => {
                // Recursively analyze operand
                self.analyze_expression(operand)?;
                Ok(())
            }

            Expression::FunctionCall { name, args } => {
                // Check function name is not empty
                if name.is_empty() {
                    return Err(CompileError::InvalidExpression(
                        "Function name cannot be empty".to_string(),
                    ));
                }

                // Analyze all arguments
                for arg in args {
                    self.analyze_expression(arg)?;
                }

                Ok(())
            }

            Expression::Ternary {
                condition,
                true_expr,
                false_expr,
            } => {
                // Analyze all three parts
                self.analyze_expression(condition)?;
                self.analyze_expression(true_expr)?;
                self.analyze_expression(false_expr)?;
                Ok(())
            }
        }
    }

    /// Define a variable in the current scope
    pub fn define_variable(&mut self, name: String) {
        self.defined_variables.insert(name);
    }

    /// Check if a variable is defined
    pub fn is_variable_defined(&self, name: &str) -> bool {
        self.defined_variables.contains(name)
    }
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::{WhenBlock, Operator};
    use corint_core::Value;

    #[test]
    fn test_analyze_valid_rule() {
        let mut analyzer = SemanticAnalyzer::new();

        let rule = Rule {
            id: "test_rule".to_string(),
            name: "Test Rule".to_string(),
            description: None,
            when: WhenBlock::new(),
            score: 50,
        };

        assert!(analyzer.analyze_rule(&rule).is_ok());
    }

    #[test]
    fn test_analyze_rule_empty_id() {
        let mut analyzer = SemanticAnalyzer::new();

        let rule = Rule {
            id: String::new(),
            name: "Test Rule".to_string(),
            description: None,
            when: WhenBlock::new(),
            score: 50,
        };

        assert!(analyzer.analyze_rule(&rule).is_err());
    }

    #[test]
    fn test_analyze_rule_empty_name() {
        let mut analyzer = SemanticAnalyzer::new();

        let rule = Rule {
            id: "test_rule".to_string(),
            name: String::new(),
            description: None,
            when: WhenBlock::new(),
            score: 50,
        };

        assert!(analyzer.analyze_rule(&rule).is_err());
    }

    #[test]
    fn test_analyze_expression_literal() {
        let mut analyzer = SemanticAnalyzer::new();
        let expr = Expression::literal(Value::Number(42.0));

        assert!(analyzer.analyze_expression(&expr).is_ok());
    }

    #[test]
    fn test_analyze_expression_field_access() {
        let mut analyzer = SemanticAnalyzer::new();
        let expr = Expression::field_access(vec!["user".to_string(), "age".to_string()]);

        assert!(analyzer.analyze_expression(&expr).is_ok());
    }

    #[test]
    fn test_analyze_expression_empty_field_path() {
        let mut analyzer = SemanticAnalyzer::new();
        let expr = Expression::field_access(vec![]);

        assert!(analyzer.analyze_expression(&expr).is_err());
    }

    #[test]
    fn test_analyze_expression_binary() {
        let mut analyzer = SemanticAnalyzer::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(10.0)),
            Operator::Add,
            Expression::literal(Value::Number(20.0)),
        );

        assert!(analyzer.analyze_expression(&expr).is_ok());
    }

    #[test]
    fn test_analyze_ruleset_duplicate_rules() {
        let mut analyzer = SemanticAnalyzer::new();

        let ruleset = Ruleset {
            id: "test_ruleset".to_string(),
            name: None,
            rules: vec!["rule1".to_string(), "rule1".to_string()], // Duplicate
            decision_logic: vec![],
        };

        assert!(analyzer.analyze_ruleset(&ruleset).is_err());
    }

    #[test]
    fn test_variable_definition() {
        let mut analyzer = SemanticAnalyzer::new();

        assert!(!analyzer.is_variable_defined("x"));

        analyzer.define_variable("x".to_string());

        assert!(analyzer.is_variable_defined("x"));
    }
}
