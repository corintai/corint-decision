//! External API client configuration and execution
//!
//! Provides a generic, configurable system for calling external APIs.

use crate::context::ExecutionContext;
use crate::error::{RuntimeError, Result};
use corint_core::Value;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// External API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API identifier (e.g., "ipinfo")
    pub name: String,
    /// Base URL for the API
    pub base_url: String,
    /// Endpoint definitions
    pub endpoints: HashMap<String, EndpointConfig>,
}

/// Endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    /// HTTP method (GET, POST, etc.)
    pub method: HttpMethod,
    /// URL path template with placeholders
    /// Examples:
    ///   - "/{ip}" - path parameter
    ///   - "/lookup" - fixed path
    pub path: String,
    /// Query parameter mappings
    /// Key: query param name, Value: parameter source field
    #[serde(default)]
    pub query_params: HashMap<String, String>,
    /// Path parameter mappings
    /// Key: placeholder name, Value: parameter source field
    #[serde(default)]
    pub path_params: HashMap<String, String>,
    /// Request body template (for POST/PUT)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_template: Option<String>,
}

/// HTTP method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

/// Generic external API client
pub struct ExternalApiClient {
    /// API configurations by name
    configs: HashMap<String, ApiConfig>,
    /// HTTP client
    client: reqwest::Client,
}

impl ExternalApiClient {
    /// Create a new external API client with default timeout
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
        }
    }

    /// Register an API configuration
    pub fn register_api(&mut self, config: ApiConfig) {
        self.configs.insert(config.name.clone(), config);
    }

    /// Call an external API endpoint
    pub async fn call(
        &self,
        api_name: &str,
        endpoint_name: &str,
        params: &HashMap<String, Value>,
        timeout: Option<u64>,
        ctx: &ExecutionContext,
    ) -> Result<Value> {
        // Get API configuration
        let api_config = self.configs
            .get(api_name)
            .ok_or_else(|| RuntimeError::ExternalCallFailed(format!("Unknown API: {}", api_name)))?;

        // Get endpoint configuration
        let endpoint = api_config.endpoints
            .get(endpoint_name)
            .ok_or_else(|| RuntimeError::ExternalCallFailed(
                format!("Unknown endpoint: {}::{}", api_name, endpoint_name)
            ))?;

        // Build the complete URL
        let url = self.build_url(api_config, endpoint, params, ctx)?;

        tracing::debug!("Calling external API: {}", url);

        // Create HTTP client with custom timeout if specified
        let client = if let Some(timeout_ms) = timeout {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(timeout_ms))
                .build()
                .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to create HTTP client: {}", e)))?
        } else {
            self.client.clone()
        };

        // Make HTTP request based on method
        let response = match endpoint.method {
            HttpMethod::GET => {
                client.get(&url)
                    .send()
                    .await
                    .map_err(|e| RuntimeError::ExternalCallFailed(format!("HTTP request failed: {}", e)))?
            }
            HttpMethod::POST => {
                let mut request = client.post(&url);
                if let Some(body_template) = &endpoint.body_template {
                    // TODO: Process body template with parameters
                    request = request.body(body_template.clone());
                }
                request
                    .send()
                    .await
                    .map_err(|e| RuntimeError::ExternalCallFailed(format!("HTTP request failed: {}", e)))?
            }
            _ => {
                return Err(RuntimeError::ExternalCallFailed(
                    format!("Unsupported HTTP method: {:?}", endpoint.method)
                ));
            }
        };

        // Check if response is successful
        if !response.status().is_success() {
            return Err(RuntimeError::ExternalCallFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        // Parse JSON response
        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| RuntimeError::ExternalCallFailed(format!("Failed to parse JSON: {}", e)))?;

        // Print the raw API response
        tracing::info!("External API raw response: {}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| format!("{:?}", json)));

        // Convert serde_json::Value to corint_core::Value
        Self::json_to_value(json)
    }

    /// Build the complete URL for an API call
    fn build_url(
        &self,
        api_config: &ApiConfig,
        endpoint: &EndpointConfig,
        params: &HashMap<String, Value>,
        ctx: &ExecutionContext,
    ) -> Result<String> {
        // Start with base URL and path
        let mut path = endpoint.path.clone();

        // Replace path parameters
        for (placeholder, param_name) in &endpoint.path_params {
            let value = self.get_param_value(param_name, params, ctx)?;
            let value_str = self.value_to_string(&value)?;
            path = path.replace(&format!("{{{}}}", placeholder), &value_str);
        }

        // Build query string
        let mut query_parts = Vec::new();
        for (query_key, param_name) in &endpoint.query_params {
            let value = self.get_param_value(param_name, params, ctx)?;
            let value_str = self.value_to_string(&value)?;
            query_parts.push(format!("{}={}", query_key, urlencoding::encode(&value_str)));
        }

        // Combine base URL, path, and query string
        let mut url = format!("{}{}", api_config.base_url, path);
        if !query_parts.is_empty() {
            url.push('?');
            url.push_str(&query_parts.join("&"));
        }

        Ok(url)
    }

    /// Get parameter value from params or context
    fn get_param_value(
        &self,
        param_name: &str,
        params: &HashMap<String, Value>,
        ctx: &ExecutionContext,
    ) -> Result<Value> {
        // First try to get from params
        if let Some(value) = params.get(param_name) {
            return Ok(value.clone());
        }

        // Try to load from context using field path
        if param_name.contains('.') {
            let path: Vec<String> = param_name.split('.').map(|s| s.to_string()).collect();
            ctx.load_field(&path)
        } else {
            ctx.load_field(&[param_name.to_string()])
        }
    }

    /// Convert Value to string for URL encoding
    fn value_to_string(&self, value: &Value) -> Result<String> {
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Null => Ok(String::new()),
            _ => Err(RuntimeError::TypeError(
                "Cannot convert complex value to string for URL parameter".to_string()
            )),
        }
    }

    /// Convert serde_json::Value to corint_core::Value
    fn json_to_value(json: serde_json::Value) -> Result<Value> {
        match json {
            serde_json::Value::Null => Ok(Value::Null),
            serde_json::Value::Bool(b) => Ok(Value::Bool(b)),
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Ok(Value::Number(f))
                } else {
                    Ok(Value::Number(n.as_i64().unwrap_or(0) as f64))
                }
            }
            serde_json::Value::String(s) => Ok(Value::String(s)),
            serde_json::Value::Array(arr) => {
                let values: Result<Vec<Value>> = arr.into_iter().map(Self::json_to_value).collect();
                Ok(Value::Array(values?))
            }
            serde_json::Value::Object(obj) => {
                let mut map = HashMap::new();
                for (key, value) in obj {
                    map.insert(key, Self::json_to_value(value)?);
                }
                Ok(Value::Object(map))
            }
        }
    }
}

impl Default for ExternalApiClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Load API configurations from YAML content
pub fn load_api_config(yaml_content: &str) -> Result<ApiConfig> {
    serde_yaml::from_str(yaml_content)
        .map_err(|e| RuntimeError::RuntimeError(format!("Failed to parse API config: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url_with_path_params() {
        let api_config = ApiConfig {
            name: "test_api".to_string(),
            base_url: "https://api.example.com".to_string(),
            endpoints: HashMap::new(),
        };

        let mut path_params = HashMap::new();
        path_params.insert("id".to_string(), "user_id".to_string());

        let endpoint = EndpointConfig {
            method: HttpMethod::GET,
            path: "/users/{id}".to_string(),
            query_params: HashMap::new(),
            path_params,
            body_template: None,
        };

        let mut params = HashMap::new();
        params.insert("user_id".to_string(), Value::String("123".to_string()));

        let client = ExternalApiClient::new();
        let ctx = ExecutionContext::new(HashMap::new());

        let url = client.build_url(&api_config, &endpoint, &params, &ctx).unwrap();
        assert_eq!(url, "https://api.example.com/users/123");
    }

    #[test]
    fn test_build_url_with_query_params() {
        let api_config = ApiConfig {
            name: "test_api".to_string(),
            base_url: "https://api.example.com".to_string(),
            endpoints: HashMap::new(),
        };

        let mut query_params = HashMap::new();
        query_params.insert("token".to_string(), "api_token".to_string());
        query_params.insert("format".to_string(), "response_format".to_string());

        let endpoint = EndpointConfig {
            method: HttpMethod::GET,
            path: "/data".to_string(),
            query_params,
            path_params: HashMap::new(),
            body_template: None,
        };

        let mut params = HashMap::new();
        params.insert("api_token".to_string(), Value::String("abc123".to_string()));
        params.insert("response_format".to_string(), Value::String("json".to_string()));

        let client = ExternalApiClient::new();
        let ctx = ExecutionContext::new(HashMap::new());

        let url = client.build_url(&api_config, &endpoint, &params, &ctx).unwrap();
        assert_eq!(url, "https://api.example.com/data?token=abc123&format=json");
    }
}
