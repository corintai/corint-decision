//! Tests for list functionality

use super::backend::ListBackend;
use super::*;
use crate::executor::Executor;
use crate::PipelineExecutor;
use corint_decision_model::ir::{Instruction, Program, ProgramMetadata};
use corint_decision_model::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_memory_backend_basic() {
    let mut backend = MemoryBackend::new();

    // Add some values
    backend
        .add(
            "email_blocklist",
            Value::String("fraud@example.com".to_string()),
        )
        .await
        .unwrap();
    backend
        .add(
            "email_blocklist",
            Value::String("spam@test.com".to_string()),
        )
        .await
        .unwrap();

    // Test contains
    assert!(backend
        .contains(
            "email_blocklist",
            &Value::String("fraud@example.com".to_string())
        )
        .await
        .unwrap());
    assert!(!backend
        .contains(
            "email_blocklist",
            &Value::String("good@example.com".to_string())
        )
        .await
        .unwrap());

    // Test get_all
    let all = backend.get_all("email_blocklist").await.unwrap();
    assert_eq!(all.len(), 2);

    // Test remove
    backend
        .remove(
            "email_blocklist",
            &Value::String("fraud@example.com".to_string()),
        )
        .await
        .unwrap();
    assert!(!backend
        .contains(
            "email_blocklist",
            &Value::String("fraud@example.com".to_string())
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn test_list_service_basic() {
    // Create a service with a pre-registered memory backend for test_list
    let mut backends: HashMap<String, Box<dyn ListBackend>> = HashMap::new();
    backends.insert("test_list".to_string(), Box::new(MemoryBackend::new()));
    let service = ListService::new_with_backends(backends);

    // Initially empty
    assert!(!service
        .contains("test_list", &Value::String("value1".to_string()))
        .await
        .unwrap());

    // Add values
    service
        .add("test_list", Value::String("value1".to_string()))
        .await
        .unwrap();
    service
        .add("test_list", Value::String("value2".to_string()))
        .await
        .unwrap();

    // Check contains
    assert!(service
        .contains("test_list", &Value::String("value1".to_string()))
        .await
        .unwrap());
    assert!(service
        .contains("test_list", &Value::String("value2".to_string()))
        .await
        .unwrap());
    assert!(!service
        .contains("test_list", &Value::String("value3".to_string()))
        .await
        .unwrap());
}

#[tokio::test]
async fn test_list_lookup_instruction_positive() {
    // Create a program with ListLookup instruction
    let program = Program::new(
        vec![
            // Load the value to check (user email)
            Instruction::LoadConst {
                value: Value::String("fraud@example.com".to_string()),
            },
            // Check if it's in the blocklist
            Instruction::ListLookup {
                list_id: "email_blocklist".to_string(),
                negate: false,
            },
            // If true, mark rule triggered
            Instruction::JumpIfFalse { offset: 2 },
            Instruction::MarkRuleTriggered {
                rule_id: "blocked_email".to_string(),
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    // Explicitly configured empty lists remain valid membership sources.
    let event_data = HashMap::new();
    let result = empty_lists_executor()
        .execute(&program, event_data)
        .await
        .unwrap();

    // Since the list is empty, it won't trigger
    assert_eq!(result.triggered_rules.len(), 0);
}

#[tokio::test]
async fn test_list_lookup_instruction_negative() {
    // Test "not in list" (negate=true)
    let program = Program::new(
        vec![
            // Load the value to check (user email)
            Instruction::LoadConst {
                value: Value::String("good@example.com".to_string()),
            },
            // Check if it's NOT in the blocklist (negate=true)
            Instruction::ListLookup {
                list_id: "email_blocklist".to_string(),
                negate: true, // NOT in list
            },
            // If true (not in blocklist), mark as safe
            Instruction::JumpIfFalse { offset: 2 },
            Instruction::MarkRuleTriggered {
                rule_id: "safe_email".to_string(),
            },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let event_data = HashMap::new();
    let result = empty_lists_executor()
        .execute(&program, event_data)
        .await
        .unwrap();

    // Since the list is empty, "not in list" should return true
    assert_eq!(result.triggered_rules.len(), 1);
    assert_eq!(result.triggered_rules[0], "safe_email");
}

#[tokio::test]
async fn test_list_lookup_with_field_access() {
    // Test loading a field from event data and checking against list
    let program = Program::new(
        vec![
            // Load user.email from event
            Instruction::LoadField {
                path: vec!["user_email".to_string()],
            },
            // Check if it's in the blocklist
            Instruction::ListLookup {
                list_id: "email_blocklist".to_string(),
                negate: false,
            },
            // If true, add score
            Instruction::JumpIfFalse { offset: 2 },
            Instruction::SetScore { value: 100 },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let mut event_data = HashMap::new();
    event_data.insert(
        "user_email".to_string(),
        Value::String("test@example.com".to_string()),
    );

    let result = empty_lists_executor()
        .execute(&program, event_data)
        .await
        .unwrap();

    // List is empty, so no score added
    assert_eq!(result.score, 0);
}

#[tokio::test]
async fn test_multiple_list_checks() {
    // Test multiple list lookups in sequence
    let program = Program::new(
        vec![
            // Check if email is in blocklist
            Instruction::LoadConst {
                value: Value::String("spam@example.com".to_string()),
            },
            Instruction::ListLookup {
                list_id: "email_blocklist".to_string(),
                negate: false,
            },
            Instruction::JumpIfFalse { offset: 2 },
            Instruction::AddScore { value: 50 },
            // Check if IP is in blocklist
            Instruction::LoadConst {
                value: Value::String("192.168.1.100".to_string()),
            },
            Instruction::ListLookup {
                list_id: "ip_blocklist".to_string(),
                negate: false,
            },
            Instruction::JumpIfFalse { offset: 2 },
            Instruction::AddScore { value: 50 },
            Instruction::Return,
        ],
        ProgramMetadata::default(),
    );

    let event_data = HashMap::new();
    let result = empty_lists_executor()
        .execute(&program, event_data)
        .await
        .unwrap();

    // Both lists are empty, so no score added
    assert_eq!(result.score, 0);
}

// NOTE: Integration tests that require compiler and parser are in the SDK crate
// These tests verify parsing and compilation of list syntax end-to-end

fn empty_lists_executor() -> PipelineExecutor {
    let backends: HashMap<String, Box<dyn ListBackend>> = ["email_blocklist", "ip_blocklist"]
        .into_iter()
        .map(|id| {
            (
                id.into(),
                Box::new(MemoryBackend::new()) as Box<dyn ListBackend>,
            )
        })
        .collect();
    PipelineExecutor::new_offline()
        .with_list_service(Arc::new(ListService::new_with_backends(backends)))
}

#[tokio::test]
async fn missing_lists_never_become_membership_results() {
    for negate in [false, true] {
        let program = Program::new(
            vec![
                Instruction::LoadConst {
                    value: Value::String("person@example.com".into()),
                },
                Instruction::ListLookup {
                    list_id: "missing_blocklist".into(),
                    negate,
                },
                Instruction::Return,
            ],
            ProgramMetadata::default(),
        );
        // Both public VM entry points must fail, including absent service and
        // an existing service that has no backend for this particular ID.
        for executor in [PipelineExecutor::new_offline(), empty_lists_executor()] {
            let error = executor
                .execute(&program, HashMap::new())
                .await
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("E_LIST_UNAVAILABLE") && error.contains("missing_blocklist"),
                "{error}"
            );
        }
        let error = Executor::execute(&program, HashMap::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("E_LIST_UNAVAILABLE"), "{error}");
    }
}
