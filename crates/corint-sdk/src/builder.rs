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
}

impl DecisionEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: EngineConfig::new(),
            feature_executor: None,
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

    /// Build the decision engine
    pub async fn build(self) -> Result<DecisionEngine> {
        DecisionEngine::new_with_feature_executor(self.config, self.feature_executor).await
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
