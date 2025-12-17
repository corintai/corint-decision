//! List backend implementations
//!
//! Backends for storing and querying list data.

pub mod file;
mod memory;
pub mod postgresql;

pub use file::FileBackend;
pub use memory::MemoryBackend;
pub use postgresql::PostgresBackend;

use crate::error::Result;
use corint_core::Value;

/// Trait for list storage backends
#[async_trait::async_trait]
pub trait ListBackend: Send + Sync {
    /// Check if a value exists in the list
    async fn contains(&self, list_id: &str, value: &Value) -> Result<bool>;

    /// Add a value to the list (for future management API)
    async fn add(&mut self, list_id: &str, value: Value) -> Result<()>;

    /// Remove a value from the list (for future management API)
    async fn remove(&mut self, list_id: &str, value: &Value) -> Result<()>;

    /// Get all values in a list (for future management API)
    async fn get_all(&self, list_id: &str) -> Result<Vec<Value>>;
}
