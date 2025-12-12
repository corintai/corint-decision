//! Configuration types for DecisionEngine

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Rule file path(s)
    pub rule_files: Vec<PathBuf>,

    /// Rule contents (id, content) - alternative to file paths
    #[serde(skip)]
    pub rule_contents: Vec<(String, String)>,

    /// Optional pipeline registry file path
    pub registry_file: Option<PathBuf>,

    /// Optional pipeline registry content - alternative to file path
    #[serde(skip)]
    pub registry_content: Option<String>,

    /// Storage configuration
    pub storage: Option<StorageConfig>,

    /// LLM configuration
    pub llm: Option<LLMConfig>,

    /// Service configuration
    pub service: Option<ServiceConfig>,

    /// Enable metrics collection
    pub enable_metrics: bool,

    /// Enable tracing
    pub enable_tracing: bool,

    /// Compiler options
    pub compiler_options: CompilerOptions,
}

impl EngineConfig {
    /// Create a new engine configuration
    pub fn new() -> Self {
        Self {
            rule_files: Vec::new(),
            rule_contents: Vec::new(),
            registry_file: None,
            registry_content: None,
            storage: None,
            llm: None,
            service: None,
            enable_metrics: true,
            enable_tracing: false,
            compiler_options: CompilerOptions::default(),
        }
    }

    /// Add a rule file
    pub fn with_rule_file(mut self, path: PathBuf) -> Self {
        self.rule_files.push(path);
        self
    }

    /// Set registry file
    pub fn with_registry_file(mut self, path: PathBuf) -> Self {
        self.registry_file = Some(path);
        self
    }

    /// Set storage configuration
    pub fn with_storage(mut self, storage: StorageConfig) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Set LLM configuration
    pub fn with_llm(mut self, llm: LLMConfig) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Set service configuration
    pub fn with_service(mut self, service: ServiceConfig) -> Self {
        self.service = Some(service);
        self
    }

    /// Enable metrics
    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }

    /// Enable tracing
    pub fn enable_tracing(mut self, enable: bool) -> Self {
        self.enable_tracing = enable;
        self
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage type (memory, redis, postgres, etc.)
    pub storage_type: StorageType,

    /// Connection string or configuration
    pub connection: String,
}

/// Storage type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    Memory,
    Redis,
    Postgres,
    MySQL,
}

/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// LLM provider (openai, anthropic, etc.)
    pub provider: LLMProvider,

    /// API key
    pub api_key: String,

    /// Default model
    pub default_model: String,

    /// Enable caching
    pub enable_cache: bool,
}

/// LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMProvider {
    OpenAI,
    Anthropic,
    Mock,
}

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service type (http, database, redis, etc.)
    pub service_type: ServiceType,

    /// Service endpoint or connection
    pub endpoint: String,
}

/// Service type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceType {
    Http,
    Database,
    Redis,
    Mock,
}

/// Compiler options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerOptions {
    /// Enable semantic analysis
    pub enable_semantic_analysis: bool,

    /// Enable constant folding
    pub enable_constant_folding: bool,

    /// Enable dead code elimination
    pub enable_dead_code_elimination: bool,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            enable_semantic_analysis: true,
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_config_builder() {
        let config = EngineConfig::new()
            .with_rule_file(PathBuf::from("rules.yaml"))
            .enable_metrics(true)
            .enable_tracing(false);

        assert_eq!(config.rule_files.len(), 1);
        assert!(config.enable_metrics);
        assert!(!config.enable_tracing);
    }

    #[test]
    fn test_storage_config() {
        let storage = StorageConfig {
            storage_type: StorageType::Memory,
            connection: String::new(),
        };

        assert!(matches!(storage.storage_type, StorageType::Memory));
    }

    #[test]
    fn test_llm_config() {
        let llm = LLMConfig {
            provider: LLMProvider::OpenAI,
            api_key: "test-key".to_string(),
            default_model: "gpt-4".to_string(),
            enable_cache: true,
        };

        assert!(matches!(llm.provider, LLMProvider::OpenAI));
        assert_eq!(llm.default_model, "gpt-4");
    }
}
