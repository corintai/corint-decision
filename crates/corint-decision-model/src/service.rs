//! HTTP service binding schema shared by loaders and runtime.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// HTTP service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpServiceConfig {
    /// Logical service name
    pub name: String,

    /// Base URL
    pub base_url: String,

    /// Optional authentication configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<ServiceAuth>,

    /// Timeout in milliseconds (default: 10000)
    #[serde(default = "default_service_timeout")]
    pub timeout_ms: u64,

    /// Named HTTP operations.
    #[serde(default)]
    pub operations: HashMap<String, ServiceOperation>,
}

fn default_service_timeout() -> u64 {
    10000
}

/// Header authentication for an HTTP service
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceAuth {
    /// Authentication type (currently only "header" is supported)
    #[serde(rename = "type")]
    pub auth_type: String,

    /// Header name (e.g., "Authorization", "X-API-Key")
    pub name: String,

    /// Resolved header value; environment interpolation is not performed.
    pub value: String,
}

/// HTTP operation binding
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceOperation {
    /// HTTP method (GET, POST, PUT, DELETE, PATCH)
    pub method: String,

    /// Path (can include path parameters like {id})
    pub path: String,

    /// Optional timeout for this endpoint (overrides service default)
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
    pub response: Option<ServiceResponseMapping>,
}

/// Response handling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceResponseMapping {
    /// Field mapping: output_field -> response_field
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub mapping: HashMap<String, String>,

    /// Fallback value on error (4xx, 5xx, timeout)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<serde_json::Value>,
}
