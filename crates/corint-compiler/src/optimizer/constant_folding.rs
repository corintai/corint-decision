//! Constant folding optimizer
//!
//! Evaluates constant expressions at compile time to improve runtime performance.

use corint_core::ast::{Expression, Operator, UnaryOperator};
use corint_core::Value;

/// Constant folding optimizer
pub struct ConstantFolder;

impl ConstantFolder {
    /// Create a new constant folder
    pub fn new() -> Self {
        Self
    }

    /// Optimize an expression by folding constants
    pub fn fold(&self, expr: &Expression) -> Expression {
        match expr {
            Expression::Literal(_) => expr.clone(),

            Expression::FieldAccess(_) => expr.clone(),

            Expression::Binary { left, op, right } => {
                let left_folded = self.fold(left);
                let right_folded = self.fold(right);

                // Try to fold if both operands are literals
                if let (Expression::Literal(left_val), Expression::Literal(right_val)) =
                    (&left_folded, &right_folded)
                {
                    if let Some(result) = self.fold_binary_op(left_val, op, right_val) {
                        return Expression::Literal(result);
                    }
                }

                Expression::Binary {
                    left: Box::new(left_folded),
                    op: *op,
                    right: Box::new(right_folded),
                }
            }

            Expression::Unary { op, operand } => {
                let operand_folded = self.fold(operand);

                // Try to fold if operand is a literal
                if let Expression::Literal(val) = &operand_folded {
                    if let Some(result) = self.fold_unary_op(op, val) {
                        return Expression::Literal(result);
                    }
                }

                Expression::Unary {
                    op: *op,
                    operand: Box::new(operand_folded),
                }
            }

            Expression::FunctionCall { name, args } => {
                // Fold all arguments
                let folded_args: Vec<Expression> = args.iter().map(|arg| self.fold(arg)).collect();

                Expression::FunctionCall {
                    name: name.clone(),
                    args: folded_args,
                }
            }

            Expression::Ternary {
                condition,
                true_expr,
                false_expr,
            } => {
                let condition_folded = self.fold(condition);

                // If condition is a constant boolean, we can eliminate the branch
                if let Expression::Literal(Value::Bool(cond)) = &condition_folded {
                    return if *cond {
                        self.fold(true_expr)
                    } else {
                        self.fold(false_expr)
                    };
                }

                // Otherwise, fold all three parts
                Expression::Ternary {
                    condition: Box::new(condition_folded),
                    true_expr: Box::new(self.fold(true_expr)),
                    false_expr: Box::new(self.fold(false_expr)),
                }
            }

            Expression::LogicalGroup { op, conditions } => {
                // Fold all conditions
                let folded_conditions: Vec<Expression> =
                    conditions.iter().map(|cond| self.fold(cond)).collect();

                Expression::LogicalGroup {
                    op: *op,
                    conditions: folded_conditions,
                }
            }

            Expression::ListReference { .. } => {
                // ListReference cannot be folded, return as-is
                expr.clone()
            }

            Expression::ResultAccess { .. } => {
                // ResultAccess cannot be folded (runtime value), return as-is
                expr.clone()
            }
        }
    }

    /// Fold a binary operation on two constant values
    fn fold_binary_op(&self, left: &Value, op: &Operator, right: &Value) -> Option<Value> {
        match (left, op, right) {
            // Arithmetic operations on numbers
            (Value::Number(l), Operator::Add, Value::Number(r)) => Some(Value::Number(l + r)),
            (Value::Number(l), Operator::Sub, Value::Number(r)) => Some(Value::Number(l - r)),
            (Value::Number(l), Operator::Mul, Value::Number(r)) => Some(Value::Number(l * r)),
            (Value::Number(l), Operator::Div, Value::Number(r)) if *r != 0.0 => {
                Some(Value::Number(l / r))
            }
            (Value::Number(l), Operator::Mod, Value::Number(r)) if *r != 0.0 => {
                Some(Value::Number(l % r))
            }

            // Comparison operations on numbers
            (Value::Number(l), Operator::Gt, Value::Number(r)) => Some(Value::Bool(l > r)),
            (Value::Number(l), Operator::Ge, Value::Number(r)) => Some(Value::Bool(l >= r)),
            (Value::Number(l), Operator::Lt, Value::Number(r)) => Some(Value::Bool(l < r)),
            (Value::Number(l), Operator::Le, Value::Number(r)) => Some(Value::Bool(l <= r)),

            // Equality operations (works on any type)
            (Value::Number(l), Operator::Eq, Value::Number(r)) => Some(Value::Bool(l == r)),
            (Value::Number(l), Operator::Ne, Value::Number(r)) => Some(Value::Bool(l != r)),
            (Value::String(l), Operator::Eq, Value::String(r)) => Some(Value::Bool(l == r)),
            (Value::String(l), Operator::Ne, Value::String(r)) => Some(Value::Bool(l != r)),
            (Value::Bool(l), Operator::Eq, Value::Bool(r)) => Some(Value::Bool(l == r)),
            (Value::Bool(l), Operator::Ne, Value::Bool(r)) => Some(Value::Bool(l != r)),

            // Logical operations on booleans
            (Value::Bool(l), Operator::And, Value::Bool(r)) => Some(Value::Bool(*l && *r)),
            (Value::Bool(l), Operator::Or, Value::Bool(r)) => Some(Value::Bool(*l || *r)),

            // String operations
            (Value::String(l), Operator::Contains, Value::String(r)) => {
                Some(Value::Bool(l.contains(r)))
            }
            (Value::String(l), Operator::StartsWith, Value::String(r)) => {
                Some(Value::Bool(l.starts_with(r)))
            }
            (Value::String(l), Operator::EndsWith, Value::String(r)) => {
                Some(Value::Bool(l.ends_with(r)))
            }

            // Can't fold
            _ => None,
        }
    }

    /// Fold a unary operation on a constant value
    fn fold_unary_op(&self, op: &UnaryOperator, operand: &Value) -> Option<Value> {
        match (op, operand) {
            (UnaryOperator::Not, Value::Bool(b)) => Some(Value::Bool(!b)),
            (UnaryOperator::Negate, Value::Number(n)) => Some(Value::Number(-n)),
            _ => None,
        }
    }
}

impl Default for ConstantFolder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_literal() {
        let folder = ConstantFolder::new();
        let expr = Expression::literal(Value::Number(42.0));

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Number(42.0)));
    }

    #[test]
    fn test_fold_arithmetic_add() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(10.0)),
            Operator::Add,
            Expression::literal(Value::Number(20.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Number(30.0)));
    }

    #[test]
    fn test_fold_arithmetic_sub() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(20.0)),
            Operator::Sub,
            Expression::literal(Value::Number(10.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Number(10.0)));
    }

    #[test]
    fn test_fold_arithmetic_mul() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(5.0)),
            Operator::Mul,
            Expression::literal(Value::Number(3.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Number(15.0)));
    }

    #[test]
    fn test_fold_arithmetic_div() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(20.0)),
            Operator::Div,
            Expression::literal(Value::Number(4.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Number(5.0)));
    }

    #[test]
    fn test_fold_comparison_gt() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(10.0)),
            Operator::Gt,
            Expression::literal(Value::Number(5.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(true)));
    }

    #[test]
    fn test_fold_comparison_lt() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(5.0)),
            Operator::Lt,
            Expression::literal(Value::Number(10.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(true)));
    }

    #[test]
    fn test_fold_equality_eq() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(10.0)),
            Operator::Eq,
            Expression::literal(Value::Number(10.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(true)));
    }

    #[test]
    fn test_fold_equality_ne() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Number(10.0)),
            Operator::Ne,
            Expression::literal(Value::Number(5.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(true)));
    }

    #[test]
    fn test_fold_logical_and_true() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Bool(true)),
            Operator::And,
            Expression::literal(Value::Bool(true)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(true)));
    }

    #[test]
    fn test_fold_logical_and_false() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Bool(true)),
            Operator::And,
            Expression::literal(Value::Bool(false)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(false)));
    }

    #[test]
    fn test_fold_logical_or() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::Bool(false)),
            Operator::Or,
            Expression::literal(Value::Bool(true)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(true)));
    }

    #[test]
    fn test_fold_unary_not() {
        let folder = ConstantFolder::new();
        let expr = Expression::unary(UnaryOperator::Not, Expression::literal(Value::Bool(true)));

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(false)));
    }

    #[test]
    fn test_fold_unary_negate() {
        let folder = ConstantFolder::new();
        let expr = Expression::unary(
            UnaryOperator::Negate,
            Expression::literal(Value::Number(42.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Number(-42.0)));
    }

    #[test]
    fn test_fold_nested_expression() {
        let folder = ConstantFolder::new();
        // (10 + 20) * 2 = 30 * 2 = 60
        let expr = Expression::binary(
            Expression::binary(
                Expression::literal(Value::Number(10.0)),
                Operator::Add,
                Expression::literal(Value::Number(20.0)),
            ),
            Operator::Mul,
            Expression::literal(Value::Number(2.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Number(60.0)));
    }

    #[test]
    fn test_fold_ternary_true() {
        let folder = ConstantFolder::new();
        // true ? 10 : 20 => 10
        let expr = Expression::ternary(
            Expression::literal(Value::Bool(true)),
            Expression::literal(Value::Number(10.0)),
            Expression::literal(Value::Number(20.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Number(10.0)));
    }

    #[test]
    fn test_fold_ternary_false() {
        let folder = ConstantFolder::new();
        // false ? 10 : 20 => 20
        let expr = Expression::ternary(
            Expression::literal(Value::Bool(false)),
            Expression::literal(Value::Number(10.0)),
            Expression::literal(Value::Number(20.0)),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Number(20.0)));
    }

    #[test]
    fn test_fold_string_contains() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::String("hello world".to_string())),
            Operator::Contains,
            Expression::literal(Value::String("world".to_string())),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(true)));
    }

    #[test]
    fn test_fold_string_starts_with() {
        let folder = ConstantFolder::new();
        let expr = Expression::binary(
            Expression::literal(Value::String("hello world".to_string())),
            Operator::StartsWith,
            Expression::literal(Value::String("hello".to_string())),
        );

        let result = folder.fold(&expr);
        assert_eq!(result, Expression::literal(Value::Bool(true)));
    }

    #[test]
    fn test_no_fold_with_field_access() {
        let folder = ConstantFolder::new();
        // user.age + 10 => cannot fold
        let expr = Expression::binary(
            Expression::field_access(vec!["user".to_string(), "age".to_string()]),
            Operator::Add,
            Expression::literal(Value::Number(10.0)),
        );

        let result = folder.fold(&expr);
        // Should remain as-is (but with folded sub-expressions)
        if let Expression::Binary { left, op, right } = result {
            assert!(matches!(*left, Expression::FieldAccess(_)));
            assert_eq!(op, Operator::Add);
            assert_eq!(*right, Expression::literal(Value::Number(10.0)));
        } else {
            panic!("Expected Binary expression");
        }
    }
}
