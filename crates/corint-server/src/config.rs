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
        /// Database type (postgresql, mysql, sqlite)
        db_type: DatabaseType,
        /// Database connection URL
        url: String,
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

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host
    pub host: String,

    /// Server port
    pub port: u16,

    /// Repository configuration for loading rules
    #[serde(default)]
    pub repository: RepositoryType,

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
            repository: RepositoryType::default(),
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
        dotenv::dotenv().ok();
        
        // Try to read from config file
        let config_result = config::Config::builder()
            .add_source(config::File::with_name("config/server").required(false))
            .add_source(config::Environment::with_prefix("CORINT"))
            .build();
        
        match config_result {
            Ok(cfg) => {
                cfg.try_deserialize()
                    .map_err(|e| anyhow::anyhow!("Failed to deserialize config: {}", e))
            }
            Err(_) => {
                // Use default config if no config file found
                tracing::info!("No config file found, using default configuration");
                Ok(Self::default())
            }
        }
    }
}

