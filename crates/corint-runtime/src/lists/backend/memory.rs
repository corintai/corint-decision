//! In-memory list backend
//!
//! Simple memory-based list storage for testing and development.

use super::ListBackend;
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

/// In-memory list backend
///
/// Stores list data in memory using HashSets.
/// This is suitable for testing and development, but not for production
/// as data is lost when the process restarts.
pub struct MemoryBackend {
    /// Map of list_id -> set of values
    lists: RwLock<HashMap<String, HashSet<String>>>,
}

impl MemoryBackend {
    /// Create a new memory backend
    pub fn new() -> Self {
        Self {
            lists: RwLock::new(HashMap::new()),
        }
    }

    /// Convert a Value to a string for storage
    fn value_to_key(value: &Value) -> Result<String> {
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Null => Ok("null".to_string()),
            Value::Array(_) | Value::Object(_) => {
                // For complex types, use JSON representation
                serde_json::to_string(value).map_err(|e| {
                    RuntimeError::InvalidValue(format!("Cannot serialize value for list: {}", e))
                })
            }
        }
    }
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ListBackend for MemoryBackend {
    async fn contains(&self, list_id: &str, value: &Value) -> Result<bool> {
        let key = Self::value_to_key(value)?;
        let lists = self.lists.read().await;

        if let Some(list) = lists.get(list_id) {
            Ok(list.contains(&key))
        } else {
            Ok(false)
        }
    }

    async fn add(&mut self, list_id: &str, value: Value) -> Result<()> {
        let key = Self::value_to_key(&value)?;
        let mut lists = self.lists.write().await;

        lists
            .entry(list_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(key);

        Ok(())
    }

    async fn remove(&mut self, list_id: &str, value: &Value) -> Result<()> {
        let key = Self::value_to_key(value)?;
        let mut lists = self.lists.write().await;

        if let Some(list) = lists.get_mut(list_id) {
            list.remove(&key);
        }

        Ok(())
    }

    async fn get_all(&self, list_id: &str) -> Result<Vec<Value>> {
        let lists = self.lists.read().await;

        if let Some(list) = lists.get(list_id) {
            // Convert all strings back to Values
            let values: Vec<Value> = list
                .iter()
                .map(|s| {
                    // Try to parse as number first
                    if let Ok(n) = s.parse::<f64>() {
                        return Value::Number(n);
                    }
                    // Try to parse as boolean
                    if let Ok(b) = s.parse::<bool>() {
                        return Value::Bool(b);
                    }
                    // Check for null
                    if s == "null" {
                        return Value::Null;
                    }
                    // Try to parse as JSON (for arrays/objects)
                    if let Ok(v) = serde_json::from_str::<Value>(s) {
                        return v;
                    }
                    // Default to string
                    Value::String(s.clone())
                })
                .collect();

            Ok(values)
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_backend_string() {
        let mut backend = MemoryBackend::new();

        // Add some values
        backend
            .add("test_list", Value::String("value1".to_string()))
            .await
            .unwrap();
        backend
            .add("test_list", Value::String("value2".to_string()))
            .await
            .unwrap();

        // Check contains
        assert!(backend
            .contains("test_list", &Value::String("value1".to_string()))
            .await
            .unwrap());
        assert!(backend
            .contains("test_list", &Value::String("value2".to_string()))
            .await
            .unwrap());
        assert!(!backend
            .contains("test_list", &Value::String("value3".to_string()))
            .await
            .unwrap());

        // Check non-existent list
        assert!(!backend
            .contains("other_list", &Value::String("value1".to_string()))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_memory_backend_number() {
        let mut backend = MemoryBackend::new();

        backend.add("numbers", Value::Number(42.0)).await.unwrap();
        backend.add("numbers", Value::Number(100.0)).await.unwrap();

        assert!(backend
            .contains("numbers", &Value::Number(42.0))
            .await
            .unwrap());
        assert!(!backend
            .contains("numbers", &Value::Number(50.0))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_memory_backend_remove() {
        let mut backend = MemoryBackend::new();

        backend
            .add("test_list", Value::String("value1".to_string()))
            .await
            .unwrap();

        assert!(backend
            .contains("test_list", &Value::String("value1".to_string()))
            .await
            .unwrap());

        backend
            .remove("test_list", &Value::String("value1".to_string()))
            .await
            .unwrap();

        assert!(!backend
            .contains("test_list", &Value::String("value1".to_string()))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_memory_backend_get_all() {
        let mut backend = MemoryBackend::new();

        backend
            .add("test_list", Value::String("value1".to_string()))
            .await
            .unwrap();
        backend
            .add("test_list", Value::String("value2".to_string()))
            .await
            .unwrap();

        let values = backend.get_all("test_list").await.unwrap();
        assert_eq!(values.len(), 2);
    }
}
