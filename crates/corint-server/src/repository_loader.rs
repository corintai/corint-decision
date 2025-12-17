//! Unified repository loading logic

use anyhow::Result;
use corint_repository::{FileSystemRepository, Repository};
use corint_sdk::DecisionEngineBuilder;
use tracing::{error, info, warn};

use crate::config::{DatabaseType, RepositoryType};

/// Load rules from repository based on configuration
///
/// This function creates the appropriate Repository implementation based on
/// configuration and loads all pipelines into the DecisionEngineBuilder.
pub async fn load_rules_from_repository(
    mut builder: DecisionEngineBuilder,
    repository: &RepositoryType,
) -> Result<DecisionEngineBuilder> {
    // Create appropriate repository implementation
    let repo: Box<dyn Repository> =
        match repository {
            RepositoryType::FileSystem { path } => {
                info!("Loading rules from file system repository: {:?}", path);
                Box::new(FileSystemRepository::new(path).map_err(|e| {
                    anyhow::anyhow!("Failed to create file system repository: {}", e)
                })?)
            }
            RepositoryType::Database { db_type, url } => {
                info!(
                    "Loading rules from database repository: {:?} at {}",
                    db_type, url
                );

                match db_type {
                    DatabaseType::PostgreSQL => {
                        #[cfg(feature = "postgres")]
                        {
                            use corint_repository::PostgresRepository;
                            Box::new(PostgresRepository::new(url).await.map_err(|e| {
                                anyhow::anyhow!("Failed to create PostgreSQL repository: {}", e)
                            })?)
                        }
                        #[cfg(not(feature = "postgres"))]
                        {
                            warn!("PostgreSQL repository feature not enabled");
                            return Err(anyhow::anyhow!(
                                "Enable 'postgres' feature to use PostgreSQL repository"
                            ));
                        }
                    }
                    DatabaseType::MySQL => {
                        warn!("MySQL repository not yet implemented");
                        return Err(anyhow::anyhow!("MySQL repository not yet implemented"));
                    }
                    DatabaseType::SQLite => {
                        warn!("SQLite repository not yet implemented");
                        return Err(anyhow::anyhow!("SQLite repository not yet implemented"));
                    }
                }
            }
            RepositoryType::Api { base_url, api_key } => {
                info!("Loading rules from API repository: {}", base_url);
                use corint_repository::ApiRepository;

                Box::new(
                    ApiRepository::new(base_url, api_key.as_deref())
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to create API repository: {}", e))?,
                )
            }
        };

    info!("✓ Repository initialized");

    // Load all pipelines from repository
    let pipeline_ids = repo
        .list_pipelines()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list pipelines: {}", e))?;

    info!("  Found {} pipelines", pipeline_ids.len());

    let pipeline_count = pipeline_ids.len();
    let mut successful_count = 0;
    let mut failed_count = 0;

    for pipeline_id in pipeline_ids {
        info!("  Loading pipeline: {}", pipeline_id);

        match repo.load_pipeline(&pipeline_id).await {
            Ok((_, content)) => {
                // Add content directly to builder (no temporary files needed!)
                builder = builder.add_rule_content(pipeline_id.clone(), content);
                successful_count += 1;
            }
            Err(e) => {
                // Log error in standard format and continue loading other pipelines
                error!("Failed to load pipeline {}: {}", pipeline_id, e);
                failed_count += 1;
            }
        }
    }

    if failed_count > 0 {
        warn!(
            "✓ Loaded {}/{} pipelines successfully ({} failed)",
            successful_count, pipeline_count, failed_count
        );
    } else {
        info!(
            "✓ Successfully loaded {} pipelines from repository",
            pipeline_count
        );
    }

    // Load registry if available
    match repo.load_registry().await {
        Ok(registry_content) => {
            info!("✓ Loading registry configuration");
            builder = builder.with_registry_content(registry_content);
        }
        Err(e) => {
            // Registry is optional, so just log a warning if not found
            warn!("Registry not found or not supported: {}", e);
            info!("  Falling back to legacy pipeline routing (evaluates all pipelines)");
        }
    }

    Ok(builder)
}
