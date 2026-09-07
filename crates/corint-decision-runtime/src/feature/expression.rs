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
    /// Resolve dependencies from the shared bounded AST, preserving whole paths.
    pub fn extract_dependencies(expr: &str) -> Result<Vec<String>> {
        Ok(Self::parse_math(expr)?.1)
    }

    fn parse_math(expr: &str) -> Result<(corint_decision_model::ast::Expression, Vec<String>)> {
        use corint_decision_model::ast::Expression;
        let ast = corint_decision_dsl_parser::ExpressionParser::parse(expr)?;
        let mut pending = vec![(&ast, 1usize)];
        let mut dependencies = Vec::new();
        while let Some((node, depth)) = pending.pop() {
            if depth > 128 {
                anyhow::bail!("Feature expression depth exceeds 128");
            }
            match node {
                Expression::Literal(Value::Number(n)) if n.is_finite() => (),
                Expression::FieldAccess(path) => match path.as_slice() {
                    [root, ..] if root == "event" && path.len() > 1 => (),
                    [name] => dependencies.push(name.clone()),
                    [root, name] if root == "features" => dependencies.push(name.clone()),
                    _ => anyhow::bail!("Unsupported feature expression path: {}", path.join(".")),
                },
                Expression::Binary { left, op, right } if Self::math_operator(op) => {
                    pending.push((right, depth + 1));
                    pending.push((left, depth + 1));
                }
                Expression::Unary {
                    op: corint_decision_model::ast::UnaryOperator::Negate,
                    operand,
                } => pending.push((operand, depth + 1)),
                Expression::FunctionCall { name, args }
                    if Self::math_function(name, args.len()) =>
                {
                    pending.extend(args.iter().rev().map(|arg| (arg, depth + 1)))
                }
                _ => anyhow::bail!("Feature expressions require numeric arithmetic"),
            }
        }
        dependencies.sort();
        dependencies.dedup();
        Ok((ast, dependencies))
    }

    fn math_operator(op: &corint_decision_model::ast::Operator) -> bool {
        use corint_decision_model::ast::Operator;
        matches!(
            op,
            Operator::Add | Operator::Sub | Operator::Mul | Operator::Div | Operator::Mod
        )
    }

    fn math_function(name: &str, count: usize) -> bool {
        matches!(
            (name, count),
            ("max" | "min", 2) | ("abs" | "sqrt" | "ceil" | "floor" | "round", 1)
        )
    }

    pub(super) fn evaluate_with_context(
        expr: &str,
        features: &HashMap<String, Value>,
        event: &HashMap<String, Value>,
    ) -> Result<Value> {
        use corint_decision_model::ast::{Expression, Operator, UnaryOperator};
        fn number(value: &Value) -> Result<Option<f64>> {
            match value {
                Value::Number(n) if n.is_finite() => Ok(Some(*n)),
                Value::Null => Ok(None),
                _ => anyhow::bail!("Feature expression operand must be a finite number or null"),
            }
        }
        fn eval(
            node: &Expression,
            features: &HashMap<String, Value>,
            event: &HashMap<String, Value>,
        ) -> Result<Option<f64>> {
            let value = match node {
                Expression::Literal(value) => return number(value),
                Expression::FieldAccess(path) => {
                    let (values, segments) = match path.as_slice() {
                        [root, rest @ ..] if root == "event" && !rest.is_empty() => (event, rest),
                        [root, name] if root == "features" => {
                            (features, std::slice::from_ref(name))
                        }
                        [name] => (features, std::slice::from_ref(name)),
                        _ => anyhow::bail!("Unsupported feature expression path"),
                    };
                    let mut value = values.get(&segments[0]).ok_or_else(|| {
                        anyhow::anyhow!("Missing expression input: {}", path.join("."))
                    })?;
                    for segment in &segments[1..] {
                        value = match value {
                            Value::Object(fields) => fields.get(segment).ok_or_else(|| {
                                anyhow::anyhow!("Missing expression input: {}", path.join("."))
                            })?,
                            _ => anyhow::bail!("Expression input path is not an object"),
                        };
                    }
                    return number(value);
                }
                Expression::Unary {
                    op: UnaryOperator::Negate,
                    operand,
                } => eval(operand, features, event)?.map(|v| -v),
                Expression::Binary { left, op, right }
                    if ExpressionEvaluator::math_operator(op) =>
                {
                    let left = eval(left, features, event)?;
                    let right = eval(right, features, event)?;
                    match (left, right) {
                        (Some(a), Some(b)) => match op {
                            Operator::Add => Some(a + b),
                            Operator::Sub => Some(a - b),
                            Operator::Mul => Some(a * b),
                            Operator::Div if b != 0.0 => Some(a / b),
                            Operator::Mod if b != 0.0 => Some(a % b),
                            _ => None,
                        },
                        _ => None,
                    }
                }
                Expression::FunctionCall { name, args }
                    if ExpressionEvaluator::math_function(name, args.len()) =>
                {
                    let values: Vec<_> = args
                        .iter()
                        .map(|arg| eval(arg, features, event))
                        .collect::<Result<_>>()?;
                    if values.iter().any(Option::is_none) {
                        return Ok(None);
                    }
                    let a = values[0].unwrap();
                    Some(match name.as_str() {
                        "max" => a.max(values[1].unwrap()),
                        "min" => a.min(values[1].unwrap()),
                        "abs" => a.abs(),
                        "sqrt" => a.sqrt(),
                        "ceil" => a.ceil(),
                        "floor" => a.floor(),
                        "round" => a.round(),
                        _ => unreachable!(),
                    })
                }
                _ => anyhow::bail!("Feature expressions require numeric arithmetic"),
            };
            if value.is_some_and(|v| !v.is_finite()) {
                anyhow::bail!("Nonfinite feature expression result");
            }
            Ok(value)
        }
        let (ast, _) = Self::parse_math(expr)?;
        Ok(eval(&ast, features, event)?
            .map(Value::Number)
            .unwrap_or(Value::Null))
    }

    #[cfg(test)]
    pub(super) fn evaluate_expression(
        expr: &str,
        features: &HashMap<String, Value>,
    ) -> Result<Value> {
        Self::evaluate_with_context(expr, features, &HashMap::new())
    }

    #[cfg(test)]
    pub(super) fn eval_math_expr(expr: &str) -> Result<Value> {
        Self::evaluate_expression(expr, &HashMap::new())
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
    fn dependencies_and_arithmetic_preserve_paths_and_token_boundaries() {
        assert_eq!(
            ExpressionEvaluator::extract_dependencies(
                "event.payment.amount / (avg_amount + 1e-4) + features.avg"
            )
            .unwrap(),
            vec!["avg", "avg_amount"]
        );
        let values = HashMap::from([
            ("avg".into(), Value::Number(2.0)),
            ("avg_amount".into(), Value::Number(4.0)),
        ]);
        let event = HashMap::from([(
            "payment".into(),
            Value::Object(HashMap::from([("amount".into(), Value::Number(8.0))])),
        )]);
        assert_eq!(
            ExpressionEvaluator::evaluate_with_context(
                "event.payment.amount / avg_amount + avg",
                &values,
                &event
            )
            .unwrap(),
            Value::Number(4.0)
        );
        for (expr, expected) in [
            ("1/0.0001", 10000.0),
            ("8/2*2", 8.0),
            ("10-4+1", 7.0),
            ("(1+2)*(3+4)", 21.0),
            ("2*-3 + 1e-3", -5.999),
            ("max(2, 4)", 4.0),
        ] {
            assert_eq!(
                ExpressionEvaluator::eval_math_expr(expr).unwrap(),
                Value::Number(expected),
                "{expr}"
            );
        }
        assert!(
            ExpressionEvaluator::evaluate_with_context("event.missing + avg", &values, &event)
                .is_err()
        );
        assert!(ExpressionEvaluator::eval_math_expr("1e308 * 1e308").is_err());
        assert_eq!(
            ExpressionEvaluator::eval_math_expr("1/(2-2)").unwrap(),
            Value::Null
        );
    }

    #[test]
    fn numeric_contract_rejects_coercion_and_propagates_null() {
        let inputs = HashMap::from([
            ("absent".into(), Value::Null),
            ("flag".into(), Value::Bool(true)),
            ("text".into(), Value::String("3".into())),
        ]);
        for expression in ["absent + 1", "max(absent, 1)", "1 % 0"] {
            assert_eq!(
                ExpressionEvaluator::evaluate_expression(expression, &inputs).unwrap(),
                Value::Null
            );
        }
        for expression in ["flag + 1", "text * 2", "sqrt(-1)", "1e308 * 1e308"] {
            assert!(ExpressionEvaluator::evaluate_expression(expression, &inputs).is_err());
        }
        for expression in [
            "max(1)",
            "unknown(1)",
            "event.amount == 1",
            "results.x",
            "1e999",
        ] {
            assert!(ExpressionEvaluator::extract_dependencies(expression).is_err());
        }
    }

    #[tokio::test]
    async fn registered_expression_reads_request_context_and_real_dependencies() {
        use crate::{
            context::{ContextInput, ExecutionContext},
            feature::{FeatureDefinition, FeatureExecutor},
        };
        let features: Vec<FeatureDefinition> = serde_yaml::from_str("- name: avg\n  type: expression\n  expression: '4'\n- name: ratio\n  type: expression\n  expression: 'event.amount / avg'\n").unwrap();
        let mut executor = FeatureExecutor::new();
        executor.register_features(features).unwrap();
        let context = ExecutionContext::new(ContextInput::new(HashMap::from([(
            "amount".into(),
            Value::Number(12.0),
        )])))
        .unwrap();
        assert_eq!(
            executor
                .execute_features(&["ratio".into()], &context)
                .await
                .unwrap()["ratio"],
            Value::Number(3.0)
        );
    }
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
