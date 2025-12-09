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
    pub fn analyze_pipeline(&mut self, pipeline: &Pipeline) -> Result<()> {
        // 1. Validate pipeline ID
        if let Some(ref id) = pipeline.id {
            if id.is_empty() {
                return Err(CompileError::InvalidExpression(
                    "Pipeline ID cannot be empty".to_string(),
                ));
            }
        }

        // 2. Track all step IDs to check for duplicates
        let mut step_ids = HashSet::new();

        // 3. Track all output variables produced by steps
        let mut produced_vars = HashSet::new();

        // 4. Track all variables referenced in expressions
        let mut referenced_vars = HashSet::new();

        // 5. Analyze each step
        for step in &pipeline.steps {
            self.analyze_step(step, &mut step_ids, &mut produced_vars, &mut referenced_vars)?;
        }

        // 6. Check for undefined variable references
        for var_ref in &referenced_vars {
            // Allow references to:
            // - Variables produced in the pipeline (features.*, etc.)
            // - Event data fields (event.* or simple field names like "payment_amount")
            // - Context variables (context.*)
            let is_defined = produced_vars.contains(var_ref)
                || var_ref.starts_with("event.")
                || var_ref.starts_with("context.")
                || var_ref.starts_with("features.")
                || !var_ref.contains('.'); // Simple field names are event fields

            if !is_defined {
                return Err(CompileError::InvalidExpression(format!(
                    "Variable '{}' is referenced but never defined in pipeline",
                    var_ref
                )));
            }
        }

        // 7. Analyze when block conditions if present
        if let Some(ref when_block) = pipeline.when {
            for condition in &when_block.conditions {
                self.analyze_expression(condition)?;
            }
        }

        Ok(())
    }

    /// Analyze a single pipeline step
    fn analyze_step(
        &mut self,
        step: &corint_core::ast::Step,
        step_ids: &mut HashSet<String>,
        produced_vars: &mut HashSet<String>,
        referenced_vars: &mut HashSet<String>,
    ) -> Result<()> {
        use corint_core::ast::Step;

        match step {
            Step::Extract { id, features } => {
                // Check for duplicate step ID
                if !step_ids.insert(id.clone()) {
                    return Err(CompileError::InvalidExpression(format!(
                        "Duplicate step ID: {}",
                        id
                    )));
                }

                // Analyze feature expressions and track produced variables
                for feature in features {
                    self.analyze_expression(&feature.value)?;
                    // Features are accessible via features.{name} or context.{name}
                    produced_vars.insert(format!("features.{}", feature.name));
                    produced_vars.insert(feature.name.clone());
                }
            }

            Step::Reason { id, prompt, .. } => {
                // Check for duplicate step ID
                if !step_ids.insert(id.clone()) {
                    return Err(CompileError::InvalidExpression(format!(
                        "Duplicate step ID: {}",
                        id
                    )));
                }

                // Validate prompt template is not empty
                if prompt.template.is_empty() {
                    return Err(CompileError::InvalidExpression(
                        "LLM prompt template cannot be empty".to_string(),
                    ));
                }
            }

            Step::Service { id, output, params, .. } => {
                // Check for duplicate step ID
                if !step_ids.insert(id.clone()) {
                    return Err(CompileError::InvalidExpression(format!(
                        "Duplicate step ID: {}",
                        id
                    )));
                }

                // Analyze parameter expressions
                for expr in params.values() {
                    self.analyze_expression(expr)?;
                    self.collect_variable_references(expr, referenced_vars);
                }

                // Track output variable if specified
                if let Some(ref output_var) = output {
                    produced_vars.insert(output_var.clone());
                }
            }

            Step::Api { id, output, params, .. } => {
                // Check for duplicate step ID
                if !step_ids.insert(id.clone()) {
                    return Err(CompileError::InvalidExpression(format!(
                        "Duplicate step ID: {}",
                        id
                    )));
                }

                // Analyze parameter expressions
                for expr in params.values() {
                    self.analyze_expression(expr)?;
                    self.collect_variable_references(expr, referenced_vars);
                }

                // Track output variable (required for API calls)
                produced_vars.insert(output.clone());
            }

            Step::Include { ruleset } => {
                // Validate ruleset reference is not empty
                if ruleset.is_empty() {
                    return Err(CompileError::InvalidExpression(
                        "Ruleset reference cannot be empty".to_string(),
                    ));
                }
            }

            Step::Branch { branches } => {
                // Analyze each branch
                for branch in branches {
                    // Analyze branch condition
                    self.analyze_expression(&branch.condition)?;
                    self.collect_variable_references(&branch.condition, referenced_vars);

                    // Recursively analyze branch pipeline steps
                    for branch_step in &branch.pipeline {
                        self.analyze_step(branch_step, step_ids, produced_vars, referenced_vars)?;
                    }
                }
            }

            Step::Parallel { steps, .. } => {
                // Analyze each parallel step
                for parallel_step in steps {
                    self.analyze_step(parallel_step, step_ids, produced_vars, referenced_vars)?;
                }
            }
        }

        Ok(())
    }

    /// Collect variable references from an expression
    fn collect_variable_references(&self, expr: &Expression, references: &mut HashSet<String>) {
        match expr {
            Expression::FieldAccess(path) => {
                if !path.is_empty() {
                    // Store the full path as a reference
                    references.insert(path.join("."));
                }
            }
            Expression::Binary { left, right, .. } => {
                self.collect_variable_references(left, references);
                self.collect_variable_references(right, references);
            }
            Expression::Unary { operand, .. } => {
                self.collect_variable_references(operand, references);
            }
            Expression::FunctionCall { args, .. } => {
                for arg in args {
                    self.collect_variable_references(arg, references);
                }
            }
            Expression::Ternary { condition, true_expr, false_expr } => {
                self.collect_variable_references(condition, references);
                self.collect_variable_references(true_expr, references);
                self.collect_variable_references(false_expr, references);
            }
            Expression::Literal(_) => {
                // Literals don't reference variables
            }
        }
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
