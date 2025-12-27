//! Comparison operator execution

use crate::error::{Result, RuntimeError};
use corint_core::ast::Operator;
use corint_core::Value;

/// Execute a comparison operation
pub(crate) fn execute_compare(left: &Value, op: &Operator, right: &Value) -> Result<bool> {
    // Handle Null comparisons - Null compared to anything returns false
    // This allows rules to gracefully handle missing fields
    match (left, right) {
        (Value::Null, _) | (_, Value::Null) => {
            tracing::debug!(
                "Null comparison: {:?} {:?} {:?}, returning false",
                left,
                op,
                right
            );
            return Ok(false);
        }
        _ => {}
    }

    match (left, op, right) {
        (Value::Number(l), Operator::Eq, Value::Number(r)) => Ok(l == r),
        (Value::Number(l), Operator::Ne, Value::Number(r)) => Ok(l != r),
        (Value::Number(l), Operator::Gt, Value::Number(r)) => Ok(l > r),
        (Value::Number(l), Operator::Ge, Value::Number(r)) => Ok(l >= r),
        (Value::Number(l), Operator::Lt, Value::Number(r)) => Ok(l < r),
        (Value::Number(l), Operator::Le, Value::Number(r)) => Ok(l <= r),

        (Value::String(l), Operator::Eq, Value::String(r)) => Ok(l == r),
        (Value::String(l), Operator::Ne, Value::String(r)) => Ok(l != r),

        (Value::Bool(l), Operator::Eq, Value::Bool(r)) => Ok(l == r),
        (Value::Bool(l), Operator::Ne, Value::Bool(r)) => Ok(l != r),

        _ => Err(RuntimeError::InvalidOperation(format!(
            "Cannot compare {:?} and {:?} with {:?}",
            left, right, op
        ))),
    }
}
