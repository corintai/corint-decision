//! Builder pattern for DecisionEngine

use crate::config::{EngineConfig, LLMConfig, ServiceConfig, StorageConfig};
use crate::decision_engine::DecisionEngine;
use crate::error::Result;
use corint_repository::{RepositoryConfig, RepositoryContent, RepositoryLoader};
use corint_runtime::feature::FeatureExecutor;
use std::path::PathBuf;
use std::sync::Arc;

/// Builder for DecisionEngine
///
/// # Example
///
/// ```rust,ignore
/// use corint_sdk::{DecisionEngineBuilder, RepositoryConfig};
///
/// // From file system repository
/// let engine = DecisionEngineBuilder::new()
///     .with_repository(RepositoryConfig::file_system("repository"))
///     .enable_metrics(true)
///     .build()
///     .await?;
///
/// // From database
/// let engine = DecisionEngineBuilder::new()
///     .with_repository(RepositoryConfig::database("postgresql://localhost/corint"))
///     .build()
///     .await?;
///
/// // Manual configuration (for testing or WASM)
/// let engine = DecisionEngineBuilder::new()
///     .add_rule_content("pipeline", yaml_content)
///     .build()
///     .await?;
/// ```
pub struct DecisionEngineBuilder {
    config: EngineConfig,
    repository_config: Option<RepositoryConfig>,
    feature_executor: Option<Arc<FeatureExecutor>>,
    list_service: Option<Arc<corint_runtime::lists::ListService>>,
    #[cfg(feature = "sqlx")]
    result_writer: Option<Arc<corint_runtime::DecisionResultWriter>>,
    #[cfg(feature = "sqlx")]
    database_url: Option<String>,
    // Store repository content for auto-initialization
    repository_content: Option<RepositoryContent>,
    // Server datasources from server.yaml (takes precedence over repository datasources)
    server_datasources: Option<std::collections::HashMap<String, corint_runtime::datasource::config::DataSourceConfig>>,
}

impl DecisionEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: EngineConfig::new(),
            repository_config: None,
            feature_executor: None,
            list_service: None,
            #[cfg(feature = "sqlx")]
            result_writer: None,
            #[cfg(feature = "sqlx")]
            database_url: None,
            repository_content: None,
            server_datasources: None,
        }
    }

    /// Set server datasources from server.yaml configuration
    /// 
    /// These datasources take precedence over datasources defined in
    /// repository/configs/datasources/ directory (for backward compatibility).
    pub fn with_server_datasources(
        mut self,
        datasources: std::collections::HashMap<String, corint_runtime::datasource::config::DataSourceConfig>,
    ) -> Self {
        self.server_datasources = Some(datasources);
        self
    }

    // ========== Repository Configuration (Recommended) ==========

    /// Set repository configuration for loading all business content
    ///
    /// This is the recommended way to configure the engine. The repository
    /// will load all pipelines, rules, rulesets, templates, API configs,
    /// datasources, features, and lists from the specified source.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use corint_sdk::{DecisionEngineBuilder, RepositoryConfig};
    ///
    /// // From file system
    /// let engine = DecisionEngineBuilder::new()
    ///     .with_repository(RepositoryConfig::file_system("repository"))
    ///     .build()
    ///     .await?;
    ///
    /// // From database
    /// let engine = DecisionEngineBuilder::new()
    ///     .with_repository(RepositoryConfig::database("postgresql://localhost/corint"))
    ///     .build()
    ///     .await?;
    ///
    /// // From API
    /// let engine = DecisionEngineBuilder::new()
    ///     .with_repository(RepositoryConfig::api("https://api.example.com/repo"))
    ///     .build()
    ///     .await?;
    /// ```
    pub fn with_repository(mut self, config: RepositoryConfig) -> Self {
        self.repository_config = Some(config);
        self
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

    /// Set database URL for automatic result writer initialization
    ///
    /// This will automatically create a database connection pool and configure
    /// the result writer when building the engine.
    #[cfg(feature = "sqlx")]
    pub fn with_database_url(mut self, url: impl Into<String>) -> Self {
        self.database_url = Some(url.into());
        self
    }

    /// Build the decision engine
    ///
    /// If `with_repository()` was called, this will first load all content
    /// from the repository and merge it with any manually added content.
    /// It will also automatically initialize FeatureExecutor and ListService
    /// from the repository content if they are not already configured.
    pub async fn build(mut self) -> Result<DecisionEngine> {
        // Load content from repository if configured
        if let Some(repo_config) = &self.repository_config {
            let loader = RepositoryLoader::new(repo_config.clone());
            match loader.load_all().await {
                Ok(content) => {
                    // Store content for auto-initialization
                    self.repository_content = Some(content.clone());
                    // Merge repository content into config
                    self.merge_repository_content(content);
                }
                Err(e) => {
                    return Err(crate::error::SdkError::Config(format!(
                        "Failed to load repository: {}",
                        e
                    )));
                }
            }
        }

        // Auto-initialize FeatureExecutor from repository content if not already set
        if self.feature_executor.is_none() {
            if let Some(ref content) = self.repository_content {
                if let Some(executor) = Self::init_feature_executor_from_content(content, &self.repository_config, &self.server_datasources).await? {
                    self.feature_executor = Some(Arc::new(executor));
                    tracing::info!("✓ Auto-initialized FeatureExecutor from repository");
                }
            }
        }

        // Auto-initialize ListService from repository content if not already set
        if self.list_service.is_none() {
            if let Some(ref content) = self.repository_content {
                #[cfg(feature = "sqlx")]
                let db_url = &self.database_url;
                #[cfg(not(feature = "sqlx"))]
                let db_url: &Option<String> = &None;
                if let Some(service) = Self::init_list_service_from_content(content, &self.repository_config, db_url).await? {
                    self.list_service = Some(Arc::new(service));
                    tracing::info!("✓ Auto-initialized ListService from repository");
                }
            }
        }

        // Auto-initialize ResultWriter from database_url if not already set
        #[cfg(feature = "sqlx")]
        {
            if self.result_writer.is_none() {
                if let Some(ref db_url) = self.database_url {
                    match sqlx::postgres::PgPoolOptions::new()
                        .max_connections(5)
                        .connect(db_url)
                        .await
                    {
                        Ok(pool) => {
                            use corint_runtime::DecisionResultWriter;
                            self.result_writer = Some(Arc::new(DecisionResultWriter::new(pool)));
                            tracing::info!("✓ Auto-initialized ResultWriter from database_url");
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create database connection pool: {}", e);
                        }
                    }
                }
            }
        }

        // Save reload information before building
        let repository_config = self.repository_config.clone();
        let feature_executor = self.feature_executor.clone();
        let list_service = self.list_service.clone();

        #[cfg_attr(not(feature = "sqlx"), allow(unused_mut))]
        let mut engine = DecisionEngine::new_with_feature_executor(
            self.config,
            self.feature_executor,
            self.list_service,
        )
        .await?;

        // Save reload information
        engine.repository_config = repository_config;
        engine.feature_executor = feature_executor;
        engine.list_service = list_service;

        // Set result writer if configured
        #[cfg(feature = "sqlx")]
        {
            if let Some(result_writer) = self.result_writer {
                engine.result_writer = Some(result_writer);
            }
        }

        Ok(engine)
    }

    /// Merge repository content into the engine config
    fn merge_repository_content(&mut self, content: RepositoryContent) {
        // Add registry content
        if let Some(registry) = content.registry {
            self.config.registry_content = Some(registry);
        }

        // Add all pipelines as rule content
        // Pipelines are the entry points for rule execution
        for (id, yaml) in content.pipelines {
            self.config.rule_contents.push((id, yaml));
        }

        // Note: Rules, rulesets, and templates are typically:
        // 1. Included in pipeline YAML files via --- separators
        // 2. Referenced via `include` directives in pipelines
        // 3. Located in library/ subdirectories and loaded by the compiler
        //
        // They should NOT be loaded as standalone entry points because:
        // - Rules don't have 'when' conditions (can't be routed)
        // - Rulesets don't have 'when' conditions (can't be routed)
        // - Templates are reusable components, not executables
        //
        // The repository loader loads them for completeness, but they are
        // not added to rule_contents here. The compiler will find them
        // in the library/ directories when needed.

        // Note: API configs, datasources, features, and lists are now automatically
        // initialized in build() method from repository content.
    }

    /// Initialize FeatureExecutor from repository content
    ///
    /// This function converts repository DataSourceConfig and FeatureDefinition
    /// to runtime types and initializes the FeatureExecutor.
    async fn init_feature_executor_from_content(
        content: &RepositoryContent,
        repo_config: &Option<RepositoryConfig>,
        server_datasources: &Option<std::collections::HashMap<String, corint_runtime::datasource::config::DataSourceConfig>>,
    ) -> Result<Option<FeatureExecutor>> {
        use corint_runtime::datasource::{DataSourceClient, DataSourceConfig as RuntimeDataSourceConfig};
        use corint_runtime::feature::registry::FeatureRegistry;

        // Get repository base path for loading feature files
        let base_path = match repo_config {
            Some(config) => {
                match &config.source {
                    corint_repository::RepositorySource::FileSystem => {
                        config.base_path.as_ref().map(|p| p.as_str())
                    }
                    _ => None, // For non-filesystem repositories, we can't load feature files directly
                }
            }
            None => None,
        };

        // If we have a filesystem repository, load features from files (more reliable than converting)
        if let Some(base_path) = base_path {
            let feature_dir = std::path::Path::new(base_path).join("configs/features");
            if feature_dir.exists() {
                let mut registry = FeatureRegistry::new();
                if let Err(e) = registry.load_from_directory(&feature_dir) {
                    tracing::warn!("Failed to load features from directory: {}", e);
                } else {
                    let mut executor = FeatureExecutor::new().with_stats();

                    let mut datasource_count = 0;

                    // Priority 1: Load datasources from server.yaml (if provided)
                    if let Some(ref server_datasources) = server_datasources {
                        tracing::info!("Loading datasources from server.yaml configuration...");
                        
                        // Track first events datasource (SQL/OLAP) and first lookup datasource (Feature Store)
                        let mut events_datasource_name: Option<String> = None;
                        let mut lookup_datasource_name: Option<String> = None;
                        
                        // First pass: register all datasources and identify logical datasources
                        for (name, config) in server_datasources.iter() {
                            use corint_runtime::datasource::config::DataSourceType;
                            
                            // Check if this is an events datasource (SQL or OLAP)
                            let is_events_ds = matches!(
                                &config.source_type,
                                DataSourceType::SQL(_) | DataSourceType::OLAP(_)
                            );
                            
                            // Check if this is a lookup datasource (Feature Store)
                            let is_lookup_ds = matches!(
                                &config.source_type,
                                DataSourceType::FeatureStore(_)
                            );
                            
                            match DataSourceClient::new(config.clone()).await {
                                Ok(client) => {
                                    executor.add_datasource(name, client);
                                    tracing::info!("  ✓ Loaded datasource from server.yaml: {}", name);
                                    datasource_count += 1;

                                    // Track first events datasource
                                    if is_events_ds && events_datasource_name.is_none() {
                                        events_datasource_name = Some(name.clone());
                                    }
                                    
                                    // Track first lookup datasource
                                    if is_lookup_ds && lookup_datasource_name.is_none() {
                                        lookup_datasource_name = Some(name.clone());
                                    }

                                    // Also register as "default" if it's the first datasource
                                    if datasource_count == 1 {
                                        let mut default_config = config.clone();
                                        default_config.name = "default".to_string();
                                        if let Ok(client_clone) = DataSourceClient::new(default_config).await {
                                            executor.add_datasource("default", client_clone);
                                            tracing::info!("  ✓ Registered as default datasource");
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("  ✗ Failed to create datasource {}: {}", name, e);
                                }
                            }
                        }
                        
                        // Second pass: Create logical datasource mappings
                        if let Some(events_name) = events_datasource_name {
                            if let Some(config) = server_datasources.get(&events_name) {
                                if let Ok(client) = DataSourceClient::new(config.clone()).await {
                                    executor.add_datasource("events_datasource", client);
                                    tracing::info!("  ✓ Mapped events_datasource -> {}", events_name);
                                }
                            }
                        }
                        
                        if let Some(lookup_name) = lookup_datasource_name {
                            if let Some(config) = server_datasources.get(&lookup_name) {
                                if let Ok(client) = DataSourceClient::new(config.clone()).await {
                                    executor.add_datasource("lookup_datasource", client);
                                    tracing::info!("  ✓ Mapped lookup_datasource -> {}", lookup_name);
                                }
                            }
                        }
                    }

                    // Priority 2: Fallback to loading datasources from repository/configs/datasources/
                    // (only if no server datasources were loaded)
                    if datasource_count == 0 {
                        let datasource_dir = std::path::Path::new(base_path).join("configs/datasources");
                        if datasource_dir.exists() {
                            tracing::info!("Loading datasources from repository/configs/datasources/...");
                            if let Ok(entries) = std::fs::read_dir(&datasource_dir) {
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                                        if let Ok(file_content) = std::fs::read_to_string(&path) {
                                            if let Ok(config) = serde_yaml::from_str::<RuntimeDataSourceConfig>(&file_content) {
                                                match DataSourceClient::new(config.clone()).await {
                                                    Ok(client) => {
                                                        executor.add_datasource(&config.name, client);
                                                        tracing::info!("  ✓ Loaded datasource from repository: {}", config.name);
                                                        datasource_count += 1;

                                                        // Also register as "default" if it's the first datasource
                                                        if datasource_count == 1 {
                                                            let mut default_config = config.clone();
                                                            default_config.name = "default".to_string();
                                                            if let Ok(client_clone) = DataSourceClient::new(default_config).await {
                                                                executor.add_datasource("default", client_clone);
                                                                tracing::info!("  ✓ Registered as default datasource");
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!("  ✗ Failed to create datasource {}: {}", config.name, e);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if datasource_count > 0 {
                        // Register features to executor
                        for feature in registry.all_features() {
                            if let Err(e) = executor.register_feature(feature.clone()) {
                                tracing::warn!("  ✗ Failed to register feature {}: {}", feature.name, e);
                            }
                        }

                        let feature_count = registry.count();
                        if feature_count > 0 {
                            tracing::info!(
                                "✓ Loaded {} datasources, {} features",
                                datasource_count, feature_count
                            );
                            return Ok(Some(executor));
                        }
                    }
                }
            }
        }

        // Fallback: Try to convert from repository content (for non-filesystem repositories)
        // This is more complex and may not work for all cases
        if !content.datasource_configs.is_empty() && !content.feature_definitions.is_empty() {
            tracing::warn!("Repository content conversion not fully implemented. Using filesystem loading is recommended.");
        }

        Ok(None)
    }

    /// Initialize ListService from repository content
    async fn init_list_service_from_content(
        _content: &RepositoryContent,
        repo_config: &Option<RepositoryConfig>,
        database_url: &Option<String>,
    ) -> Result<Option<corint_runtime::lists::ListService>> {
        use corint_runtime::lists::ListLoader;

        // Get repository base path
        let base_path = match repo_config {
            Some(config) => {
                match &config.source {
                    corint_repository::RepositorySource::FileSystem => {
                        config.base_path.as_ref().map(|p| p.as_str())
                    }
                    _ => None,
                }
            }
            None => None,
        };

        if let Some(base_path) = base_path {
            let lists_dir = std::path::Path::new(base_path).join("configs/lists");
            if !lists_dir.exists() {
                return Ok(None);
            }

            // Create list loader
            let mut loader = ListLoader::new(base_path);

            // Load datasources from repository configs/datasources/
            use corint_runtime::lists::loader::DatasourceInfo;
            use corint_runtime::datasource::config::{DataSourceConfig as RuntimeDataSourceConfig, DataSourceType};
            use std::collections::HashMap;

            let datasource_dir = std::path::Path::new(base_path).join("configs/datasources");
            let mut datasources: HashMap<String, DatasourceInfo> = HashMap::new();

            if datasource_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&datasource_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                            if let Ok(file_content) = std::fs::read_to_string(&path) {
                                if let Ok(config) = serde_yaml::from_str::<RuntimeDataSourceConfig>(&file_content) {
                                    // Extract provider and connection_string from source_type
                                    let (provider, connection_string) = match &config.source_type {
                                        DataSourceType::SQL(sql_config) => {
                                            let provider = match sql_config.provider {
                                                corint_runtime::datasource::config::SQLProvider::SQLite => "sqlite",
                                                corint_runtime::datasource::config::SQLProvider::PostgreSQL => "postgresql",
                                                corint_runtime::datasource::config::SQLProvider::MySQL => "mysql",
                                            };
                                            (provider.to_string(), sql_config.connection_string.clone())
                                        }
                                        DataSourceType::OLAP(olap_config) => {
                                            let provider = match olap_config.provider {
                                                corint_runtime::datasource::config::OLAPProvider::ClickHouse => "clickhouse",
                                                corint_runtime::datasource::config::OLAPProvider::Druid => "druid",
                                                corint_runtime::datasource::config::OLAPProvider::TimescaleDB => "timescaledb",
                                                corint_runtime::datasource::config::OLAPProvider::InfluxDB => "influxdb",
                                            };
                                            (provider.to_string(), olap_config.connection_string.clone())
                                        }
                                        DataSourceType::FeatureStore(fs_config) => {
                                            let provider = match fs_config.provider {
                                                corint_runtime::datasource::config::FeatureStoreProvider::Redis => "redis",
                                                corint_runtime::datasource::config::FeatureStoreProvider::Feast => "feast",
                                                corint_runtime::datasource::config::FeatureStoreProvider::Http => "http",
                                            };
                                            (provider.to_string(), fs_config.connection_string.clone())
                                        }
                                    };

                                    datasources.insert(config.name.clone(), DatasourceInfo {
                                        name: config.name.clone(),
                                        provider,
                                        connection_string,
                                    });
                                    tracing::debug!("  ✓ Loaded datasource for lists: {}", config.name);
                                }
                            }
                        }
                    }
                }
            }

            // Add datasources to loader
            if !datasources.is_empty() {
                let datasources_count = datasources.len();
                loader = loader.with_datasources(datasources);
                tracing::debug!("✓ Configured {} datasource(s) for list backends", datasources_count);
            }

            // Configure with database pool if available (for PostgreSQL/SQLite backends)
            #[cfg(feature = "sqlx")]
            {
                if let Some(db_url) = database_url {
                    // Try to create appropriate pool based on URL scheme
                    if db_url.starts_with("sqlite://") || db_url.starts_with("sqlite:") {
                        // Extract SQLite database path and set it for SQLite list backends
                        let db_path = db_url
                            .strip_prefix("sqlite://")
                            .or_else(|| db_url.strip_prefix("sqlite:"))
                            .unwrap_or(db_url);
                        loader = loader.with_sqlite_db_path(db_path);
                        tracing::info!("✓ SQLite database path configured for list backends: {}", db_path);
                    } else {
                        // PostgreSQL connection
                        match sqlx::postgres::PgPoolOptions::new()
                            .max_connections(5)
                            .connect(db_url)
                            .await
                        {
                            Ok(pool) => {
                                loader = loader.with_db_pool(std::sync::Arc::new(pool));
                                tracing::info!("✓ Database connection established for list backends");
                            }
                            Err(e) => {
                                tracing::warn!("Failed to connect to database for lists: {}", e);
                            }
                        }
                    }
                }
            }

            // Load all list configurations
            match loader.load_all().await {
                Ok(backends) => {
                    if backends.is_empty() {
                        Ok(None)
                    } else {
                        let list_count = backends.len();
                        let list_ids: Vec<&str> = backends.keys().map(|s| s.as_str()).collect();
                        tracing::info!("✓ Loaded {} list(s): {:?}", list_count, list_ids);
                        Ok(Some(corint_runtime::lists::ListService::new_with_backends(
                            backends,
                        )))
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load lists: {}", e);
                    Ok(None)
                }
            }
        } else {
            // For non-filesystem repositories, list configs would need to be converted
            // This is not yet implemented
            Ok(None)
        }
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
