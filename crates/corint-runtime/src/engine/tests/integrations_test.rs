//! Integration tests for LLM and Service clients in pipeline executor

use crate::engine::PipelineExecutor;
use crate::llm::MockProvider;
use crate::service::http::MockHttpClient;
use std::sync::Arc;

#[tokio::test]
async fn test_llm_integration() {
    let llm_client = Arc::new(MockProvider::with_response("Risk detected".to_string()));
    let executor = PipelineExecutor::new().with_llm_client(llm_client);

    // We need to check CallLLM instruction structure first
    // For now, just test that the executor can be created with LLM client
    assert!(executor.llm_client.is_some());
}

#[tokio::test]
async fn test_service_integration() {
    let service_client = Arc::new(MockHttpClient::new());
    let executor = PipelineExecutor::new().with_service_client(service_client);

    assert!(executor.service_client.is_some());
}
