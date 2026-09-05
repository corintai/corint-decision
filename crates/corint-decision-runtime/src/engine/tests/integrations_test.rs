//! Integration tests for Service clients in pipeline executor

use crate::engine::PipelineExecutor;
use crate::service::http::MockHttpClient;
use std::sync::Arc;

#[tokio::test]
async fn test_service_integration() {
    let service_client = Arc::new(MockHttpClient::new());
    let executor = PipelineExecutor::new().with_service_client(service_client);

    assert!(executor.service_client.is_some());
}

#[tokio::test]
async fn offline_executor_does_not_create_connectors_or_use_api_fallback() {
    use corint_decision_model::ir::{Instruction, Program, ProgramMetadata};
    use corint_decision_model::Value;
    use std::collections::HashMap;
    let executor = PipelineExecutor::new_offline();
    assert!(executor.external_api_client.is_none());
    assert!(executor.service_client.is_none());
    assert!(executor.feature_executor.is_none());
    assert!(executor.list_service.is_none());
    let program = Program::new(
        vec![Instruction::CallExternal {
            api: "must_not_run".into(),
            endpoint: "test".into(),
            params: HashMap::new(),
            timeout: None,
            fallback: Some(Value::Number(0.0)),
        }],
        ProgramMetadata::for_rule("offline_test".into()),
    );
    let error = executor
        .execute(&program, HashMap::new())
        .await
        .unwrap_err();
    assert!(
        matches!(error, crate::RuntimeError::InvalidOperation(message) if message == "External APIs are disabled in the offline executor")
    );
}
