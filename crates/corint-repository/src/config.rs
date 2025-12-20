//! Repository configuration types
//!
//! This module provides configuration types for different repository sources
//! (file system, database, API, memory).

use serde::{Deserialize, Serialize};

/// Repository source type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepositorySource {
    /// Load from file system
    FileSystem,
    /// Load from database
    Database,
    /// Load from HTTP API
    Api,
    /// In-memory configuration (for testing or WASM with manual content)
    Memory,
}

impl Default for RepositorySource {
    fn default() -> Self {
        Self::FileSystem
    }
}

/// Repository configuration
///
/// Specifies where and how to load repository content (pipelines, rules, configs, etc.)
///
/// # Examples
///
/// ```rust
/// use corint_repository::RepositoryConfig;
///
/// // File system repository
/// let config = RepositoryConfig::file_system("repository");
///
/// // Database repository
/// let config = RepositoryConfig::database("postgresql://localhost/corint");
///
/// // API repository
/// let config = RepositoryConfig::api("https://api.example.com/repository")
///     .with_api_key("secret-key");
///
/// // Memory repository (for testing)
/// let config = RepositoryConfig::memory();
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepositoryConfig {
    /// Configuration source type
    pub source: RepositorySource,

    /// File system base path (required for FileSystem source)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_path: Option<String>,

    /// Database connection URL (required for Database source)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,

    /// API base URL (required for Api source)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,

    /// API key for authentication (optional for Api source)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

impl RepositoryConfig {
    /// Create a file system repository configuration
    ///
    /// # Arguments
    /// * `path` - Base path to the repository directory
    ///
    /// # Example
    /// ```rust
    /// use corint_repository::RepositoryConfig;
    ///
    /// let config = RepositoryConfig::file_system("repository");
    /// ```
    pub fn file_system(path: impl Into<String>) -> Self {
        Self {
            source: RepositorySource::FileSystem,
            base_path: Some(path.into()),
            database_url: None,
            api_url: None,
            api_key: None,
        }
    }

    /// Create a database repository configuration
    ///
    /// # Arguments
    /// * `url` - Database connection URL
    ///
    /// # Example
    /// ```rust
    /// use corint_repository::RepositoryConfig;
    ///
    /// let config = RepositoryConfig::database("postgresql://localhost/corint");
    /// ```
    pub fn database(url: impl Into<String>) -> Self {
        Self {
            source: RepositorySource::Database,
            base_path: None,
            database_url: Some(url.into()),
            api_url: None,
            api_key: None,
        }
    }

    /// Create an API repository configuration
    ///
    /// # Arguments
    /// * `url` - API base URL
    ///
    /// # Example
    /// ```rust
    /// use corint_repository::RepositoryConfig;
    ///
    /// let config = RepositoryConfig::api("https://api.example.com/repository");
    /// ```
    pub fn api(url: impl Into<String>) -> Self {
        Self {
            source: RepositorySource::Api,
            base_path: None,
            database_url: None,
            api_url: Some(url.into()),
            api_key: None,
        }
    }

    /// Create a memory repository configuration
    ///
    /// Use this for testing or when content will be added manually via builder methods.
    ///
    /// # Example
    /// ```rust
    /// use corint_repository::RepositoryConfig;
    ///
    /// let config = RepositoryConfig::memory();
    /// ```
    pub fn memory() -> Self {
        Self {
            source: RepositorySource::Memory,
            base_path: None,
            database_url: None,
            api_url: None,
            api_key: None,
        }
    }

    /// Set API key for authentication
    ///
    /// # Arguments
    /// * `key` - API key
    ///
    /// # Example
    /// ```rust
    /// use corint_repository::RepositoryConfig;
    ///
    /// let config = RepositoryConfig::api("https://api.example.com")
    ///     .with_api_key("secret-key");
    /// ```
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Validate the configuration
    ///
    /// Returns an error if required fields are missing for the selected source.
    pub fn validate(&self) -> Result<(), ConfigError> {
        match self.source {
            RepositorySource::FileSystem => {
                if self.base_path.is_none() {
                    return Err(ConfigError::MissingField {
                        source: "FileSystem".to_string(),
                        field: "base_path".to_string(),
                    });
                }
            }
            RepositorySource::Database => {
                if self.database_url.is_none() {
                    return Err(ConfigError::MissingField {
                        source: "Database".to_string(),
                        field: "database_url".to_string(),
                    });
                }
            }
            RepositorySource::Api => {
                if self.api_url.is_none() {
                    return Err(ConfigError::MissingField {
                        source: "Api".to_string(),
                        field: "api_url".to_string(),
                    });
                }
            }
            RepositorySource::Memory => {
                // Memory source doesn't require any fields
            }
        }
        Ok(())
    }
}

/// Configuration error
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// A required field is missing for the selected source
    MissingField { source: String, field: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingField { source, field } => {
                write!(f, "{} source requires {} to be set", source, field)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_system_config() {
        let config = RepositoryConfig::file_system("repository");

        assert_eq!(config.source, RepositorySource::FileSystem);
        assert_eq!(config.base_path, Some("repository".to_string()));
        assert!(config.database_url.is_none());
        assert!(config.api_url.is_none());
        assert!(config.api_key.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_database_config() {
        let config = RepositoryConfig::database("postgresql://localhost/corint");

        assert_eq!(config.source, RepositorySource::Database);
        assert!(config.base_path.is_none());
        assert_eq!(
            config.database_url,
            Some("postgresql://localhost/corint".to_string())
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_api_config() {
        let config = RepositoryConfig::api("https://api.example.com").with_api_key("secret");

        assert_eq!(config.source, RepositorySource::Api);
        assert_eq!(
            config.api_url,
            Some("https://api.example.com".to_string())
        );
        assert_eq!(config.api_key, Some("secret".to_string()));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_memory_config() {
        let config = RepositoryConfig::memory();

        assert_eq!(config.source, RepositorySource::Memory);
        assert!(config.base_path.is_none());
        assert!(config.database_url.is_none());
        assert!(config.api_url.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_missing_base_path() {
        let config = RepositoryConfig {
            source: RepositorySource::FileSystem,
            base_path: None,
            database_url: None,
            api_url: None,
            api_key: None,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_missing_database_url() {
        let config = RepositoryConfig {
            source: RepositorySource::Database,
            base_path: None,
            database_url: None,
            api_url: None,
            api_key: None,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_missing_api_url() {
        let config = RepositoryConfig {
            source: RepositorySource::Api,
            base_path: None,
            database_url: None,
            api_url: None,
            api_key: None,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_default_source() {
        assert_eq!(RepositorySource::default(), RepositorySource::FileSystem);
    }
}
