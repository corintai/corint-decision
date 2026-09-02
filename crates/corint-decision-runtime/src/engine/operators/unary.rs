//! Unary operator execution

use crate::error::{Result, RuntimeError};
use corint_decision_model::ast::UnaryOperator;
use corint_decision_model::Value;

/// Execute a unary operation
pub(crate) fn execute_unary_op(operand: &Value, op: &UnaryOperator) -> Result<Value> {
    match (op, operand) {
        (UnaryOperator::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
        (UnaryOperator::Negate, Value::Number(n)) => Ok(Value::Number(-n)),
        _ => Err(RuntimeError::InvalidOperation(format!(
            "Cannot apply {:?} to {:?}",
            op, operand
        ))),
    }
}
