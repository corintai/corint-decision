//! List configuration loader

use super::backend::{FileBackend, ListBackend, MemoryBackend, SqliteBackend};
use super::config::{ListBackendType, ListConfig, ListsConfig};
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

#[cfg(feature = "sqlx")]
use sqlx::PgPool;

#[cfg(feature = "sqlx")]
use super::backend::PostgresBackend;

/// Datasource information for list backends
#[derive(Debug, Clone)]
pub struct DatasourceInfo {
    /// Datasource name
    pub name: String,
    /// Provider type (sqlite, postgresql, etc.)
    pub provider: String,
    /// Connection string or path
    pub connection_string: String,
}

/// List configuration loader
pub struct ListLoader {
    /// Base directory for list configurations
    base_dir: PathBuf,

    /// Datasources map (name -> info)
    datasources: HashMap<String, DatasourceInfo>,

    /// Default SQLite database path (for sqlite backend - deprecated, use datasources)
    sqlite_db_path: Option<PathBuf>,

    /// Database pool (optional, for PostgreSQL backend)
    #[cfg(feature = "sqlx")]
    db_pool: Option<Arc<PgPool>>,
}

impl ListLoader {
    /// Create a new list loader
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            datasources: HashMap::new(),
            sqlite_db_path: None,
            #[cfg(feature = "sqlx")]
            db_pool: None,
        }
    }

    /// Set the database pool for PostgreSQL backends
    #[cfg(feature = "sqlx")]
    pub fn with_db_pool(mut self, pool: Arc<PgPool>) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Add a datasource configuration
    pub fn with_datasource(mut self, name: String, provider: String, connection_string: String) -> Self {
        self.datasources.insert(name.clone(), DatasourceInfo {
            name,
            provider,
            connection_string,
        });
        self
    }

    /// Add multiple datasources
    pub fn with_datasources(mut self, datasources: HashMap<String, DatasourceInfo>) -> Self {
        self.datasources.extend(datasources);
        self
    }

    /// Set the default SQLite database path for SQLite backends (deprecated)
    pub fn with_sqlite_db_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.sqlite_db_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Load all list configurations from the base directory
    pub async fn load_all(&self) -> Result<HashMap<String, Box<dyn ListBackend>>> {
        let mut backends = HashMap::new();

        // Find all YAML files in the lists directory
        let lists_dir = self.base_dir.join("configs/lists");
        if !lists_dir.exists() {
            tracing::warn!("Lists directory does not exist: {:?}", lists_dir);
            return Ok(backends);
        }

        let mut entries = fs::read_dir(&lists_dir)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("Failed to read lists directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                match self.load_from_file(&path).await {
                    Ok(mut file_backends) => {
                        backends.extend(file_backends.drain());
                    }
                    Err(e) => {
                        tracing::error!("Failed to load list config from {:?}: {}", path, e);
                    }
                }
            }
        }

        tracing::info!("Loaded {} list backends", backends.len());
        Ok(backends)
    }

    /// Load list configurations from a single file
    async fn load_from_file(&self, path: &Path) -> Result<HashMap<String, Box<dyn ListBackend>>> {
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("Failed to read file: {}", e)))?;

        // Try parsing as ListsConfig first (multiple lists)
        if let Ok(lists_config) = serde_yaml::from_str::<ListsConfig>(&content) {
            let mut backends = HashMap::new();
            for config in lists_config.lists {
                match self.create_backend(config).await {
                    Ok(backend) => {
                        backends.insert(backend.0, backend.1);
                    }
                    Err(e) => {
                        tracing::error!("Failed to create backend: {}", e);
                    }
                }
            }
            return Ok(backends);
        }

        // Try parsing as single ListConfig
        if let Ok(config) = serde_yaml::from_str::<ListConfig>(&content) {
            let backend = self.create_backend(config).await?;
            return Ok(vec![backend].into_iter().collect());
        }

        Err(RuntimeError::InvalidOperation(format!(
            "Failed to parse list config from {:?}",
            path
        )))
    }

    /// Create a backend from configuration
    async fn create_backend(&self, config: ListConfig) -> Result<(String, Box<dyn ListBackend>)> {
        let list_id = config.id.clone();

        // Check if using datasource reference
        if let Some(datasource_name) = config.datasource_name() {
            return self.create_datasource_backend(&list_id, datasource_name, &config).await;
        }

        // Otherwise use backend type
        let backend_type = config.backend.clone().ok_or_else(|| {
            RuntimeError::InvalidOperation(format!(
                "List '{}' must specify either 'datasource' or 'backend'",
                list_id
            ))
        })?;

        let backend: Box<dyn ListBackend> = match backend_type {
            ListBackendType::Memory => {
                let mut backend = MemoryBackend::new();
                // Load initial values
                for value_str in config.initial_values {
                    backend
                        .add(&list_id, Value::String(value_str))
                        .await?;
                }
                Box::new(backend)
            }

            ListBackendType::File => {
                let file_path = config
                    .file_path()
                    .ok_or_else(|| RuntimeError::InvalidOperation("File backend requires 'path' field".to_string()))?;

                // Resolve path relative to repository root
                let full_path = if Path::new(&file_path).is_absolute() {
                    PathBuf::from(file_path)
                } else {
                    self.base_dir.join(&file_path)
                };

                let backend = if let Some(reload_interval) = config.file_reload_interval() {
                    FileBackend::new_with_reload(full_path, reload_interval)
                } else {
                    FileBackend::new(full_path)
                };

                // Load initial data
                backend.load().await?;
                Box::new(backend)
            }

            #[cfg(feature = "sqlx")]
            ListBackendType::PostgreSQL => {
                let pool = self.db_pool.as_ref().ok_or_else(|| {
                    RuntimeError::InvalidOperation("PostgreSQL backend requires database pool".to_string())
                })?;

                let backend = if let Some(postgres_config) = &config.postgres_config {
                    PostgresBackend::new_with_custom_table(
                        Arc::clone(pool),
                        config.postgres_table(),
                        config.postgres_value_column(),
                        postgres_config.expiration_column.clone(),
                    )
                } else {
                    PostgresBackend::new(Arc::clone(pool))
                };

                Box::new(backend)
            }

            #[cfg(not(feature = "sqlx"))]
            ListBackendType::PostgreSQL => {
                return Err(RuntimeError::InvalidOperation(
                    "PostgreSQL backend requires 'sqlx' feature".to_string(),
                ));
            }

            ListBackendType::Redis => {
                return Err(RuntimeError::InvalidOperation(
                    "Redis backend not yet implemented".to_string(),
                ));
            }

            ListBackendType::Api => {
                return Err(RuntimeError::InvalidOperation(
                    "API backend not yet implemented".to_string(),
                ));
            }

            ListBackendType::Sqlite => {
                // Get database path from config or use default
                let db_path = config
                    .sqlite_db_path()
                    .map(PathBuf::from)
                    .or_else(|| self.sqlite_db_path.clone())
                    .ok_or_else(|| {
                        RuntimeError::InvalidOperation(
                            "SQLite backend requires 'db_path' field or default sqlite_db_path".to_string(),
                        )
                    })?;

                // For SQLite, use the path as-is (relative to current working directory)
                // This is different from file backend which is relative to repository root
                // because SQLite databases are typically shared across the application
                let full_path = db_path;

                let backend = SqliteBackend::new_with_custom_table(
                    full_path,
                    config.sqlite_table(),
                    config.sqlite_value_column(),
                    config.sqlite_expiration_column(),
                );

                Box::new(backend)
            }
        };

        Ok((list_id, backend))
    }

    /// Create a backend from datasource reference
    async fn create_datasource_backend(
        &self,
        list_id: &str,
        datasource_name: &str,
        config: &ListConfig,
    ) -> Result<(String, Box<dyn ListBackend>)> {
        // Look up the datasource
        let datasource = self.datasources.get(datasource_name).ok_or_else(|| {
            RuntimeError::InvalidOperation(format!(
                "Datasource '{}' not found for list '{}'",
                datasource_name, list_id
            ))
        })?;

        // Create backend based on provider type
        let backend: Box<dyn ListBackend> = match datasource.provider.as_str() {
            "sqlite" => {
                // Extract database path from connection string
                let db_path = datasource
                    .connection_string
                    .strip_prefix("sqlite://")
                    .or_else(|| datasource.connection_string.strip_prefix("sqlite:"))
                    .unwrap_or(&datasource.connection_string);

                // Use table config from ListConfig or defaults
                let table = config.table().unwrap_or_else(|| "list_entries".to_string());
                let value_column = config.value_column().unwrap_or_else(|| "value".to_string());
                // For SQLite, default to "expires_at" if not specified
                let expiration_column = config.expiration_column()
                    .or_else(|| Some("expires_at".to_string()));

                let backend = SqliteBackend::new_with_custom_table(
                    PathBuf::from(db_path),
                    table,
                    value_column,
                    expiration_column,
                );

                Box::new(backend)
            }

            #[cfg(feature = "sqlx")]
            "postgresql" => {
                let pool = self.db_pool.as_ref().ok_or_else(|| {
                    RuntimeError::InvalidOperation(format!(
                        "PostgreSQL datasource '{}' for list '{}' requires database pool",
                        datasource_name, list_id
                    ))
                })?;

                // Use table config from ListConfig or defaults
                let table = config.table().unwrap_or_else(|| "list_entries".to_string());
                let value_column = config.value_column().unwrap_or_else(|| "value".to_string());
                // For PostgreSQL, expiration_column is optional (None if not configured)
                let expiration_column = config.expiration_column();

                let backend = PostgresBackend::new_with_custom_table(
                    Arc::clone(pool),
                    table,
                    value_column,
                    expiration_column,
                );

                Box::new(backend)
            }

            #[cfg(not(feature = "sqlx"))]
            "postgresql" => {
                return Err(RuntimeError::InvalidOperation(format!(
                    "PostgreSQL datasource '{}' for list '{}' requires 'sqlx' feature",
                    datasource_name, list_id
                )));
            }

            provider => {
                return Err(RuntimeError::InvalidOperation(format!(
                    "Unsupported datasource provider '{}' for list '{}'. Supported: sqlite, postgresql",
                    provider, list_id
                )));
            }
        };

        Ok((list_id.to_string(), backend))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_memory_list() {
        let temp_dir = TempDir::new().unwrap();
        let lists_dir = temp_dir.path().join("configs/lists");
        fs::create_dir_all(&lists_dir).await.unwrap();

        let config_file = lists_dir.join("test.yaml");
        let yaml = r#"
lists:
  - id: test_list
    description: "Test list"
    backend: memory
    initial_values:
      - "value1"
      - "value2"
"#;
        fs::write(&config_file, yaml).await.unwrap();

        let loader = ListLoader::new(temp_dir.path());
        let backends = loader.load_all().await.unwrap();

        assert_eq!(backends.len(), 1);
        assert!(backends.contains_key("test_list"));

        let backend = backends.get("test_list").unwrap();
        assert!(backend
            .contains("test_list", &Value::String("value1".to_string()))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_load_file_list() {
        let temp_dir = TempDir::new().unwrap();
        let lists_dir = temp_dir.path().join("configs/lists");
        fs::create_dir_all(&lists_dir).await.unwrap();

        // Create data file
        let data_dir = temp_dir.path().join("configs/lists/data");
        fs::create_dir_all(&data_dir).await.unwrap();
        let data_file = data_dir.join("values.txt");
        fs::write(&data_file, "item1\nitem2\nitem3").await.unwrap();

        // Create list config
        let config_file = lists_dir.join("file_list.yaml");
        let yaml = format!(
            r#"
id: file_list
description: "File-based list"
backend: file
path: "configs/lists/data/values.txt"
"#
        );
        fs::write(&config_file, yaml).await.unwrap();

        let loader = ListLoader::new(temp_dir.path());
        let backends = loader.load_all().await.unwrap();

        assert_eq!(backends.len(), 1);
        let backend = backends.get("file_list").unwrap();
        assert!(backend
            .contains("file_list", &Value::String("item1".to_string()))
            .await
            .unwrap());
    }
}
