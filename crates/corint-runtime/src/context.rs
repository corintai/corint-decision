//! Execution context
//!
//! Manages the state during program execution including:
//! - Stack for intermediate values
//! - Event data
//! - Execution result

use corint_core::ast::Action;
use corint_core::Value;
use crate::error::{RuntimeError, Result};
use crate::result::{DecisionResult, ExecutionResult};
use std::collections::HashMap;

/// Execution context for running IR programs
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Value stack for intermediate calculations
    pub stack: Vec<Value>,

    /// Event data (input)
    pub event_data: HashMap<String, Value>,

    /// Execution result (accumulated state)
    pub result: ExecutionResult,
}

impl ExecutionContext {
    /// Create a new execution context with event data
    pub fn new(event_data: HashMap<String, Value>) -> Self {
        Self {
            stack: Vec::new(),
            event_data,
            result: ExecutionResult::new(),
        }
    }

    /// Create a new execution context with event data and existing result state
    pub fn with_result(event_data: HashMap<String, Value>, result: ExecutionResult) -> Self {
        Self {
            stack: Vec::new(),
            event_data,
            result,
        }
    }

    /// Push a value onto the stack
    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    /// Pop a value from the stack
    pub fn pop(&mut self) -> Result<Value> {
        self.stack.pop().ok_or(RuntimeError::StackUnderflow)
    }

    /// Peek at the top value without popping
    pub fn peek(&self) -> Result<&Value> {
        self.stack.last().ok_or(RuntimeError::StackUnderflow)
    }

    /// Duplicate the top stack value
    pub fn dup(&mut self) -> Result<()> {
        let value = self.peek()?.clone();
        self.push(value);
        Ok(())
    }

    /// Swap the top two stack values
    pub fn swap(&mut self) -> Result<()> {
        if self.stack.len() < 2 {
            return Err(RuntimeError::StackUnderflow);
        }
        let len = self.stack.len();
        self.stack.swap(len - 1, len - 2);
        Ok(())
    }

    /// Load a field value from event data
    pub fn load_field(&self, path: &[String]) -> Result<Value> {
        if path.is_empty() {
            return Err(RuntimeError::FieldNotFound("empty path".to_string()));
        }

        // First, try to load from event_data
        let mut current = self.event_data.get(&path[0]);

        // If not found in event_data, try variables (stored context)
        if current.is_none() {
            current = self.result.variables.get(&path[0]);
        }

        // If still not found and it's a single-path special field, use computed value
        if current.is_none() && path.len() == 1 {
            match path[0].as_str() {
                "total_score" => {
                    return Ok(Value::Number(self.result.score as f64));
                }
                "triggered_rules" => {
                    return Ok(Value::Array(
                        self.result.triggered_rules
                            .iter()
                            .map(|s| Value::String(s.clone()))
                            .collect()
                    ));
                }
                "triggered_count" => {
                    return Ok(Value::Number(self.result.triggered_rules.len() as f64));
                }
                _ => {}
            }
        }

        let mut current = current.ok_or_else(|| {
            RuntimeError::FieldNotFound(path[0].clone())
        })?;

        for segment in &path[1..] {
            match current {
                Value::Object(map) => {
                    current = map.get(segment).ok_or_else(|| {
                        RuntimeError::FieldNotFound(segment.clone())
                    })?;
                }
                _ => {
                    return Err(RuntimeError::TypeError(format!(
                        "Cannot access field '{}' on non-object",
                        segment
                    )));
                }
            }
        }

        Ok(current.clone())
    }

    /// Store a variable in the result
    pub fn store_variable(&mut self, name: String, value: Value) {
        self.result.store_variable(name, value);
    }

    /// Load a variable from the result
    pub fn load_variable(&self, name: &str) -> Result<Value> {
        self.result
            .load_variable(name)
            .cloned()
            .ok_or_else(|| RuntimeError::FieldNotFound(name.to_string()))
    }

    /// Add to the score
    pub fn add_score(&mut self, value: i32) {
        self.result.add_score(value);
    }

    /// Set the score
    pub fn set_score(&mut self, value: i32) {
        self.result.set_score(value);
    }

    /// Mark a rule as triggered
    pub fn mark_rule_triggered(&mut self, rule_id: String) {
        self.result.mark_rule_triggered(rule_id);
    }

    /// Set the action
    pub fn set_action(&mut self, action: Action) {
        self.result.action = Some(action);
    }

    /// Convert context into a DecisionResult
    pub fn into_decision_result(self) -> DecisionResult {
        DecisionResult {
            action: self.result.action,
            score: self.result.score,
            triggered_rules: self.result.triggered_rules,
            explanation: String::new(), // TODO: Build explanation from context
            context: self.result.variables,
        }
    }

    /// Get the current stack depth
    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    /// Clear the stack
    pub fn clear_stack(&mut self) {
        self.stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event() -> HashMap<String, Value> {
        let mut event = HashMap::new();
        event.insert("user_id".to_string(), Value::String("123".to_string()));
        event.insert("amount".to_string(), Value::Number(1000.0));

        // Nested object
        let mut user = HashMap::new();
        user.insert("age".to_string(), Value::Number(25.0));
        user.insert("country".to_string(), Value::String("US".to_string()));
        event.insert("user".to_string(), Value::Object(user));

        event
    }

    #[test]
    fn test_stack_operations() {
        let mut ctx = ExecutionContext::new(HashMap::new());

        // Push and pop
        ctx.push(Value::Number(42.0));
        ctx.push(Value::String("test".to_string()));

        assert_eq!(ctx.stack_depth(), 2);

        let val = ctx.pop().unwrap();
        assert_eq!(val, Value::String("test".to_string()));

        let val = ctx.pop().unwrap();
        assert_eq!(val, Value::Number(42.0));

        assert_eq!(ctx.stack_depth(), 0);
    }

    #[test]
    fn test_stack_underflow() {
        let mut ctx = ExecutionContext::new(HashMap::new());
        let result = ctx.pop();
        assert!(result.is_err());
    }

    #[test]
    fn test_dup() {
        let mut ctx = ExecutionContext::new(HashMap::new());
        ctx.push(Value::Number(42.0));

        ctx.dup().unwrap();

        assert_eq!(ctx.stack_depth(), 2);
        assert_eq!(ctx.pop().unwrap(), Value::Number(42.0));
        assert_eq!(ctx.pop().unwrap(), Value::Number(42.0));
    }

    #[test]
    fn test_swap() {
        let mut ctx = ExecutionContext::new(HashMap::new());
        ctx.push(Value::Number(1.0));
        ctx.push(Value::Number(2.0));

        ctx.swap().unwrap();

        assert_eq!(ctx.pop().unwrap(), Value::Number(1.0));
        assert_eq!(ctx.pop().unwrap(), Value::Number(2.0));
    }

    #[test]
    fn test_load_field() {
        let event = create_test_event();
        let ctx = ExecutionContext::new(event);

        // Load simple field
        let value = ctx.load_field(&[String::from("user_id")]).unwrap();
        assert_eq!(value, Value::String("123".to_string()));

        // Load nested field
        let value = ctx.load_field(&[String::from("user"), String::from("age")]).unwrap();
        assert_eq!(value, Value::Number(25.0));
    }

    #[test]
    fn test_load_field_not_found() {
        let event = create_test_event();
        let ctx = ExecutionContext::new(event);

        let result = ctx.load_field(&[String::from("nonexistent")]);
        assert!(result.is_err());
    }

    #[test]
    fn test_variables() {
        let mut ctx = ExecutionContext::new(HashMap::new());

        ctx.store_variable("temp".to_string(), Value::Number(42.0));

        let value = ctx.load_variable("temp").unwrap();
        assert_eq!(value, Value::Number(42.0));
    }

    #[test]
    fn test_score_operations() {
        let mut ctx = ExecutionContext::new(HashMap::new());

        ctx.set_score(50);
        assert_eq!(ctx.result.score, 50);

        ctx.add_score(25);
        assert_eq!(ctx.result.score, 75);
    }

    #[test]
    fn test_mark_rule_triggered() {
        let mut ctx = ExecutionContext::new(HashMap::new());

        ctx.mark_rule_triggered("rule_1".to_string());
        ctx.mark_rule_triggered("rule_2".to_string());

        assert_eq!(ctx.result.triggered_rules.len(), 2);
        assert_eq!(ctx.result.triggered_rules[0], "rule_1");
    }
}
