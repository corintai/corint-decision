//! Server configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Repository type for loading rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RepositoryType {
    /// File system repository
    FileSystem {
        /// Base path for repository (default: "repository")
        #[serde(default = "default_repository_path")]
        path: PathBuf,
    },
    /// Database repository
    Database {
        /// Database type (postgresql, mysql, sqlite) - deprecated, use datasource instead
        #[serde(default)]
        db_type: Option<DatabaseType>,
        /// Database connection URL - deprecated, use datasource instead
        #[serde(default)]
        url: Option<String>,
        /// Reference to a datasource defined in datasources section
        #[serde(default)]
        datasource: Option<String>,
    },
    /// HTTP API repository
    Api {
        /// Base URL for the API
        base_url: String,
        /// Optional API key for authentication
        #[serde(default)]
        api_key: Option<String>,
    },
}

/// Database type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite,
}

fn default_repository_path() -> PathBuf {
    PathBuf::from("repository")
}

impl Default for RepositoryType {
    fn default() -> Self {
        RepositoryType::FileSystem {
            path: default_repository_path(),
        }
    }
}

/// Data source configuration for server-level usage
/// 
/// Server-level datasources are used for:
/// - Repository storage (rules, pipelines, etc.)
/// - User authentication/authorization
/// - System-level data storage
/// 
/// Note: Feature calculation datasources are defined in repository/configs/datasources/
/// and are automatically loaded by the SDK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasourceConfig {
    /// Data source type (sql, feature_store, olap)
    #[serde(rename = "type")]
    pub source_type: String,

    /// Provider (postgresql, mysql, sqlite, redis, clickhouse, etc.)
    pub provider: String,

    /// Connection string or URL
    pub connection_string: String,

    /// Database name (for SQL databases)
    #[serde(default)]
    pub database: Option<String>,

    /// Events table name (for SQL/OLAP databases)
    #[serde(default)]
    pub events_table: Option<String>,

    /// Additional options
    #[serde(default)]
    pub options: std::collections::HashMap<String, String>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host
    pub host: String,

    /// Server port (HTTP)
    pub port: u16,

    /// gRPC server port (optional, if not set, gRPC server will not start)
    #[serde(default)]
    pub grpc_port: Option<u16>,

    /// Repository configuration for loading rules
    #[serde(default)]
    pub repository: RepositoryType,

    /// Data sources configuration (similar to llm configuration)
    /// Key is the datasource name, value is the datasource configuration
    #[serde(default)]
    pub datasources: std::collections::HashMap<String, DatasourceConfig>,

    /// Default datasource name (optional)
    #[serde(default)]
    pub default_datasource: Option<String>,

    /// Enable metrics
    pub enable_metrics: bool,

    /// Enable tracing
    pub enable_tracing: bool,

    /// Log level
    pub log_level: String,

    /// Database URL for decision result persistence (optional)
    /// If not set, decision results will not be persisted to database
    #[serde(default)]
    pub database_url: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            grpc_port: None,
            repository: RepositoryType::default(),
            datasources: std::collections::HashMap::new(),
            default_datasource: None,
            enable_metrics: true,
            enable_tracing: true,
            log_level: "info".to_string(),
            database_url: None,
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables and config file
    pub fn load() -> anyhow::Result<Self> {
        // Load .env file if exists
        dotenvy::dotenv().ok();

        // Try to read from config file
        let config_result = config::Config::builder()
            .add_source(config::File::with_name("config/server").required(false))
            .add_source(config::Environment::with_prefix("CORINT"))
            .build();

        match config_result {
            Ok(cfg) => cfg
                .try_deserialize()
                .map_err(|e| anyhow::anyhow!("Failed to deserialize config: {}", e)),
            Err(_) => {
                // Use default config if no config file found
                tracing::info!("No config file found, using default configuration");
                Ok(Self::default())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert!(config.enable_metrics);
        assert!(config.enable_tracing);
        assert_eq!(config.log_level, "info");
        assert!(config.database_url.is_none());
        assert!(config.datasources.is_empty());
        assert!(config.default_datasource.is_none());
    }

    #[test]
    fn test_repository_type_default() {
        let repo = RepositoryType::default();

        if let RepositoryType::FileSystem { path } = repo {
            assert_eq!(path, PathBuf::from("repository"));
        } else {
            panic!("Expected FileSystem repository type");
        }
    }

    #[test]
    fn test_repository_type_filesystem() {
        let repo = RepositoryType::FileSystem {
            path: PathBuf::from("/custom/path"),
        };

        if let RepositoryType::FileSystem { path } = repo {
            assert_eq!(path, PathBuf::from("/custom/path"));
        } else {
            panic!("Expected FileSystem repository type");
        }
    }

    #[test]
    fn test_repository_type_database_legacy() {
        let repo = RepositoryType::Database {
            db_type: Some(DatabaseType::PostgreSQL),
            url: Some("postgresql://localhost/db".to_string()),
            datasource: None,
        };

        if let RepositoryType::Database { db_type, url, datasource } = repo {
            assert!(matches!(db_type, Some(DatabaseType::PostgreSQL)));
            assert_eq!(url, Some("postgresql://localhost/db".to_string()));
            assert!(datasource.is_none());
        } else {
            panic!("Expected Database repository type");
        }
    }

    #[test]
    fn test_repository_type_database_with_datasource() {
        let repo = RepositoryType::Database {
            db_type: None,
            url: None,
            datasource: Some("postgres_events".to_string()),
        };

        if let RepositoryType::Database { datasource, .. } = repo {
            assert_eq!(datasource, Some("postgres_events".to_string()));
        } else {
            panic!("Expected Database repository type");
        }
    }

    #[test]
    fn test_repository_type_api() {
        let repo = RepositoryType::Api {
            base_url: "https://api.example.com".to_string(),
            api_key: Some("secret-key".to_string()),
        };

        if let RepositoryType::Api { base_url, api_key } = repo {
            assert_eq!(base_url, "https://api.example.com");
            assert_eq!(api_key, Some("secret-key".to_string()));
        } else {
            panic!("Expected Api repository type");
        }
    }

    #[test]
    fn test_repository_type_api_without_key() {
        let repo = RepositoryType::Api {
            base_url: "https://api.example.com".to_string(),
            api_key: None,
        };

        if let RepositoryType::Api { base_url, api_key } = repo {
            assert_eq!(base_url, "https://api.example.com");
            assert!(api_key.is_none());
        } else {
            panic!("Expected Api repository type");
        }
    }

    #[test]
    fn test_database_type_variants() {
        let pg = DatabaseType::PostgreSQL;
        let mysql = DatabaseType::MySQL;
        let sqlite = DatabaseType::SQLite;

        assert!(matches!(pg, DatabaseType::PostgreSQL));
        assert!(matches!(mysql, DatabaseType::MySQL));
        assert!(matches!(sqlite, DatabaseType::SQLite));
    }

    #[test]
    fn test_server_config_with_database_url() {
        let config = ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 3000,
            repository: RepositoryType::default(),
            datasources: std::collections::HashMap::new(),
            default_datasource: None,
            enable_metrics: true,
            enable_tracing: false,
            log_level: "debug".to_string(),
            database_url: Some("postgresql://localhost/test".to_string()),
        };

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3000);
        assert!(config.enable_metrics);
        assert!(!config.enable_tracing);
        assert_eq!(config.log_level, "debug");
        assert_eq!(
            config.database_url,
            Some("postgresql://localhost/test".to_string())
        );
    }

    #[test]
    fn test_default_repository_path() {
        let path = default_repository_path();
        assert_eq!(path, PathBuf::from("repository"));
    }

    #[test]
    fn test_server_config_clone() {
        let config = ServerConfig::default();
        let cloned = config.clone();

        assert_eq!(config.host, cloned.host);
        assert_eq!(config.port, cloned.port);
        assert_eq!(config.enable_metrics, cloned.enable_metrics);
    }

    #[test]
    fn test_repository_type_clone() {
        let repo = RepositoryType::FileSystem {
            path: PathBuf::from("/test"),
        };
        let cloned = repo.clone();

        if let (RepositoryType::FileSystem { path: p1 }, RepositoryType::FileSystem { path: p2 }) =
            (&repo, &cloned)
        {
            assert_eq!(p1, p2);
        } else {
            panic!("Clone failed");
        }
    }

    #[test]
    fn test_database_type_clone() {
        let db_type = DatabaseType::PostgreSQL;
        let cloned = db_type.clone();

        assert!(matches!(cloned, DatabaseType::PostgreSQL));
    }

    #[test]
    fn test_server_config_debug_format() {
        let config = ServerConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("ServerConfig"));
        assert!(debug_str.contains("127.0.0.1"));
        assert!(debug_str.contains("8080"));
    }
}
