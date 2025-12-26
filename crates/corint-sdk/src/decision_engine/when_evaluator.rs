//! When block and condition evaluation logic

use corint_core::ast::{Condition, ConditionGroup, Expression, Operator, WhenBlock};
use corint_core::Value;
use corint_runtime::ConditionTrace;
use std::collections::HashMap;

pub(super) struct WhenEvaluator;

impl WhenEvaluator {
pub(super) fn evaluate_when_block(when: &WhenBlock, event_data: &HashMap<String, Value>) -> bool {
    tracing::debug!("evaluate_when_block: when={:?}, event_data={:?}", when, event_data);

    // Check event_type if specified
    // Note: event_type field in WhenBlock corresponds to event.type in YAML,
    // which is stored as "type" key in event_data HashMap
    if let Some(ref expected_type) = when.event_type {
        if let Some(Value::String(actual)) = event_data.get("type") {
            if actual != expected_type {
                tracing::debug!("event_type mismatch: expected={}, actual={}", expected_type, actual);
                return false; // Event type mismatch
            }
        } else {
            tracing::debug!("No type field in event data");
            return false; // No type field in event data or type is not a string
        }
    }

    // Evaluate condition_group (new format: all/any/not)
    if let Some(ref condition_group) = when.condition_group {
        let result = WhenEvaluator::evaluate_condition_group(condition_group, event_data);
        tracing::debug!("condition_group evaluation result: {}", result);
        return result;
    }

    // Evaluate all conditions (legacy format - AND logic)
    if let Some(ref conditions) = when.conditions {
        for condition in conditions {
            if !WhenEvaluator::evaluate_expression(condition, event_data) {
                return false; // Condition failed
            }
        }
    }

    true // All checks passed
}

/// Evaluate a condition group (all/any/not)
pub(super) fn evaluate_condition_group(
    group: &ConditionGroup,
    event_data: &HashMap<String, Value>,
) -> bool {
    match group {
        ConditionGroup::All(conditions) => {
            // All conditions must be true (AND logic)
            for condition in conditions {
                let result = WhenEvaluator::evaluate_condition(condition, event_data);
                tracing::debug!("Evaluating condition in All group: {:?}, result={}", condition, result);
                if !result {
                    return false;
                }
            }
            true
        }
        ConditionGroup::Any(conditions) => {
            // At least one condition must be true (OR logic)
            for condition in conditions {
                if WhenEvaluator::evaluate_condition(condition, event_data) {
                    return true;
                }
            }
            false
        }
        ConditionGroup::Not(conditions) => {
            // None of the conditions should be true (NOT logic)
            for condition in conditions {
                if WhenEvaluator::evaluate_condition(condition, event_data) {
                    return false;
                }
            }
            true
        }
    }
}

/// Evaluate a single condition (expression or nested group)
pub(super) fn evaluate_condition(
    condition: &Condition,
    event_data: &HashMap<String, Value>,
) -> bool {
    match condition {
        Condition::Expression(expr) => WhenEvaluator::evaluate_expression(expr, event_data),
        Condition::Group(group) => WhenEvaluator::evaluate_condition_group(group, event_data),
    }
}

/// Evaluate a condition group with tracing support
pub(super) fn evaluate_condition_group_with_trace(
    group: &ConditionGroup,
    event_data: &HashMap<String, Value>,
) -> (bool, Vec<ConditionTrace>) {
    match group {
        ConditionGroup::All(conditions) => {
            let mut traces = Vec::new();
            let mut all_true = true;

            for condition in conditions {
                let (result, mut cond_traces) = WhenEvaluator::evaluate_condition_with_trace(condition, event_data);
                traces.append(&mut cond_traces);
                if !result {
                    all_true = false;
                }
            }

            (all_true, traces)
        }
        ConditionGroup::Any(conditions) => {
            let mut traces = Vec::new();
            let mut any_true = false;

            for condition in conditions {
                let (result, mut cond_traces) = WhenEvaluator::evaluate_condition_with_trace(condition, event_data);
                traces.append(&mut cond_traces);
                if result {
                    any_true = true;
                }
            }

            (any_true, traces)
        }
        ConditionGroup::Not(conditions) => {
            let mut traces = Vec::new();
            let mut all_true = true;

            for condition in conditions {
                let (result, mut cond_traces) = WhenEvaluator::evaluate_condition_with_trace(condition, event_data);
                traces.append(&mut cond_traces);
                if !result {
                    all_true = false;
                }
            }

            // NOT inverts the result
            (!all_true, traces)
        }
    }
}

/// Evaluate a single condition with tracing support
pub(super) fn evaluate_condition_with_trace(
    condition: &Condition,
    event_data: &HashMap<String, Value>,
) -> (bool, Vec<ConditionTrace>) {
    match condition {
        Condition::Expression(expr) => {
            // For expression, evaluate it and create a trace
            let result = WhenEvaluator::evaluate_expression(expr, event_data);
            let expr_string = WhenEvaluator::expression_to_string(expr);
            let trace = ConditionTrace::new(expr_string, result);
            (result, vec![trace])
        }
        Condition::Group(group) => {
            // For nested group, recursively evaluate
            WhenEvaluator::evaluate_condition_group_with_trace(group, event_data)
        }
    }
}

/// Evaluate an expression against event data
pub(super) fn evaluate_expression(expr: &Expression, event_data: &HashMap<String, Value>) -> bool {
    match expr {
        Expression::Literal(val) => {
            // Literal is truthy if non-zero, non-empty, non-null
            WhenEvaluator::is_truthy(val)
        }
        Expression::FieldAccess(path) => {
            // Field access - get value and check if truthy
            if let Some(val) = WhenEvaluator::get_field_value(event_data, path) {
                WhenEvaluator::is_truthy(&val)
            } else {
                false
            }
        }
        Expression::Binary { left, op, right } => {
            WhenEvaluator::evaluate_binary_expression(left, op, right, event_data)
        }
        Expression::Unary { .. } => {
            // Unary not supported yet in this simple evaluator
            false
        }
        Expression::FunctionCall { .. } => {
            // Function calls not supported in this simple evaluator
            false
        }
        Expression::Ternary { .. } => {
            // Ternary expressions not supported in this simple evaluator
            false
        }
        Expression::LogicalGroup { op, conditions } => {
            // Evaluate logical group (any/all)
            use corint_core::ast::LogicalGroupOp;
            match op {
                LogicalGroupOp::Any => {
                    // OR logic: return true if ANY condition is true
                    conditions
                        .iter()
                        .any(|cond| WhenEvaluator::evaluate_expression(cond, event_data))
                }
                LogicalGroupOp::All => {
                    // AND logic: return true if ALL conditions are true
                    conditions
                        .iter()
                        .all(|cond| WhenEvaluator::evaluate_expression(cond, event_data))
                }
            }
        }
        Expression::ListReference { .. } => {
            // List references cannot be directly evaluated to boolean in this simple evaluator
            // They are only used in conjunction with InList/NotInList operators
            false
        }
        Expression::ResultAccess { .. } => {
            // Result access requires runtime context, not supported in this simple evaluator
            false
        }
    }
}

/// Evaluate a binary expression
pub(super) fn evaluate_binary_expression(
    left: &Expression,
    op: &Operator,
    right: &Expression,
    event_data: &HashMap<String, Value>,
) -> bool {
    let left_val = WhenEvaluator::expression_to_value(left, event_data);
    let right_val = WhenEvaluator::expression_to_value(right, event_data);

    match op {
        Operator::Eq => left_val == right_val,
        Operator::Ne => left_val != right_val,
        Operator::Lt => {
            WhenEvaluator::compare_values(&left_val, &right_val) == Some(std::cmp::Ordering::Less)
        }
        Operator::Gt => {
            WhenEvaluator::compare_values(&left_val, &right_val) == Some(std::cmp::Ordering::Greater)
        }
        Operator::Le => matches!(
            WhenEvaluator::compare_values(&left_val, &right_val),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        ),
        Operator::Ge => matches!(
            WhenEvaluator::compare_values(&left_val, &right_val),
            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        ),
        Operator::And => WhenEvaluator::is_truthy(&left_val) && WhenEvaluator::is_truthy(&right_val),
        Operator::Or => WhenEvaluator::is_truthy(&left_val) || WhenEvaluator::is_truthy(&right_val),
        Operator::In => {
            // Check if left value is in right array
            if let Value::Array(arr) = &right_val {
                arr.contains(&left_val)
            } else {
                false
            }
        }
        Operator::InList | Operator::NotInList => {
            // List membership operators are not supported in this simple evaluator
            // They require runtime list lookup which is handled by the VM
            false
        }
        _ => false,
    }
}

/// Convert expression to value
pub(super) fn expression_to_value(expr: &Expression, event_data: &HashMap<String, Value>) -> Value {
    match expr {
        Expression::Literal(val) => val.clone(),
        Expression::FieldAccess(path) => {
            WhenEvaluator::get_field_value(event_data, path).unwrap_or(Value::Null)
        }
        Expression::Binary { left, op, right } => {
            let result = WhenEvaluator::evaluate_binary_expression(left, op, right, event_data);
            Value::Bool(result)
        }
        Expression::Unary { .. } => Value::Null,
        Expression::FunctionCall { .. } => Value::Null,
        Expression::Ternary { .. } => Value::Null,
        Expression::LogicalGroup { op, conditions } => {
            // Convert logical group to boolean value
            use corint_core::ast::LogicalGroupOp;
            let result = match op {
                LogicalGroupOp::Any => conditions
                    .iter()
                    .any(|cond| WhenEvaluator::evaluate_expression(cond, event_data)),
                LogicalGroupOp::All => conditions
                    .iter()
                    .all(|cond| WhenEvaluator::evaluate_expression(cond, event_data)),
            };
            Value::Bool(result)
        }
        Expression::ListReference { .. } => {
            // List references cannot be directly evaluated to a value in this context
            // They are only used in conjunction with InList/NotInList operators
            Value::Null
        }
        Expression::ResultAccess { .. } => {
            // Result access requires runtime context, not supported in this simple evaluator
            Value::Null
        }
    }
}

/// Get field value from nested path
pub(super) fn get_field_value(event_data: &HashMap<String, Value>, path: &[String]) -> Option<Value> {
    if path.is_empty() {
        return None;
    }

    // Special case: if path starts with "event", skip it since event_data IS the event
    let actual_path = if path.len() > 0 && path[0] == "event" {
        &path[1..]
    } else {
        path
    };

    if actual_path.is_empty() {
        return None;
    }

    let mut current = event_data.get(&actual_path[0])?;

    for key in &actual_path[1..] {
        match current {
            Value::Object(map) => {
                current = map.get(key)?;
            }
            _ => return None,
        }
    }

    Some(current.clone())
}

/// Compare two values
pub(super) fn compare_values(left: &Value, right: &Value) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => a.partial_cmp(b),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

/// Check if a value is truthy
pub(super) fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => *n != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

// =========================================================================
// TRACE-ENABLED EVALUATION FUNCTIONS
// These functions are prepared for detailed condition-level tracing.
// They will be used when we integrate more detailed trace collection.
// =========================================================================

/// Convert an Expression to a string representation for tracing
#[allow(dead_code)]
pub(super) fn expression_to_string(expr: &Expression) -> String {
    match expr {
        Expression::Literal(val) => match val {
            Value::String(s) => format!("\"{}\"", s),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Array(arr) => format!(
                "[{}]",
                arr.iter()
                    .map(|v| match v {
                        Value::String(s) => format!("\"{}\"", s),
                        Value::Number(n) => n.to_string(),
                        _ => format!("{:?}", v),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Object(_) => "{...}".to_string(),
        },
        Expression::FieldAccess(path) => path.join("."),
        Expression::Binary { left, op, right } => {
            let op_str = match op {
                Operator::Eq => "==",
                Operator::Ne => "!=",
                Operator::Lt => "<",
                Operator::Gt => ">",
                Operator::Le => "<=",
                Operator::Ge => ">=",
                Operator::And => "&&",
                Operator::Or => "||",
                Operator::Add => "+",
                Operator::Sub => "-",
                Operator::Mul => "*",
                Operator::Div => "/",
                Operator::Mod => "%",
                Operator::In => "in",
                Operator::NotIn => "not in",
                Operator::Contains => "contains",
                Operator::StartsWith => "starts_with",
                Operator::EndsWith => "ends_with",
                Operator::Regex => "=~",
                Operator::InList => "in list",
                Operator::NotInList => "not in list",
            };
            format!(
                "{} {} {}",
                WhenEvaluator::expression_to_string(left),
                op_str,
                WhenEvaluator::expression_to_string(right)
            )
        }
        Expression::Unary { op, operand } => {
            format!("{:?} {}", op, WhenEvaluator::expression_to_string(operand))
        }
        Expression::FunctionCall { name, args } => {
            let args_str = args
                .iter()
                .map(|a| WhenEvaluator::expression_to_string(a))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", name, args_str)
        }
        Expression::Ternary { .. } => "?:".to_string(),
        Expression::LogicalGroup { op, .. } => {
            use corint_core::ast::LogicalGroupOp;
            match op {
                LogicalGroupOp::Any => "any:[...]".to_string(),
                LogicalGroupOp::All => "all:[...]".to_string(),
            }
        }
        Expression::ListReference { list_id } => {
            format!("list.{}", list_id)
        }
        Expression::ResultAccess { ruleset_id, field } => {
            match ruleset_id {
                Some(id) => format!("result.{}.{}", id, field),
                None => format!("result.{}", field),
            }
        }
    }
}

/// Convert Operator to string for tracing
#[allow(dead_code)]
pub(super) fn operator_to_string(op: &Operator) -> &'static str {
    match op {
        Operator::Eq => "==",
        Operator::Ne => "!=",
        Operator::Lt => "<",
        Operator::Gt => ">",
        Operator::Le => "<=",
        Operator::Ge => ">=",
        Operator::And => "&&",
        Operator::Or => "||",
        Operator::Add => "+",
        Operator::Sub => "-",
        Operator::Mul => "*",
        Operator::Div => "/",
        Operator::Mod => "%",
        Operator::In => "in",
        Operator::NotIn => "not in",
        Operator::Contains => "contains",
        Operator::StartsWith => "starts_with",
        Operator::EndsWith => "ends_with",
        Operator::Regex => "=~",
        Operator::InList => "in list",
        Operator::NotInList => "not in list",
    }
}

}
