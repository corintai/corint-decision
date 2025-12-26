//! Expression evaluation module
//!
//! This module provides utilities for evaluating mathematical expressions
//! and template substitution for feature computation.

use anyhow::Result;
use corint_core::Value;
use std::collections::HashMap;

/// Expression evaluator for feature computations
pub(super) struct ExpressionEvaluator;

impl ExpressionEvaluator {
    /// Evaluate a mathematical expression with feature values
    /// Supports: +, -, *, /, feature names, numbers
    pub(super) fn evaluate_expression(
        expr: &str,
        feature_values: &HashMap<String, Value>,
    ) -> Result<Value> {
        // Replace feature names with their values
        let mut expr_normalized = expr.to_string();

        // Extract all feature names and replace with values
        for (name, value) in feature_values {
            let value_num = match value {
                Value::Number(n) => *n,
                Value::Null => 0.0,
                Value::Bool(b) => if *b { 1.0 } else { 0.0 },
                _ => return Err(anyhow::anyhow!("Feature '{}' has non-numeric value", name)),
            };

            // Replace feature name with its numeric value
            expr_normalized = expr_normalized.replace(name, &value_num.to_string());
        }

        // Evaluate the expression using a simple parser
        Self::eval_math_expr(&expr_normalized)
    }

    /// Simple math expression evaluator
    /// Supports: +, -, *, /, parentheses, numbers
    pub(super) fn eval_math_expr(expr: &str) -> Result<Value> {
        // Remove whitespace
        let expr = expr.replace(' ', "");

        // Try to parse as a simple number first
        if let Ok(num) = expr.parse::<f64>() {
            return Ok(Value::Number(num));
        }

        // Handle division by zero
        if expr.contains("/0") || expr.contains("/ 0") {
            return Ok(Value::Null);
        }

        // Very simple expression parser (handles basic operations)
        // For production, consider using a proper expression parser crate like `evalexpr`

        // Handle simple binary operations (a op b)
        for op in &['/', '*', '+', '-'] {
            if let Some(idx) = expr.rfind(*op) {
                // Skip if it's a negative sign at the beginning
                if *op == '-' && idx == 0 {
                    continue;
                }

                let left = &expr[..idx];
                let right = &expr[idx+1..];

                let left_val = match Self::eval_math_expr(left)? {
                    Value::Number(n) => n,
                    _ => return Err(anyhow::anyhow!("Invalid expression: {}", expr)),
                };

                let right_val = match Self::eval_math_expr(right)? {
                    Value::Number(n) => n,
                    _ => return Err(anyhow::anyhow!("Invalid expression: {}", expr)),
                };

                let result = match op {
                    '+' => left_val + right_val,
                    '-' => left_val - right_val,
                    '*' => left_val * right_val,
                    '/' => {
                        if right_val == 0.0 {
                            return Ok(Value::Null);
                        }
                        left_val / right_val
                    }
                    _ => unreachable!(),
                };

                return Ok(Value::Number(result));
            }
        }

        Err(anyhow::anyhow!("Unable to evaluate expression: {}", expr))
    }

    /// Substitute template variables with context values
    /// Example: "{event.user_id}" -> "user123" from context["user_id"]
    pub(super) fn substitute_template(template: &str, context: &HashMap<String, Value>) -> Result<String> {
        // Simple template substitution: {event.user_id} -> context["user_id"]
        let mut result = template.to_string();

        // Extract all {xxx} patterns
        if let Some(start) = result.find('{') {
            if let Some(end) = result.find('}') {
                let var_path = &result[start+1..end];

                // Parse path like "event.user_id" -> ["event", "user_id"]
                let parts: Vec<&str> = var_path.split('.').collect();

                // For now, just use the last part as the key
                if let Some(key) = parts.last() {
                    if let Some(value) = context.get(*key) {
                        let value_str = match value {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            _ => return Err(anyhow::anyhow!("Unsupported template value type")),
                        };
                        result = result.replace(&result[start..=end], &value_str);
                    } else {
                        return Err(anyhow::anyhow!(
                            "Template variable '{}' not found in context. Available keys: {:?}",
                            key, context.keys().collect::<Vec<_>>()
                        ));
                    }
                }
            }
        }

        Ok(result)
    }
}
