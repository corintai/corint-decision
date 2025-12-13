//! Service client interface and types

use crate::error::Result;
use async_trait::async_trait;
use corint_core::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request to an external service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRequest {
    /// Service name/identifier
    pub service: String,

    /// Operation/method to call
    pub operation: String,

    /// Request parameters
    pub params: HashMap<String, Value>,
}

impl ServiceRequest {
    /// Create a new service request
    pub fn new(service: String, operation: String) -> Self {
        Self {
            service,
            operation,
            params: HashMap::new(),
        }
    }

    /// Add a parameter
    pub fn with_param(mut self, key: String, value: Value) -> Self {
        self.params.insert(key, value);
        self
    }

    /// Add multiple parameters
    pub fn with_params(mut self, params: HashMap<String, Value>) -> Self {
        self.params.extend(params);
        self
    }
}

/// Response from an external service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceResponse {
    /// The response data
    pub data: Value,

    /// Status code or result indicator
    pub status: String,

    /// Additional metadata
    pub metadata: HashMap<String, Value>,
}

impl ServiceResponse {
    /// Create a new service response
    pub fn new(data: Value) -> Self {
        Self {
            data,
            status: "success".to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Set status
    pub fn with_status(mut self, status: String) -> Self {
        self.status = status;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Async service client trait
#[async_trait]
pub trait ServiceClient: Send + Sync {
    /// Call the service with a request
    async fn call(&self, request: ServiceRequest) -> Result<ServiceResponse>;

    /// Get the name of this client
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_request_builder() {
        let mut params = HashMap::new();
        params.insert("key1".to_string(), Value::String("value1".to_string()));

        let request = ServiceRequest::new("test_service".to_string(), "get".to_string())
            .with_param("key2".to_string(), Value::Number(42.0))
            .with_params(params);

        assert_eq!(request.service, "test_service");
        assert_eq!(request.operation, "get");
        assert_eq!(request.params.len(), 2);
    }

    #[test]
    fn test_service_response_builder() {
        let response = ServiceResponse::new(Value::String("result".to_string()))
            .with_status("ok".to_string())
            .with_metadata("duration_ms".to_string(), Value::Number(123.0));

        assert_eq!(response.status, "ok");
        assert_eq!(response.metadata.len(), 1);
    }
}
