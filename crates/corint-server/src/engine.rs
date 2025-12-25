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
        RepositoryType::Database { url, .. } => RepositoryConfig::database(url.clone()),
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


