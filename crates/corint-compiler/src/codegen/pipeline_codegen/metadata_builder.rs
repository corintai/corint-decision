//! Pipeline metadata builder
//!
//! Builds metadata for compiled pipelines including step information for tracing.

use corint_core::ast::pipeline::{PipelineStep, StepDetails, StepNext};
use corint_core::ast::rule::{Condition, ConditionGroup};
use corint_core::ast::WhenBlock;

/// Build steps metadata JSON for tracing
pub(super) fn build_steps_metadata(steps: &[&PipelineStep]) -> String {
    let steps_info: Vec<serde_json::Value> = steps
        .iter()
        .map(|step| {
            let mut info = serde_json::json!({
                "id": step.id,
                "name": step.name,
                "type": step.step_type,
            });

            // Add routes if present
            if let Some(routes) = &step.routes {
                let routes_info: Vec<serde_json::Value> = routes
                    .iter()
                    .map(|route| {
                        let when_str = when_block_to_string(&route.when);
                        serde_json::json!({
                            "next": route.next,
                            "when": when_str
                        })
                    })
                    .collect();
                info["routes"] = serde_json::Value::Array(routes_info);
            }

            // Add default route if present
            if let Some(default) = &step.default {
                info["default"] = serde_json::Value::String(default.clone());
            }

            // Add next step if present
            if let Some(next) = &step.next {
                let StepNext::StepId(next_id) = next;
                info["next"] = serde_json::Value::String(next_id.clone());
            }

            // Add step-specific details
            match &step.details {
                StepDetails::Ruleset { ruleset } => {
                    info["ruleset"] = serde_json::Value::String(ruleset.clone());
                }
                StepDetails::Api {
                    api_target,
                    endpoint,
                    output,
                    ..
                } => {
                    use corint_core::ast::pipeline::ApiTarget;
                    let api_name = match api_target {
                        ApiTarget::Single { api } => api.clone(),
                        ApiTarget::Any { any } => format!("any:{}", any.join(",")),
                        ApiTarget::All { all } => format!("all:{}", all.join(",")),
                    };
                    info["api"] = serde_json::Value::String(api_name);
                    if let Some(ep) = endpoint {
                        info["endpoint"] = serde_json::Value::String(ep.clone());
                    }
                    if let Some(out) = output {
                        info["output"] = serde_json::Value::String(out.clone());
                    }
                }
                StepDetails::Service { service, query, output, .. } => {
                    info["service"] = serde_json::Value::String(service.clone());
                    if let Some(q) = query {
                        info["query"] = serde_json::Value::String(q.clone());
                    }
                    if let Some(out) = output {
                        info["output"] = serde_json::Value::String(out.clone());
                    }
                }
                StepDetails::Function { function, .. } => {
                    info["function"] = serde_json::Value::String(function.clone());
                }
                StepDetails::Rule { rule } => {
                    info["rule"] = serde_json::Value::String(rule.clone());
                }
                StepDetails::SubPipeline { pipeline_id } => {
                    info["sub_pipeline"] = serde_json::Value::String(pipeline_id.clone());
                }
                _ => {}
            }

            info
        })
        .collect();

    serde_json::to_string(&steps_info).unwrap_or_else(|_| "[]".to_string())
}

/// Convert WhenBlock to a human-readable string for tracing
pub(super) fn when_block_to_string(when: &WhenBlock) -> String {
    if let Some(ref group) = when.condition_group {
        condition_group_to_string(group)
    } else if let Some(ref conditions) = when.conditions {
        conditions
            .iter()
            .map(|expr| format!("{:?}", expr))
            .collect::<Vec<_>>()
            .join(" AND ")
    } else if let Some(ref event_type) = when.event_type {
        format!("event_type == '{}'", event_type)
    } else {
        "true".to_string()
    }
}

/// Convert ConditionGroup to string
fn condition_group_to_string(group: &ConditionGroup) -> String {
    match group {
        ConditionGroup::All(conditions) => {
            let parts: Vec<String> = conditions
                .iter()
                .map(|c| condition_to_string(c))
                .collect();
            if parts.len() == 1 {
                parts[0].clone()
            } else {
                format!("({})", parts.join(" AND "))
            }
        }
        ConditionGroup::Any(conditions) => {
            let parts: Vec<String> = conditions
                .iter()
                .map(|c| condition_to_string(c))
                .collect();
            if parts.len() == 1 {
                parts[0].clone()
            } else {
                format!("({})", parts.join(" OR "))
            }
        }
        ConditionGroup::Not(conditions) => {
            let parts: Vec<String> = conditions
                .iter()
                .map(|c| condition_to_string(c))
                .collect();
            format!("NOT ({})", parts.join(" AND "))
        }
    }
}

/// Convert Condition to string
fn condition_to_string(condition: &Condition) -> String {
    match condition {
        Condition::Expression(expr) => expression_to_string(expr),
        Condition::Group(group) => condition_group_to_string(group),
    }
}

/// Convert Expression to a human-readable string
fn expression_to_string(expr: &corint_core::ast::Expression) -> String {
    use corint_core::ast::{Expression, UnaryOperator};
    match expr {
        Expression::FieldAccess(path) => path.join("."),
        Expression::Literal(value) => value_to_readable_string(value),
        Expression::Binary { left, op, right } => {
            format!(
                "{} {} {}",
                expression_to_string(left),
                operator_to_symbol(op),
                expression_to_string(right)
            )
        }
        Expression::Unary { op, operand } => {
            let op_symbol = match op {
                UnaryOperator::Not => "!",
                UnaryOperator::Negate => "-",
            };
            format!("{}{}", op_symbol, expression_to_string(operand))
        }
        Expression::FunctionCall { name, args } => {
            let args_str: Vec<String> =
                args.iter().map(|a| expression_to_string(a)).collect();
            format!("{}({})", name, args_str.join(", "))
        }
        Expression::ListReference { list_id } => {
            format!("list.{}", list_id)
        }
        Expression::Ternary { condition, true_expr, false_expr } => {
            format!(
                "{} ? {} : {}",
                expression_to_string(condition),
                expression_to_string(true_expr),
                expression_to_string(false_expr)
            )
        }
        Expression::LogicalGroup { op, conditions } => {
            use corint_core::ast::LogicalGroupOp;
            let parts: Vec<String> = conditions
                .iter()
                .map(expression_to_string)
                .collect();
            let separator = match op {
                LogicalGroupOp::Any => " || ",
                LogicalGroupOp::All => " && ",
            };
            if parts.len() == 1 {
                parts[0].clone()
            } else {
                format!("({})", parts.join(separator))
            }
        }
        Expression::ResultAccess { ruleset_id, field } => {
            match ruleset_id {
                Some(id) => format!("result.{}.{}", id, field),
                None => format!("result.{}", field),
            }
        }
    }
}

/// Convert operator to readable symbol
fn operator_to_symbol(op: &corint_core::ast::Operator) -> &'static str {
    use corint_core::ast::Operator;
    match op {
        Operator::Eq => "==",
        Operator::Ne => "!=",
        Operator::Gt => ">",
        Operator::Ge => ">=",
        Operator::Lt => "<",
        Operator::Le => "<=",
        Operator::Add => "+",
        Operator::Sub => "-",
        Operator::Mul => "*",
        Operator::Div => "/",
        Operator::Mod => "%",
        Operator::And => "&&",
        Operator::Or => "||",
        Operator::Contains => "contains",
        Operator::StartsWith => "starts_with",
        Operator::EndsWith => "ends_with",
        Operator::Regex => "regex",
        Operator::In => "in",
        Operator::NotIn => "not in",
        Operator::InList => "in",
        Operator::NotInList => "not in",
    }
}

/// Convert Value to readable string
fn value_to_readable_string(value: &corint_core::Value) -> String {
    use corint_core::Value;
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            // Format numbers nicely (remove trailing .0 for integers)
            if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        Value::String(s) => format!("\"{}\"", s),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(value_to_readable_string).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            let pairs: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}: {}", k, value_to_readable_string(v)))
                .collect();
            format!("{{{}}}", pairs.join(", "))
        }
    }
}
