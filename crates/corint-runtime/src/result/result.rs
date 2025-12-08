//! Execution result types

use corint_core::ast::Action;
use corint_core::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of executing a decision program
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionResult {
    /// The final action to take
    pub action: Option<Action>,

    /// Total risk score
    pub score: i32,

    /// List of triggered rule IDs
    pub triggered_rules: Vec<String>,

    /// Explanation/reason for the decision
    pub explanation: String,

    /// Additional context data
    pub context: HashMap<String, Value>,
}

/// Result of executing an IR program
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Final score
    pub score: i32,

    /// Triggered rules
    pub triggered_rules: Vec<String>,

    /// Action
    pub action: Option<Action>,

    /// Variables stored during execution
    pub variables: HashMap<String, Value>,
}

impl DecisionResult {
    /// Create a new decision result
    pub fn new(action: Action, score: i32) -> Self {
        Self {
            action: Some(action),
            score,
            triggered_rules: Vec::new(),
            explanation: String::new(),
            context: HashMap::new(),
        }
    }

    /// Add a triggered rule
    pub fn add_triggered_rule(&mut self, rule_id: String) {
        self.triggered_rules.push(rule_id);
    }

    /// Set explanation
    pub fn with_explanation(mut self, explanation: String) -> Self {
        self.explanation = explanation;
        self
    }

    /// Add context data
    pub fn add_context(&mut self, key: String, value: Value) {
        self.context.insert(key, value);
    }
}

impl ExecutionResult {
    /// Create a new execution result
    pub fn new() -> Self {
        Self {
            score: 0,
            triggered_rules: Vec::new(),
            action: None,
            variables: HashMap::new(),
        }
    }

    /// Add score
    pub fn add_score(&mut self, value: i32) {
        self.score += value;
    }

    /// Set score
    pub fn set_score(&mut self, value: i32) {
        self.score = value;
    }

    /// Mark a rule as triggered
    pub fn mark_rule_triggered(&mut self, rule_id: String) {
        self.triggered_rules.push(rule_id);
    }

    /// Store a variable
    pub fn store_variable(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    /// Load a variable
    pub fn load_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_result() {
        let mut result = DecisionResult::new(Action::Approve, 0);

        result.add_triggered_rule("rule_1".to_string());
        result.add_triggered_rule("rule_2".to_string());
        result.add_context("user_id".to_string(), Value::String("123".to_string()));

        assert_eq!(result.triggered_rules.len(), 2);
        assert_eq!(result.context.len(), 1);
    }

    #[test]
    fn test_execution_result() {
        let mut result = ExecutionResult::new();

        result.add_score(50);
        result.add_score(25);
        assert_eq!(result.score, 75);

        result.set_score(100);
        assert_eq!(result.score, 100);

        result.mark_rule_triggered("rule_1".to_string());
        assert_eq!(result.triggered_rules.len(), 1);

        result.store_variable("temp".to_string(), Value::Number(42.0));
        assert_eq!(
            result.load_variable("temp"),
            Some(&Value::Number(42.0))
        );
    }
}
