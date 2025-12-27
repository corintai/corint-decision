//! Trace building utilities for execution traces

use super::when_evaluator::WhenEvaluator;
use corint_core::ast::{Expression, WhenBlock};
use corint_core::Value;
use corint_runtime::{ConclusionTrace, ConditionTrace, StepTrace};
use std::collections::HashMap;

pub(super) struct TraceBuilder;

impl TraceBuilder {
pub(super) fn evaluate_expression_with_trace(
    expr: &Expression,
    event_data: &HashMap<String, Value>,
) -> (bool, ConditionTrace) {
    match expr {
        Expression::Literal(val) => {
            let result = WhenEvaluator::is_truthy(val);
            let trace = ConditionTrace::new(WhenEvaluator::expression_to_string(expr), result);
            (result, trace)
        }
        Expression::FieldAccess(path) => {
            let val = WhenEvaluator::get_field_value(event_data, path).unwrap_or(Value::Null);
            let result = WhenEvaluator::is_truthy(&val);
            let mut trace = ConditionTrace::new(path.join("."), result);
            trace.left_value = Some(val);
            (result, trace)
        }
        Expression::Binary { left, op, right } => {
            let left_val = WhenEvaluator::expression_to_value(left, event_data);
            let right_val = WhenEvaluator::expression_to_value(right, event_data);
            let result = WhenEvaluator::evaluate_binary_expression(left, op, right, event_data);

            let trace = ConditionTrace::binary(
                WhenEvaluator::expression_to_string(expr),
                left_val,
                WhenEvaluator::operator_to_string(op),
                right_val,
                result,
            );
            (result, trace)
        }
        Expression::LogicalGroup { op, conditions } => {
            use corint_core::ast::LogicalGroupOp;

            let mut nested_traces = Vec::new();
            let result = match op {
                LogicalGroupOp::Any => {
                    let mut any_true = false;
                    for cond in conditions {
                        let (r, t) = TraceBuilder::evaluate_expression_with_trace(cond, event_data);
                        nested_traces.push(t);
                        if r {
                            any_true = true;
                        }
                    }
                    any_true
                }
                LogicalGroupOp::All => {
                    let mut all_true = true;
                    for cond in conditions {
                        let (r, t) = TraceBuilder::evaluate_expression_with_trace(cond, event_data);
                        nested_traces.push(t);
                        if !r {
                            all_true = false;
                        }
                    }
                    all_true
                }
            };

            let group_type = match op {
                LogicalGroupOp::Any => "any",
                LogicalGroupOp::All => "all",
            };
            let trace = ConditionTrace::group(group_type, nested_traces, result);
            (result, trace)
        }
        _ => {
            // Unary, FunctionCall, Ternary - not fully supported
            let result = WhenEvaluator::evaluate_expression(expr, event_data);
            let trace = ConditionTrace::new(WhenEvaluator::expression_to_string(expr), result);
            (result, trace)
        }
    }
}

/// Evaluate a when block with tracing enabled
#[allow(dead_code)]
pub(super) fn evaluate_when_block_with_trace(
    when: &WhenBlock,
    event_data: &HashMap<String, Value>,
) -> (bool, Vec<ConditionTrace>) {
    let mut traces = Vec::new();

    // Check event_type if specified
    if let Some(ref expected_type) = when.event_type {
        let actual = event_data
            .get("type")
            .and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let matched = &actual == expected_type;

        traces.push(ConditionTrace::binary(
            format!("event.type == \"{}\"", expected_type),
            Value::String(actual),
            "==",
            Value::String(expected_type.clone()),
            matched,
        ));

        if !matched {
            return (false, traces);
        }
    }

    // Evaluate condition_group (new format: all/any/not)
    if let Some(ref condition_group) = when.condition_group {
        let (result, group_traces) =
            WhenEvaluator::evaluate_condition_group_with_trace(condition_group, event_data);
        traces.extend(group_traces);
        return (result, traces);
    }

    // Evaluate all conditions (legacy format - AND logic)
    if let Some(ref conditions) = when.conditions {
        for condition in conditions {
            let (result, trace) = TraceBuilder::evaluate_expression_with_trace(condition, event_data);
            traces.push(trace);
            if !result {
                return (false, traces);
            }
        }
    }

    (true, traces)
}

/// Convert conditions JSON string to Vec<ConditionTrace>
/// The JSON format is an array of structured condition objects from expression_to_json
pub(super) fn json_to_condition_traces(
    conditions_json: &str,
    triggered: bool,
    event_data: &HashMap<String, Value>,
) -> Vec<ConditionTrace> {
    // Parse the JSON string
    let conditions: Vec<serde_json::Value> = match serde_json::from_str(conditions_json) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    conditions
        .into_iter()
        .map(|cond| TraceBuilder::json_value_to_condition_trace(&cond, triggered, event_data))
        .collect()
}

/// Convert condition group JSON string to Vec<ConditionTrace>
/// The JSON format is a ConditionGroup enum: {"all": [...]} or {"any": [...]} or {"not": [...]}
pub(super) fn condition_group_json_to_traces(
    condition_group_json: &str,
    _triggered: bool,
    event_data: &HashMap<String, Value>,
) -> Vec<ConditionTrace> {
    // Parse the JSON string
    let group: serde_json::Value = match serde_json::from_str(condition_group_json) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    // Determine the group type and get conditions
    let (group_type, conditions) = if let Some(all_conditions) = group.get("all") {
        ("all", all_conditions.as_array())
    } else if let Some(any_conditions) = group.get("any") {
        ("any", any_conditions.as_array())
    } else if let Some(not_conditions) = group.get("not") {
        ("not", not_conditions.as_array())
    } else {
        return vec![];
    };

    let conditions = match conditions {
        Some(arr) => arr,
        None => return vec![],
    };

    // Convert each condition to a trace
    let nested: Vec<ConditionTrace> = conditions
        .iter()
        .map(|cond| TraceBuilder::condition_to_trace(cond, false, event_data))
        .collect();

    // Calculate actual group result based on nested results
    let group_result = match group_type {
        "all" => nested.iter().all(|c| c.result),
        "any" => nested.iter().any(|c| c.result),
        "not" => !nested.iter().all(|c| c.result),
        _ => false,
    };

    // Return a single group trace containing all nested conditions
    vec![ConditionTrace::group(group_type, nested, group_result)]
}

/// Convert a single condition (Expression or nested Group) to ConditionTrace
pub(super) fn condition_to_trace(
    cond: &serde_json::Value,
    _triggered: bool,
    event_data: &HashMap<String, Value>,
) -> ConditionTrace {
    // Check if it's a nested group (has "all", "any", or "not" key)
    if let Some(all_conditions) = cond.get("all") {
        if let Some(arr) = all_conditions.as_array() {
            let nested: Vec<ConditionTrace> = arr
                .iter()
                .map(|c| TraceBuilder::condition_to_trace(c, false, event_data))
                .collect();
            // "all" is true only if all nested conditions are true
            let group_result = nested.iter().all(|c| c.result);
            return ConditionTrace::group("all", nested, group_result);
        }
    }
    if let Some(any_conditions) = cond.get("any") {
        if let Some(arr) = any_conditions.as_array() {
            let nested: Vec<ConditionTrace> = arr
                .iter()
                .map(|c| TraceBuilder::condition_to_trace(c, false, event_data))
                .collect();
            // "any" is true if any nested condition is true
            let group_result = nested.iter().any(|c| c.result);
            return ConditionTrace::group("any", nested, group_result);
        }
    }
    if let Some(not_conditions) = cond.get("not") {
        if let Some(arr) = not_conditions.as_array() {
            let nested: Vec<ConditionTrace> = arr
                .iter()
                .map(|c| TraceBuilder::condition_to_trace(c, false, event_data))
                .collect();
            // "not" is true if all nested conditions are false (negation of "all")
            let group_result = !nested.iter().all(|c| c.result);
            return ConditionTrace::group("not", nested, group_result);
        }
    }

    // It's an expression - convert to string representation
    TraceBuilder::expression_json_to_trace(cond, false, event_data)
}

/// Convert an expression JSON to ConditionTrace
fn expression_json_to_trace(
    expr: &serde_json::Value,
    _triggered: bool,
    event_data: &HashMap<String, Value>,
) -> ConditionTrace {
    // Try to build a human-readable expression string
    let expression = TraceBuilder::expr_json_to_string(expr);

    // Default result - will be calculated for Binary expressions
    let mut result = false;
    let mut left_value_for_display: Option<Value> = None;
    let mut right_value_for_display: Option<Value> = None;

    // For Binary expressions, extract left/right values and calculate actual result
    if let Some(binary) = expr.get("Binary") {
        if let Some(obj) = binary.as_object() {
            let left_expr = obj.get("left");
            let right_expr = obj.get("right");
            let op = obj.get("op").and_then(|o| o.as_str()).unwrap_or("");

            // Extract actual values for evaluation
            let left_val = left_expr.and_then(|e| TraceBuilder::extract_value_from_expr_json(e, event_data));
            let right_val = right_expr.and_then(|e| TraceBuilder::extract_value_from_expr_json(e, event_data));

            // Set display values (only for non-constants)
            // Always show the value for FieldAccess, even if null (to indicate field not found)
            if let Some(left_e) = left_expr {
                if TraceBuilder::should_display_value(left_e) {
                    left_value_for_display = Some(left_val.clone().unwrap_or(Value::Null));
                }
            }
            if let Some(right_e) = right_expr {
                if TraceBuilder::should_display_value(right_e) {
                    right_value_for_display = Some(right_val.clone().unwrap_or(Value::Null));
                }
            }

            // Calculate actual result
            if let (Some(lv), Some(rv)) = (&left_val, &right_val) {
                result = TraceBuilder::evaluate_comparison(lv, op, rv);
            }
        }
    }

    let mut trace = ConditionTrace::new(expression, result);
    trace.left_value = left_value_for_display;
    trace.right_value = right_value_for_display;
    trace
}

/// Evaluate a comparison between two values
pub(super) fn evaluate_comparison(left: &Value, op: &str, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(l), Value::Number(r)) => {
            match op {
                // Enum-style names
                "Gt" | ">" => l > r,
                "Ge" | ">=" => l >= r,
                "Lt" | "<" => l < r,
                "Le" | "<=" => l <= r,
                "Eq" | "==" => (l - r).abs() < f64::EPSILON,
                "Ne" | "!=" => (l - r).abs() >= f64::EPSILON,
                _ => false,
            }
        }
        (Value::String(l), Value::String(r)) => {
            match op {
                "Eq" | "==" => l == r,
                "Ne" | "!=" => l != r,
                "Gt" | ">" => l > r,
                "Ge" | ">=" => l >= r,
                "Lt" | "<" => l < r,
                "Le" | "<=" => l <= r,
                "Contains" | "contains" => l.contains(r.as_str()),
                "StartsWith" | "starts_with" => l.starts_with(r.as_str()),
                "EndsWith" | "ends_with" => l.ends_with(r.as_str()),
                _ => false,
            }
        }
        (Value::Bool(l), Value::Bool(r)) => {
            match op {
                "Eq" | "==" => l == r,
                "Ne" | "!=" => l != r,
                "And" | "&&" => *l && *r,
                "Or" | "||" => *l || *r,
                _ => false,
            }
        }
        // Handle cross-type comparisons for equality
        _ => {
            match op {
                "Eq" | "==" => left == right,
                "Ne" | "!=" => left != right,
                _ => false,
            }
        }
    }
}

/// Check if an expression's value should be displayed in trace
/// Returns true for FieldAccess and complex expressions, false for Literal, ListReference, and boolean values
pub(super) fn should_display_value(expr: &serde_json::Value) -> bool {
    // Skip Literal values (constants)
    if let Some(literal) = expr.get("Literal") {
        // Also skip boolean literals
        if literal.is_boolean() {
            return false;
        }
        return false;
    }

    // Skip ListReference (lists)
    if expr.get("ListReference").is_some() {
        return false;
    }

    // Skip direct boolean values
    if expr.is_boolean() {
        return false;
    }

    // FieldAccess should be displayed
    if expr.get("FieldAccess").is_some() {
        return true;
    }

    // FunctionCall should be displayed
    if expr.get("FunctionCall").is_some() {
        return true;
    }

    // Binary expressions (complex calculations) should be displayed
    if expr.get("Binary").is_some() {
        return true;
    }

    // Unary expressions should be displayed
    if expr.get("Unary").is_some() {
        return true;
    }

    false
}

/// Extract actual value from expression JSON (new format with Binary/FieldAccess keys)
pub(super) fn extract_value_from_expr_json(
    expr: &serde_json::Value,
    event_data: &HashMap<String, Value>,
) -> Option<Value> {
    // Handle FieldAccess: {"FieldAccess": ["event", "transaction", "amount"]} or {"FieldAccess": ["features", "transaction_sum_7d"]}
    if let Some(field_access) = expr.get("FieldAccess") {
        if let Some(arr) = field_access.as_array() {
            let path: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
            // Strip namespace prefixes since event_data contains merged data
            // Field paths may be:
            // - ["event", "transaction", "amount"] - strip "event" prefix
            // - ["features", "transaction_sum_7d"] - strip "features" prefix
            // - ["api", "device_fingerprint", "score"] - strip "api" prefix
            let effective_path: Vec<String> =
                if let Some(first) = path.first() {
                    match first.as_str() {
                        // Known namespaces - strip the prefix since trace_data is a flat merge
                        "event" | "features" | "api" | "service" | "llm" | "vars" => {
                            path.into_iter().skip(1).collect()
                        }
                        _ => path,
                    }
                } else {
                    path
                };
            let result = WhenEvaluator::get_field_value(event_data, &effective_path);
            if result.is_none() {
                tracing::debug!("Failed to get field value for path {:?}, event_data keys: {:?}", effective_path, event_data.keys().collect::<Vec<_>>());
            }
            return result;
        }
    }

    // Handle Literal: {"Literal": 10000.0} or nested like {"Literal": {"Number": 70}}
    if let Some(literal) = expr.get("Literal") {
        // Try direct conversion first
        if let Some(val) = TraceBuilder::json_to_core_value(literal) {
            return Some(val);
        }
        // Handle nested Value format: {"Literal": {"Number": 70}}
        if let Some(obj) = literal.as_object() {
            if let Some(num) = obj.get("Number") {
                return num.as_f64().map(Value::Number);
            }
            if let Some(s) = obj.get("String") {
                return s.as_str().map(|s| Value::String(s.to_string()));
            }
            if let Some(b) = obj.get("Bool") {
                return b.as_bool().map(Value::Bool);
            }
            if obj.contains_key("Null") {
                return Some(Value::Null);
            }
        }
        return None;
    }

    // Handle Binary expression (calculate the result)
    if let Some(binary) = expr.get("Binary") {
        if let Some(obj) = binary.as_object() {
            let left = obj.get("left")?;
            let right = obj.get("right")?;
            let op = obj.get("op").and_then(|o| o.as_str())?;

            let left_val = TraceBuilder::extract_value_from_expr_json(left, event_data)?;
            let right_val = TraceBuilder::extract_value_from_expr_json(right, event_data)?;

            // Handle numeric operations
            if let (Value::Number(l), Value::Number(r)) = (&left_val, &right_val) {
                let result = match op {
                    "Add" => l + r,
                    "Sub" => l - r,
                    "Mul" => l * r,
                    "Div" => if *r != 0.0 { l / r } else { return None },
                    "Mod" => l % r,
                    _ => return None,
                };
                return Some(Value::Number(result));
            }
        }
    }

    // Handle FunctionCall - cannot extract value without runtime context
    if expr.get("FunctionCall").is_some() {
        return None;
    }

    None
}

/// Convert expression JSON to readable string
pub(super) fn expr_json_to_string(expr: &serde_json::Value) -> String {
    // Check for Binary expression: {"Binary": {"left": ..., "op": "Gt", "right": ...}}
    if let Some(binary) = expr.get("Binary") {
        if let Some(obj) = binary.as_object() {
            let left = obj.get("left").map(|l| TraceBuilder::expr_json_to_string(l)).unwrap_or_default();
            let op = obj.get("op").map(|o| TraceBuilder::operator_to_symbol(o)).unwrap_or("?".to_string());
            let right = obj.get("right").map(|r| TraceBuilder::expr_json_to_string(r)).unwrap_or_default();
            return format!("{} {} {}", left, op, right);
        }
    }

    // Check for FieldAccess: {"FieldAccess": ["event", "transaction", "amount"]}
    if let Some(field_access) = expr.get("FieldAccess") {
        if let Some(arr) = field_access.as_array() {
            let parts: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
            return parts.join(".");
        }
    }

    // Check for Literal: {"Literal": 10000.0} or {"Literal": "USD"} or {"Literal": true}
    if let Some(literal) = expr.get("Literal") {
        return TraceBuilder::value_to_string(literal);
    }

    // Check for Unary: {"Unary": {"op": "Not", "operand": ...}}
    if let Some(unary) = expr.get("Unary") {
        if let Some(obj) = unary.as_object() {
            let op = obj.get("op").and_then(|o| o.as_str()).unwrap_or("!");
            let operand = obj.get("operand").map(|o| TraceBuilder::expr_json_to_string(o)).unwrap_or_default();
            let op_symbol = if op == "Not" { "!" } else if op == "Negate" { "-" } else { op };
            return format!("{}{}", op_symbol, operand);
        }
    }

    // Check for ListReference: {"ListReference": {"list_id": "..."}}
    if let Some(list_ref) = expr.get("ListReference") {
        if let Some(obj) = list_ref.as_object() {
            if let Some(list_id) = obj.get("list_id").and_then(|v| v.as_str()) {
                return format!("list.{}", list_id);
            }
        }
    }

    // Check for FunctionCall: {"FunctionCall": {"name": "count", "args": [...]}}
    if let Some(func) = expr.get("FunctionCall") {
        if let Some(obj) = func.as_object() {
            let name = obj.get("name").and_then(|n| n.as_str()).unwrap_or("func");
            let args = obj.get("args")
                .and_then(|a| a.as_array())
                .map(|arr| arr.iter().map(|a| TraceBuilder::expr_json_to_string(a)).collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            return format!("{}({})", name, args);
        }
    }

    // Fallback: try to format nicely
    if let Some(s) = expr.as_str() {
        return s.to_string();
    }
    if let Some(n) = expr.as_f64() {
        return format!("{}", n);
    }
    if let Some(b) = expr.as_bool() {
        return format!("{}", b);
    }
    if expr.is_null() {
        return "null".to_string();
    }

    // Last resort: serialize as JSON
    expr.to_string()
}

/// Convert operator JSON to readable symbol
pub(super) fn operator_to_symbol(op: &serde_json::Value) -> String {
    let op_str = op.as_str().unwrap_or("");
    match op_str {
        "Eq" => "==".to_string(),
        "Ne" => "!=".to_string(),
        "Gt" => ">".to_string(),
        "Ge" => ">=".to_string(),
        "Lt" => "<".to_string(),
        "Le" => "<=".to_string(),
        "Add" => "+".to_string(),
        "Sub" => "-".to_string(),
        "Mul" => "*".to_string(),
        "Div" => "/".to_string(),
        "Mod" => "%".to_string(),
        "And" => "&&".to_string(),
        "Or" => "||".to_string(),
        "Contains" => "contains".to_string(),
        "StartsWith" => "starts_with".to_string(),
        "EndsWith" => "ends_with".to_string(),
        "Regex" => "regex".to_string(),
        "In" => "in".to_string(),
        "NotIn" => "not in".to_string(),
        "InList" => "in".to_string(),
        "NotInList" => "not in".to_string(),
        _ => op_str.to_string(),
    }
}

/// Convert JSON value to readable string representation
pub(super) fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => format!("{}", b),
        serde_json::Value::Number(n) => format!("{}", n),
        serde_json::Value::String(s) => format!("\"{}\"", s),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(TraceBuilder::value_to_string).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(obj) => {
            // Check if it's a Value enum variant
            if let Some(v) = obj.get("String") {
                return format!("\"{}\"", v.as_str().unwrap_or(""));
            }
            if let Some(v) = obj.get("Number") {
                return format!("{}", v);
            }
            if let Some(v) = obj.get("Bool") {
                return format!("{}", v);
            }
            if obj.contains_key("Null") {
                return "null".to_string();
            }
            // Default: just serialize
            format!("{}", value)
        }
    }
}

/// Convert a single JSON value to ConditionTrace
pub(super) fn json_value_to_condition_trace(
    json: &serde_json::Value,
    _triggered: bool,
    event_data: &HashMap<String, Value>,
) -> ConditionTrace {
    let expr_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let expression = json
        .get("expression")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    match expr_type {
        "group" => {
            // Logical group (any/all)
            let group_type = json
                .get("group_type")
                .and_then(|v| v.as_str())
                .unwrap_or("all");
            let nested_conditions: Vec<ConditionTrace> = json
                .get("conditions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|c| TraceBuilder::json_value_to_condition_trace(c, false, event_data))
                        .collect()
                })
                .unwrap_or_default();

            // Calculate actual result based on nested results
            let result = match group_type {
                "all" => nested_conditions.iter().all(|c| c.result),
                "any" => nested_conditions.iter().any(|c| c.result),
                "not" => !nested_conditions.iter().all(|c| c.result),
                _ => false,
            };

            ConditionTrace::group(group_type, nested_conditions, result)
        }
        "binary" => {
            // Check right side type
            let right_json = json.get("right");
            let right_type = right_json
                .and_then(|r| r.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Check if right side is a boolean literal - if so, skip both left and right values
            let is_boolean_literal = right_type == "literal"
                && right_json
                    .and_then(|r| r.get("value"))
                    .map(|v| v.is_boolean())
                    .unwrap_or(false);

            // Check if right side is a simple literal (not boolean)
            let is_simple_literal = right_type == "literal" && !is_boolean_literal;

            // Extract left value from event data
            let left_val = json.get("left")
                .and_then(|left| TraceBuilder::extract_value_from_json_expr(left, event_data));

            // Extract right value
            let right_val = right_json.and_then(|right| {
                TraceBuilder::extract_value_from_json_expr(right, event_data)
            });

            // Calculate the actual result
            let operator = json.get("operator").and_then(|v| v.as_str()).unwrap_or("");
            let result = if let (Some(ref lv), Some(ref rv)) = (&left_val, &right_val) {
                TraceBuilder::evaluate_comparison(lv, operator, rv)
            } else {
                false
            };

            // For display: skip values for boolean literals and simple literals
            let left_value_display = if is_boolean_literal {
                None
            } else {
                left_val
            };

            let right_value_display = if is_boolean_literal || is_simple_literal {
                None
            } else {
                right_val
            };

            ConditionTrace {
                expression,
                left_value: left_value_display,
                operator: None, // Operator is already visible in expression string
                right_value: right_value_display,
                result,
                nested: None,
                group_type: None,
            }
        }
        _ => {
            // Literal, field access, or other - evaluate to determine result
            let result = if let Some(val) = TraceBuilder::extract_value_from_json_expr(json, event_data) {
                WhenEvaluator::is_truthy(&val)
            } else {
                false
            };
            ConditionTrace::new(expression, result)
        }
    }
}

/// Extract the actual value from a JSON expression (field access, literal, or binary)
pub(super) fn extract_value_from_json_expr(
    json: &serde_json::Value,
    event_data: &HashMap<String, Value>,
) -> Option<Value> {
    let expr_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match expr_type {
        "field" => {
            // Field access - extract path and look up value
            let path: Vec<String> = json
                .get("path")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            // Handle namespace prefixes
            // event_data contains both event data and computed features/vars merged together
            // Field paths may be:
            // - ["event", "transaction", "amount"] - strip "event" prefix
            // - ["features", "transaction_sum_7d"] - strip "features" prefix (features are flat in trace_data)
            // - ["api", "device_fingerprint", "score"] - strip "api" prefix
            // - ["user_id"] - no prefix, look up directly
            let effective_path: Vec<String> =
                if let Some(first) = path.first() {
                    match first.as_str() {
                        // Known namespaces - strip the prefix since trace_data is a flat merge
                        "event" | "features" | "api" | "service" | "llm" | "vars" => {
                            path.into_iter().skip(1).collect()
                        }
                        _ => path,
                    }
                } else {
                    path
                };

            WhenEvaluator::get_field_value(event_data, &effective_path)
        }
        "literal" => {
            // Literal value - convert from serde_json::Value to corint_core::Value
            json.get("value").and_then(TraceBuilder::json_to_core_value)
        }
        "binary" => {
            // Binary expression (e.g., event.user.average_transaction * 3)
            // Recursively evaluate left and right, then apply operator
            let left = json.get("left")?;
            let right = json.get("right")?;
            let operator = json.get("operator").and_then(|v| v.as_str())?;

            let left_val = TraceBuilder::extract_value_from_json_expr(left, event_data)?;
            let right_val = TraceBuilder::extract_value_from_json_expr(right, event_data)?;

            // Only handle numeric operations for now
            if let (Value::Number(l), Value::Number(r)) = (&left_val, &right_val) {
                let result = match operator {
                    "+" | "Add" => l + r,
                    "-" | "Sub" => l - r,
                    "*" | "Mul" => l * r,
                    "/" | "Div" => {
                        if *r != 0.0 {
                            l / r
                        } else {
                            return None;
                        }
                    }
                    "%" | "Mod" => {
                        if *r != 0.0 {
                            l % r
                        } else {
                            return None;
                        }
                    }
                    _ => return None, // Comparison operators return bool, skip
                };
                Some(Value::Number(result))
            } else {
                // For string concatenation
                if operator == "+" || operator == "Add" {
                    if let (Value::String(l), Value::String(r)) = (&left_val, &right_val) {
                        return Some(Value::String(format!("{}{}", l, r)));
                    }
                }
                None
            }
        }
        _ => None,
    }
}

/// Convert serde_json::Value to corint_core::Value
pub(super) fn json_to_core_value(json: &serde_json::Value) -> Option<Value> {
    match json {
        serde_json::Value::Null => Some(Value::Null),
        serde_json::Value::Bool(b) => Some(Value::Bool(*b)),
        serde_json::Value::Number(n) => n.as_f64().map(Value::Number),
        serde_json::Value::String(s) => Some(Value::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let values: Vec<Value> = arr.iter().filter_map(TraceBuilder::json_to_core_value).collect();
            Some(Value::Array(values))
        }
        serde_json::Value::Object(obj) => {
            let map: HashMap<String, Value> = obj
                .iter()
                .filter_map(|(k, v)| TraceBuilder::json_to_core_value(v).map(|val| (k.clone(), val)))
                .collect();
            Some(Value::Object(map))
        }
    }
}

/// Build decision_logic traces from JSON
pub(super) fn build_decision_logic_traces(
    decision_logic_json: &str,
    matched_action: Option<&str>,
    total_score: i32,
    _event_data: &HashMap<String, Value>,
) -> Vec<ConclusionTrace> {
    let mut traces = Vec::new();

    // Parse the decision_logic JSON
    let decision_rules: Vec<serde_json::Value> = match serde_json::from_str(decision_logic_json) {
        Ok(rules) => rules,
        Err(e) => {
            tracing::warn!("Failed to parse decision_logic_json: {}", e);
            return traces;
        }
    };

    let mut matched_found = false;

    for rule in decision_rules {
        let is_default = rule.get("default").and_then(|v| v.as_bool()).unwrap_or(false);
        let condition = rule.get("condition").and_then(|v| v.as_str()).map(|s| s.to_string());
        // Read signal from JSON (compiled format uses "signal" field with uppercase values)
        let signal_upper = rule.get("signal").and_then(|v| v.as_str()).map(|s| s.to_string());
        let reason = rule.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());

        // Normalize signal to lowercase for comparison
        let signal_lower = signal_upper.as_ref().map(|s| s.to_lowercase());

        // Build condition string for display
        let condition_str = if is_default {
            "default".to_string()
        } else {
            condition.clone().unwrap_or_else(|| "unknown".to_string())
        };

        // Determine if this rule matched
        // A rule is matched if:
        // 1. matched_action matches this rule's signal AND we haven't found a match yet
        // 2. This is a default rule and no previous rule matched
        let matched = if !matched_found {
            if let Some(ref rule_signal) = signal_lower {
                if matched_action == Some(rule_signal.as_str()) {
                    // This could be the matched rule - check condition if present
                    if is_default {
                        // Default rule matches if we reach it
                        true
                    } else if let Some(ref _cond) = condition {
                        // Try to evaluate the condition (simplified - just check if signal matches)
                        // In practice, we trust the signal match since the VM already evaluated it
                        true
                    } else {
                        true
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if matched {
            matched_found = true;
        }

        let mut trace = ConclusionTrace::new(condition_str, matched);
        // Use signal_lower (normalized) for trace
        trace.signal = signal_lower;
        trace.reason = reason;
        // Add total_score for matched conclusion rules
        if matched {
            trace.total_score = Some(total_score);
        }

        traces.push(trace);

        // If this rule matched and has terminate, stop processing
        if matched {
            let terminate = rule.get("terminate").and_then(|v| v.as_bool()).unwrap_or(false);
            if terminate {
                break;
            }
        }
    }

    traces
}

/// Build step traces from the steps_json metadata and executed steps list
pub(super) fn build_step_traces_from_json(
    steps_json_str: &str,
    execution_variables: &HashMap<String, Value>,
) -> Vec<StepTrace> {
    let mut step_traces = Vec::new();

    // Parse the steps JSON (step definitions from compilation)
    let steps: Vec<serde_json::Value> = match serde_json::from_str(steps_json_str) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Failed to parse steps_json: {}", e);
            return step_traces;
        }
    };

    // Parse the executed steps from runtime (actual execution path)
    let executed_steps: Vec<serde_json::Value> = execution_variables
        .get("__executed_steps__")
        .and_then(|v| {
            if let Value::Array(arr) = v {
                Some(
                    arr.iter()
                        .filter_map(|item| {
                            if let Value::String(s) = item {
                                serde_json::from_str(s).ok()
                            } else {
                                None
                            }
                        })
                        .collect(),
                )
            } else {
                None
            }
        })
        .unwrap_or_default();

    // Build a map of step_id -> execution info for quick lookup
    let executed_map: HashMap<String, &serde_json::Value> = executed_steps
        .iter()
        .filter_map(|exec| {
            exec.get("step_id")
                .and_then(|v| v.as_str())
                .map(|id| (id.to_string(), exec))
        })
        .collect();

    for step_json in steps {
        let step_id = step_json
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let step_type = step_json
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let step_name = step_json
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut step_trace = StepTrace::new(step_id.clone(), step_type.clone());

        if let Some(name) = step_name {
            step_trace = step_trace.with_name(name);
        }

        // Add ruleset ID if present
        if let Some(ruleset_id) = step_json.get("ruleset").and_then(|v| v.as_str()) {
            step_trace = step_trace.with_ruleset(ruleset_id.to_string());
        }

        // Check if this step was actually executed and get execution details
        if let Some(exec_info) = executed_map.get(&step_id) {
            step_trace = step_trace.mark_executed();

            // Get next_step_id from execution info
            if let Some(next_step) = exec_info.get("next_step_id").and_then(|v| v.as_str()) {
                step_trace = step_trace.with_next_step(next_step.to_string());
            }

            // Get route info for router steps - add condition trace for the matched route
            if let Some(route_idx) = exec_info.get("route_index").and_then(|v| v.as_u64()) {
                // Get the route condition from step definition
                let route_condition = step_json
                    .get("routes")
                    .and_then(|v| v.as_array())
                    .and_then(|routes| routes.get(route_idx as usize))
                    .and_then(|route| route.get("when"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Add condition trace for the matched route
                if let Some(cond) = route_condition {
                    let condition_trace = ConditionTrace::new(cond, true);
                    step_trace = step_trace.add_condition(condition_trace);
                }
            }

            // Check if default route was taken
            if exec_info
                .get("is_default_route")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                step_trace = step_trace.with_default_route();
            }
        } else {
            // Step was not executed - use step definition's next for reference
            if let Some(next) = step_json.get("next").and_then(|v| v.as_str()) {
                step_trace = step_trace.with_next_step(next.to_string());
            }
        }

        step_traces.push(step_trace);
    }

    step_traces
}

/// Create a rule execution record
pub(super) fn create_rule_execution_record(
    request_id: &str,
    ruleset_id: Option<&str>,
    rule_id: &str,
    rule_name: Option<&str>,
    triggered: bool,
    score: i32,
    execution_time_ms: u64,
    rule_conditions: Option<String>,
    conditions_json: Option<String>,
    condition_group_json: Option<String>,
) -> corint_runtime::RuleExecutionRecord {
    // Convert conditions string to JSON value for storage
    let rule_conditions_json = rule_conditions.map(|s| serde_json::Value::String(s));

    corint_runtime::RuleExecutionRecord {
        request_id: request_id.to_string(),
        ruleset_id: ruleset_id.map(|s| s.to_string()),
        rule_id: rule_id.to_string(),
        rule_name: rule_name.map(|s| s.to_string()),
        triggered,
        score: if triggered { Some(score) } else { None },
        execution_time_ms: Some(execution_time_ms),
        feature_values: None,
        rule_conditions: rule_conditions_json,
        conditions_json,
        condition_group_json,
    }
}

}
