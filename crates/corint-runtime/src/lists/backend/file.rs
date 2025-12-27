//! File-based list backend

use super::ListBackend;
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// File-based list backend
///
/// Loads list entries from a text file (one entry per line).
/// Supports automatic reloading at specified intervals.
pub struct FileBackend {
    /// Path to the list file
    file_path: PathBuf,

    /// Cached entries (list_id -> set of values)
    entries: Arc<RwLock<HashSet<String>>>,

    /// Whether automatic reloading is enabled
    #[allow(dead_code)]
    reload_enabled: bool,
}

impl FileBackend {
    /// Create a new file backend
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            entries: Arc::new(RwLock::new(HashSet::new())),
            reload_enabled: false,
        }
    }

    /// Create a new file backend with auto-reload
    pub fn new_with_reload(file_path: PathBuf, reload_interval_secs: u64) -> Self {
        let backend = Self {
            file_path: file_path.clone(),
            entries: Arc::new(RwLock::new(HashSet::new())),
            reload_enabled: true,
        };

        // Start background reload task
        let entries = Arc::clone(&backend.entries);
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(reload_interval_secs));
            loop {
                interval_timer.tick().await;
                if let Ok(loaded) = Self::load_from_file(&file_path).await {
                    let mut entries_write = entries.write().await;
                    *entries_write = loaded;
                }
            }
        });

        backend
    }

    /// Load entries from file
    async fn load_from_file(path: &PathBuf) -> Result<HashSet<String>> {
        let content = fs::read_to_string(path).await.map_err(|e| {
            RuntimeError::InvalidOperation(format!("Failed to read list file: {}", e))
        })?;

        let mut entries = HashSet::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                entries.insert(trimmed.to_string());
            }
        }

        Ok(entries)
    }

    /// Load entries immediately (called during initialization)
    pub async fn load(&self) -> Result<()> {
        let loaded = Self::load_from_file(&self.file_path).await?;
        let mut entries = self.entries.write().await;
        *entries = loaded;
        Ok(())
    }

    /// Get the number of entries
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Check if the list is empty
    pub async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }
}

#[async_trait::async_trait]
impl ListBackend for FileBackend {
    async fn contains(&self, _list_id: &str, value: &Value) -> Result<bool> {
        // Convert value to string for matching
        let search_str = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => {
                // Null values are never in a list
                return Ok(false);
            }
            _ => {
                return Err(RuntimeError::InvalidValue(format!(
                    "File backend only supports string, number, and boolean values, got: {:?}",
                    value
                )))
            }
        };

        let entries = self.entries.read().await;
        Ok(entries.contains(&search_str))
    }

    async fn add(&mut self, _list_id: &str, _value: Value) -> Result<()> {
        // File backend is read-only for now
        // Adding values would require writing back to the file
        Err(RuntimeError::InvalidOperation(
            "File backend is read-only".to_string(),
        ))
    }

    async fn remove(&mut self, _list_id: &str, _value: &Value) -> Result<()> {
        // File backend is read-only
        Err(RuntimeError::InvalidOperation(
            "File backend is read-only".to_string(),
        ))
    }

    async fn get_all(&self, _list_id: &str) -> Result<Vec<Value>> {
        let entries = self.entries.read().await;
        Ok(entries
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_file_backend_basic() {
        // Create a temporary file with test data
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "value1").unwrap();
        writeln!(temp_file, "value2").unwrap();
        writeln!(temp_file, "# comment").unwrap();
        writeln!(temp_file, "value3").unwrap();
        writeln!(temp_file, "").unwrap(); // empty line
        temp_file.flush().unwrap();

        let backend = FileBackend::new(temp_file.path().to_path_buf());
        backend.load().await.unwrap();

        // Test contains
        assert!(backend
            .contains("test", &Value::String("value1".to_string()))
            .await
            .unwrap());
        assert!(backend
            .contains("test", &Value::String("value2".to_string()))
            .await
            .unwrap());
        assert!(backend
            .contains("test", &Value::String("value3".to_string()))
            .await
            .unwrap());
        assert!(!backend
            .contains("test", &Value::String("value4".to_string()))
            .await
            .unwrap());

        // Comment should not be included
        assert!(!backend
            .contains("test", &Value::String("# comment".to_string()))
            .await
            .unwrap());

        // Check count
        assert_eq!(backend.len().await, 3);
    }

    #[tokio::test]
    async fn test_file_backend_get_all() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "a").unwrap();
        writeln!(temp_file, "b").unwrap();
        temp_file.flush().unwrap();

        let backend = FileBackend::new(temp_file.path().to_path_buf());
        backend.load().await.unwrap();

        let all = backend.get_all("test").await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_file_backend_read_only() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut backend = FileBackend::new(temp_file.path().to_path_buf());
        backend.load().await.unwrap();

        // Add should fail (read-only)
        let result = backend.add("test", Value::String("new".to_string())).await;
        assert!(result.is_err());

        // Remove should fail (read-only)
        let result = backend
            .remove("test", &Value::String("any".to_string()))
            .await;
        assert!(result.is_err());
    }
}
