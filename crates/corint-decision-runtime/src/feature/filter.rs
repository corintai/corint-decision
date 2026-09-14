//! One bounded parser shared by publication validation and SQL execution.
use super::definition::WhenCondition;
use crate::datasource::query::{Filter, FilterOperator};
use anyhow::{bail, ensure, Result};
use corint_decision_model::{
    ast::{Expression, Operator, UnaryOperator},
    condition::ConditionParser,
    Value,
};
use std::collections::HashMap;

pub(crate) fn parse(
    when: &WhenCondition,
    context: Option<&HashMap<String, Value>>,
) -> Result<Vec<Filter>> {
    fn literal(node: Expression) -> Result<Value> {
        match node {
            Expression::Literal(value) => Ok(value),
            Expression::Unary {
                op: UnaryOperator::Negate,
                operand,
            } => match literal(*operand)? {
                Value::Number(n) if n.is_finite() => Ok(Value::Number(-n)),
                _ => bail!("Expected finite numeric literal"),
            },
            _ => bail!("Filter right operand must be a literal or whole-value template"),
        }
    }
    fn visit(
        node: Expression,
        context: Option<&HashMap<String, Value>>,
        out: &mut Vec<Filter>,
    ) -> Result<()> {
        let Expression::Binary { left, op, right } = node else {
            bail!("Expected a filter predicate")
        };
        if op == Operator::And {
            visit(*left, context, out)?;
            return visit(*right, context, out);
        }
        let Expression::FieldAccess(path) = *left else {
            bail!("Filter left operand must be a column")
        };
        ensure!(
            path.len() == 1
                && path[0].bytes().enumerate().all(|(i, b)| b == b'_'
                    || b.is_ascii_alphabetic()
                    || i > 0 && b.is_ascii_digit()),
            "Filter requires a simple SQL column name"
        );
        let operator = match op {
            Operator::Eq => FilterOperator::Eq,
            Operator::Ne => FilterOperator::Ne,
            Operator::Gt => FilterOperator::Gt,
            Operator::Ge => FilterOperator::Ge,
            Operator::Lt => FilterOperator::Lt,
            Operator::Le => FilterOperator::Le,
            Operator::In => FilterOperator::In,
            Operator::NotIn => FilterOperator::NotIn,
            Operator::Contains => FilterOperator::Contains,
            Operator::StartsWith => FilterOperator::StartsWith,
            Operator::EndsWith => FilterOperator::EndsWith,
            _ => bail!(
                "Unsupported feature filter operator; use conjunctions of supported predicates"
            ),
        };
        let mut value = literal(*right)?;
        let template =
            matches!(&value, Value::String(s) if s.starts_with("${") || s.starts_with('{'));
        ensure!(
            !matches!(&value, Value::Number(n) if !n.is_finite()),
            "Filter number must be finite"
        );
        ensure!(
            !matches!(&value, Value::String(s) if s.contains("${") && !template),
            "Filter templates must occupy a whole value"
        );
        if let Value::Array(values) = &value {
            ensure!(
                !values.iter().any(
                    |v| matches!(v, Value::String(s) if s.contains("${") || s.starts_with('{'))
                ),
                "Templates inside IN literals are unsupported"
            );
        }
        if template {
            ensure!(
                !matches!(operator, FilterOperator::In | FilterOperator::NotIn),
                "IN requires literal array operands"
            );
            let Value::String(text) = &value else {
                unreachable!()
            };
            let parser = ConditionParser::with_context(context.cloned().unwrap_or_default());
            let parsed = parser.parse_value(text)?;
            if context.is_some() {
                value = parsed
                    .try_to_value()
                    .ok_or_else(|| anyhow::anyhow!("Unresolved filter template"))?;
            }
        }
        if !template || context.is_some() {
            match operator {
                FilterOperator::In | FilterOperator::NotIn => ensure!(
                    matches!(&value, Value::Array(a) if a.iter().all(|v| matches!(v, Value::Null | Value::String(_) | Value::Bool(_)) || matches!(v, Value::Number(n) if n.is_finite()))),
                    "IN requires an array of scalar literals"
                ),
                FilterOperator::Contains
                | FilterOperator::StartsWith
                | FilterOperator::EndsWith => ensure!(
                    matches!(value, Value::String(_)),
                    "String predicate requires a string"
                ),
                FilterOperator::Eq | FilterOperator::Ne => ensure!(
                    !matches!(value, Value::Array(_) | Value::Object(_)),
                    "Equality requires a scalar"
                ),
                _ => ensure!(
                    matches!(&value, Value::Number(n) if n.is_finite())
                        || matches!(value, Value::String(_)),
                    "Ordered comparison requires a non-null number or string"
                ),
            }
        }
        out.push(Filter {
            field: path[0].clone(),
            operator,
            value,
        });
        ensure!(out.len() <= 64, "At most 64 filter predicates");
        Ok(())
    }
    let mut out = Vec::new();
    for condition in when.conditions().map_err(anyhow::Error::msg)? {
        ensure!(condition.len() <= 8192, "Filter expression too large");
        visit(
            corint_decision_dsl_parser::ExpressionParser::parse(&logical_words(condition))?,
            context,
            &mut out,
        )?;
    }
    Ok(out)
}

// Normalize word conjunctions only outside quoted literals. The shared lexer
// retains ownership of escaping, precedence, trailing tokens and AST bounds.
fn logical_words(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' || ch == '$' && chars.peek() == Some(&'{') {
            let mut template = String::from(ch);
            for c in chars.by_ref() {
                template.push(c);
                if c == '}' {
                    break;
                }
            }
            out.push_str(&serde_json::to_string(&template).expect("template string"));
        } else if ch == '\'' || ch == '"' {
            out.push(ch);
            while let Some(c) = chars.next() {
                out.push(c);
                if c == '\\' {
                    if let Some(next) = chars.next() {
                        out.push(next);
                    }
                } else if c == ch {
                    break;
                }
            }
        } else if ch.is_alphabetic() || ch == '_' {
            let mut word = String::from(ch);
            while chars
                .peek()
                .is_some_and(|c| c.is_alphanumeric() || *c == '_')
            {
                word.push(chars.next().unwrap());
            }
            out.push_str(match word.as_str() {
                "and" => "&&",
                "or" => "||",
                _ => &word,
            });
        } else {
            out.push(ch);
        }
    }
    out
}
