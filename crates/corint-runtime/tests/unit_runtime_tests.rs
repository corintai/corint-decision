//! Comprehensive unit tests for corint-runtime
//!
//! Tests all major runtime components to achieve 80%+ coverage

use corint_core::ast::{Signal, Operator, UnaryOperator};
use corint_core::ir::{Instruction, Program, ProgramMetadata};
use corint_core::Value;
use corint_runtime::context::{ContextInput, ExecutionContext};
use corint_runtime::error::{Result, RuntimeError};
use corint_runtime::executor::Executor;
use corint_runtime::result::{DecisionResult, ExecutionResult};
use corint_runtime::validation;
use std::collections::HashMap;

// ========== Context Tests ==========

#[test]
fn test_context_input_builder() {
    let mut event = HashMap::new();
    event.insert("user_id".to_string(), Value::String("123".to_string()));

    let mut features = HashMap::new();
    features.insert("txn_count".to_string(), Value::Number(15.0));

    let mut api = HashMap::new();
    api.insert("risk_score".to_string(), Value::Number(0.75));

    let input = ContextInput::new(event.clone())
        .with_features(features.clone())
        .with_api(api.clone());

    assert_eq!(input.event.len(), 1);
    assert!(input.features.is_some());
    assert!(input.api.is_some());
}

#[test]
fn test_context_creation_from_event() {
    let mut event = HashMap::new();
    event.insert("user_id".to_string(), Value::String("123".to_string()));
    event.insert("amount".to_string(), Value::Number(1000.0));

    let result = ExecutionContext::from_event(event);
    assert!(result.is_ok());

    let ctx = result.unwrap();
    assert_eq!(ctx.stack_depth(), 0);
    assert_eq!(ctx.event.len(), 2);
}

#[test]
fn test_context_multi_namespace_storage() {
    let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    // Store in different namespaces
    ctx.store_feature("user_velocity", Value::Number(25.5));
    ctx.store_api_result("device_fp", Value::Number(0.8));
    ctx.store_service_result("user_profile", Value::String("premium".to_string()));
    ctx.store_llm_result("fraud_check", Value::Bool(false));
    ctx.store_var("threshold", Value::Number(100.0));

    // Verify storage
    assert_eq!(ctx.features.len(), 1);
    assert_eq!(ctx.api.len(), 1);
    assert_eq!(ctx.service.len(), 1);
    assert_eq!(ctx.llm.len(), 1);
    assert_eq!(ctx.vars.len(), 1);

    // Verify retrieval
    let value = ctx.load_field(&[
        String::from("features"),
        String::from("user_velocity"),
    ]);
    assert!(value.is_ok());
    assert_eq!(value.unwrap(), Value::Number(25.5));
}

#[test]
fn test_context_stack_operations() {
    let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    // Push values
    ctx.push(Value::Number(10.0));
    ctx.push(Value::String("test".to_string()));
    ctx.push(Value::Bool(true));

    assert_eq!(ctx.stack_depth(), 3);

    // Peek
    let top = ctx.peek().unwrap();
    assert_eq!(*top, Value::Bool(true));
    assert_eq!(ctx.stack_depth(), 3); // Peek doesn't remove

    // Pop
    let val1 = ctx.pop().unwrap();
    assert_eq!(val1, Value::Bool(true));
    assert_eq!(ctx.stack_depth(), 2);

    let val2 = ctx.pop().unwrap();
    assert_eq!(val2, Value::String("test".to_string()));

    let val3 = ctx.pop().unwrap();
    assert_eq!(val3, Value::Number(10.0));

    assert_eq!(ctx.stack_depth(), 0);
}

#[test]
fn test_context_stack_underflow() {
    let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    // Try to pop from empty stack
    let result = ctx.pop();
    assert!(result.is_err());
    assert!(matches!(result.err().unwrap(), RuntimeError::StackUnderflow));
}

#[test]
fn test_context_dup_operation() {
    let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    ctx.push(Value::Number(42.0));
    ctx.dup().unwrap();

    assert_eq!(ctx.stack_depth(), 2);
    assert_eq!(ctx.pop().unwrap(), Value::Number(42.0));
    assert_eq!(ctx.pop().unwrap(), Value::Number(42.0));
}

#[test]
fn test_context_swap_operation() {
    let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    ctx.push(Value::Number(1.0));
    ctx.push(Value::Number(2.0));
    ctx.swap().unwrap();

    assert_eq!(ctx.pop().unwrap(), Value::Number(1.0));
    assert_eq!(ctx.pop().unwrap(), Value::Number(2.0));
}

#[test]
fn test_context_field_lookup_event_namespace() {
    let mut event = HashMap::new();
    event.insert("user_id".to_string(), Value::String("123".to_string()));

    let mut user = HashMap::new();
    user.insert("age".to_string(), Value::Number(30.0));
    user.insert("country".to_string(), Value::String("US".to_string()));
    event.insert("user".to_string(), Value::Object(user));

    let ctx = ExecutionContext::from_event(event).unwrap();

    // Simple field access
    let value = ctx.load_field(&[String::from("event"), String::from("user_id")]);
    assert_eq!(value.unwrap(), Value::String("123".to_string()));

    // Nested field access
    let value = ctx.load_field(&[
        String::from("event"),
        String::from("user"),
        String::from("age"),
    ]);
    assert_eq!(value.unwrap(), Value::Number(30.0));
}

#[test]
fn test_context_field_not_found_returns_null() {
    let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    let result = ctx.load_field(&[String::from("nonexistent")]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Null);
}

#[test]
fn test_context_sys_namespace_exists() {
    let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    // Check core sys variables
    assert!(ctx.sys.contains_key("request_id"));
    assert!(ctx.sys.contains_key("timestamp"));
    assert!(ctx.sys.contains_key("year"));
    assert!(ctx.sys.contains_key("month"));
    assert!(ctx.sys.contains_key("day"));
    assert!(ctx.sys.contains_key("hour"));
    assert!(ctx.sys.contains_key("minute"));
    assert!(ctx.sys.contains_key("is_weekend"));
    assert!(ctx.sys.contains_key("day_of_week"));
}

#[test]
fn test_context_env_namespace_defaults() {
    let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    assert!(ctx.env.contains_key("max_score"));
    assert!(ctx.env.contains_key("default_action"));
    assert!(ctx.env.contains_key("feature_flags"));

    // Access via load_field
    let value = ctx.load_field(&[String::from("env"), String::from("max_score")]);
    assert_eq!(value.unwrap(), Value::Number(100.0));
}

#[test]
fn test_context_score_operations() {
    let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    ctx.set_score(50);
    assert_eq!(ctx.result.score, 50);

    ctx.add_score(25);
    assert_eq!(ctx.result.score, 75);

    ctx.add_score(-10);
    assert_eq!(ctx.result.score, 65);
}

#[test]
fn test_context_rule_triggering() {
    let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    ctx.mark_rule_triggered("rule_1".to_string());
    ctx.mark_rule_triggered("rule_2".to_string());

    assert_eq!(ctx.result.triggered_rules.len(), 2);
    assert_eq!(ctx.result.triggered_rules[0], "rule_1");
    assert_eq!(ctx.result.triggered_rules[1], "rule_2");
}

#[test]
fn test_context_variable_operations() {
    let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    ctx.store_variable("temp".to_string(), Value::Number(42.0));
    ctx.store_variable("name".to_string(), Value::String("test".to_string()));

    let value = ctx.load_variable("temp");
    assert!(value.is_ok());
    assert_eq!(value.unwrap(), Value::Number(42.0));

    let value = ctx.load_variable("nonexistent");
    assert!(value.is_err());
}

#[test]
fn test_context_into_decision_result() {
    let mut ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

    ctx.set_score(75);
    ctx.mark_rule_triggered("rule_1".to_string());
    ctx.set_signal(Signal::Review);
    ctx.store_feature("user_count", Value::Number(10.0));

    let decision = ctx.into_decision_result();

    assert_eq!(decision.score, 75);
    assert_eq!(decision.triggered_rules.len(), 1);
    assert_eq!(decision.signal, Some(Signal::Review));
    assert!(decision.context.contains_key("user_count"));
}

// ========== Executor Tests ==========

#[tokio::test]
async fn test_executor_load_const() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(42.0),
            },
            Instruction::LoadConst {
                value: Value::String("hello".to_string()),
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_load_field() {
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(1000.0));
    event.insert("user_id".to_string(), Value::String("123".to_string()));

    let program = Program::new(
        vec![
            Instruction::LoadField {
                path: vec!["amount".to_string()],
            },
            Instruction::LoadField {
                path: vec!["user_id".to_string()],
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_binary_arithmetic() {
    let program = Program::new(
        vec![
            // 10 + 5
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::LoadConst {
                value: Value::Number(5.0),
            },
            Instruction::BinaryOp { op: Operator::Add },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_binary_subtraction() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(20.0),
            },
            Instruction::LoadConst {
                value: Value::Number(8.0),
            },
            Instruction::BinaryOp { op: Operator::Sub },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_binary_multiplication() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(6.0),
            },
            Instruction::LoadConst {
                value: Value::Number(7.0),
            },
            Instruction::BinaryOp { op: Operator::Mul },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_binary_division() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(100.0),
            },
            Instruction::LoadConst {
                value: Value::Number(4.0),
            },
            Instruction::BinaryOp { op: Operator::Div },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_division_by_zero() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::LoadConst {
                value: Value::Number(0.0),
            },
            Instruction::BinaryOp { op: Operator::Div },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_executor_binary_modulo() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(17.0),
            },
            Instruction::LoadConst {
                value: Value::Number(5.0),
            },
            Instruction::BinaryOp { op: Operator::Mod },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_binary_logical_and() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Bool(true),
            },
            Instruction::LoadConst {
                value: Value::Bool(false),
            },
            Instruction::BinaryOp { op: Operator::And },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_binary_logical_or() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Bool(true),
            },
            Instruction::LoadConst {
                value: Value::Bool(false),
            },
            Instruction::BinaryOp { op: Operator::Or },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_compare_greater_than() {
    let mut event = HashMap::new();
    event.insert("age".to_string(), Value::Number(25.0));

    let program = Program::new(
        vec![
            Instruction::LoadField {
                path: vec!["age".to_string()],
            },
            Instruction::LoadConst {
                value: Value::Number(18.0),
            },
            Instruction::Compare { op: Operator::Gt },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_compare_less_than() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(5.0),
            },
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::Compare { op: Operator::Lt },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_compare_equal() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::String("test".to_string()),
            },
            Instruction::LoadConst {
                value: Value::String("test".to_string()),
            },
            Instruction::Compare { op: Operator::Eq },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_compare_not_equal() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(5.0),
            },
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::Compare { op: Operator::Ne },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_compare_greater_or_equal() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::Compare { op: Operator::Ge },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_compare_less_or_equal() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(5.0),
            },
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::Compare { op: Operator::Le },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_unary_not() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Bool(true),
            },
            Instruction::UnaryOp {
                op: UnaryOperator::Not,
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_unary_negate() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(42.0),
            },
            Instruction::UnaryOp {
                op: UnaryOperator::Negate,
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_jump() {
    let program = Program::new(
        vec![
            Instruction::Jump { offset: 2 }, // Skip next instruction
            Instruction::SetScore { value: 100 }, // Should be skipped
            Instruction::SetScore { value: 50 }, // Should execute
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 50);
}

#[tokio::test]
async fn test_executor_jump_if_true() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Bool(true),
            },
            Instruction::JumpIfTrue { offset: 2 }, // Skip next instruction
            Instruction::SetScore { value: 100 }, // Should be skipped
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 0); // Score should not be set
}

#[tokio::test]
async fn test_executor_jump_if_false() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Bool(false),
            },
            Instruction::JumpIfFalse { offset: 2 }, // Skip next instruction
            Instruction::SetScore { value: 100 },   // Should be skipped
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 0);
}

#[tokio::test]
async fn test_executor_set_score() {
    let program = Program::new(
        vec![
            Instruction::SetScore { value: 75 },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 75);
}

#[tokio::test]
async fn test_executor_add_score() {
    let program = Program::new(
        vec![
            Instruction::AddScore { value: 30 },
            Instruction::AddScore { value: 20 },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.score, 50);
}

#[tokio::test]
async fn test_executor_mark_rule_triggered() {
    let program = Program::new(
        vec![
            Instruction::MarkRuleTriggered {
                rule_id: "rule_1".to_string(),
            },
            Instruction::MarkRuleTriggered {
                rule_id: "rule_2".to_string(),
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await.unwrap();
    assert_eq!(result.triggered_rules.len(), 2);
    assert_eq!(result.triggered_rules[0], "rule_1");
    assert_eq!(result.triggered_rules[1], "rule_2");
}

#[tokio::test]
async fn test_executor_stack_dup() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(42.0),
            },
            Instruction::Dup,
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_stack_pop() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(42.0),
            },
            Instruction::Pop,
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_stack_swap() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(1.0),
            },
            Instruction::LoadConst {
                value: Value::Number(2.0),
            },
            Instruction::Swap,
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_store_and_load_variable() {
    let program = Program::new(
        vec![
            Instruction::LoadConst {
                value: Value::Number(100.0),
            },
            Instruction::Store {
                name: "threshold".to_string(),
            },
            Instruction::Load {
                name: "threshold".to_string(),
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_complete_rule_simulation() {
    // Simulates: if age > 18 then score = 50 and mark rule triggered
    let mut event = HashMap::new();
    event.insert("age".to_string(), Value::Number(25.0));

    let program = Program::new(
        vec![
            // Load age
            Instruction::LoadField {
                path: vec!["age".to_string()],
            },
            // Load 18
            Instruction::LoadConst {
                value: Value::Number(18.0),
            },
            // Compare: age > 18
            Instruction::Compare { op: Operator::Gt },
            // If false, skip to end
            Instruction::JumpIfFalse { offset: 3 },
            // Set score
            Instruction::SetScore { value: 50 },
            // Mark rule triggered
            Instruction::MarkRuleTriggered {
                rule_id: "age_check".to_string(),
            },
            // Return
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, event).await.unwrap();
    assert_eq!(result.score, 50);
    assert_eq!(result.triggered_rules.len(), 1);
    assert_eq!(result.triggered_rules[0], "age_check");
}

#[tokio::test]
async fn test_executor_check_event_type_match() {
    let mut event = HashMap::new();
    event.insert("event_type".to_string(), Value::String("login".to_string()));

    let program = Program::new(
        vec![
            Instruction::CheckEventType {
                expected: "login".to_string(),
            },
            Instruction::SetScore { value: 10 },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, event).await.unwrap();
    assert_eq!(result.score, 10);
}

#[tokio::test]
async fn test_executor_check_event_type_mismatch() {
    let mut event = HashMap::new();
    event.insert(
        "event_type".to_string(),
        Value::String("payment".to_string()),
    );

    let program = Program::new(
        vec![
            Instruction::CheckEventType {
                expected: "login".to_string(),
            },
            Instruction::SetScore { value: 10 }, // Should not execute
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, event).await.unwrap();
    assert_eq!(result.score, 0); // Score should not be set
}

// ========== Result Tests ==========

#[test]
fn test_execution_result_creation() {
    let result = ExecutionResult::new();
    assert_eq!(result.score, 0);
    assert_eq!(result.triggered_rules.len(), 0);
    assert!(result.signal.is_none());
    assert_eq!(result.variables.len(), 0);
}

#[test]
fn test_execution_result_score_operations() {
    let mut result = ExecutionResult::new();

    result.add_score(50);
    assert_eq!(result.score, 50);

    result.add_score(25);
    assert_eq!(result.score, 75);

    result.set_score(100);
    assert_eq!(result.score, 100);
}

#[test]
fn test_execution_result_rule_triggering() {
    let mut result = ExecutionResult::new();

    result.mark_rule_triggered("rule_1".to_string());
    result.mark_rule_triggered("rule_2".to_string());
    result.mark_rule_triggered("rule_3".to_string());

    assert_eq!(result.triggered_rules.len(), 3);
    assert_eq!(result.triggered_rules[0], "rule_1");
    assert_eq!(result.triggered_rules[2], "rule_3");
}

#[test]
fn test_execution_result_variable_storage() {
    let mut result = ExecutionResult::new();

    result.store_variable("temp".to_string(), Value::Number(42.0));
    result.store_variable("name".to_string(), Value::String("test".to_string()));
    result.store_variable("active".to_string(), Value::Bool(true));

    assert_eq!(result.variables.len(), 3);
    assert_eq!(result.load_variable("temp"), Some(&Value::Number(42.0)));
    assert_eq!(
        result.load_variable("name"),
        Some(&Value::String("test".to_string()))
    );
    assert_eq!(result.load_variable("active"), Some(&Value::Bool(true)));
    assert_eq!(result.load_variable("nonexistent"), None);
}

#[test]
fn test_decision_result_creation() {
    let result = DecisionResult::new(Signal::Approve, 0);
    assert_eq!(result.signal, Some(Signal::Approve));
    assert_eq!(result.score, 0);
    assert_eq!(result.triggered_rules.len(), 0);
    assert_eq!(result.context.len(), 0);
}

#[test]
fn test_decision_result_with_triggered_rules() {
    let mut result = DecisionResult::new(Signal::Review, 75);

    result.add_triggered_rule("rule_1".to_string());
    result.add_triggered_rule("rule_2".to_string());

    assert_eq!(result.triggered_rules.len(), 2);
}

#[test]
fn test_decision_result_with_explanation() {
    let result = DecisionResult::new(Signal::Decline, 100)
        .with_explanation("High risk score detected".to_string());

    assert_eq!(result.explanation, "High risk score detected");
}

#[test]
fn test_decision_result_with_context() {
    let mut result = DecisionResult::new(Signal::Hold, 50);

    result.add_context("user_id".to_string(), Value::String("123".to_string()));
    result.add_context("risk_level".to_string(), Value::String("medium".to_string()));

    assert_eq!(result.context.len(), 2);
    assert_eq!(
        result.context.get("user_id"),
        Some(&Value::String("123".to_string()))
    );
}

// ========== Validation Tests ==========

#[test]
fn test_validation_valid_event() {
    let mut event = HashMap::new();
    event.insert("user_id".to_string(), Value::String("123".to_string()));
    event.insert("amount".to_string(), Value::Number(1000.0));
    event.insert("custom_field".to_string(), Value::String("data".to_string()));

    assert!(validation::validate_event_data(&event).is_ok());
}

#[test]
fn test_validation_reserved_field_total_score() {
    let mut event = HashMap::new();
    event.insert("total_score".to_string(), Value::Number(100.0));

    let result = validation::validate_event_data(&event);
    assert!(result.is_err());
}

#[test]
fn test_validation_reserved_field_triggered_rules() {
    let mut event = HashMap::new();
    event.insert(
        "triggered_rules".to_string(),
        Value::Array(vec![Value::String("rule1".to_string())]),
    );

    assert!(validation::validate_event_data(&event).is_err());
}

#[test]
fn test_validation_reserved_field_action() {
    let mut event = HashMap::new();
    event.insert("action".to_string(), Value::String("approve".to_string()));

    assert!(validation::validate_event_data(&event).is_err());
}

#[test]
fn test_validation_reserved_prefix_sys() {
    let mut event = HashMap::new();
    event.insert("sys_custom".to_string(), Value::String("test".to_string()));

    assert!(validation::validate_event_data(&event).is_err());
}

#[test]
fn test_validation_reserved_prefix_features() {
    let mut event = HashMap::new();
    event.insert("features_count".to_string(), Value::Number(10.0));

    assert!(validation::validate_event_data(&event).is_err());
}

#[test]
fn test_validation_reserved_prefix_api() {
    let mut event = HashMap::new();
    event.insert("api_result".to_string(), Value::String("data".to_string()));

    assert!(validation::validate_event_data(&event).is_err());
}

#[test]
fn test_validation_reserved_prefix_service() {
    let mut event = HashMap::new();
    event.insert("service_data".to_string(), Value::Number(42.0));

    assert!(validation::validate_event_data(&event).is_err());
}

#[test]
fn test_validation_reserved_prefix_llm() {
    let mut event = HashMap::new();
    event.insert("llm_analysis".to_string(), Value::String("result".to_string()));

    assert!(validation::validate_event_data(&event).is_err());
}

#[test]
fn test_validation_nested_reserved_field() {
    let mut event = HashMap::new();
    let mut nested = HashMap::new();
    nested.insert("total_score".to_string(), Value::Number(50.0));
    event.insert("data".to_string(), Value::Object(nested));

    assert!(validation::validate_event_data(&event).is_err());
}

#[test]
fn test_validation_is_reserved_field() {
    assert!(validation::is_reserved_field("total_score"));
    assert!(validation::is_reserved_field("triggered_rules"));
    assert!(validation::is_reserved_field("sys_custom"));
    assert!(validation::is_reserved_field("features_data"));
    assert!(validation::is_reserved_field("api_result"));

    assert!(!validation::is_reserved_field("user_id"));
    assert!(!validation::is_reserved_field("amount"));
    assert!(!validation::is_reserved_field("system")); // "system" is ok, "sys_" is not
}

// ========== Error Tests ==========

#[test]
fn test_error_stack_underflow() {
    let error = RuntimeError::StackUnderflow;
    assert_eq!(error.to_string(), "Stack underflow");
}

#[test]
fn test_error_type_error() {
    let error = RuntimeError::TypeError("Cannot add string and number".to_string());
    assert!(error.to_string().contains("Type error"));
}

#[test]
fn test_error_field_not_found() {
    let error = RuntimeError::FieldNotFound("user.email".to_string());
    assert!(error.to_string().contains("Field not found"));
}

#[test]
fn test_error_division_by_zero() {
    let error = RuntimeError::DivisionByZero;
    assert_eq!(error.to_string(), "Division by zero");
}

#[test]
fn test_error_reserved_field() {
    let error = RuntimeError::ReservedField {
        field: "total_score".to_string(),
        reason: "System reserved field".to_string(),
    };
    assert!(error.to_string().contains("total_score"));
    assert!(error.to_string().contains("Reserved field"));
}

// ========== Integration Tests ==========

#[tokio::test]
async fn test_integration_simple_decision_flow() {
    // Create event data
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(5000.0));
    event.insert("user_age".to_string(), Value::Number(25.0));

    // Create program: if amount > 1000 then score = 50
    let program = Program::new(
        vec![
            Instruction::LoadField {
                path: vec!["amount".to_string()],
            },
            Instruction::LoadConst {
                value: Value::Number(1000.0),
            },
            Instruction::Compare { op: Operator::Gt },
            Instruction::JumpIfFalse { offset: 3 },
            Instruction::SetScore { value: 50 },
            Instruction::MarkRuleTriggered {
                rule_id: "high_amount".to_string(),
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, event).await.unwrap();
    assert_eq!(result.score, 50);
    assert_eq!(result.triggered_rules.len(), 1);
}

#[tokio::test]
async fn test_integration_multiple_rules() {
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(5000.0));
    event.insert("user_age".to_string(), Value::Number(17.0));

    let program = Program::new(
        vec![
            // Rule 1: amount > 1000 => +50
            Instruction::LoadField {
                path: vec!["amount".to_string()],
            },
            Instruction::LoadConst {
                value: Value::Number(1000.0),
            },
            Instruction::Compare { op: Operator::Gt },
            Instruction::JumpIfFalse { offset: 3 },
            Instruction::AddScore { value: 50 },
            Instruction::MarkRuleTriggered {
                rule_id: "high_amount".to_string(),
            },
            // Rule 2: user_age < 18 => +30
            Instruction::LoadField {
                path: vec!["user_age".to_string()],
            },
            Instruction::LoadConst {
                value: Value::Number(18.0),
            },
            Instruction::Compare { op: Operator::Lt },
            Instruction::JumpIfFalse { offset: 3 },
            Instruction::AddScore { value: 30 },
            Instruction::MarkRuleTriggered {
                rule_id: "underage".to_string(),
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let result = Executor::execute(&program, event).await.unwrap();
    assert_eq!(result.score, 80); // 50 + 30
    assert_eq!(result.triggered_rules.len(), 2);
}

#[tokio::test]
async fn test_integration_context_multi_namespace() {
    let mut event = HashMap::new();
    event.insert("user_id".to_string(), Value::String("123".to_string()));

    let mut features = HashMap::new();
    features.insert("txn_count_7d".to_string(), Value::Number(25.0));

    let input = ContextInput::new(event).with_features(features);

    let ctx = ExecutionContext::new(input).unwrap();

    // Verify event namespace
    let value = ctx.load_field(&[String::from("event"), String::from("user_id")]);
    assert_eq!(value.unwrap(), Value::String("123".to_string()));

    // Verify features namespace
    let value = ctx.load_field(&[
        String::from("features"),
        String::from("txn_count_7d"),
    ]);
    assert_eq!(value.unwrap(), Value::Number(25.0));
}
