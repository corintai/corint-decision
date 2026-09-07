//! Unified repository loader
//!
//! This module provides a unified interface for loading all repository content
//! from different sources (file system, database, API).

use crate::config::{RepositoryConfig, RepositorySource};
use crate::content::{
    DataSourceConfig, FeatureCache, FeatureDefinition, FeatureFilter, HttpServiceConfig,
    ListConfig, PoolConfig, RepositoryContent, TimeWindow,
};
use crate::error::{RepositoryError, RepositoryResult};
use crate::Repository;
use std::path::Path;

/// Unified repository loader
///
/// Loads all repository content based on the provided configuration.
///
/// # Example
///
/// ```rust,ignore
/// use corint_decision_repository::{RepositoryConfig, RepositoryLoader};
///
/// let config = RepositoryConfig::file_system("repository");
/// let loader = RepositoryLoader::new(config);
/// let content = loader.load_all().await?;
/// ```
pub struct RepositoryLoader {
    config: RepositoryConfig,
}

impl RepositoryLoader {
    /// Create a new repository loader
    pub fn new(config: RepositoryConfig) -> Self {
        Self { config }
    }

    /// Load all content from the repository
    ///
    /// This loads:
    /// - Registry (pipeline routing)
    /// - Pipelines
    /// - Rules
    /// - Rulesets
    /// - HTTP service configs
    /// - Data source configs
    /// - Feature definitions
    /// - List configs
    pub async fn load_all(&self) -> RepositoryResult<RepositoryContent> {
        // Validate configuration
        self.config
            .validate()
            .map_err(|e| RepositoryError::Config(e.to_string()))?;

        match self.config.source {
            RepositorySource::FileSystem => self.load_from_filesystem().await,
            RepositorySource::Database => self.load_from_database().await,
            RepositorySource::Api => self.load_from_api().await,
            RepositorySource::Memory => Ok(RepositoryContent::default()),
        }
    }

    /// Load content from file system
    async fn load_from_filesystem(&self) -> RepositoryResult<RepositoryContent> {
        let base_path = self.config.base_path.as_ref().ok_or_else(|| {
            RepositoryError::Config("base_path required for FileSystem source".to_string())
        })?;

        let repo = crate::FileSystemRepository::new(base_path)?;
        let mut content = RepositoryContent::default();

        // 1. Load registry
        match repo.load_registry().await {
            Ok(registry) => content.registry = Some(registry),
            Err(RepositoryError::NotFound { .. }) => (),
            Err(error) => return Err(error),
        }

        // 2. Load pipelines
        let mut pipeline_ids = repo.list_pipelines().await?;
        pipeline_ids.sort();
        for id in pipeline_ids {
            let (_, yaml) = repo.load_pipeline(&id).await?;
            content.pipelines.push((id, yaml));
        }

        // 3. Load rules
        let rule_ids = repo.list_rules().await?;
        for id in rule_ids {
            if let Ok((_, yaml)) = repo.load_rule(&id).await {
                content.rules.push((id, yaml));
            }
        }

        // 4. Load rulesets
        let ruleset_ids = repo.list_rulesets().await?;
        for id in ruleset_ids {
            if let Ok((_, yaml)) = repo.load_ruleset(&id).await {
                content.rulesets.push((id, yaml));
            }
        }

        // 5. Load resource definitions directly from the repository root.
        let repository_path = Path::new(base_path);
        content.service_configs = self.load_service_configs(repository_path).await?;
        content.feature_definitions = self
            .load_feature_definitions(repository_path)
            .await
            .unwrap_or_default();
        content.list_configs = self
            .load_list_configs(repository_path)
            .await
            .unwrap_or_default();

        // Datasource deployment configuration retains its separate legacy path.
        content.datasource_configs = self
            .load_datasource_configs(&repository_path.join("configs"))
            .await
            .unwrap_or_default();

        Ok(content)
    }

    /// Load HTTP service configurations from services/
    async fn load_service_configs(
        &self,
        repository_path: &Path,
    ) -> RepositoryResult<Vec<HttpServiceConfig>> {
        let services_path = repository_path.join("services");
        if !services_path.exists() {
            return Ok(Vec::new());
        }

        let mut configs = Vec::new();

        let entries = std::fs::read_dir(&services_path).map_err(|e| {
            RepositoryError::Other(format!("Failed to read services directory: {}", e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                configs.push(self.load_service_config_file(&path).await?);
            }
        }

        Ok(configs)
    }

    /// Load a single HTTP service config file
    async fn load_service_config_file(&self, path: &Path) -> RepositoryResult<HttpServiceConfig> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| RepositoryError::Other(format!("Failed to read {:?}: {}", path, e)))?;

        // Parse YAML directly into HttpServiceConfig structure
        // serde will handle the deserialization based on our struct definition
        let config: HttpServiceConfig = serde_yaml::from_str(&content).map_err(|e| {
            RepositoryError::ParseError(format!("Failed to parse {:?}: {}", path, e))
        })?;

        Ok(config)
    }

    /// Load datasource configurations from configs/datasources/ (deprecated, datasources should be in server.yaml)
    async fn load_datasource_configs(
        &self,
        configs_path: &Path,
    ) -> RepositoryResult<Vec<DataSourceConfig>> {
        let ds_path = configs_path.join("datasources");
        if !ds_path.exists() {
            return Ok(Vec::new());
        }

        let mut configs = Vec::new();

        let entries = std::fs::read_dir(&ds_path).map_err(|e| {
            RepositoryError::Other(format!("Failed to read datasources directory: {}", e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                if let Ok(config) = self.load_datasource_config_file(&path).await {
                    configs.push(config);
                }
            }
        }

        Ok(configs)
    }

    /// Load a single datasource config file
    async fn load_datasource_config_file(&self, path: &Path) -> RepositoryResult<DataSourceConfig> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| RepositoryError::Other(format!("Failed to read {:?}: {}", path, e)))?;

        let yaml: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|e| {
            RepositoryError::ParseError(format!("Failed to parse {:?}: {}", path, e))
        })?;

        let name = yaml
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let source_type = yaml
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let connection = yaml
            .get("connection")
            .or_else(|| yaml.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let entity = yaml
            .get("entity")
            .and_then(|v| v.as_str())
            .map(String::from);

        let pool = yaml.get("pool").map(|p| PoolConfig {
            min_connections: p
                .get("min_connections")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u32,
            max_connections: p
                .get("max_connections")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as u32,
            connect_timeout: p
                .get("connect_timeout")
                .and_then(|v| v.as_u64())
                .unwrap_or(30),
        });

        Ok(DataSourceConfig {
            name,
            source_type,
            connection,
            entity,
            pool,
        })
    }

    /// Load feature definitions from features/
    async fn load_feature_definitions(
        &self,
        repository_path: &Path,
    ) -> RepositoryResult<Vec<FeatureDefinition>> {
        let features_path = repository_path.join("features");
        if !features_path.exists() {
            return Ok(Vec::new());
        }

        let mut definitions = Vec::new();

        let entries = std::fs::read_dir(&features_path).map_err(|e| {
            RepositoryError::Other(format!("Failed to read features directory: {}", e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                if let Ok(defs) = self.load_feature_definitions_file(&path).await {
                    definitions.extend(defs);
                }
            }
        }

        Ok(definitions)
    }

    /// Load feature definitions from a single file
    async fn load_feature_definitions_file(
        &self,
        path: &Path,
    ) -> RepositoryResult<Vec<FeatureDefinition>> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| RepositoryError::Other(format!("Failed to read {:?}: {}", path, e)))?;

        let yaml: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|e| {
            RepositoryError::ParseError(format!("Failed to parse {:?}: {}", path, e))
        })?;

        let mut definitions = Vec::new();

        if let Some(features) = yaml.get("features").and_then(|v| v.as_sequence()) {
            for feature in features {
                if let Some(def) = self.parse_feature_definition(feature) {
                    definitions.push(def);
                }
            }
        }

        Ok(definitions)
    }

    /// Parse a single feature definition from YAML
    fn parse_feature_definition(&self, yaml: &serde_yaml::Value) -> Option<FeatureDefinition> {
        let name = yaml.get("name")?.as_str()?.to_string();
        let operator = yaml.get("operator")?.as_str()?.to_string();
        let datasource = yaml.get("datasource")?.as_str()?.to_string();

        let description = yaml
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        let entity = yaml
            .get("entity")
            .and_then(|v| v.as_str())
            .map(String::from);

        let dimension = yaml
            .get("dimension")
            .or_else(|| yaml.get("primary_dimension"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let dimension_value = yaml
            .get("dimension_value")
            .or_else(|| yaml.get("primary_value"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let window = yaml.get("window").map(|w| TimeWindow {
            value: w.get("value").and_then(|v| v.as_u64()).unwrap_or(1),
            unit: w
                .get("unit")
                .and_then(|v| v.as_str())
                .unwrap_or("hours")
                .to_string(),
        });

        let filters = yaml
            .get("filters")
            .and_then(|f| f.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|f| {
                        Some(FeatureFilter {
                            field: f.get("field")?.as_str()?.to_string(),
                            operator: f.get("operator")?.as_str()?.to_string(),
                            value: serde_json::to_value(f.get("value")?).ok()?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let cache = yaml.get("cache").map(|c| FeatureCache {
            enabled: c.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
            ttl: c.get("ttl").and_then(|v| v.as_u64()).unwrap_or(300),
        });

        Some(FeatureDefinition {
            name,
            description,
            operator,
            datasource,
            entity,
            dimension,
            dimension_value,
            window,
            filters,
            cache,
        })
    }

    /// Load list configurations from lists/
    async fn load_list_configs(&self, repository_path: &Path) -> RepositoryResult<Vec<ListConfig>> {
        let lists_path = repository_path.join("lists");
        if !lists_path.exists() {
            return Ok(Vec::new());
        }

        let mut configs = Vec::new();

        let entries = std::fs::read_dir(&lists_path).map_err(|e| {
            RepositoryError::Other(format!("Failed to read lists directory: {}", e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                if let Ok(list_configs) = self.load_list_configs_file(&path).await {
                    configs.extend(list_configs);
                }
            }
        }

        Ok(configs)
    }

    /// Load list configs from a single file
    async fn load_list_configs_file(&self, path: &Path) -> RepositoryResult<Vec<ListConfig>> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| RepositoryError::Other(format!("Failed to read {:?}: {}", path, e)))?;

        let yaml: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|e| {
            RepositoryError::ParseError(format!("Failed to parse {:?}: {}", path, e))
        })?;

        let mut configs = Vec::new();

        if let Some(lists) = yaml.get("lists").and_then(|v| v.as_sequence()) {
            for list in lists {
                if let Some(config) = self.parse_list_config(list) {
                    configs.push(config);
                }
            }
        }

        Ok(configs)
    }

    /// Parse a single list config from YAML
    fn parse_list_config(&self, yaml: &serde_yaml::Value) -> Option<ListConfig> {
        let id = yaml.get("id")?.as_str()?.to_string();
        let backend = yaml.get("backend")?.as_str()?.to_string();

        let description = yaml
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        let table = yaml.get("table").and_then(|v| v.as_str()).map(String::from);

        let value_column = yaml
            .get("value_column")
            .and_then(|v| v.as_str())
            .map(String::from);

        let expiration_column = yaml
            .get("expiration_column")
            .and_then(|v| v.as_str())
            .map(String::from);

        let path = yaml.get("path").and_then(|v| v.as_str()).map(String::from);

        let reload_interval = yaml.get("reload_interval").and_then(|v| v.as_u64());

        let initial_values = yaml
            .get("initial_values")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Some(ListConfig {
            id,
            description,
            backend,
            table,
            value_column,
            expiration_column,
            path,
            reload_interval,
            initial_values,
        })
    }

    /// Load content from database
    #[cfg(feature = "postgres")]
    async fn load_from_database(&self) -> RepositoryResult<RepositoryContent> {
        let db_url = self.config.database_url.as_ref().ok_or_else(|| {
            RepositoryError::Config("database_url required for Database source".to_string())
        })?;

        let repo = crate::PostgresRepository::new(db_url).await?;
        let mut content = RepositoryContent::default();

        // Load pipelines
        let mut pipeline_ids = repo.list_pipelines().await?;
        pipeline_ids.sort();
        for id in pipeline_ids {
            let (_, yaml) = repo.load_pipeline(&id).await?;
            content.pipelines.push((id, yaml));
        }

        // Load rules
        let rule_ids = repo.list_rules().await?;
        for id in rule_ids {
            if let Ok((_, yaml)) = repo.load_rule(&id).await {
                content.rules.push((id, yaml));
            }
        }

        // Load rulesets
        let ruleset_ids = repo.list_rulesets().await?;
        for id in ruleset_ids {
            if let Ok((_, yaml)) = repo.load_ruleset(&id).await {
                content.rulesets.push((id, yaml));
            }
        }

        Ok(content)
    }

    #[cfg(not(feature = "postgres"))]
    async fn load_from_database(&self) -> RepositoryResult<RepositoryContent> {
        Err(RepositoryError::Config(
            "Database source requires 'postgres' feature to be enabled".to_string(),
        ))
    }

    /// Load content from API
    #[cfg(feature = "api")]
    async fn load_from_api(&self) -> RepositoryResult<RepositoryContent> {
        let api_url = self.config.api_url.as_ref().ok_or_else(|| {
            RepositoryError::Config("api_url required for Api source".to_string())
        })?;

        let repo = crate::ApiRepository::new(api_url, self.config.api_key.as_deref()).await?;
        let mut content = RepositoryContent::default();

        // Load pipelines
        let mut pipeline_ids = repo.list_pipelines().await?;
        pipeline_ids.sort();
        for id in pipeline_ids {
            let (_, yaml) = repo.load_pipeline(&id).await?;
            content.pipelines.push((id, yaml));
        }

        // Load rules
        let rule_ids = repo.list_rules().await?;
        for id in rule_ids {
            if let Ok((_, yaml)) = repo.load_rule(&id).await {
                content.rules.push((id, yaml));
            }
        }

        // Load rulesets
        let ruleset_ids = repo.list_rulesets().await?;
        for id in ruleset_ids {
            if let Ok((_, yaml)) = repo.load_ruleset(&id).await {
                content.rulesets.push((id, yaml));
            }
        }

        Ok(content)
    }

    #[cfg(not(feature = "api"))]
    async fn load_from_api(&self) -> RepositoryResult<RepositoryContent> {
        Err(RepositoryError::Config(
            "API source requires 'api' feature to be enabled".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_loader() {
        let config = RepositoryConfig::memory();
        let loader = RepositoryLoader::new(config);
        let content = loader.load_all().await.unwrap();

        assert!(content.is_empty());
    }

    #[tokio::test]
    async fn test_filesystem_resources_without_configs_directory() {
        let root = tempfile::tempdir().unwrap();
        for name in ["services", "features", "lists"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        std::fs::write(
            root.path().join("services/risk.yaml"),
            "name: risk\nbase_url: https://risk.example.com\noperations:\n  assess:\n    method: GET\n    path: /assess\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("features/count.yaml"),
            "features:\n  - name: event_count\n    operator: count\n    datasource: events\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("lists/blocked.yaml"),
            "lists:\n  - id: blocked\n    backend: memory\n    initial_values: [blocked_user]\n",
        )
        .unwrap();

        let config = RepositoryConfig::file_system(root.path().to_str().unwrap());
        let content = RepositoryLoader::new(config).load_all().await.unwrap();

        assert!(!root.path().join("configs").exists());
        assert_eq!(content.service_configs.len(), 1);
        assert_eq!(content.service_configs[0].name, "risk");
        assert_eq!(content.feature_definitions.len(), 1);
        assert_eq!(content.feature_definitions[0].name, "event_count");
        assert_eq!(content.list_configs.len(), 1);
        assert_eq!(content.list_configs[0].id, "blocked");
    }

    #[tokio::test]
    async fn test_filesystem_loader_missing_path() {
        let config = RepositoryConfig {
            source: RepositorySource::FileSystem,
            base_path: None,
            database_url: None,
            api_url: None,
            api_key: None,
        };
        let loader = RepositoryLoader::new(config);
        let result = loader.load_all().await;

        assert!(result.is_err());
    }
}
