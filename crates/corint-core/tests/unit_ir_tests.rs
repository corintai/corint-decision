//! Unit tests for IR (Intermediate Representation) types
//!
//! Tests the IR instructions and program structures used by the compiler and runtime

use corint_core::ir::*;
use corint_core::types::Value;
use corint_core::ast::{Operator, UnaryOperator};

// =============================================================================
// Instruction Tests
// =============================================================================

#[test]
fn test_instruction_load_const() {
    let instr = Instruction::LoadConst {
        value: Value::Number(42.0),
    };
    match instr {
        Instruction::LoadConst { value } => {
            assert_eq!(value, Value::Number(42.0));
        }
        _ => panic!("Expected LoadConst instruction"),
    }
}

#[test]
fn test_instruction_load_field() {
    let instr = Instruction::LoadField {
        path: vec!["event".to_string(), "amount".to_string()],
    };
    match instr {
        Instruction::LoadField { path } => {
            assert_eq!(path.len(), 2);
            assert_eq!(path[0], "event");
            assert_eq!(path[1], "amount");
        }
        _ => panic!("Expected LoadField instruction"),
    }
}

#[test]
fn test_instruction_load_result() {
    let instr = Instruction::LoadResult {
        ruleset_id: Some("fraud_detection".to_string()),
        field: "total_score".to_string(),
    };

    match instr {
        Instruction::LoadResult { ruleset_id, field } => {
            assert_eq!(ruleset_id, Some("fraud_detection".to_string()));
            assert_eq!(field, "total_score");
        }
        _ => panic!("Expected LoadResult instruction"),
    }
}

#[test]
fn test_instruction_binary_op() {
    let instr = Instruction::BinaryOp {
        op: Operator::Add,
    };

    match instr {
        Instruction::BinaryOp { op } => assert_eq!(op, Operator::Add),
        _ => panic!("Expected BinaryOp instruction"),
    }
}

#[test]
fn test_instruction_compare() {
    let instr = Instruction::Compare {
        op: Operator::Gt,
    };

    match instr {
        Instruction::Compare { op } => assert_eq!(op, Operator::Gt),
        _ => panic!("Expected Compare instruction"),
    }
}

#[test]
fn test_instruction_unary_op() {
    let instr = Instruction::UnaryOp {
        op: UnaryOperator::Not,
    };

    match instr {
        Instruction::UnaryOp { op } => assert_eq!(op, UnaryOperator::Not),
        _ => panic!("Expected UnaryOp instruction"),
    }
}

#[test]
fn test_instruction_jump() {
    let instr = Instruction::Jump { offset: 10 };
    match instr {
        Instruction::Jump { offset } => assert_eq!(offset, 10),
        _ => panic!("Expected Jump instruction"),
    }
}

#[test]
fn test_instruction_jump_if_false() {
    let instr = Instruction::JumpIfFalse { offset: -5 };
    match instr {
        Instruction::JumpIfFalse { offset } => assert_eq!(offset, -5),
        _ => panic!("Expected JumpIfFalse instruction"),
    }
}

#[test]
fn test_instruction_jump_if_true() {
    let instr = Instruction::JumpIfTrue { offset: 3 };
    match instr {
        Instruction::JumpIfTrue { offset } => assert_eq!(offset, 3),
        _ => panic!("Expected JumpIfTrue instruction"),
    }
}

#[test]
fn test_instruction_return() {
    let instr = Instruction::Return;
    assert!(matches!(instr, Instruction::Return));
}

#[test]
fn test_instruction_check_event_type() {
    let instr = Instruction::CheckEventType {
        expected: "transaction".to_string(),
    };

    match instr {
        Instruction::CheckEventType { expected } => {
            assert_eq!(expected, "transaction");
        }
        _ => panic!("Expected CheckEventType instruction"),
    }
}

// =============================================================================
// Instruction Equality and Clone Tests
// =============================================================================

#[test]
fn test_instruction_equality() {
    let instr1 = Instruction::LoadConst {
        value: Value::Number(42.0),
    };
    let instr2 = Instruction::LoadConst {
        value: Value::Number(42.0),
    };
    let instr3 = Instruction::LoadConst {
        value: Value::Number(43.0),
    };

    assert_eq!(instr1, instr2);
    assert_ne!(instr1, instr3);
}

#[test]
fn test_instruction_clone() {
    let instr = Instruction::BinaryOp {
        op: Operator::Mul,
    };
    let cloned = instr.clone();
    assert_eq!(instr, cloned);
}

#[test]
fn test_instruction_debug_format() {
    let instr = Instruction::LoadConst {
        value: Value::Number(42.0),
    };
    let debug_str = format!("{:?}", instr);
    assert!(debug_str.contains("LoadConst"));
}

// =============================================================================
// Serde Tests
// =============================================================================

#[test]
fn test_instruction_serde_load_const() {
    let instr = Instruction::LoadConst {
        value: Value::Number(42.0),
    };
    let json = serde_json::to_string(&instr).unwrap();
    let deserialized: Instruction = serde_json::from_str(&json).unwrap();
    assert_eq!(instr, deserialized);
}

#[test]
fn test_instruction_serde_load_field() {
    let instr = Instruction::LoadField {
        path: vec!["event".to_string(), "amount".to_string()],
    };
    let json = serde_json::to_string(&instr).unwrap();
    let deserialized: Instruction = serde_json::from_str(&json).unwrap();
    assert_eq!(instr, deserialized);
}

#[test]
fn test_instruction_serde_binary_op() {
    let instr = Instruction::BinaryOp {
        op: Operator::Add,
    };
    let json = serde_json::to_string(&instr).unwrap();
    let deserialized: Instruction = serde_json::from_str(&json).unwrap();
    assert_eq!(instr, deserialized);
}

#[test]
fn test_instruction_serde_jump() {
    let instr = Instruction::Jump { offset: 10 };
    let json = serde_json::to_string(&instr).unwrap();
    let deserialized: Instruction = serde_json::from_str(&json).unwrap();
    assert_eq!(instr, deserialized);
}

// =============================================================================
// Instruction Sequence Tests
// =============================================================================

#[test]
fn test_simple_arithmetic_sequence() {
    // Compile: 10 + 5
    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::LoadConst {
            value: Value::Number(5.0),
        },
        Instruction::BinaryOp {
            op: Operator::Add,
        },
    ];

    assert_eq!(instructions.len(), 3);
}

#[test]
fn test_comparison_sequence() {
    // Compile: event.amount > 1000
    let instructions = vec![
        Instruction::LoadField {
            path: vec!["event".to_string(), "amount".to_string()],
        },
        Instruction::LoadConst {
            value: Value::Number(1000.0),
        },
        Instruction::Compare {
            op: Operator::Gt,
        },
    ];

    assert_eq!(instructions.len(), 3);
}

#[test]
fn test_conditional_jump_sequence() {
    // Compile: if (condition) { true_branch } else { false_branch }
    let instructions = vec![
        Instruction::LoadField {
            path: vec!["condition".to_string()],
        },
        Instruction::JumpIfFalse { offset: 3 }, // Jump to else
        Instruction::LoadConst {
            value: Value::Number(1.0),
        }, // Then branch
        Instruction::Jump { offset: 2 }, // Skip else
        Instruction::LoadConst {
            value: Value::Number(2.0),
        }, // Else branch
    ];

    assert_eq!(instructions.len(), 5);
}

#[test]
fn test_result_access_sequence() {
    // Compile: result.action == "deny"
    let instructions = vec![
        Instruction::LoadResult {
            ruleset_id: None,
            field: "action".to_string(),
        },
        Instruction::LoadConst {
            value: Value::String("deny".to_string()),
        },
        Instruction::Compare {
            op: Operator::Eq,
        },
    ];

    assert_eq!(instructions.len(), 3);
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_nested_field_access() {
    let instr = Instruction::LoadField {
        path: vec![
            "event".to_string(),
            "user".to_string(),
            "profile".to_string(),
            "age".to_string(),
        ],
    };

    match instr {
        Instruction::LoadField { path } => {
            assert_eq!(path.len(), 4);
            assert_eq!(path[3], "age");
        }
        _ => panic!("Expected LoadField"),
    }
}

#[test]
fn test_negative_jump_offset() {
    let instr = Instruction::Jump { offset: -10 };
    match instr {
        Instruction::Jump { offset } => assert_eq!(offset, -10),
        _ => panic!("Expected Jump"),
    }
}

#[test]
fn test_zero_jump_offset() {
    let instr = Instruction::JumpIfTrue { offset: 0 };
    match instr {
        Instruction::JumpIfTrue { offset } => assert_eq!(offset, 0),
        _ => panic!("Expected JumpIfTrue"),
    }
}

#[test]
fn test_all_comparison_operators() {
    let operators = vec![
        Operator::Eq,
        Operator::Ne,
        Operator::Lt,
        Operator::Le,
        Operator::Gt,
        Operator::Ge,
    ];

    for op in operators {
        let instr = Instruction::Compare { op };
        assert!(matches!(instr, Instruction::Compare { .. }));
    }
}

#[test]
fn test_all_binary_operators() {
    let operators = vec![
        Operator::Add,
        Operator::Sub,
        Operator::Mul,
        Operator::Div,
        Operator::Mod,
    ];

    for op in operators {
        let instr = Instruction::BinaryOp { op };
        assert!(matches!(instr, Instruction::BinaryOp { .. }));
    }
}

#[test]
fn test_all_unary_operators() {
    let operators = vec![UnaryOperator::Not, UnaryOperator::Negate];

    for op in operators {
        let instr = Instruction::UnaryOp { op };
        assert!(matches!(instr, Instruction::UnaryOp { .. }));
    }
}
