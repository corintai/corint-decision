//! HTTP connector for unified service invocations
//!
//! Deployment location does not affect the service contract.

use crate::context::ExecutionContext;
use crate::error::{Result, RuntimeError};
use corint_decision_model::Value;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

pub use corint_decision_model::service::{
    HttpServiceConfig, ServiceAuth, ServiceOperation, ServiceResponseMapping,
};

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
    /// Parse HTTP method from string. Keep the legacy Option-returning API.
    #[allow(
        clippy::should_implement_trait,
        reason = "Compatibility API returns Option instead of FromStr Result"
    )]
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

/// Generic HTTP service client
pub struct HttpServiceClient {
    /// HTTP service configurations by name
    configs: HashMap<String, HttpServiceConfig>,
    /// Retain the constructor-time client validation; calls build per-endpoint clients.
    _client: reqwest::Client,
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
                RuntimeError::ServiceCallFailed(format!(
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
                RuntimeError::ServiceCallFailed(format!(
                    "Failed to create HTTP client: {}",
                    fallback_err
                ))
            })
        }
    }
}

impl HttpServiceClient {
    /// Create a new HTTP service client with default timeout
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            _client: build_http_client(Duration::from_secs(10), None).unwrap_or_else(|err| {
                panic!("Failed to create HTTP client: {}", err);
            }),
        }
    }

    /// Register an HTTP service configuration
    /// Register a named HTTP service. Reject ambiguous bindings.
    pub fn register_service(&mut self, config: HttpServiceConfig) -> Result<()> {
        Self::validate_config(&config)?;
        if self.configs.contains_key(&config.name) {
            return Err(RuntimeError::InvalidOperation(format!(
                "Duplicate service binding: {}",
                config.name
            )));
        }
        self.configs.insert(config.name.clone(), config);
        Ok(())
    }

    /// Validate a declaration without constructing a client or executing a request.
    pub fn validate_config(config: &HttpServiceConfig) -> Result<()> {
        if config.name.trim().is_empty() || config.operations.is_empty() || config.timeout_ms == 0 {
            return Err(RuntimeError::InvalidOperation(
                "Service name, operations and positive timeout are required".into(),
            ));
        }
        let url = reqwest::Url::parse(&config.base_url).map_err(|_| {
            RuntimeError::InvalidOperation(format!(
                "Invalid HTTP base_url for service {}",
                config.name
            ))
        })?;
        if !["http", "https"].contains(&url.scheme())
            || url.host_str().is_none()
            || url.fragment().is_some()
        {
            return Err(RuntimeError::InvalidOperation(
                "Service base_url requires HTTP or HTTPS, a host and no fragment".into(),
            ));
        }
        for (name, operation) in &config.operations {
            if name.trim().is_empty()
                || operation.timeout_ms == Some(0)
                || HttpMethod::from_str(&operation.method).is_none()
            {
                return Err(RuntimeError::InvalidOperation(format!(
                    "Invalid service operation: {}::{name}",
                    config.name
                )));
            }
            Self::validate_operation_path(&operation.path)?;
            if operation.request_body.is_some()
                && !matches!(
                    HttpMethod::from_str(&operation.method),
                    Some(HttpMethod::POST | HttpMethod::PUT | HttpMethod::PATCH)
                )
            {
                return Err(RuntimeError::InvalidOperation(format!(
                    "Service {}::{name}: request_body requires POST, PUT or PATCH",
                    config.name
                )));
            }
        }
        if config
            .auth
            .as_ref()
            .is_some_and(|auth| auth.auth_type != "header")
        {
            return Err(RuntimeError::InvalidOperation(
                "Only header service authentication is supported".into(),
            ));
        }
        Ok(())
    }

    pub fn contains_service(&self, name: &str) -> bool {
        self.configs.contains_key(name)
    }

    /// Call an HTTP service endpoint
    pub async fn call(
        &self,
        service_name: &str,
        endpoint_name: &str,
        params: &HashMap<String, Value>,
        timeout: Option<u64>,
        ctx: &ExecutionContext,
    ) -> Result<Value> {
        // Get HTTP service configuration
        let service_config = self.configs.get(service_name).ok_or_else(|| {
            RuntimeError::ServiceCallFailed(format!("Unknown service: {}", service_name))
        })?;

        // Get endpoint configuration
        let endpoint = service_config
            .operations
            .get(endpoint_name)
            .ok_or_else(|| {
                RuntimeError::ServiceCallFailed(format!(
                    "Unknown endpoint: {}::{}",
                    service_name, endpoint_name
                ))
            })?;

        // Determine effective timeout (priority: param > endpoint > API > default)
        let effective_timeout = timeout
            .or(endpoint.timeout_ms)
            .unwrap_or(service_config.timeout_ms);

        // Build the complete URL
        let url = self.build_url(service_config, endpoint, params, ctx)?;

        tracing::debug!(
            "Calling HTTP service: {} (timeout: {}ms)",
            url,
            effective_timeout
        );

        // Add authentication headers if configured
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(auth) = &service_config.auth {
            if auth.auth_type == "header" {
                let header_name = reqwest::header::HeaderName::from_bytes(auth.name.as_bytes())
                    .map_err(|e| {
                        RuntimeError::ServiceCallFailed(format!("Invalid header name: {}", e))
                    })?;
                let header_value =
                    reqwest::header::HeaderValue::from_str(&auth.value).map_err(|e| {
                        RuntimeError::ServiceCallFailed(format!("Invalid header value: {}", e))
                    })?;
                headers.insert(header_name, header_value);
            }
        }

        let client = build_http_client(Duration::from_millis(effective_timeout), Some(headers))?;

        // Parse HTTP method
        let method = HttpMethod::from_str(&endpoint.method).ok_or_else(|| {
            RuntimeError::ServiceCallFailed(format!("Invalid HTTP method: {}", endpoint.method))
        })?;

        // Make HTTP request based on method
        let response = match method {
            HttpMethod::GET => client.get(&url).send().await.map_err(|e| {
                RuntimeError::ServiceCallFailed(format!("HTTP request failed: {}", e))
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
                    let body = self.substitute_body_template(
                        body_template,
                        &endpoint.params,
                        params,
                        ctx,
                    )?;
                    request = request
                        .header("Content-Type", "application/json")
                        .body(body);
                }

                request.send().await.map_err(|e| {
                    RuntimeError::ServiceCallFailed(format!("HTTP request failed: {}", e))
                })?
            }
            HttpMethod::DELETE => client.delete(&url).send().await.map_err(|e| {
                RuntimeError::ServiceCallFailed(format!("HTTP request failed: {}", e))
            })?,
        };

        // Check if response is successful
        if !response.status().is_success() {
            // If there's a fallback in endpoint.response, use it
            if let Some(response_config) = &endpoint.response {
                if let Some(fallback) = &response_config.fallback {
                    tracing::warn!(
                        "HTTP service {}::{} failed with status {}, using endpoint fallback",
                        service_name,
                        endpoint_name,
                        response.status()
                    );
                    return Self::json_to_value(fallback.clone());
                }
            }

            return Err(RuntimeError::ServiceCallFailed(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        // Read errors (including a truncated body) are transport failures.
        // Only JSON parsing after a complete read is eligible for fallback.
        let bytes = response.bytes().await.map_err(|error| {
            RuntimeError::ServiceCallFailed(format!("Service response transport failed: {error}"))
        })?;
        let json: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(j) => j,
            Err(e) => {
                // Explicit fallback covers invalid JSON, not transport failure.
                if let Some(response_config) = &endpoint.response {
                    if let Some(fallback) = &response_config.fallback {
                        tracing::warn!(
                            "HTTP service {}::{} response parsing failed: {}, using endpoint fallback",
                            service_name, endpoint_name, e
                        );
                        return Self::json_to_value(fallback.clone());
                    }
                }
                return Err(RuntimeError::ServiceCallFailed(format!(
                    "Failed to parse JSON: {}",
                    e
                )));
            }
        };

        // Apply response mapping if configured
        let mut value = Self::json_to_value(json)?;
        if let Some(response_config) = &endpoint.response {
            tracing::debug!(
                "Response config exists with {} mappings",
                response_config.mapping.len()
            );
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

    fn validate_operation_path(path: &str) -> Result<()> {
        let has_dot_segment = path.split('/').any(|part| {
            matches!(
                part.replace("%2e", ".").replace("%2E", ".").as_str(),
                "." | ".."
            )
        });
        if path.trim().is_empty()
            || path.contains(['?', '#', '\\'])
            || path.contains("://")
            || path.chars().any(char::is_control)
            || has_dot_segment
        {
            return Err(RuntimeError::InvalidOperation(
                "Service path must be non-empty, with no URL, query, fragment, backslash or dot segments".into(),
            ));
        }
        Ok(())
    }

    /// Append the operation path to the base prefix and encode parameter data.
    fn build_url(
        &self,
        service_config: &HttpServiceConfig,
        endpoint: &ServiceOperation,
        params: &HashMap<String, Value>,
        ctx: &ExecutionContext,
    ) -> Result<String> {
        Self::validate_operation_path(&endpoint.path)?;
        let resolved_params = self.resolve_params(&endpoint.params, params, ctx)?;
        let mut path = String::new();
        let mut remaining = endpoint.path.as_str();
        while let Some((prefix, suffix)) = remaining.split_once('{') {
            let (name, rest) = suffix.split_once('}').ok_or_else(|| {
                RuntimeError::ServiceCallFailed("Unclosed service path placeholder".into())
            })?;
            if prefix.contains('}') || name.trim().is_empty() || name.contains('{') {
                return Err(RuntimeError::ServiceCallFailed(
                    "Invalid service path placeholder".into(),
                ));
            }
            let value = resolved_params.get(name).ok_or_else(|| {
                RuntimeError::ServiceCallFailed(format!("Missing service path parameter: {name}"))
            })?;
            path.push_str(prefix);
            path.push_str(&urlencoding::encode(&self.value_to_string(value)?));
            remaining = rest;
        }
        if remaining.contains('}') {
            return Err(RuntimeError::ServiceCallFailed(
                "Unexpected closing brace in service path".into(),
            ));
        }
        path.push_str(remaining);
        // A parameter consisting of a dot segment must not be normalized away.
        Self::validate_operation_path(&path)?;
        let mut url = reqwest::Url::parse(&service_config.base_url).map_err(|error| {
            RuntimeError::ServiceCallFailed(format!("Invalid service base URL: {error}"))
        })?;
        let combined_path = format!(
            "{}/{}",
            url.path().trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        url.set_path(&combined_path);
        for param_name in &endpoint.query_params {
            if let Some(value) = resolved_params.get(param_name) {
                url.query_pairs_mut()
                    .append_pair(param_name, &self.value_to_string(value)?);
            }
        }
        Ok(url.into())
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
            if pipeline_params.contains_key(key) {
                continue;
            }
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
    /// Defaults use namespace paths or scalar/null literals, without expressions.
    fn resolve_param_value(
        &self,
        value: &serde_json::Value,
        ctx: &ExecutionContext,
    ) -> Result<Value> {
        match value {
            serde_json::Value::String(s) => {
                if ["event.", "service.", "vars.", "features.", "sys.", "env."]
                    .iter()
                    .any(|prefix| s.starts_with(prefix))
                {
                    let mut path = s.split('.');
                    let data = match path.next().expect("namespace prefix") {
                        "event" => &ctx.event,
                        "service" => &ctx.service,
                        "vars" => &ctx.vars,
                        "features" => &ctx.features,
                        "sys" => &ctx.sys,
                        "env" => &ctx.env,
                        _ => unreachable!("checked namespace prefix"),
                    };
                    let missing = || RuntimeError::FieldNotFound(s.clone());
                    let mut value = data
                        .get(path.next().expect("field after prefix"))
                        .ok_or_else(missing)?;
                    for part in path {
                        value = match value {
                            Value::Object(object) => object.get(part).ok_or_else(missing)?,
                            _ => return Err(missing()),
                        };
                    }
                    Ok(value.clone())
                } else {
                    Ok(Value::String(s.clone()))
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
        Self::render_body_template(template, |name| {
            let value = resolved_params.get(name).ok_or_else(|| {
                RuntimeError::ServiceCallFailed(format!("Missing service body parameter: {name}"))
            })?;
            serde_json::to_string(value).map_err(|error| {
                RuntimeError::ServiceCallFailed(format!("Cannot encode service parameter: {error}"))
            })
        })
    }

    /// Check JSON template syntax with placeholder values, without executing a request.
    pub fn validate_body_template(template: &str) -> Result<()> {
        Self::render_body_template(template, |_| Ok("null".into())).map(|_| ())
    }

    fn render_body_template(template: &str, resolve: impl Fn(&str) -> Result<String>) -> Result<String> {
        let invalid = |message: &str| RuntimeError::ServiceCallFailed(message.into());
        let replace = |placeholder: &str, head: &str, tail: &str| -> Result<String> {
            let name = placeholder
                .strip_prefix("${")
                .and_then(|name| name.strip_suffix('}'))
                .filter(|name| !name.trim().is_empty() && !name.contains(['{', '}']))
                .ok_or_else(|| {
                    invalid("Service body placeholders must occupy a complete JSON value")
                })?;
            if tail.trim_start().starts_with(':') {
                return Err(invalid("Service body placeholders cannot be object keys"));
            }
            if !matches!(
                head.trim_end().chars().next_back(),
                None | Some(':' | '[' | ',')
            ) || !matches!(
                tail.trim_start().chars().next(),
                None | Some(',' | ']' | '}')
            ) {
                return Err(invalid(
                    "Service body placeholders must occupy a complete JSON value",
                ));
            }
            resolve(name)
        };

        // Scan only the original template. Inserted values are never interpreted
        // as new placeholders, regardless of parameter iteration order.
        let mut body = String::new();
        let mut chars = template.char_indices().peekable();
        while let Some((start, ch)) = chars.next() {
            if ch == '"' {
                let mut escaped = false;
                let mut end = None;
                for (index, ch) in chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        end = Some(index + 1);
                        break;
                    }
                }
                let end = end.ok_or_else(|| invalid("Unclosed JSON string in service body"))?;
                let token = &template[start..end];
                let text: String = serde_json::from_str(token).map_err(|error| {
                    RuntimeError::ServiceCallFailed(format!("Invalid service body string: {error}"))
                })?;
                if text.contains("${") {
                    body.push_str(&replace(&text, &template[..start], &template[end..])?);
                } else {
                    body.push_str(token);
                }
            } else if ch == '$' && chars.peek().is_some_and(|(_, ch)| *ch == '{') {
                chars.next();
                let end = chars
                    .by_ref()
                    .find(|(_, ch)| *ch == '}')
                    .map(|(index, _)| index + 1)
                    .ok_or_else(|| invalid("Unclosed service body placeholder"))?;
                body.push_str(&replace(
                    &template[start..end],
                    &template[..start],
                    &template[end..],
                )?);
            } else {
                body.push(ch);
            }
        }

        serde_json::from_str::<serde_json::Value>(&body).map_err(|error| {
            RuntimeError::ServiceCallFailed(format!("Invalid service request body: {error}"))
        })?;
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
                    let path: Vec<String> =
                        response_field.split('.').map(|s| s.to_string()).collect();
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

    /// Convert serde_json::Value to corint_decision_model::Value
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

impl Default for HttpServiceClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Load HTTP service configurations from YAML content
pub fn load_service_config(yaml_content: &str) -> Result<HttpServiceConfig> {
    serde_yaml::from_str(yaml_content)
        .map_err(|e| RuntimeError::RuntimeError(format!("Failed to parse API config: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url_with_path_params() {
        let mut operations = HashMap::new();
        let mut endpoint_params = HashMap::new();
        endpoint_params.insert(
            "id".to_string(),
            serde_json::Value::String("user_id".to_string()),
        );

        operations.insert(
            "get_user".to_string(),
            ServiceOperation {
                method: "GET".to_string(),
                path: "/users/{id}".to_string(),
                timeout_ms: None,
                params: endpoint_params,
                query_params: vec![],
                request_body: None,
                response: None,
            },
        );

        let service_config = HttpServiceConfig {
            name: "test_api".to_string(),
            base_url: "https://api.example.com".to_string(),
            auth: None,
            timeout_ms: 10000,
            operations,
        };

        let endpoint = service_config.operations.get("get_user").unwrap();

        // Pipeline params override endpoint params using the same key names
        let mut params = HashMap::new();
        params.insert("id".to_string(), Value::String("123".to_string()));

        let client = HttpServiceClient::new();
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        let url = client
            .build_url(&service_config, endpoint, &params, &ctx)
            .unwrap();
        assert_eq!(url, "https://api.example.com/users/123");
    }

    #[test]
    fn test_build_url_with_query_params() {
        let mut operations = HashMap::new();
        let mut endpoint_params = HashMap::new();
        endpoint_params.insert(
            "token".to_string(),
            serde_json::Value::String("api_token".to_string()),
        );
        endpoint_params.insert(
            "format".to_string(),
            serde_json::Value::String("response_format".to_string()),
        );

        operations.insert(
            "get_data".to_string(),
            ServiceOperation {
                method: "GET".to_string(),
                path: "/data".to_string(),
                timeout_ms: None,
                params: endpoint_params,
                query_params: vec!["token".to_string(), "format".to_string()],
                request_body: None,
                response: None,
            },
        );

        let service_config = HttpServiceConfig {
            name: "test_api".to_string(),
            base_url: "https://api.example.com".to_string(),
            auth: None,
            timeout_ms: 10000,
            operations,
        };

        let endpoint = service_config.operations.get("get_data").unwrap();

        // Pipeline params override endpoint params using the same key names
        let mut params = HashMap::new();
        params.insert("token".to_string(), Value::String("abc123".to_string()));
        params.insert("format".to_string(), Value::String("json".to_string()));

        let client = HttpServiceClient::new();
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();

        let url = client
            .build_url(&service_config, endpoint, &params, &ctx)
            .unwrap();
        // Query params may be in any order
        assert!(url.starts_with("https://api.example.com/data?"));
        assert!(url.contains("token=abc123"));
        assert!(url.contains("format=json"));
    }

    fn binding(method: &str, path: &str) -> HttpServiceConfig {
        serde_json::from_value(serde_json::json!({
            "name": "risk", "base_url": "https://api.example.com",
            "operations": {"score": {"method": method, "path": path}}
        }))
        .unwrap()
    }

    #[test]
    fn url_preserves_base_prefix_and_encodes_parameter_data() {
        let client = HttpServiceClient::new();
        let mut config = binding("GET", "/users/{id}");
        config.base_url = "https://api.example.com/v1/?fixed=yes".into();
        config.operations.get_mut("score").unwrap().query_params =
            vec!["tag&kind".into(), "optional".into()];
        let params = HashMap::from([
            ("id".into(), Value::String("a/b?x=1#é".into())),
            ("tag&kind".into(), Value::String("x=y&z".into())),
        ]);
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();
        let url = client
            .build_url(&config, &config.operations["score"], &params, &ctx)
            .unwrap();
        let url = reqwest::Url::parse(&url).unwrap();
        assert_eq!(url.path(), "/v1/users/a%2Fb%3Fx%3D1%23%C3%A9");
        assert_eq!(url.fragment(), None);
        assert_eq!(
            url.query_pairs().into_owned().collect::<Vec<_>>(),
            [
                ("fixed".into(), "yes".into()),
                ("tag&kind".into(), "x=y&z".into())
            ]
        );
    }

    #[test]
    fn missing_malformed_and_dot_path_parameters_are_rejected() {
        let client = HttpServiceClient::new();
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();
        for path in [
            "/users/{missing}",
            "/users/{id",
            "/users/id}",
            "/users/../admin",
            "/users/%2E%2e/admin",
            "/users?admin=true",
            "/users#fragment",
        ] {
            let config = binding("GET", path);
            assert!(
                client
                    .build_url(&config, &config.operations["score"], &HashMap::new(), &ctx)
                    .is_err(),
                "{path}"
            );
        }
        let config = binding("GET", "/users/{id}");
        for id in [".", ".."] {
            let params = HashMap::from([("id".into(), Value::String(id.into()))]);
            assert!(client
                .build_url(&config, &config.operations["score"], &params, &ctx)
                .is_err());
        }
    }

    #[test]
    fn body_placeholders_preserve_types_without_recursive_substitution() {
        let client = HttpServiceClient::new();
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();
        let params = HashMap::from([
            ("id".into(), Value::String("a\"\\${other}".into())),
            ("other".into(), Value::Number(7.0)),
            ("number".into(), Value::Number(2.0)),
            (
                "payload".into(),
                Value::Object(HashMap::from([("active".into(), Value::Bool(true))])),
            ),
            ("items".into(), Value::Array(vec![Value::Null])),
            ("empty".into(), Value::Null),
        ]);
        let template = r#"{"quoted":"${id}","bare":${id},"number":"${number}","payload":${payload},"items":"${items}","null":${empty}}"#;
        let body = client
            .substitute_body_template(template, &HashMap::new(), &params, &ctx)
            .unwrap();
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["quoted"], "a\"\\${other}");
        assert_eq!(body["bare"], body["quoted"]);
        assert_eq!(body["number"].as_f64(), Some(2.0));
        assert_eq!(body["payload"], serde_json::json!({"active": true}));
        assert_eq!(body["items"], serde_json::json!([null]));
        assert!(body["null"].is_null());
    }

    #[test]
    fn body_missing_values_partial_strings_and_dynamic_keys_are_rejected() {
        let client = HttpServiceClient::new();
        let ctx = ExecutionContext::from_event(HashMap::new()).unwrap();
        let params = HashMap::from([
            ("id".into(), Value::String("42".into())),
            ("number".into(), Value::Number(2.0)),
        ]);
        for template in [
            r#"{"id":"${missing}"}"#,
            r#"{"id":${missing}}"#,
            r#"{"id":"prefix-${id}"}"#,
            r#"{"${id}":1}"#,
            r#"{"id":"${}"}"#,
            r#"{"id":"${id"}"#,
            r#"{"id":1${number}}"#,
            r#"{"id":${number}0}"#,
            r#"{"id":1"${number}"}"#,
        ] {
            assert!(
                client
                    .substitute_body_template(template, &HashMap::new(), &params, &ctx)
                    .is_err(),
                "{template}"
            );
        }
    }

    #[test]
    fn operation_defaults_have_explicit_literal_and_override_semantics() {
        let client = HttpServiceClient::new();
        let ctx = ExecutionContext::from_event(HashMap::from([
            ("id".into(), Value::Number(42.0)),
            (
                "profile".into(),
                Value::Object(HashMap::from([
                    ("nullable".into(), Value::Null),
                    ("items".into(), Value::Array(vec![Value::Bool(true)])),
                ])),
            ),
        ]))
        .unwrap();
        let defaults = HashMap::from([
            ("id".into(), serde_json::json!("event.id")),
            ("expression".into(), serde_json::json!("${event.id + 1}")),
            ("unused".into(), serde_json::json!("event.missing")),
            ("nil".into(), serde_json::Value::Null),
            (
                "nullable".into(),
                serde_json::json!("event.profile.nullable"),
            ),
            ("items".into(), serde_json::json!("event.profile.items")),
        ]);
        let overrides = HashMap::from([("unused".into(), Value::Array(vec![Value::Bool(true)]))]);
        let resolved = client.resolve_params(&defaults, &overrides, &ctx).unwrap();
        assert_eq!(resolved["id"], Value::Number(42.0));
        assert_eq!(
            resolved["expression"],
            Value::String("${event.id + 1}".into())
        );
        assert_eq!(resolved["unused"], overrides["unused"]);
        assert_eq!(resolved["nil"], Value::Null);
        assert_eq!(resolved["nullable"], Value::Null);
        assert_eq!(resolved["items"], Value::Array(vec![Value::Bool(true)]));
        for path in ["event.profile.missing", "event.id.child"] {
            assert!(client
                .resolve_param_value(&serde_json::json!(path), &ctx)
                .is_err());
        }
        assert!(client
            .resolve_params(&defaults, &HashMap::new(), &ctx)
            .is_err());
        for value in [serde_json::json!([]), serde_json::json!({})] {
            assert!(client.resolve_param_value(&value, &ctx).is_err());
        }
    }

    #[test]
    fn response_mapping_defines_missing_paths_and_non_object_results() {
        let client = HttpServiceClient::new();
        let mapping = HashMap::from([
            ("result.score".into(), "risk.score".into()),
            ("missing".into(), "risk.missing".into()),
            ("array".into(), "items.0".into()),
        ]);
        let response = HttpServiceClient::json_to_value(
            serde_json::json!({"risk":{"score":42},"items":[7],"extra":true}),
        )
        .unwrap();
        let result = client.apply_response_mapping(response, &mapping).unwrap();
        let Value::Object(result) = result else {
            panic!("mapped object")
        };
        assert_eq!(result["result.score"], Value::Number(42.0));
        assert_eq!(result["missing"], Value::Null);
        assert_eq!(result["array"], Value::Null);
        assert!(!result.contains_key("extra"));
        for value in [
            Value::Null,
            Value::Number(42.0),
            Value::Array(vec![Value::Bool(true)]),
        ] {
            assert_eq!(
                client
                    .apply_response_mapping(value.clone(), &mapping)
                    .unwrap(),
                value
            );
        }
    }

    #[test]
    fn request_body_on_bodyless_methods_is_rejected_at_registration() {
        for method in ["GET", "DELETE", "POST", "PUT", "PATCH"] {
            let mut config = binding(method, "/users");
            config.operations.get_mut("score").unwrap().request_body = Some("{}".into());
            let result = HttpServiceClient::new().register_service(config);
            assert_eq!(
                result.is_ok(),
                matches!(method, "POST" | "PUT" | "PATCH"),
                "{method}"
            );
        }
    }
}
