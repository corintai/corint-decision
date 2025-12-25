//! Decision engine initialization
//!
//! This module provides a simple wrapper around SDK's DecisionEngineBuilder
//! to convert server configuration to SDK configuration. All complex initialization
//! logic (FeatureExecutor, ListService, etc.) is now handled automatically by the SDK.

use crate::config::{RepositoryType, ServerConfig};
use anyhow::Result;
use corint_sdk::{DecisionEngineBuilder, RepositoryConfig};
use tracing::{info, warn};

/// Initialize decision engine
///
/// This function uses SDK's automatic initialization feature to load and configure
/// all components (FeatureExecutor, ListService, ResultWriter) from the repository.
/// This makes it easy to use the SDK from other languages via FFI without
/// needing to implement complex initialization logic.
pub async fn init_engine(config: &ServerConfig) -> Result<corint_sdk::DecisionEngine> {
    // Convert server repository config to SDK repository config
    let repo_config = match &config.repository {
        RepositoryType::FileSystem { path } => {
            RepositoryConfig::file_system(path.to_string_lossy().to_string())
        }
        RepositoryType::Database { datasource, url, db_type } => {
            // If datasource name is provided, look it up in server.yaml datasources
            if let Some(ds_name) = datasource {
                if let Some(ds_config) = config.datasources.get(ds_name) {
                    // Use connection string from server.yaml datasource config
                    RepositoryConfig::database(ds_config.connection_string.clone())
                } else {
                    return Err(anyhow::anyhow!(
                        "Datasource '{}' not found in server.yaml datasources section. \
                        Please define it in the 'datasources' section of server.yaml.",
                        ds_name
                    ));
                }
            } else if let Some(url) = url {
                // Fallback to legacy url field for backward compatibility
                RepositoryConfig::database(url.clone())
            } else {
                return Err(anyhow::anyhow!(
                    "Database repository requires either 'datasource' or 'url' field"
                ));
            }
        }
        RepositoryType::Api { base_url, api_key } => {
            let config = RepositoryConfig::api(base_url.clone());
            if let Some(key) = api_key {
                config.with_api_key(key.clone())
            } else {
                config
            }
        }
    };

    let mut builder = DecisionEngineBuilder::new()
        .with_repository(repo_config)
        .enable_metrics(config.enable_metrics)
        .enable_tracing(config.enable_tracing);

    // Set database URL for automatic ResultWriter initialization
    #[cfg(feature = "sqlx")]
    {
        // Try to get database URL from config first, then fall back to environment variable
        let database_url = config.database_url.clone().or_else(|| {
            std::env::var("DATABASE_URL").ok()
        });

        if let Some(db_url) = database_url {
            builder = builder.with_database_url(db_url);
            info!("✓ Database URL configured for result persistence");
        } else {
            warn!("Database URL not configured, decision result persistence will be disabled");
        }
    }

    // Build engine - SDK will automatically initialize:
    // - FeatureExecutor from repository/configs/datasources and repository/configs/features
    // - ListService from repository/configs/lists
    // - ResultWriter from database_url (if configured)
    let engine = builder.build().await?;

    Ok(engine)
}
