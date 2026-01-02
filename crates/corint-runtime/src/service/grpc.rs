//! gRPC service client

use crate::error::Result;
use crate::service::client::{ServiceClient, ServiceRequest, ServiceResponse};
use async_trait::async_trait;
use corint_core::Value;
use std::collections::HashMap;

/// gRPC client trait
#[async_trait]
pub trait GrpcClient: Send + Sync {
    /// Call a gRPC method
    async fn call_method(
        &self,
        service: String,
        method: String,
        params: HashMap<String, Value>,
    ) -> Result<ServiceResponse>;
}

/// Mock gRPC client for testing
pub struct MockGrpcClient {
    name: String,
    default_response: Value,
}

impl MockGrpcClient {
    /// Create a new mock gRPC client
    pub fn new() -> Self {
        Self {
            name: "mock_grpc".to_string(),
            default_response: Value::Object(HashMap::new()),
        }
    }

    /// Create with custom default response
    pub fn with_response(response: Value) -> Self {
        Self {
            name: "mock_grpc".to_string(),
            default_response: response,
        }
    }

    /// Create with custom name
    pub fn with_name(name: String) -> Self {
        Self {
            name,
            default_response: Value::Object(HashMap::new()),
        }
    }
}

impl Default for MockGrpcClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GrpcClient for MockGrpcClient {
    async fn call_method(
        &self,
        service: String,
        method: String,
        params: HashMap<String, Value>,
    ) -> Result<ServiceResponse> {
        // Create response with metadata
        let mut metadata = HashMap::new();
        metadata.insert("service".to_string(), Value::String(service.clone()));
        metadata.insert("method".to_string(), Value::String(method.clone()));
        metadata.insert("grpc_status".to_string(), Value::Number(0.0)); // OK status

        // Add param count to metadata
        metadata.insert(
            "param_count".to_string(),
            Value::Number(params.len() as f64),
        );

        Ok(ServiceResponse {
            data: self.default_response.clone(),
            status: "success".to_string(),
            metadata,
        })
    }
}

#[async_trait]
impl ServiceClient for MockGrpcClient {
    async fn call(&self, request: ServiceRequest) -> Result<ServiceResponse> {
        // Extract service and method from operation
        // Format: "ServiceName.MethodName" or just use operation as method
        let (service, method) = if request.operation.contains('.') {
            let parts: Vec<&str> = request.operation.split('.').collect();
            if parts.len() >= 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                ("UnknownService".to_string(), request.operation.clone())
            }
        } else {
            ("UnknownService".to_string(), request.operation.clone())
        };

        self.call_method(service, method, request.params).await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_grpc_client_new() {
        let client = MockGrpcClient::new();
        assert_eq!(client.name, "mock_grpc");
    }

    #[tokio::test]
    async fn test_mock_grpc_client_with_response() {
        let mut response_data = HashMap::new();
        response_data.insert("score".to_string(), Value::Number(0.85));
        let response = Value::Object(response_data);

        let client = MockGrpcClient::with_response(response.clone());
        let result = client
            .call_method(
                "RiskScoringService".to_string(),
                "CalculateScore".to_string(),
                HashMap::new(),
            )
            .await
            .unwrap();

        assert_eq!(result.data, response);
    }

    #[tokio::test]
    async fn test_mock_grpc_client_call_method() {
        let client = MockGrpcClient::new();
        let mut params = HashMap::new();
        params.insert("user_id".to_string(), Value::String("user123".to_string()));

        let result = client
            .call_method(
                "RiskScoringService".to_string(),
                "CalculateScore".to_string(),
                params,
            )
            .await
            .unwrap();

        // Check metadata
        assert_eq!(
            result.metadata.get("service"),
            Some(&Value::String("RiskScoringService".to_string()))
        );
        assert_eq!(
            result.metadata.get("method"),
            Some(&Value::String("CalculateScore".to_string()))
        );
        assert_eq!(
            result.metadata.get("grpc_status"),
            Some(&Value::Number(0.0))
        );
        assert_eq!(
            result.metadata.get("param_count"),
            Some(&Value::Number(1.0))
        );
    }

    #[tokio::test]
    async fn test_service_client_implementation() {
        let client = MockGrpcClient::new();
        let request = ServiceRequest {
            service: "risk_scoring".to_string(),
            operation: "RiskScoringService.CalculateScore".to_string(),
            params: HashMap::new(),
        };

        let result = client.call(request).await.unwrap();

        // Verify service and method were parsed correctly
        assert_eq!(
            result.metadata.get("service"),
            Some(&Value::String("RiskScoringService".to_string()))
        );
        assert_eq!(
            result.metadata.get("method"),
            Some(&Value::String("CalculateScore".to_string()))
        );
    }

    #[tokio::test]
    async fn test_service_client_without_dot_separator() {
        let client = MockGrpcClient::new();
        let request = ServiceRequest {
            service: "risk_scoring".to_string(),
            operation: "CalculateScore".to_string(),
            params: HashMap::new(),
        };

        let result = client.call(request).await.unwrap();

        // When no dot separator, should use UnknownService
        assert_eq!(
            result.metadata.get("service"),
            Some(&Value::String("UnknownService".to_string()))
        );
        assert_eq!(
            result.metadata.get("method"),
            Some(&Value::String("CalculateScore".to_string()))
        );
    }

    #[tokio::test]
    async fn test_mock_grpc_client_with_name() {
        let client = MockGrpcClient::with_name("custom_grpc".to_string());
        assert_eq!(client.name, "custom_grpc");
    }

    #[tokio::test]
    async fn test_mock_grpc_client_default() {
        let client = MockGrpcClient::default();
        assert_eq!(client.name, "mock_grpc");
    }
}
