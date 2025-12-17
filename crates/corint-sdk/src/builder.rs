//! Builder pattern for DecisionEngine

use crate::config::{EngineConfig, LLMConfig, ServiceConfig, StorageConfig};
use crate::decision_engine::DecisionEngine;
use crate::error::Result;
use corint_runtime::feature::FeatureExecutor;
use std::path::PathBuf;
use std::sync::Arc;

/// Builder for DecisionEngine
pub struct DecisionEngineBuilder {
    config: EngineConfig,
    feature_executor: Option<Arc<FeatureExecutor>>,
    list_service: Option<Arc<corint_runtime::lists::ListService>>,
    #[cfg(feature = "sqlx")]
    result_writer: Option<Arc<corint_runtime::DecisionResultWriter>>,
}

impl DecisionEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: EngineConfig::new(),
            feature_executor: None,
            list_service: None,
            #[cfg(feature = "sqlx")]
            result_writer: None,
        }
    }

    /// Add a rule file
    pub fn add_rule_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.rule_files.push(path.into());
        self
    }

    /// Add multiple rule files
    pub fn add_rule_files(mut self, paths: Vec<PathBuf>) -> Self {
        self.config.rule_files.extend(paths);
        self
    }

    /// Add rule content directly (alternative to file path)
    ///
    /// # Arguments
    /// * `id` - Unique identifier for the rule (e.g., pipeline ID)
    /// * `content` - YAML content of the rule/pipeline
    pub fn add_rule_content(mut self, id: impl Into<String>, content: impl Into<String>) -> Self {
        self.config.rule_contents.push((id.into(), content.into()));
        self
    }

    /// Set registry file for pipeline routing
    pub fn with_registry_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.registry_file = Some(path.into());
        self
    }

    /// Set registry content directly (alternative to file path)
    ///
    /// # Arguments
    /// * `content` - YAML content of the registry
    pub fn with_registry_content(mut self, content: impl Into<String>) -> Self {
        self.config.registry_content = Some(content.into());
        self
    }

    /// Set storage configuration
    pub fn with_storage(mut self, storage: StorageConfig) -> Self {
        self.config.storage = Some(storage);
        self
    }

    /// Set LLM configuration
    pub fn with_llm(mut self, llm: LLMConfig) -> Self {
        self.config.llm = Some(llm);
        self
    }

    /// Set service configuration
    pub fn with_service(mut self, service: ServiceConfig) -> Self {
        self.config.service = Some(service);
        self
    }

    /// Enable metrics
    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.config.enable_metrics = enable;
        self
    }

    /// Enable tracing
    pub fn enable_tracing(mut self, enable: bool) -> Self {
        self.config.enable_tracing = enable;
        self
    }

    /// Enable semantic analysis
    pub fn enable_semantic_analysis(mut self, enable: bool) -> Self {
        self.config.compiler_options.enable_semantic_analysis = enable;
        self
    }

    /// Enable constant folding
    pub fn enable_constant_folding(mut self, enable: bool) -> Self {
        self.config.compiler_options.enable_constant_folding = enable;
        self
    }

    /// Enable dead code elimination
    pub fn enable_dead_code_elimination(mut self, enable: bool) -> Self {
        self.config.compiler_options.enable_dead_code_elimination = enable;
        self
    }

    /// Set feature executor for lazy feature calculation
    pub fn with_feature_executor(mut self, executor: Arc<FeatureExecutor>) -> Self {
        self.feature_executor = Some(executor);
        self
    }

    /// Set list service for list lookup operations
    pub fn with_list_service(mut self, service: Arc<corint_runtime::lists::ListService>) -> Self {
        self.list_service = Some(service);
        self
    }

    /// Enable decision result persistence to database
    #[cfg(feature = "sqlx")]
    pub fn with_result_writer(mut self, pool: sqlx::PgPool) -> Self {
        use corint_runtime::DecisionResultWriter;
        tracing::info!("Configuring DecisionResultWriter with database pool");
        self.result_writer = Some(Arc::new(DecisionResultWriter::new(pool)));
        tracing::info!("DecisionResultWriter configured successfully");
        self
    }

    /// Build the decision engine
    pub async fn build(self) -> Result<DecisionEngine> {
        #[cfg_attr(not(feature = "sqlx"), allow(unused_mut))]
        let mut engine = DecisionEngine::new_with_feature_executor(
            self.config,
            self.feature_executor,
            self.list_service,
        )
        .await?;

        // Set result writer if configured
        #[cfg(feature = "sqlx")]
        {
            tracing::info!("Builder.build() - Checking result_writer in builder...");
            tracing::info!(
                "  Builder has result_writer: {}",
                self.result_writer.is_some()
            );

            if let Some(result_writer) = self.result_writer {
                tracing::info!("Setting result_writer on DecisionEngine");
                engine.result_writer = Some(result_writer);
                tracing::info!("Result writer successfully set on DecisionEngine");
                tracing::info!(
                    "  Engine has result_writer: {}",
                    engine.result_writer.is_some()
                );
            } else {
                tracing::warn!("No result_writer configured in builder - this should not happen if database_url was set!");
            }
        }
        #[cfg(not(feature = "sqlx"))]
        {
            tracing::warn!("sqlx feature not enabled, result persistence will not be available");
        }

        Ok(engine)
    }
}

impl Default for DecisionEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builder() {
        let engine = DecisionEngineBuilder::new()
            .enable_metrics(true)
            .enable_semantic_analysis(true)
            .build()
            .await;

        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn test_builder_with_multiple_options() {
        let builder = DecisionEngineBuilder::new()
            .add_rule_file("test1.yaml")
            .add_rule_file("test2.yaml")
            .enable_metrics(true)
            .enable_tracing(false)
            .enable_constant_folding(true);

        assert_eq!(builder.config.rule_files.len(), 2);
        assert!(builder.config.enable_metrics);
        assert!(!builder.config.enable_tracing);
    }
}
