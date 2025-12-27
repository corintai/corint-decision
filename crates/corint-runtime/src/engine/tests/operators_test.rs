//! Operator tests for pipeline executor

use crate::engine::PipelineExecutor;
use corint_core::ast::{Operator, UnaryOperator};
use corint_core::ir::{Instruction, Program, ProgramMetadata};
use corint_core::Value;
use std::collections::HashMap;

// ===========================================
// String operator tests
// ===========================================

#[tokio::test]
async fn test_string_contains() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::String("hello world".to_string()),
        },
        Instruction::LoadConst {
            value: Value::String("world".to_string()),
        },
        Instruction::BinaryOp {
            op: Operator::Contains,
        },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // Contains should be true
}

#[tokio::test]
async fn test_string_contains_negative() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::String("hello".to_string()),
        },
        Instruction::LoadConst {
            value: Value::String("world".to_string()),
        },
        Instruction::BinaryOp {
            op: Operator::Contains,
        },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 0); // Contains should be false
}

#[tokio::test]
async fn test_string_starts_with() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::String("hello world".to_string()),
        },
        Instruction::LoadConst {
            value: Value::String("hello".to_string()),
        },
        Instruction::BinaryOp {
            op: Operator::StartsWith,
        },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100);
}

#[tokio::test]
async fn test_string_ends_with() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::String("hello world".to_string()),
        },
        Instruction::LoadConst {
            value: Value::String("world".to_string()),
        },
        Instruction::BinaryOp {
            op: Operator::EndsWith,
        },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100);
}

// ===========================================
// Array/List operator tests
// ===========================================

#[tokio::test]
async fn test_value_in_array() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::String("apple".to_string()),
        },
        Instruction::LoadConst {
            value: Value::Array(vec![
                Value::String("apple".to_string()),
                Value::String("banana".to_string()),
                Value::String("cherry".to_string()),
            ]),
        },
        Instruction::BinaryOp { op: Operator::In },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // "apple" in array
}

#[tokio::test]
async fn test_value_in_array_negative() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::String("grape".to_string()),
        },
        Instruction::LoadConst {
            value: Value::Array(vec![
                Value::String("apple".to_string()),
                Value::String("banana".to_string()),
            ]),
        },
        Instruction::BinaryOp { op: Operator::In },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 0); // "grape" not in array
}

#[tokio::test]
async fn test_value_not_in_array() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::String("grape".to_string()),
        },
        Instruction::LoadConst {
            value: Value::Array(vec![
                Value::String("apple".to_string()),
                Value::String("banana".to_string()),
            ]),
        },
        Instruction::BinaryOp {
            op: Operator::NotIn,
        },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // "grape" not in array
}

#[tokio::test]
async fn test_array_contains_value() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
            ]),
        },
        Instruction::LoadConst {
            value: Value::Number(2.0),
        },
        Instruction::BinaryOp {
            op: Operator::Contains,
        },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // array contains 2.0
}

#[tokio::test]
async fn test_number_in_array() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(42.0),
        },
        Instruction::LoadConst {
            value: Value::Array(vec![
                Value::Number(10.0),
                Value::Number(42.0),
                Value::Number(100.0),
            ]),
        },
        Instruction::BinaryOp { op: Operator::In },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // 42 in array
}

// ===========================================
// Boolean operator tests
// ===========================================

#[tokio::test]
async fn test_boolean_and() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Bool(true),
        },
        Instruction::LoadConst {
            value: Value::Bool(true),
        },
        Instruction::BinaryOp { op: Operator::And },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // true && true = true
}

#[tokio::test]
async fn test_boolean_and_false() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Bool(true),
        },
        Instruction::LoadConst {
            value: Value::Bool(false),
        },
        Instruction::BinaryOp { op: Operator::And },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 0); // true && false = false
}

#[tokio::test]
async fn test_boolean_or() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Bool(false),
        },
        Instruction::LoadConst {
            value: Value::Bool(true),
        },
        Instruction::BinaryOp { op: Operator::Or },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // false || true = true
}

#[tokio::test]
async fn test_unary_not() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Bool(false),
        },
        Instruction::UnaryOp {
            op: UnaryOperator::Not,
        },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // !false = true
}

#[tokio::test]
async fn test_unary_negate() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(5.0),
        },
        Instruction::UnaryOp {
            op: UnaryOperator::Negate,
        },
        Instruction::LoadConst {
            value: Value::Number(-5.0),
        },
        Instruction::Compare { op: Operator::Eq },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // -5.0 == -5.0
}

// ===========================================
// Comparison operator tests
// ===========================================

#[tokio::test]
async fn test_string_equality() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::String("hello".to_string()),
        },
        Instruction::LoadConst {
            value: Value::String("hello".to_string()),
        },
        Instruction::Compare { op: Operator::Eq },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100);
}

#[tokio::test]
async fn test_string_inequality() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::String("hello".to_string()),
        },
        Instruction::LoadConst {
            value: Value::String("world".to_string()),
        },
        Instruction::Compare { op: Operator::Ne },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100);
}

#[tokio::test]
async fn test_null_comparison_returns_false() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst { value: Value::Null },
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::Compare { op: Operator::Eq },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 0); // Null comparison returns false
}

// ===========================================
// Arithmetic operator tests
// ===========================================

#[tokio::test]
async fn test_subtraction() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::LoadConst {
            value: Value::Number(3.0),
        },
        Instruction::BinaryOp { op: Operator::Sub },
        Instruction::LoadConst {
            value: Value::Number(7.0),
        },
        Instruction::Compare { op: Operator::Eq },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // 10 - 3 == 7
}

#[tokio::test]
async fn test_multiplication() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(5.0),
        },
        Instruction::LoadConst {
            value: Value::Number(4.0),
        },
        Instruction::BinaryOp { op: Operator::Mul },
        Instruction::LoadConst {
            value: Value::Number(20.0),
        },
        Instruction::Compare { op: Operator::Eq },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // 5 * 4 == 20
}

#[tokio::test]
async fn test_modulo() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::LoadConst {
            value: Value::Number(3.0),
        },
        Instruction::BinaryOp { op: Operator::Mod },
        Instruction::LoadConst {
            value: Value::Number(1.0),
        },
        Instruction::Compare { op: Operator::Eq },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // 10 % 3 == 1
}

#[tokio::test]
async fn test_number_comparisons() {
    let executor = PipelineExecutor::new();

    // Test less than
    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(5.0),
        },
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::Compare { op: Operator::Lt },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // 5 < 10
}

#[tokio::test]
async fn test_less_than_or_equal() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::Compare { op: Operator::Le },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // 10 <= 10
}

#[tokio::test]
async fn test_greater_than_or_equal() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(15.0),
        },
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::Compare { op: Operator::Ge },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 100); // 15 >= 10
}
