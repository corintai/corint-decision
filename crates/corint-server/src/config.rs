//! Server configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host
    pub host: String,
    
    /// Server port
    pub port: u16,
    
    /// Rule files directory
    pub rules_dir: PathBuf,
    
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
            rules_dir: PathBuf::from("examples/pipelines"),
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

