//! Repository content types
//!
//! This module defines the content types loaded from a repository,
//! including pipelines, rules, rulesets, and various configurations.

use serde::{Deserialize, Serialize};

/// All content loaded from a repository
///
/// This struct contains all the business configuration loaded from a repository,
/// including pipelines, rules, rulesets, and various config types.
#[derive(Debug, Clone, Default)]
pub struct RepositoryContent {
    /// Registry content (pipeline routing configuration)
    pub registry: Option<String>,

    /// Pipeline definitions (id, yaml content)
    pub pipelines: Vec<(String, String)>,

    /// Rule definitions (id, yaml content)
    pub rules: Vec<(String, String)>,

    /// Ruleset definitions (id, yaml content)
    pub rulesets: Vec<(String, String)>,

    /// API configurations
    pub api_configs: Vec<ApiConfig>,

    /// Data source configurations
    pub datasource_configs: Vec<DataSourceConfig>,

    /// Feature definitions
    pub feature_definitions: Vec<FeatureDefinition>,

    /// List configurations
    pub list_configs: Vec<ListConfig>,
}

impl RepositoryContent {
    /// Create empty repository content
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pipeline
    pub fn add_pipeline(&mut self, id: impl Into<String>, content: impl Into<String>) {
        self.pipelines.push((id.into(), content.into()));
    }

    /// Add a rule
    pub fn add_rule(&mut self, id: impl Into<String>, content: impl Into<String>) {
        self.rules.push((id.into(), content.into()));
    }

    /// Add a ruleset
    pub fn add_ruleset(&mut self, id: impl Into<String>, content: impl Into<String>) {
        self.rulesets.push((id.into(), content.into()));
    }

    /// Merge another RepositoryContent into this one
    ///
    /// The other content is appended to existing content.
    pub fn merge(&mut self, other: RepositoryContent) {
        // Registry: prefer other if set
        if other.registry.is_some() {
            self.registry = other.registry;
        }

        self.pipelines.extend(other.pipelines);
        self.rules.extend(other.rules);
        self.rulesets.extend(other.rulesets);
        self.api_configs.extend(other.api_configs);
        self.datasource_configs.extend(other.datasource_configs);
        self.feature_definitions.extend(other.feature_definitions);
        self.list_configs.extend(other.list_configs);
    }

    /// Check if the content is empty
    pub fn is_empty(&self) -> bool {
        self.registry.is_none()
            && self.pipelines.is_empty()
            && self.rules.is_empty()
            && self.rulesets.is_empty()
            && self.api_configs.is_empty()
            && self.datasource_configs.is_empty()
            && self.feature_definitions.is_empty()
            && self.list_configs.is_empty()
    }

    /// Get total count of all artifacts
    pub fn total_count(&self) -> usize {
        (if self.registry.is_some() { 1 } else { 0 })
            + self.pipelines.len()
            + self.rules.len()
            + self.rulesets.len()
            + self.api_configs.len()
            + self.datasource_configs.len()
            + self.feature_definitions.len()
            + self.list_configs.len()
    }
}

/// External API configuration
///
/// Defines how to connect to an external API service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API name/identifier
    pub name: String,

    /// Base URL
    pub base_url: String,

    /// Endpoint definitions
    #[serde(default)]
    pub endpoints: Vec<ApiEndpoint>,

    /// Default headers
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,

    /// Timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000
}

/// API endpoint definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    /// Endpoint name
    pub name: String,

    /// HTTP method
    pub method: String,

    /// Path (can include path parameters like {id})
    pub path: String,

    /// Path parameter mappings
    #[serde(default)]
    pub path_params: std::collections::HashMap<String, String>,

    /// Query parameter mappings
    #[serde(default)]
    pub query_params: std::collections::HashMap<String, String>,
}

/// Data source configuration
///
/// Defines a data source for feature calculations (PostgreSQL, Redis, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    /// Data source name/identifier
    pub name: String,

    /// Data source type (postgresql, redis, clickhouse, etc.)
    #[serde(rename = "type")]
    pub source_type: String,

    /// Connection string or URL
    pub connection: String,

    /// Entity/table name
    #[serde(default)]
    pub entity: Option<String>,

    /// Connection pool settings
    #[serde(default)]
    pub pool: Option<PoolConfig>,
}

/// Connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Minimum connections
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Maximum connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,
}

fn default_min_connections() -> u32 {
    1
}

fn default_max_connections() -> u32 {
    10
}

fn default_connect_timeout() -> u64 {
    30
}

/// Feature definition
///
/// Defines a computed feature for use in rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDefinition {
    /// Feature name
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: Option<String>,

    /// Aggregation operator (count, sum, avg, min, max, etc.)
    pub operator: String,

    /// Data source name
    pub datasource: String,

    /// Entity/table name
    #[serde(default)]
    pub entity: Option<String>,

    /// Primary dimension field
    #[serde(default)]
    pub dimension: Option<String>,

    /// Dimension value expression
    #[serde(default)]
    pub dimension_value: Option<String>,

    /// Time window
    #[serde(default)]
    pub window: Option<TimeWindow>,

    /// Filters to apply
    #[serde(default)]
    pub filters: Vec<FeatureFilter>,

    /// Cache configuration
    #[serde(default)]
    pub cache: Option<FeatureCache>,
}

/// Time window for feature calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    /// Window value
    pub value: u64,

    /// Window unit (seconds, minutes, hours, days)
    pub unit: String,
}

/// Feature filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFilter {
    /// Field to filter on
    pub field: String,

    /// Operator (eq, ne, gt, lt, etc.)
    pub operator: String,

    /// Value to compare against
    pub value: serde_json::Value,
}

/// Feature cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureCache {
    /// Enable caching
    #[serde(default)]
    pub enabled: bool,

    /// TTL in seconds
    #[serde(default = "default_cache_ttl")]
    pub ttl: u64,
}

fn default_cache_ttl() -> u64 {
    300
}

/// List configuration
///
/// Defines a list for lookup operations (blocklist, allowlist, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListConfig {
    /// List identifier
    pub id: String,

    /// Description
    #[serde(default)]
    pub description: Option<String>,

    /// Backend type (memory, postgresql, file)
    pub backend: String,

    /// Table name (for database backend)
    #[serde(default)]
    pub table: Option<String>,

    /// Value column name
    #[serde(default)]
    pub value_column: Option<String>,

    /// Expiration column name
    #[serde(default)]
    pub expiration_column: Option<String>,

    /// File path (for file backend)
    #[serde(default)]
    pub path: Option<String>,

    /// Reload interval in seconds (for file backend)
    #[serde(default)]
    pub reload_interval: Option<u64>,

    /// Initial values (for memory backend)
    #[serde(default)]
    pub initial_values: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_content_default() {
        let content = RepositoryContent::default();

        assert!(content.is_empty());
        assert_eq!(content.total_count(), 0);
    }

    #[test]
    fn test_repository_content_add() {
        let mut content = RepositoryContent::new();

        content.add_pipeline("pipeline1", "yaml content");
        content.add_rule("rule1", "rule yaml");
        content.add_ruleset("ruleset1", "ruleset yaml");

        assert!(!content.is_empty());
        assert_eq!(content.pipelines.len(), 1);
        assert_eq!(content.rules.len(), 1);
        assert_eq!(content.rulesets.len(), 1);
        assert_eq!(content.total_count(), 3);
    }

    #[test]
    fn test_repository_content_merge() {
        let mut content1 = RepositoryContent::new();
        content1.add_pipeline("p1", "content1");

        let mut content2 = RepositoryContent::new();
        content2.add_pipeline("p2", "content2");
        content2.registry = Some("registry content".to_string());

        content1.merge(content2);

        assert_eq!(content1.pipelines.len(), 2);
        assert!(content1.registry.is_some());
    }

    #[test]
    fn test_api_config() {
        let config = ApiConfig {
            name: "ipinfo".to_string(),
            base_url: "https://ipinfo.io".to_string(),
            endpoints: vec![],
            headers: std::collections::HashMap::new(),
            timeout_ms: 5000,
        };

        assert_eq!(config.name, "ipinfo");
        assert_eq!(config.timeout_ms, 5000);
    }

    #[test]
    fn test_list_config() {
        let config = ListConfig {
            id: "blocklist".to_string(),
            description: Some("Email blocklist".to_string()),
            backend: "memory".to_string(),
            table: None,
            value_column: None,
            expiration_column: None,
            path: None,
            reload_interval: None,
            initial_values: vec!["bad@example.com".to_string()],
        };

        assert_eq!(config.id, "blocklist");
        assert_eq!(config.backend, "memory");
        assert_eq!(config.initial_values.len(), 1);
    }
}
