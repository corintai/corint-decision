//! Unit tests for AST (Abstract Syntax Tree) types
//!
//! Tests the core AST data structures used throughout CORINT

use corint_core::ast::*;
use corint_core::types::Value;

// =============================================================================
// Expression Tests
// =============================================================================

#[test]
fn test_expression_literal_number() {
    let expr = Expression::literal(Value::Number(42.0));
    match expr {
        Expression::Literal(Value::Number(n)) => assert_eq!(n, 42.0),
        _ => panic!("Expected literal number"),
    }
}

#[test]
fn test_expression_literal_string() {
    let expr = Expression::literal(Value::String("hello".to_string()));
    match expr {
        Expression::Literal(Value::String(s)) => assert_eq!(s, "hello"),
        _ => panic!("Expected literal string"),
    }
}

#[test]
fn test_expression_literal_bool() {
    let expr = Expression::literal(Value::Bool(true));
    match expr {
        Expression::Literal(Value::Bool(b)) => assert!(b),
        _ => panic!("Expected literal bool"),
    }
}

#[test]
fn test_expression_field_access_simple() {
    let expr = Expression::field_access(vec!["event".to_string(), "amount".to_string()]);
    match expr {
        Expression::FieldAccess(path) => {
            assert_eq!(path.len(), 2);
            assert_eq!(path[0], "event");
            assert_eq!(path[1], "amount");
        }
        _ => panic!("Expected field access"),
    }
}

#[test]
fn test_expression_field_access_nested() {
    let expr = Expression::field_access(vec![
        "event".to_string(),
        "user".to_string(),
        "profile".to_string(),
        "age".to_string(),
    ]);
    match expr {
        Expression::FieldAccess(path) => {
            assert_eq!(path.len(), 4);
            assert_eq!(path[3], "age");
        }
        _ => panic!("Expected field access"),
    }
}

#[test]
fn test_expression_binary_comparison() {
    let left = Expression::field_access(vec!["event".to_string(), "amount".to_string()]);
    let right = Expression::literal(Value::Number(1000.0));
    let expr = Expression::binary(left, Operator::Gt, right);

    match expr {
        Expression::Binary { left, op, right } => {
            assert_eq!(op, Operator::Gt);
            assert!(matches!(*left, Expression::FieldAccess(_)));
            assert!(matches!(*right, Expression::Literal(_)));
        }
        _ => panic!("Expected binary expression"),
    }
}

#[test]
fn test_expression_binary_arithmetic() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Add, right);

    match expr {
        Expression::Binary { op, .. } => {
            assert_eq!(op, Operator::Add);
        }
        _ => panic!("Expected binary expression"),
    }
}

#[test]
fn test_expression_unary_not() {
    let operand = Expression::literal(Value::Bool(true));
    let expr = Expression::unary(UnaryOperator::Not, operand);

    match expr {
        Expression::Unary { op, operand } => {
            assert_eq!(op, UnaryOperator::Not);
            assert!(matches!(*operand, Expression::Literal(Value::Bool(true))));
        }
        _ => panic!("Expected unary expression"),
    }
}

#[test]
fn test_expression_unary_negate() {
    let operand = Expression::literal(Value::Number(42.0));
    let expr = Expression::unary(UnaryOperator::Negate, operand);

    match expr {
        Expression::Unary { op, .. } => {
            assert_eq!(op, UnaryOperator::Negate);
        }
        _ => panic!("Expected unary expression"),
    }
}

#[test]
fn test_expression_function_call_no_args() {
    let expr = Expression::function_call("now".to_string(), vec![]);

    match expr {
        Expression::FunctionCall { name, args } => {
            assert_eq!(name, "now");
            assert_eq!(args.len(), 0);
        }
        _ => panic!("Expected function call"),
    }
}

#[test]
fn test_expression_function_call_with_args() {
    let args = vec![
        Expression::field_access(vec!["event".to_string(), "email".to_string()]),
        Expression::literal(Value::String("@gmail.com".to_string())),
    ];
    let expr = Expression::function_call("contains".to_string(), args);

    match expr {
        Expression::FunctionCall { name, args } => {
            assert_eq!(name, "contains");
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Expected function call"),
    }
}

#[test]
fn test_expression_ternary() {
    let condition = Expression::field_access(vec!["event".to_string(), "is_vip".to_string()]);
    let true_expr = Expression::literal(Value::Number(10.0));
    let false_expr = Expression::literal(Value::Number(50.0));
    let expr = Expression::ternary(condition, true_expr, false_expr);

    match expr {
        Expression::Ternary {
            condition,
            true_expr,
            false_expr,
        } => {
            assert!(matches!(*condition, Expression::FieldAccess(_)));
            assert!(matches!(*true_expr, Expression::Literal(Value::Number(10.0))));
            assert!(matches!(*false_expr, Expression::Literal(Value::Number(50.0))));
        }
        _ => panic!("Expected ternary expression"),
    }
}

#[test]
fn test_expression_result_access_last() {
    let expr = Expression::result_access("action".to_string());

    match expr {
        Expression::ResultAccess { ruleset_id, field } => {
            assert_eq!(ruleset_id, None);
            assert_eq!(field, "action");
        }
        _ => panic!("Expected result access"),
    }
}

#[test]
fn test_expression_result_access_specific() {
    let expr = Expression::result_access_for("fraud_detection".to_string(), "total_score".to_string());

    match expr {
        Expression::ResultAccess { ruleset_id, field } => {
            assert_eq!(ruleset_id, Some("fraud_detection".to_string()));
            assert_eq!(field, "total_score");
        }
        _ => panic!("Expected result access"),
    }
}

#[test]
fn test_expression_logical_group_any() {
    let conditions = vec![
        Expression::binary(
            Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(1000.0)),
        ),
        Expression::binary(
            Expression::field_access(vec!["event".to_string(), "country".to_string()]),
            Operator::Eq,
            Expression::literal(Value::String("US".to_string())),
        ),
    ];

    let expr = Expression::LogicalGroup {
        op: LogicalGroupOp::Any,
        conditions,
    };

    match expr {
        Expression::LogicalGroup { op, conditions } => {
            assert_eq!(op, LogicalGroupOp::Any);
            assert_eq!(conditions.len(), 2);
        }
        _ => panic!("Expected logical group"),
    }
}

#[test]
fn test_expression_logical_group_all() {
    let conditions = vec![
        Expression::field_access(vec!["event".to_string(), "is_verified".to_string()]),
        Expression::field_access(vec!["event".to_string(), "is_active".to_string()]),
    ];

    let expr = Expression::LogicalGroup {
        op: LogicalGroupOp::All,
        conditions,
    };

    match expr {
        Expression::LogicalGroup { op, conditions } => {
            assert_eq!(op, LogicalGroupOp::All);
            assert_eq!(conditions.len(), 2);
        }
        _ => panic!("Expected logical group"),
    }
}

#[test]
fn test_expression_list_reference() {
    let expr = Expression::ListReference {
        list_id: "email_blocklist".to_string(),
    };

    match expr {
        Expression::ListReference { list_id } => {
            assert_eq!(list_id, "email_blocklist");
        }
        _ => panic!("Expected list reference"),
    }
}

// =============================================================================
// Operator Tests
// =============================================================================

#[test]
fn test_operator_equality() {
    assert_eq!(Operator::Eq, Operator::Eq);
    assert_ne!(Operator::Eq, Operator::Ne);
}

#[test]
fn test_operator_comparison_variants() {
    let operators = vec![
        Operator::Eq,
        Operator::Ne,
        Operator::Lt,
        Operator::Le,
        Operator::Gt,
        Operator::Ge,
    ];

    assert_eq!(operators.len(), 6);
}

#[test]
fn test_operator_logical_variants() {
    let operators = vec![Operator::And, Operator::Or];
    assert_eq!(operators.len(), 2);
}

#[test]
fn test_operator_arithmetic_variants() {
    let operators = vec![
        Operator::Add,
        Operator::Sub,
        Operator::Mul,
        Operator::Div,
        Operator::Mod,
    ];
    assert_eq!(operators.len(), 5);
}

#[test]
fn test_operator_string_variants() {
    let operators = vec![
        Operator::Contains,
        Operator::StartsWith,
        Operator::EndsWith,
        Operator::Regex,
    ];
    assert_eq!(operators.len(), 4);
}

#[test]
fn test_operator_collection_variants() {
    let operators = vec![
        Operator::In,
        Operator::NotIn,
        Operator::InList,
        Operator::NotInList,
    ];
    assert_eq!(operators.len(), 4);
}

// =============================================================================
// Signal Tests
// =============================================================================

#[test]
fn test_signal_variants() {
    let signals = vec![
        Signal::Approve,
        Signal::Decline,
        Signal::Review,
        Signal::Hold,
    ];

    assert_eq!(signals.len(), 4);
}

#[test]
fn test_signal_equality() {
    assert_eq!(Signal::Approve, Signal::Approve);
    assert_ne!(Signal::Approve, Signal::Decline);
}

#[test]
fn test_signal_clone() {
    let signal = Signal::Review;
    let cloned = signal.clone();
    assert_eq!(signal, cloned);
}

// =============================================================================
// Condition Tests
// =============================================================================

#[test]
fn test_condition_simple() {
    let condition = Condition::Expression(Expression::binary(
        Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
        Operator::Gt,
        Expression::literal(Value::Number(100.0)),
    ));

    match condition {
        Condition::Expression(expr) => {
            assert!(matches!(expr, Expression::Binary { .. }));
        }
        _ => panic!("Expected expression condition"),
    }
}

#[test]
fn test_condition_all() {
    let conditions = vec![
        Condition::Expression(Expression::literal(Value::Bool(true))),
        Condition::Expression(Expression::literal(Value::Bool(true))),
    ];

    let condition_group = ConditionGroup::All(conditions);
    let condition = Condition::from_group(condition_group);

    match condition {
        Condition::Group(boxed_group) => {
            match *boxed_group {
                ConditionGroup::All(ref conds) => {
                    assert_eq!(conds.len(), 2);
                }
                _ => panic!("Expected All condition group"),
            }
        }
        _ => panic!("Expected Group condition"),
    }
}

#[test]
fn test_condition_any() {
    let conditions = vec![
        Condition::Expression(Expression::literal(Value::Bool(false))),
        Condition::Expression(Expression::literal(Value::Bool(true))),
    ];

    let condition_group = ConditionGroup::Any(conditions);
    let condition = Condition::from_group(condition_group);

    match condition {
        Condition::Group(boxed_group) => {
            match *boxed_group {
                ConditionGroup::Any(ref conds) => {
                    assert_eq!(conds.len(), 2);
                }
                _ => panic!("Expected Any condition group"),
            }
        }
        _ => panic!("Expected Group condition"),
    }
}

#[test]
fn test_condition_not() {
    let inner = Condition::Expression(Expression::literal(Value::Bool(true)));
    let condition_group = ConditionGroup::Not(vec![inner]);
    let condition = Condition::from_group(condition_group);

    match condition {
        Condition::Group(boxed_group) => {
            match *boxed_group {
                ConditionGroup::Not(ref conds) => {
                    assert_eq!(conds.len(), 1);
                    assert!(matches!(conds[0], Condition::Expression(_)));
                }
                _ => panic!("Expected Not condition group"),
            }
        }
        _ => panic!("Expected Group condition"),
    }
}

// =============================================================================
// Serde Tests
// =============================================================================

#[test]
fn test_expression_serde_literal() {
    let expr = Expression::literal(Value::Number(42.0));
    let json = serde_json::to_string(&expr).unwrap();
    let deserialized: Expression = serde_json::from_str(&json).unwrap();
    assert_eq!(expr, deserialized);
}

#[test]
fn test_expression_serde_binary() {
    let expr = Expression::binary(
        Expression::literal(Value::Number(10.0)),
        Operator::Add,
        Expression::literal(Value::Number(5.0)),
    );

    let json = serde_json::to_string(&expr).unwrap();
    let deserialized: Expression = serde_json::from_str(&json).unwrap();
    assert_eq!(expr, deserialized);
}

#[test]
fn test_operator_serde() {
    let op = Operator::Gt;
    let json = serde_json::to_string(&op).unwrap();
    let deserialized: Operator = serde_json::from_str(&json).unwrap();
    assert_eq!(op, deserialized);
}

#[test]
fn test_signal_serde() {
    let signal = Signal::Decline;
    let json = serde_json::to_string(&signal).unwrap();
    let deserialized: Signal = serde_json::from_str(&json).unwrap();
    assert_eq!(signal, deserialized);
}
