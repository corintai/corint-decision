//! Basic execution tests for pipeline executor

use crate::engine::PipelineExecutor;
use crate::observability::Metrics;
use corint_core::ast::Signal;
use corint_core::ir::{Instruction, Program, ProgramMetadata};
use corint_core::Value;
use std::collections::HashMap;

#[tokio::test]
async fn test_execute_simple_program() {
    let executor = PipelineExecutor::new();

    let instructions = vec![Instruction::SetScore { value: 50 }, Instruction::Return];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();

    assert_eq!(result.score, 50);
}

#[tokio::test]
async fn test_execute_with_arithmetic() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::LoadConst {
            value: Value::Number(20.0),
        },
        Instruction::BinaryOp {
            op: corint_core::ast::Operator::Add,
        },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();

    // Result should be on stack but not reflected in score
    // This tests the stack operations work correctly
    assert_eq!(result.score, 0);
}

#[tokio::test]
async fn test_execute_with_comparison() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::LoadConst {
            value: Value::Number(5.0),
        },
        Instruction::Compare {
            op: corint_core::ast::Operator::Gt,
        },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();

    assert_eq!(result.score, 0);
}

#[tokio::test]
async fn test_execute_with_jump() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Bool(false),
        },
        Instruction::JumpIfFalse { offset: 2 },
        Instruction::SetScore { value: 100 },
        Instruction::SetScore { value: 50 },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();

    assert_eq!(result.score, 50);
}

#[tokio::test]
async fn test_execute_with_signal() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::SetSignal {
            signal: Signal::Approve,
        },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();

    assert_eq!(result.signal, Some(Signal::Approve));
}

#[tokio::test]
async fn test_execute_mark_rule_triggered() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::MarkRuleTriggered {
            rule_id: "test_rule".to_string(),
        },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await.unwrap();

    assert_eq!(result.triggered_rules.len(), 1);
    assert!(result.triggered_rules.contains(&"test_rule".to_string()));
}

#[tokio::test]
async fn test_division_by_zero() {
    let executor = PipelineExecutor::new();

    let instructions = vec![
        Instruction::LoadConst {
            value: Value::Number(10.0),
        },
        Instruction::LoadConst {
            value: Value::Number(0.0),
        },
        Instruction::BinaryOp {
            op: corint_core::ast::Operator::Div,
        },
        Instruction::Return,
    ];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    let result = executor.execute(&program, HashMap::new()).await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        crate::error::RuntimeError::DivisionByZero
    ));
}

#[tokio::test]
async fn test_metrics_collection() {
    let executor = PipelineExecutor::new();

    let instructions = vec![Instruction::SetScore { value: 10 }, Instruction::Return];

    let program = Program::new(instructions, ProgramMetadata::for_rule("test".to_string()));

    executor.execute(&program, HashMap::new()).await.unwrap();

    let metrics = executor.metrics();
    let executions = metrics.counter("executions_total");
    assert_eq!(executions.get(), 1);

    let duration_hist = metrics.histogram("program_execution_duration");
    assert_eq!(duration_hist.count(), 1);
}
