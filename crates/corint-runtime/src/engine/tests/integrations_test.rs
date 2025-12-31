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
