//! List service for managing lists

use super::backend::ListBackend;
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// List service that manages list lookups across multiple backends
pub struct ListService {
    /// Map of list_id to backend
    backends: Arc<RwLock<HashMap<String, Box<dyn ListBackend>>>>,
}

impl ListService {
    /// Create a new list service with memory backend
    pub fn new_with_memory() -> Self {
        Self {
            backends: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new list service with multiple backends
    pub fn new_with_backends(backends: HashMap<String, Box<dyn ListBackend>>) -> Self {
        Self {
            backends: Arc::new(RwLock::new(backends)),
        }
    }

    /// Check if a value exists in a list
    pub async fn contains(&self, list_id: &str, value: &Value) -> Result<bool> {
        let backends = self.backends.read().await;

        let backend = backends.get(list_id).ok_or_else(|| {
            // If list not configured, return false (list is empty)
            // This allows rules to work even if list configuration is missing
            tracing::warn!("List '{}' not configured, treating as empty", list_id);
            RuntimeError::InvalidOperation(format!("List '{}' not found", list_id))
        });

        match backend {
            Ok(backend) => backend.contains(list_id, value).await,
            Err(_) => Ok(false), // Treat missing list as empty
        }
    }

    /// Add a value to a list
    pub async fn add(&self, list_id: &str, value: Value) -> Result<()> {
        let mut backends = self.backends.write().await;

        let backend = backends.get_mut(list_id).ok_or_else(|| {
            RuntimeError::InvalidOperation(format!("List '{}' not found", list_id))
        })?;

        backend.add(list_id, value).await
    }

    /// Remove a value from a list
    pub async fn remove(&self, list_id: &str, value: &Value) -> Result<()> {
        let mut backends = self.backends.write().await;

        let backend = backends.get_mut(list_id).ok_or_else(|| {
            RuntimeError::InvalidOperation(format!("List '{}' not found", list_id))
        })?;

        backend.remove(list_id, value).await
    }

    /// Get all values in a list
    pub async fn get_all(&self, list_id: &str) -> Result<Vec<Value>> {
        let backends = self.backends.read().await;

        let backend = backends.get(list_id).ok_or_else(|| {
            RuntimeError::InvalidOperation(format!("List '{}' not found", list_id))
        })?;

        backend.get_all(list_id).await
    }

    /// Get list of all configured list IDs
    pub async fn list_ids(&self) -> Vec<String> {
        let backends = self.backends.read().await;
        backends.keys().cloned().collect()
    }

    /// Check if a list is configured
    pub async fn has_list(&self, list_id: &str) -> bool {
        let backends = self.backends.read().await;
        backends.contains_key(list_id)
    }
}

impl Clone for ListService {
    fn clone(&self) -> Self {
        Self {
            backends: Arc::clone(&self.backends),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_service_multiple_backends() {
        let mut backends: HashMap<String, Box<dyn ListBackend>> = HashMap::new();

        // Create backend for list1
        let mut backend1 = MemoryBackend::new();
        backend1.add("list1", Value::String("value1".to_string())).await.unwrap();
        backends.insert("list1".to_string(), Box::new(backend1));

        // Create backend for list2
        let mut backend2 = MemoryBackend::new();
        backend2.add("list2", Value::String("value2".to_string())).await.unwrap();
        backends.insert("list2".to_string(), Box::new(backend2));

        let service = ListService::new_with_backends(backends);

        // Check list1
        assert!(service
            .contains("list1", &Value::String("value1".to_string()))
            .await
            .unwrap());
        assert!(!service
            .contains("list1", &Value::String("value2".to_string()))
            .await
            .unwrap());

        // Check list2
        assert!(service
            .contains("list2", &Value::String("value2".to_string()))
            .await
            .unwrap());
        assert!(!service
            .contains("list2", &Value::String("value1".to_string()))
            .await
            .unwrap());

        // Check missing list (should return false)
        assert!(!service
            .contains("list3", &Value::String("any".to_string()))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_list_service_list_ids() {
        let mut backends: HashMap<String, Box<dyn ListBackend>> = HashMap::new();
        backends.insert("list1".to_string(), Box::new(MemoryBackend::new()));
        backends.insert("list2".to_string(), Box::new(MemoryBackend::new()));

        let service = ListService::new_with_backends(backends);

        let list_ids = service.list_ids().await;
        assert_eq!(list_ids.len(), 2);
        assert!(list_ids.contains(&"list1".to_string()));
        assert!(list_ids.contains(&"list2".to_string()));
    }
}
