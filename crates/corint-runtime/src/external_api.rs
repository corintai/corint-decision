//! External API client configuration and execution
//!
//! Provides a generic, configurable system for calling external APIs.

use crate::context::ExecutionContext;
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// External API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API name/identifier
    pub name: String,

    /// Base URL
    pub base_url: String,

    /// Optional authentication configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<ApiAuth>,

    /// Timeout in milliseconds (default: 10000)
    #[serde(default = "default_api_timeout")]
    pub timeout_ms: u64,

    /// Endpoint definitions (as a map: endpoint_name -> endpoint_config)
    #[serde(default)]
    pub endpoints: HashMap<String, ApiEndpoint>,
}

fn default_api_timeout() -> u64 {
    10000
}

/// Authentication configuration for API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiAuth {
    /// Authentication type (currently only "header" is supported)
    #[serde(rename = "type")]
    pub auth_type: String,

    /// Header name (e.g., "Authorization", "X-API-Key")
    pub name: String,

    /// Header value (can use ${env.x.y.z} for environment variables)
    pub value: String,
}

/// API endpoint definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    /// HTTP method (GET, POST, PUT, DELETE, PATCH)
    pub method: String,

    /// Path (can include path parameters like {id})
    pub path: String,

    /// Optional timeout for this endpoint (overrides API default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Parameter mapping from context or literals
    /// Key: param name, Value: context path (e.g., "event.user.id") or literal value
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, serde_json::Value>,

    /// Query parameter names (array of param names to include in query string)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_params: Vec<String>,

    /// Request body template for POST/PUT/PATCH (with ${param_name} placeholders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,

    /// Response handling configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<ApiResponse>,
}

/// Response handling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    /// Field mapping: output_field -> response_field
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub mapping: HashMap<String, String>,

    /// Fallback value on error (4xx, 5xx, timeout)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<serde_json::Value>,
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

impl HttpMethod {
    /// Parse HTTP method from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Some(HttpMethod::GET),
            "POST" => Some(HttpMethod::POST),
            "PUT" => Some(HttpMethod::PUT),
            "DELETE" => Some(HttpMethod::DELETE),
            "PATCH" => Some(HttpMethod::PATCH),
            _ => None,
        }
    }
}

/// Generic external API client
pub struct ExternalApiClient {
    /// API configurations by name
    configs: HashMap<String, ApiConfig>,
    /// HTTP client
    client: reqwest::Client,
}

fn build_http_client(
    timeout: Duration,
    headers: Option<HeaderMap>,
) -> std::result::Result<reqwest::Client, RuntimeError> {
    let build = |disable_proxy: bool| {
        let mut builder = reqwest::Client::builder().timeout(timeout);
        if disable_proxy {
            builder = builder.no_proxy();
        }
        if let Some(ref headers) = headers {
            builder = builder.default_headers(headers.clone());
        }
        builder.build()
    };

    match std::panic::catch_unwind(|| build(false)) {
        Ok(Ok(client)) => Ok(client),
        Ok(Err(err)) => {
            tracing::warn!(
                "Failed to build HTTP client with system proxy: {}. Retrying without system proxy.",
                err
            );
            build(true).map_err(|fallback_err| {
                RuntimeError::ExternalCallFailed(format!(
                    "Failed to create HTTP client: {}",
                    fallback_err
                ))
            })
        }
        Err(_) => {
            tracing::warn!(
                "HTTP client build panicked when using system proxy. Retrying without system proxy."
            );
            build(true).map_err(|fallback_err| {
                RuntimeError::ExternalCallFailed(format!(
                    "Failed to create HTTP client: {}",
                    fallback_err
                ))
            })
        }
    }
}

impl ExternalApiClient {
    /// Create a new external API client with default timeout
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            client: build_http_client(Duration::from_secs(10), None).unwrap_or_else(|err| {
                panic!("Failed to create HTTP client: {}", err);
            }),
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
        let api_config = self.configs.get(api_name).ok_or_else(|| {
            RuntimeError::ExternalCallFailed(format!("Unknown API: {}", api_name))
        })?;

        // Get endpoint configuration
        let endpoint = api_config.endpoints.get(endpoint_name).ok_or_else(|| {
            RuntimeError::ExternalCallFailed(format!(
                "Unknown endpoint: {}::{}",
                api_name, endpoint_name
            ))
        })?;

        // Determine effective timeout (priority: param > endpoint > API > default)
        let effective_timeout = timeout
            .or(endpoint.timeout_ms)
            .unwrap_or(api_config.timeout_ms);

        // Build the complete URL
        let url = self.build_url(api_config, endpoint, params, ctx)?;

        tracing::debug!("Calling external API: {} (timeout: {}ms)", url, effective_timeout);

        // Add authentication headers if configured
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(auth) = &api_config.auth {
            if auth.auth_type == "header" {
                let header_name = reqwest::header::HeaderName::from_bytes(auth.name.as_bytes())
                    .map_err(|e| {
                        RuntimeError::ExternalCallFailed(format!("Invalid header name: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(&auth.value)
                    .map_err(|e| {
                        RuntimeError::ExternalCallFailed(format!("Invalid header value: {}", e))
                    })?;
                headers.insert(header_name, header_value);
            }
        }

        let client = build_http_client(
            Duration::from_millis(effective_timeout),
            Some(headers),
        )?;

        // Parse HTTP method
        let method = HttpMethod::from_str(&endpoint.method).ok_or_else(|| {
            RuntimeError::ExternalCallFailed(format!("Invalid HTTP method: {}", endpoint.method))
        })?;

        // Make HTTP request based on method
        let response = match method {
            HttpMethod::GET => client.get(&url).send().await.map_err(|e| {
                RuntimeError::ExternalCallFailed(format!("HTTP request failed: {}", e))
            })?,
            HttpMethod::POST | HttpMethod::PUT | HttpMethod::PATCH => {
                let mut request = match method {
                    HttpMethod::POST => client.post(&url),
                    HttpMethod::PUT => client.put(&url),
                    HttpMethod::PATCH => client.patch(&url),
                    _ => unreachable!(),
                };

                // Process request body if present
                if let Some(body_template) = &endpoint.request_body {
                    let body = self.substitute_body_template(body_template, &endpoint.params, params, ctx)?;
                    request = request
                        .header("Content-Type", "application/json")
                        .body(body);
                }

                request.send().await.map_err(|e| {
                    RuntimeError::ExternalCallFailed(format!("HTTP request failed: {}", e))
                })?
            }
            HttpMethod::DELETE => client.delete(&url).send().await.map_err(|e| {
                RuntimeError::ExternalCallFailed(format!("HTTP request failed: {}", e))
            })?,
        };

        // Check if response is successful
        if !response.status().is_success() {
            // If there's a fallback in endpoint.response, use it
            if let Some(response_config) = &endpoint.response {
                if let Some(fallback) = &response_config.fallback {
                    tracing::warn!(
                        "External API {}::{} failed with status {}, using endpoint fallback",
                        api_name, endpoint_name, response.status()
                    );
                    return Ok(Self::json_to_value(fallback.clone())?);
                }
            }

            return Err(RuntimeError::ExternalCallFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        // Parse JSON response
        let json: serde_json::Value = match response.json().await {
            Ok(j) => j,
            Err(e) => {
                // If JSON parsing fails and there's a fallback, use it
                if let Some(response_config) = &endpoint.response {
                    if let Some(fallback) = &response_config.fallback {
                        tracing::warn!(
                            "External API {}::{} response parsing failed: {}, using endpoint fallback",
                            api_name, endpoint_name, e
                        );
                        return Ok(Self::json_to_value(fallback.clone())?);
                    }
                }
                return Err(RuntimeError::ExternalCallFailed(format!("Failed to parse JSON: {}", e)));
            }
        };

        // Print the raw API response
        tracing::info!(
            "External API raw response: {}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| format!("{:?}", json))
        );

        // Apply response mapping if configured
        let mut value = Self::json_to_value(json)?;
        if let Some(response_config) = &endpoint.response {
            tracing::debug!("Response config exists with {} mappings", response_config.mapping.len());
            if !response_config.mapping.is_empty() {
                tracing::debug!("Applying response mapping: {:?}", response_config.mapping);
                value = self.apply_response_mapping(value, &response_config.mapping)?;
                tracing::debug!("Mapped response: {:?}", value);
            }
        } else {
            tracing::debug!("No response config for endpoint");
        }

        Ok(value)
    }

    /// Build the complete URL for an API call
    fn build_url(
        &self,
        api_config: &ApiConfig,
        endpoint: &ApiEndpoint,
        params: &HashMap<String, Value>,
        ctx: &ExecutionContext,
    ) -> Result<String> {
        // Start with base URL and path
        let mut path = endpoint.path.clone();

        // Resolve parameters (merge endpoint defaults with pipeline overrides)
        let resolved_params = self.resolve_params(&endpoint.params, params, ctx)?;

        // Replace path parameters (find all {placeholder} in path)
        for (key, value) in &resolved_params {
            let placeholder = format!("{{{}}}", key);
            if path.contains(&placeholder) {
                let value_str = self.value_to_string(value)?;
                path = path.replace(&placeholder, &value_str);
            }
        }

        // Build query string from query_params list
        let mut query_parts = Vec::new();
        for param_name in &endpoint.query_params {
            if let Some(value) = resolved_params.get(param_name) {
                let value_str = self.value_to_string(value)?;
                query_parts.push(format!("{}={}", param_name, urlencoding::encode(&value_str)));
            }
        }

        // Combine base URL, path, and query string
        let mut url = format!("{}{}", api_config.base_url, path);
        if !query_parts.is_empty() {
            url.push('?');
            url.push_str(&query_parts.join("&"));
        }

        Ok(url)
    }

    /// Resolve parameters by merging endpoint defaults with pipeline overrides
    /// Priority: pipeline params > endpoint params
    fn resolve_params(
        &self,
        endpoint_params: &HashMap<String, serde_json::Value>,
        pipeline_params: &HashMap<String, Value>,
        ctx: &ExecutionContext,
    ) -> Result<HashMap<String, Value>> {
        let mut resolved = HashMap::new();

        // First, resolve endpoint default params
        for (key, value) in endpoint_params {
            let resolved_value = self.resolve_param_value(value, ctx)?;
            resolved.insert(key.clone(), resolved_value);
        }

        // Then, override with pipeline params (these have priority)
        for (key, value) in pipeline_params {
            resolved.insert(key.clone(), value.clone());
        }

        Ok(resolved)
    }

    /// Resolve a single parameter value
    /// If it's a string without quotes and contains '.', treat as context path
    /// Otherwise, treat as literal value
    fn resolve_param_value(
        &self,
        value: &serde_json::Value,
        ctx: &ExecutionContext,
    ) -> Result<Value> {
        match value {
            serde_json::Value::String(s) => {
                // If it contains '.', it's likely a context path (e.g., "event.user.id")
                if s.contains('.') {
                    let path: Vec<String> = s.split('.').map(|s| s.to_string()).collect();
                    ctx.load_field(&path).or_else(|_| {
                        // If context load fails, return as literal string
                        Ok(Value::String(s.clone()))
                    })
                } else {
                    // Single token could be a context field or literal
                    ctx.load_field(&[s.clone()]).or_else(|_| {
                        Ok(Value::String(s.clone()))
                    })
                }
            }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Value::Number(i as f64))
                } else if let Some(f) = n.as_f64() {
                    Ok(Value::Number(f))
                } else {
                    Ok(Value::Number(0.0))
                }
            }
            serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
            serde_json::Value::Null => Ok(Value::Null),
            _ => Err(RuntimeError::TypeError(format!(
                "Unsupported parameter type: {:?}",
                value
            ))),
        }
    }

    /// Substitute placeholders in body template
    fn substitute_body_template(
        &self,
        template: &str,
        endpoint_params: &HashMap<String, serde_json::Value>,
        pipeline_params: &HashMap<String, Value>,
        ctx: &ExecutionContext,
    ) -> Result<String> {
        let resolved_params = self.resolve_params(endpoint_params, pipeline_params, ctx)?;

        let mut body = template.to_string();

        // Replace ${param_name} with actual values
        for (key, value) in &resolved_params {
            let placeholder = format!("${{{}}}", key);
            if body.contains(&placeholder) {
                // For JSON, we need to preserve type information
                let replacement = match value {
                    Value::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "null".to_string(),
                    _ => serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string()),
                };

                // If placeholder is already quoted (e.g., "${param}"), replace including quotes
                let quoted_placeholder = format!("\"{}\"", placeholder);
                if body.contains(&quoted_placeholder) {
                    body = body.replace(&quoted_placeholder, &replacement);
                } else {
                    body = body.replace(&placeholder, &replacement);
                }
            }
        }

        Ok(body)
    }

    /// Apply response field mapping
    fn apply_response_mapping(
        &self,
        value: Value,
        mapping: &HashMap<String, String>,
    ) -> Result<Value> {
        if let Value::Object(obj) = value {
            let mut mapped = HashMap::new();

            for (output_field, response_field) in mapping {
                // Extract nested field from response
                let field_value = if response_field.contains('.') {
                    let path: Vec<String> = response_field.split('.').map(|s| s.to_string()).collect();
                    self.extract_nested_field(&Value::Object(obj.clone()), &path)
                        .unwrap_or(Value::Null)
                } else {
                    obj.get(response_field).cloned().unwrap_or(Value::Null)
                };

                mapped.insert(output_field.clone(), field_value);
            }

            Ok(Value::Object(mapped))
        } else {
            Ok(value)
        }
    }

    /// Extract nested field from object
    fn extract_nested_field(&self, value: &Value, path: &[String]) -> Option<Value> {
        if path.is_empty() {
            return Some(value.clone());
        }

        if let Value::Object(obj) = value {
            if let Some(field_value) = obj.get(&path[0]) {
                if path.len() == 1 {
                    return Some(field_value.clone());
                } else {
                    return self.extract_nested_field(field_value, &path[1..]);
                }
            }
        }

        None
    }

    /// Convert Value to string for URL encoding
    fn value_to_string(&self, value: &Value) -> Result<String> {
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Null => Ok(String::new()),
            _ => Err(RuntimeError::TypeError(
                "Cannot convert complex value to string for URL parameter".to_string(),
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
        let mut endpoints = HashMap::new();
        let mut endpoint_params = HashMap::new();
        endpoint_params.insert("id".to_string(), serde_json::Value::String("user_id".to_string()));

        endpoints.insert(
            "get_user".to_string(),
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/users/{id}".to_string(),
                timeout_ms: None,
                params: endpoint_params,
                query_params: vec![],
                request_body: None,
                response: None,
            },
        );

        let api_config = ApiConfig {
            name: "test_api".to_string(),
            base_url: "https://api.example.com".to_string(),
            auth: None,
            timeout_ms: 10000,
            endpoints,
        };

        let endpoint = api_config.endpoints.get("get_user").unwrap();

        // Pipeline params override endpoint params using the same key names
        let mut params = HashMap::new();
        params.insert("id".to_string(), Value::String("123".to_string()));

        let client = ExternalApiClient::new();
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        let url = client
            .build_url(&api_config, &endpoint, &params, &ctx)
            .unwrap();
        assert_eq!(url, "https://api.example.com/users/123");
    }

    #[test]
    fn test_build_url_with_query_params() {
        let mut endpoints = HashMap::new();
        let mut endpoint_params = HashMap::new();
        endpoint_params.insert("token".to_string(), serde_json::Value::String("api_token".to_string()));
        endpoint_params.insert("format".to_string(), serde_json::Value::String("response_format".to_string()));

        endpoints.insert(
            "get_data".to_string(),
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/data".to_string(),
                timeout_ms: None,
                params: endpoint_params,
                query_params: vec!["token".to_string(), "format".to_string()],
                request_body: None,
                response: None,
            },
        );

        let api_config = ApiConfig {
            name: "test_api".to_string(),
            base_url: "https://api.example.com".to_string(),
            auth: None,
            timeout_ms: 10000,
            endpoints,
        };

        let endpoint = api_config.endpoints.get("get_data").unwrap();

        // Pipeline params override endpoint params using the same key names
        let mut params = HashMap::new();
        params.insert("token".to_string(), Value::String("abc123".to_string()));
        params.insert("format".to_string(), Value::String("json".to_string()));

        let client = ExternalApiClient::new();
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        let url = client
            .build_url(&api_config, &endpoint, &params, &ctx)
            .unwrap();
        // Query params may be in any order
        assert!(url.starts_with("https://api.example.com/data?"));
        assert!(url.contains("token=abc123"));
        assert!(url.contains("format=json"));
    }
}
