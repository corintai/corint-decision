//! Expression AST nodes

use super::operator::Operator;
use crate::types::Value;
use serde::{Deserialize, Serialize};

/// Expression AST node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    /// Literal value
    Literal(Value),

    /// Field access (e.g., user.age, device.id)
    FieldAccess(Vec<String>),

    /// Binary operation
    Binary {
        left: Box<Expression>,
        op: Operator,
        right: Box<Expression>,
    },

    /// Unary operation
    Unary {
        op: UnaryOperator,
        operand: Box<Expression>,
    },

    /// Function call
    FunctionCall { name: String, args: Vec<Expression> },

    /// Ternary conditional (condition ? true_expr : false_expr)
    Ternary {
        condition: Box<Expression>,
        true_expr: Box<Expression>,
        false_expr: Box<Expression>,
    },

    /// Logical grouping (any/all conditions)
    LogicalGroup {
        /// Type of logical operation: "any" (OR) or "all" (AND)
        op: LogicalGroupOp,
        /// List of conditions to evaluate
        conditions: Vec<Expression>,
    },

    /// List reference (e.g., list.email_blocklist)
    ListReference {
        /// List ID from configuration
        list_id: String,
    },

    /// Result access (e.g., result.action, result.fraud_detection_ruleset.total_score)
    /// Used in pipeline router conditions to access ruleset execution results
    ResultAccess {
        /// Optional ruleset ID. If None, refers to the last executed ruleset's result
        ruleset_id: Option<String>,
        /// Field to access (e.g., "action", "total_score", "reason")
        field: String,
    },
}

/// Logical group operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicalGroupOp {
    /// Any condition must be true (OR logic)
    Any,
    /// All conditions must be true (AND logic)
    All,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    /// Logical NOT (!)
    Not,
    /// Arithmetic negation (-)
    Negate,
}

impl Expression {
    /// Create a literal expression
    pub fn literal(value: Value) -> Self {
        Expression::Literal(value)
    }

    /// Create a field access expression
    pub fn field_access(path: Vec<String>) -> Self {
        Expression::FieldAccess(path)
    }

    /// Create a binary expression
    pub fn binary(left: Expression, op: Operator, right: Expression) -> Self {
        Expression::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    /// Create a function call expression
    pub fn function_call(name: String, args: Vec<Expression>) -> Self {
        Expression::FunctionCall { name, args }
    }

    /// Create a unary expression
    pub fn unary(op: UnaryOperator, operand: Expression) -> Self {
        Expression::Unary {
            op,
            operand: Box::new(operand),
        }
    }

    /// Create a ternary expression
    pub fn ternary(condition: Expression, true_expr: Expression, false_expr: Expression) -> Self {
        Expression::Ternary {
            condition: Box::new(condition),
            true_expr: Box::new(true_expr),
            false_expr: Box::new(false_expr),
        }
    }

    /// Create a result access expression (last ruleset result)
    pub fn result_access(field: String) -> Self {
        Expression::ResultAccess {
            ruleset_id: None,
            field,
        }
    }

    /// Create a result access expression for a specific ruleset
    pub fn result_access_for(ruleset_id: String, field: String) -> Self {
        Expression::ResultAccess {
            ruleset_id: Some(ruleset_id),
            field,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_expression() {
        let expr = Expression::literal(Value::Number(42.0));
        assert_eq!(expr, Expression::Literal(Value::Number(42.0)));
    }

    #[test]
    fn test_field_access_expression() {
        let expr = Expression::field_access(vec!["user".to_string(), "age".to_string()]);
        assert_eq!(
            expr,
            Expression::FieldAccess(vec!["user".to_string(), "age".to_string()])
        );
    }

    #[test]
    fn test_binary_expression() {
        // user.age > 18
        let expr = Expression::binary(
            Expression::field_access(vec!["user".to_string(), "age".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(18.0)),
        );

        match expr {
            Expression::Binary { left, op, right } => {
                assert_eq!(op, Operator::Gt);
                assert_eq!(
                    *left,
                    Expression::FieldAccess(vec!["user".to_string(), "age".to_string()])
                );
                assert_eq!(*right, Expression::Literal(Value::Number(18.0)));
            }
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_complex_binary_expression() {
        // (5 + 3) * 2
        let expr = Expression::binary(
            Expression::binary(
                Expression::literal(Value::Number(5.0)),
                Operator::Add,
                Expression::literal(Value::Number(3.0)),
            ),
            Operator::Mul,
            Expression::literal(Value::Number(2.0)),
        );

        match expr {
            Expression::Binary { left, op, right } => {
                assert_eq!(op, Operator::Mul);
                // Left should be (5 + 3)
                match *left {
                    Expression::Binary { op, .. } => {
                        assert_eq!(op, Operator::Add);
                    }
                    _ => panic!("Expected Binary expression for left"),
                }
                assert_eq!(*right, Expression::Literal(Value::Number(2.0)));
            }
            _ => panic!("Expected Binary expression"),
        }
    }

    #[test]
    fn test_function_call_expression() {
        // count(user.logins, last_7d)
        let expr = Expression::function_call(
            "count".to_string(),
            vec![
                Expression::field_access(vec!["user".to_string(), "logins".to_string()]),
                Expression::literal(Value::String("last_7d".to_string())),
            ],
        );

        match expr {
            Expression::FunctionCall { name, args } => {
                assert_eq!(name, "count");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected FunctionCall expression"),
        }
    }

    #[test]
    fn test_unary_expression() {
        // !user.is_verified
        let expr = Expression::Unary {
            op: UnaryOperator::Not,
            operand: Box::new(Expression::field_access(vec![
                "user".to_string(),
                "is_verified".to_string(),
            ])),
        };

        match expr {
            Expression::Unary { op, .. } => {
                assert_eq!(op, UnaryOperator::Not);
            }
            _ => panic!("Expected Unary expression"),
        }
    }

    #[test]
    fn test_expression_clone() {
        let expr = Expression::binary(
            Expression::literal(Value::Number(5.0)),
            Operator::Add,
            Expression::literal(Value::Number(3.0)),
        );

        let cloned = expr.clone();
        assert_eq!(expr, cloned);
    }

    #[test]
    fn test_ternary_expression() {
        // user.age < 18 ? 0 : 100
        let expr = Expression::Ternary {
            condition: Box::new(Expression::binary(
                Expression::field_access(vec!["user".to_string(), "age".to_string()]),
                Operator::Lt,
                Expression::literal(Value::Number(18.0)),
            )),
            true_expr: Box::new(Expression::literal(Value::Number(0.0))),
            false_expr: Box::new(Expression::literal(Value::Number(100.0))),
        };

        match expr {
            Expression::Ternary {
                condition,
                true_expr,
                false_expr,
            } => {
                assert!(matches!(*condition, Expression::Binary { .. }));
                assert_eq!(*true_expr, Expression::Literal(Value::Number(0.0)));
                assert_eq!(*false_expr, Expression::Literal(Value::Number(100.0)));
            }
            _ => panic!("Expected Ternary expression"),
        }
    }
}
