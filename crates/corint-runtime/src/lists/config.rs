//! List configuration structures

use serde::{Deserialize, Serialize};

/// List configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListConfig {
    /// List ID (unique identifier)
    pub id: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Backend type
    pub backend: ListBackendType,

    /// PostgreSQL configuration (when backend = postgresql)
    #[serde(flatten)]
    pub postgres_config: Option<PostgresListConfig>,

    /// Redis configuration (when backend = redis)
    #[serde(flatten)]
    pub redis_config: Option<RedisListConfig>,

    /// File configuration (when backend = file)
    #[serde(flatten)]
    pub file_config: Option<FileListConfig>,

    /// Memory configuration (when backend = memory)
    #[serde(default)]
    pub initial_values: Vec<String>,

    /// Cache TTL in seconds (optional)
    #[serde(default)]
    pub cache_ttl: Option<u64>,
}

/// Backend type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ListBackendType {
    Memory,
    PostgreSQL,
    Redis,
    File,
    Api,
}

/// PostgreSQL list configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresListConfig {
    /// Custom table name (optional, defaults to "list_entries")
    #[serde(default)]
    pub table: Option<String>,

    /// Value column name (optional, defaults to "value")
    #[serde(default)]
    pub value_column: Option<String>,

    /// Expiration column name (optional)
    #[serde(default)]
    pub expiration_column: Option<String>,

    /// Match type (exact, prefix, regex)
    #[serde(default)]
    pub match_type: Option<String>,
}

/// Redis list configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisListConfig {
    /// Redis key for the list
    #[serde(default)]
    pub redis_key: Option<String>,

    /// Redis URL (optional, uses default if not specified)
    #[serde(default)]
    pub redis_url: Option<String>,
}

/// File list configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListConfig {
    /// Path to the list file
    #[serde(default)]
    pub path: Option<String>,

    /// Reload interval in seconds
    #[serde(default)]
    pub reload_interval: Option<u64>,

    /// File format (txt, csv, json)
    #[serde(default = "default_file_format")]
    pub format: String,
}

fn default_file_format() -> String {
    "txt".to_string()
}

/// Container for multiple list configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListsConfig {
    pub lists: Vec<ListConfig>,
}

impl ListConfig {
    /// Get the default table name for PostgreSQL backend
    pub fn postgres_table(&self) -> String {
        self.postgres_config
            .as_ref()
            .and_then(|c| c.table.clone())
            .unwrap_or_else(|| "list_entries".to_string())
    }

    /// Get the default value column name for PostgreSQL backend
    pub fn postgres_value_column(&self) -> String {
        self.postgres_config
            .as_ref()
            .and_then(|c| c.value_column.clone())
            .unwrap_or_else(|| "value".to_string())
    }

    /// Get the Redis key for Redis backend
    pub fn redis_key(&self) -> String {
        self.redis_config
            .as_ref()
            .and_then(|c| c.redis_key.clone())
            .unwrap_or_else(|| format!("lists:{}", self.id))
    }

    /// Get the file path for File backend
    pub fn file_path(&self) -> Option<String> {
        self.file_config.as_ref().and_then(|c| c.path.clone())
    }

    /// Get the reload interval for File backend (in seconds)
    pub fn file_reload_interval(&self) -> Option<u64> {
        self.file_config
            .as_ref()
            .and_then(|c| c.reload_interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_memory_list() {
        let yaml = r#"
id: test_list
description: "Test memory list"
backend: memory
initial_values:
  - "value1"
  - "value2"
"#;

        let config: ListConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.id, "test_list");
        assert_eq!(config.backend, ListBackendType::Memory);
        assert_eq!(config.initial_values.len(), 2);
    }

    #[test]
    fn test_parse_postgresql_list() {
        let yaml = r#"
id: email_blocklist
description: "Email blocklist"
backend: postgresql
"#;

        let config: ListConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.id, "email_blocklist");
        assert_eq!(config.backend, ListBackendType::PostgreSQL);
        assert_eq!(config.postgres_table(), "list_entries");
        assert_eq!(config.postgres_value_column(), "value");
    }

    #[test]
    fn test_parse_file_list() {
        let yaml = r#"
id: high_risk_countries
description: "High risk countries"
backend: file
path: "repository/configs/lists/data/countries.txt"
reload_interval: 3600
"#;

        let config: ListConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.id, "high_risk_countries");
        assert_eq!(config.backend, ListBackendType::File);
        assert_eq!(
            config.file_path(),
            Some("repository/configs/lists/data/countries.txt".to_string())
        );
        assert_eq!(config.file_reload_interval(), Some(3600));
    }

    #[test]
    fn test_parse_lists_config() {
        let yaml = r#"
lists:
  - id: list1
    description: "List 1"
    backend: memory
    initial_values: ["a", "b"]

  - id: list2
    description: "List 2"
    backend: postgresql
"#;

        let config: ListsConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.lists.len(), 2);
        assert_eq!(config.lists[0].id, "list1");
        assert_eq!(config.lists[1].id, "list2");
    }
}
