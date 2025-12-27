//! Binary operator execution

use crate::error::{Result, RuntimeError};
use corint_core::ast::Operator;
use corint_core::Value;

/// Execute a binary operation
pub(crate) fn execute_binary_op(left: &Value, op: &Operator, right: &Value) -> Result<Value> {
    // Handle Null operands - Null in any operation returns Null
    // This allows expressions with missing fields to propagate Null
    match (left, right) {
        (Value::Null, _) | (_, Value::Null) => {
            tracing::debug!(
                "Null in binary operation: {:?} {:?} {:?}, returning Null",
                left,
                op,
                right
            );
            return Ok(Value::Null);
        }
        _ => {}
    }

    match (left, op, right) {
        // Arithmetic operations
        (Value::Number(l), Operator::Add, Value::Number(r)) => Ok(Value::Number(l + r)),
        (Value::Number(l), Operator::Sub, Value::Number(r)) => Ok(Value::Number(l - r)),
        (Value::Number(l), Operator::Mul, Value::Number(r)) => Ok(Value::Number(l * r)),
        (Value::Number(l), Operator::Div, Value::Number(r)) => {
            if *r == 0.0 {
                Err(RuntimeError::DivisionByZero)
            } else {
                Ok(Value::Number(l / r))
            }
        }
        (Value::Number(l), Operator::Mod, Value::Number(r)) => {
            if *r == 0.0 {
                Err(RuntimeError::DivisionByZero)
            } else {
                Ok(Value::Number(l % r))
            }
        }

        // Logical operations
        (Value::Bool(l), Operator::And, Value::Bool(r)) => Ok(Value::Bool(*l && *r)),
        (Value::Bool(l), Operator::Or, Value::Bool(r)) => Ok(Value::Bool(*l || *r)),

        // String operations
        (Value::String(l), Operator::Contains, Value::String(r)) => {
            Ok(Value::Bool(l.contains(r)))
        }
        (Value::String(l), Operator::StartsWith, Value::String(r)) => {
            Ok(Value::Bool(l.starts_with(r)))
        }
        (Value::String(l), Operator::EndsWith, Value::String(r)) => {
            Ok(Value::Bool(l.ends_with(r)))
        }

        // Array operations
        (Value::Array(arr), Operator::Contains, val) => {
            Ok(Value::Bool(arr.iter().any(|v| v == val)))
        }

        // In operator
        (val, Operator::In, Value::Array(arr)) => Ok(Value::Bool(arr.iter().any(|v| v == val))),
        (val, Operator::NotIn, Value::Array(arr)) => {
            Ok(Value::Bool(!arr.iter().any(|v| v == val)))
        }

        _ => Err(RuntimeError::InvalidOperation(format!(
            "Cannot apply {:?} to {:?} and {:?}",
            op, left, right
        ))),
    }
}
