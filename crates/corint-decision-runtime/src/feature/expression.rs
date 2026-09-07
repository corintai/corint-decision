//! Expression evaluation module
//!
//! This module provides utilities for evaluating mathematical expressions
//! and template substitution for feature computation.

use anyhow::Result;
use corint_decision_model::Value;
use std::collections::HashMap;

/// Expression evaluator for feature computations
pub(super) struct ExpressionEvaluator;

impl ExpressionEvaluator {
    /// Extract feature dependencies from an expression string
    /// Parses the expression and returns a list of feature names referenced in it
    ///
    /// Example: "unique_devices_24h / max(unique_devices_7d, 1)"
    /// Returns: ["unique_devices_24h", "unique_devices_7d"]
    pub fn extract_dependencies(expr: &str) -> Vec<String> {
        // Known function names that should not be treated as features
        let functions = ["max", "min", "abs", "sqrt", "ceil", "floor", "round"];

        // Context prefixes that should not be treated as features
        let context_prefixes = ["event", "user", "context"];

        // Regular expression-like parsing: extract identifiers (alphanumeric + underscore)
        let mut dependencies = Vec::new();
        let mut current_token = String::new();

        for ch in expr.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                current_token.push(ch);
            } else {
                if !current_token.is_empty() {
                    // Check if it's not a function name, not a context prefix, and not a number
                    if !functions.contains(&current_token.as_str())
                        && !context_prefixes.contains(&current_token.as_str())
                        && current_token.chars().next().unwrap().is_alphabetic()
                        && !dependencies.contains(&current_token)
                    {
                        dependencies.push(current_token.clone());
                    }
                    current_token.clear();
                }
            }
        }

        // Don't forget the last token
        if !current_token.is_empty()
            && !functions.contains(&current_token.as_str())
            && !context_prefixes.contains(&current_token.as_str())
            && current_token.chars().next().unwrap().is_alphabetic()
            && !dependencies.contains(&current_token)
        {
            dependencies.push(current_token);
        }

        dependencies
    }

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
                Value::Bool(b) => {
                    if *b {
                        1.0
                    } else {
                        0.0
                    }
                }
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

        // Handle parentheses - evaluate inner expression first
        if expr.starts_with('(') && expr.ends_with(')') {
            return Self::eval_math_expr(&expr[1..expr.len() - 1]);
        }

        // Handle division by zero
        if expr.contains("/0") || expr.contains("/ 0") {
            return Ok(Value::Null);
        }

        // Very simple expression parser (handles basic operations)
        // For production, consider using a proper expression parser crate like `evalexpr`

        // Handle simple binary operations (a op b)
        // Process operators with correct precedence: +/- before */÷
        for op in &['+', '-', '/', '*'] {
            let mut depth = 0;
            for (idx, ch) in expr.char_indices().rev() {
                if ch == ')' {
                    depth += 1;
                } else if ch == '(' {
                    depth -= 1;
                } else if depth == 0 && ch == *op {
                    // Skip if it's a negative sign at the beginning
                    if *op == '-' && idx == 0 {
                        continue;
                    }

                    let left = &expr[..idx];
                    let right = &expr[idx + 1..];

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
        }

        Err(anyhow::anyhow!("Unable to evaluate expression: {}", expr))
    }

    /// Substitute template variables with context values
    /// Supports:
    /// - Direct reference: "event.user_id" -> lookup context["user_id"]
    /// - String interpolation: "prefix:${event.user_id}:suffix" -> "prefix:value:suffix"
    pub(super) fn substitute_template(
        template: &str,
        context: &HashMap<String, Value>,
    ) -> Result<String> {
        fn resolve(path: &str, context: &HashMap<String, Value>) -> Result<String> {
            let path = path.strip_prefix("event.").unwrap_or(path);
            let mut segments = path.split('.');
            let first = segments
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("Empty template path"))?;
            let mut value = context
                .get(first)
                .ok_or_else(|| anyhow::anyhow!("Template variable not found: {path}"))?;
            for segment in segments {
                value = match value {
                    Value::Object(fields) => fields
                        .get(segment)
                        .ok_or_else(|| anyhow::anyhow!("Template variable not found: {path}"))?,
                    _ => return Err(anyhow::anyhow!("Template path is not an object: {path}")),
                };
            }
            match value {
                Value::String(s) => Ok(s.clone()),
                Value::Number(n) if n.is_finite() => Ok(n.to_string()),
                Value::Bool(b) => Ok(b.to_string()),
                _ => Err(anyhow::anyhow!("Unsupported template value type: {path}")),
            }
        }
        if template.contains("${") {
            let mut result = String::new();
            let mut rest = template;
            while let Some(start) = rest.find("${") {
                result.push_str(&rest[..start]);
                let variable = &rest[start + 2..];
                let end = variable
                    .find('}')
                    .ok_or_else(|| anyhow::anyhow!("Unclosed template variable"))?;
                result.push_str(&resolve(&variable[..end], context)?);
                rest = &variable[end + 1..];
            }
            result.push_str(rest);
            return Ok(result);
        }
        if template.starts_with("event.") || context.contains_key(template) {
            return resolve(template, context);
        }
        if template.contains("{event.") {
            return Err(anyhow::anyhow!(
                "Dimension templates require $ followed by braces"
            ));
        }
        Ok(template.to_string())
    }
}

#[cfg(test)]
mod template_tests {
    use super::*;
    #[test]
    fn nested_multiple_and_missing_templates_keep_full_paths() {
        let context = HashMap::from([
            (
                "user".into(),
                Value::Object(HashMap::from([(
                    "id".into(),
                    Value::String("nested".into()),
                )])),
            ),
            ("id".into(), Value::String("flat".into())),
        ]);
        assert_eq!(
            ExpressionEvaluator::substitute_template("${event.user.id}:${event.id}", &context)
                .unwrap(),
            "nested:flat"
        );
        assert_eq!(
            ExpressionEvaluator::substitute_template("event.user.id", &context).unwrap(),
            "nested"
        );
        for value in [
            "event.missing.id",
            "${event.missing.id}",
            "${event.id",
            "{event.user.id}",
        ] {
            assert!(
                ExpressionEvaluator::substitute_template(value, &context).is_err(),
                "{value}"
            );
        }
    }
}
