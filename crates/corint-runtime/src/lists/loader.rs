//! List configuration loader

use super::backend::{FileBackend, ListBackend, MemoryBackend, PostgresBackend};
use super::config::{ListBackendType, ListConfig, ListsConfig};
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

#[cfg(feature = "sqlx")]
use sqlx::PgPool;

/// List configuration loader
pub struct ListLoader {
    /// Base directory for list configurations
    base_dir: PathBuf,

    /// Database pool (optional, for PostgreSQL backend)
    #[cfg(feature = "sqlx")]
    db_pool: Option<Arc<PgPool>>,
}

impl ListLoader {
    /// Create a new list loader
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
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

        let backend: Box<dyn ListBackend> = match config.backend {
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
        };

        Ok((list_id, backend))
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
