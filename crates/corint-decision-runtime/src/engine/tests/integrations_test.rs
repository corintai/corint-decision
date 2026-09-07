//! Offline execution must not admit unbound service invocations.
use crate::engine::PipelineExecutor;
use corint_decision_model::ir::{Instruction, Program, ProgramMetadata};
use std::collections::HashMap;

#[tokio::test]
async fn offline_executor_rejects_service_invocations() {
    let executor = PipelineExecutor::new_offline();
    assert!(executor.http_service_client.is_none());
    assert!(executor.services.is_empty());
    let program = Program::new(
        vec![Instruction::InvokeService {
            service: "missing".into(),
            operation: "lookup".into(),
            parameter_names: vec![],
            timeout_ms: None,
        }],
        ProgramMetadata::for_rule("offline_test".into()),
    );
    assert!(executor
        .execute(&program, HashMap::new())
        .await
        .unwrap_err()
        .to_string()
        .contains("No service binding"));
}
