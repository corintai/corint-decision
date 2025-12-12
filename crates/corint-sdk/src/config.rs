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

    #[test]
    fn test_engine_config_default() {
        let config = EngineConfig::default();

        assert_eq!(config.rule_files.len(), 0);
        assert_eq!(config.rule_contents.len(), 0);
        assert!(config.registry_file.is_none());
        assert!(config.registry_content.is_none());
        assert!(config.storage.is_none());
        assert!(config.llm.is_none());
        assert!(config.service.is_none());
        assert!(config.enable_metrics); // Default is true
        assert!(!config.enable_tracing); // Default is false
    }

    #[test]
    fn test_engine_config_with_registry() {
        let config = EngineConfig::new()
            .with_registry_file(PathBuf::from("registry.yaml"));

        assert!(config.registry_file.is_some());
        assert_eq!(config.registry_file.unwrap(), PathBuf::from("registry.yaml"));
    }

    #[test]
    fn test_engine_config_with_storage() {
        let storage = StorageConfig {
            storage_type: StorageType::Redis,
            connection: "redis://localhost:6379".to_string(),
        };

        let config = EngineConfig::new()
            .with_storage(storage.clone());

        assert!(config.storage.is_some());
        let config_storage = config.storage.unwrap();
        assert!(matches!(config_storage.storage_type, StorageType::Redis));
        assert_eq!(config_storage.connection, "redis://localhost:6379");
    }

    #[test]
    fn test_engine_config_with_llm() {
        let llm = LLMConfig {
            provider: LLMProvider::Anthropic,
            api_key: "sk-test".to_string(),
            default_model: "claude-3-sonnet".to_string(),
            enable_cache: false,
        };

        let config = EngineConfig::new()
            .with_llm(llm.clone());

        assert!(config.llm.is_some());
        let config_llm = config.llm.unwrap();
        assert!(matches!(config_llm.provider, LLMProvider::Anthropic));
        assert_eq!(config_llm.default_model, "claude-3-sonnet");
        assert!(!config_llm.enable_cache);
    }

    #[test]
    fn test_engine_config_with_service() {
        let service = ServiceConfig {
            service_type: ServiceType::Http,
            endpoint: "https://api.example.com".to_string(),
        };

        let config = EngineConfig::new()
            .with_service(service.clone());

        assert!(config.service.is_some());
        let config_service = config.service.unwrap();
        assert!(matches!(config_service.service_type, ServiceType::Http));
        assert_eq!(config_service.endpoint, "https://api.example.com");
    }

    #[test]
    fn test_storage_type_variants() {
        let memory = StorageType::Memory;
        let redis = StorageType::Redis;
        let postgres = StorageType::Postgres;
        let mysql = StorageType::MySQL;

        assert!(matches!(memory, StorageType::Memory));
        assert!(matches!(redis, StorageType::Redis));
        assert!(matches!(postgres, StorageType::Postgres));
        assert!(matches!(mysql, StorageType::MySQL));
    }

    #[test]
    fn test_llm_provider_variants() {
        let openai = LLMProvider::OpenAI;
        let anthropic = LLMProvider::Anthropic;
        let mock = LLMProvider::Mock;

        assert!(matches!(openai, LLMProvider::OpenAI));
        assert!(matches!(anthropic, LLMProvider::Anthropic));
        assert!(matches!(mock, LLMProvider::Mock));
    }

    #[test]
    fn test_service_type_variants() {
        let http = ServiceType::Http;
        let database = ServiceType::Database;
        let redis = ServiceType::Redis;
        let mock = ServiceType::Mock;

        assert!(matches!(http, ServiceType::Http));
        assert!(matches!(database, ServiceType::Database));
        assert!(matches!(redis, ServiceType::Redis));
        assert!(matches!(mock, ServiceType::Mock));
    }

    #[test]
    fn test_compiler_options_default() {
        let options = CompilerOptions::default();

        assert!(options.enable_semantic_analysis);
        assert!(options.enable_constant_folding);
        assert!(options.enable_dead_code_elimination);
    }

    #[test]
    fn test_compiler_options_custom() {
        let options = CompilerOptions {
            enable_semantic_analysis: false,
            enable_constant_folding: true,
            enable_dead_code_elimination: false,
        };

        assert!(!options.enable_semantic_analysis);
        assert!(options.enable_constant_folding);
        assert!(!options.enable_dead_code_elimination);
    }

    #[test]
    fn test_engine_config_chaining() {
        let storage = StorageConfig {
            storage_type: StorageType::Postgres,
            connection: "postgres://localhost/db".to_string(),
        };

        let llm = LLMConfig {
            provider: LLMProvider::OpenAI,
            api_key: "key".to_string(),
            default_model: "gpt-4".to_string(),
            enable_cache: true,
        };

        let config = EngineConfig::new()
            .with_rule_file(PathBuf::from("rule1.yaml"))
            .with_rule_file(PathBuf::from("rule2.yaml"))
            .with_registry_file(PathBuf::from("registry.yaml"))
            .with_storage(storage)
            .with_llm(llm)
            .enable_metrics(false)
            .enable_tracing(true);

        assert_eq!(config.rule_files.len(), 2);
        assert!(config.registry_file.is_some());
        assert!(config.storage.is_some());
        assert!(config.llm.is_some());
        assert!(!config.enable_metrics);
        assert!(config.enable_tracing);
    }

    #[test]
    fn test_multiple_rule_files() {
        let config = EngineConfig::new()
            .with_rule_file(PathBuf::from("rules/fraud.yaml"))
            .with_rule_file(PathBuf::from("rules/payment.yaml"))
            .with_rule_file(PathBuf::from("rules/auth.yaml"));

        assert_eq!(config.rule_files.len(), 3);
        assert_eq!(config.rule_files[0], PathBuf::from("rules/fraud.yaml"));
        assert_eq!(config.rule_files[1], PathBuf::from("rules/payment.yaml"));
        assert_eq!(config.rule_files[2], PathBuf::from("rules/auth.yaml"));
    }

    #[test]
    fn test_llm_config_with_cache_disabled() {
        let llm = LLMConfig {
            provider: LLMProvider::Mock,
            api_key: String::new(),
            default_model: "mock-model".to_string(),
            enable_cache: false,
        };

        assert!(matches!(llm.provider, LLMProvider::Mock));
        assert!(!llm.enable_cache);
        assert_eq!(llm.api_key, "");
    }

    #[test]
    fn test_storage_config_postgres() {
        let storage = StorageConfig {
            storage_type: StorageType::Postgres,
            connection: "postgresql://user:pass@localhost:5432/dbname".to_string(),
        };

        assert!(matches!(storage.storage_type, StorageType::Postgres));
        assert!(storage.connection.starts_with("postgresql://"));
    }

    #[test]
    fn test_service_config_database() {
        let service = ServiceConfig {
            service_type: ServiceType::Database,
            endpoint: "postgresql://localhost:5432/db".to_string(),
        };

        assert!(matches!(service.service_type, ServiceType::Database));
        assert!(service.endpoint.contains("postgresql"));
    }
}
