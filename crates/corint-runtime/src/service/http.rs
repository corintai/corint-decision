//! HTTP service client

use crate::error::Result;
use crate::service::client::{ServiceClient, ServiceRequest, ServiceResponse};
use async_trait::async_trait;
use corint_core::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// HTTP method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

/// HTTP client trait
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// Make an HTTP request
    async fn request(
        &self,
        method: HttpMethod,
        url: String,
        headers: HashMap<String, String>,
        body: Option<Value>,
    ) -> Result<ServiceResponse>;
}

/// Mock HTTP client for testing
pub struct MockHttpClient {
    name: String,
    default_response: Value,
}

impl MockHttpClient {
    /// Create a new mock HTTP client
    pub fn new() -> Self {
        Self {
            name: "mock_http".to_string(),
            default_response: Value::Object(HashMap::new()),
        }
    }

    /// Create with custom default response
    pub fn with_response(response: Value) -> Self {
        Self {
            name: "mock_http".to_string(),
            default_response: response,
        }
    }
}

impl Default for MockHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for MockHttpClient {
    async fn request(
        &self,
        _method: HttpMethod,
        url: String,
        _headers: HashMap<String, String>,
        _body: Option<Value>,
    ) -> Result<ServiceResponse> {
        Ok(ServiceResponse::new(self.default_response.clone())
            .with_metadata("url".to_string(), Value::String(url))
            .with_metadata("status_code".to_string(), Value::Number(200.0)))
    }
}

#[async_trait]
impl ServiceClient for MockHttpClient {
    async fn call(&self, request: ServiceRequest) -> Result<ServiceResponse> {
        let method = match request.operation.to_uppercase().as_str() {
            "GET" => HttpMethod::GET,
            "POST" => HttpMethod::POST,
            "PUT" => HttpMethod::PUT,
            "DELETE" => HttpMethod::DELETE,
            "PATCH" => HttpMethod::PATCH,
            _ => HttpMethod::GET,
        };

        let url = request
            .params
            .get("url")
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let body = request.params.get("body").cloned();

        self.request(method, url, HashMap::new(), body).await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_get() {
        let client = MockHttpClient::new();

        let response = client
            .request(
                HttpMethod::GET,
                "https://api.example.com/users".to_string(),
                HashMap::new(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_http_post() {
        let mut body = HashMap::new();
        body.insert("name".to_string(), Value::String("Alice".to_string()));

        let client = MockHttpClient::new();

        let response = client
            .request(
                HttpMethod::POST,
                "https://api.example.com/users".to_string(),
                HashMap::new(),
                Some(Value::Object(body)),
            )
            .await
            .unwrap();

        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_http_service_client() {
        let client = MockHttpClient::new();

        let request = ServiceRequest::new("http".to_string(), "GET".to_string()).with_param(
            "url".to_string(),
            Value::String("https://api.example.com/data".to_string()),
        );

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_http_custom_response() {
        let mut custom_data = HashMap::new();
        custom_data.insert(
            "message".to_string(),
            Value::String("Custom response".to_string()),
        );

        let client = MockHttpClient::with_response(Value::Object(custom_data));

        let response = client
            .request(
                HttpMethod::GET,
                "https://test.com".to_string(),
                HashMap::new(),
                None,
            )
            .await
            .unwrap();

        if let Value::Object(data) = &response.data {
            assert_eq!(
                data.get("message"),
                Some(&Value::String("Custom response".to_string()))
            );
        } else {
            panic!("Expected object response");
        }
    }

    #[tokio::test]
    async fn test_http_put_method() {
        let client = MockHttpClient::new();

        let request = ServiceRequest::new("http".to_string(), "PUT".to_string()).with_param(
            "url".to_string(),
            Value::String("https://api.example.com/users/1".to_string()),
        );

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_http_delete_method() {
        let client = MockHttpClient::new();

        let request = ServiceRequest::new("http".to_string(), "DELETE".to_string()).with_param(
            "url".to_string(),
            Value::String("https://api.example.com/users/1".to_string()),
        );

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_http_patch_method() {
        let client = MockHttpClient::new();

        let request = ServiceRequest::new("http".to_string(), "PATCH".to_string()).with_param(
            "url".to_string(),
            Value::String("https://api.example.com/users/1".to_string()),
        );

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_http_unknown_method_defaults_to_get() {
        let client = MockHttpClient::new();

        let request = ServiceRequest::new("http".to_string(), "UNKNOWN".to_string()).with_param(
            "url".to_string(),
            Value::String("https://api.example.com/data".to_string()),
        );

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_http_client_name() {
        let client = MockHttpClient::new();
        assert_eq!(client.name(), "mock_http");
    }

    #[tokio::test]
    async fn test_http_with_body() {
        let client = MockHttpClient::new();

        let mut body_data = HashMap::new();
        body_data.insert("user_id".to_string(), Value::String("user123".to_string()));
        body_data.insert("amount".to_string(), Value::Number(100.0));

        let request = ServiceRequest::new("http".to_string(), "POST".to_string())
            .with_param(
                "url".to_string(),
                Value::String("https://api.example.com/transactions".to_string()),
            )
            .with_param("body".to_string(), Value::Object(body_data));

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_http_metadata_includes_url() {
        let client = MockHttpClient::new();

        let test_url = "https://api.example.com/test".to_string();
        let response = client
            .request(HttpMethod::GET, test_url.clone(), HashMap::new(), None)
            .await
            .unwrap();

        assert_eq!(response.metadata.get("url"), Some(&Value::String(test_url)));
    }

    #[tokio::test]
    async fn test_http_metadata_includes_status_code() {
        let client = MockHttpClient::new();

        let response = client
            .request(
                HttpMethod::GET,
                "https://api.example.com/test".to_string(),
                HashMap::new(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            response.metadata.get("status_code"),
            Some(&Value::Number(200.0))
        );
    }

    #[tokio::test]
    async fn test_http_method_enum_equality() {
        assert_eq!(HttpMethod::GET, HttpMethod::GET);
        assert_eq!(HttpMethod::POST, HttpMethod::POST);
        assert_ne!(HttpMethod::GET, HttpMethod::POST);
    }

    #[tokio::test]
    async fn test_http_default_response() {
        let client = MockHttpClient::new();

        let response = client
            .request(
                HttpMethod::GET,
                "https://test.com".to_string(),
                HashMap::new(),
                None,
            )
            .await
            .unwrap();

        // Default response should be an empty object
        if let Value::Object(data) = &response.data {
            assert_eq!(data.len(), 0);
        } else {
            panic!("Expected object response");
        }
    }

    #[tokio::test]
    async fn test_http_service_request_without_url() {
        let client = MockHttpClient::new();

        let request = ServiceRequest::new("http".to_string(), "GET".to_string());

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
        // URL should be empty string when not provided
        assert_eq!(
            response.metadata.get("url"),
            Some(&Value::String("".to_string()))
        );
    }
}
