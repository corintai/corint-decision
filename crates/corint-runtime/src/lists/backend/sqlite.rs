//! SQLite-based list backend

use super::ListBackend;
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "sqlx")]
use sqlx::{Row, SqlitePool};

/// SQLite list backend
///
/// Stores list entries in a SQLite database.
/// Default table: `list_entries` with columns (list_id, value, created_at, expires_at)
pub struct SqliteBackend {
    /// Database file path
    db_path: PathBuf,

    /// Connection pool (lazy initialized)
    #[cfg(feature = "sqlx")]
    pool: Arc<RwLock<Option<SqlitePool>>>,

    /// Table name for the list
    table: String,

    /// Column name for the value
    value_column: String,

    /// Optional expiration column
    expiration_column: Option<String>,
}

impl SqliteBackend {
    /// Create a new SQLite backend
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            #[cfg(feature = "sqlx")]
            pool: Arc::new(RwLock::new(None)),
            table: "list_entries".to_string(),
            value_column: "value".to_string(),
            expiration_column: Some("expires_at".to_string()),
        }
    }

    /// Create a new SQLite backend with custom table configuration
    pub fn new_with_custom_table(
        db_path: PathBuf,
        table: String,
        value_column: String,
        expiration_column: Option<String>,
    ) -> Self {
        Self {
            db_path,
            #[cfg(feature = "sqlx")]
            pool: Arc::new(RwLock::new(None)),
            table,
            value_column,
            expiration_column,
        }
    }

    /// Get or create connection pool
    #[cfg(feature = "sqlx")]
    async fn get_pool(&self) -> Result<SqlitePool> {
        // Check if pool already exists
        {
            let pool_guard = self.pool.read().await;
            if let Some(pool) = pool_guard.as_ref() {
                return Ok(pool.clone());
            }
        }

        // Create new pool
        let db_url = format!("sqlite:{}", self.db_path.display());
        let pool = SqlitePool::connect(&db_url).await.map_err(|e| {
            RuntimeError::InvalidOperation(format!(
                "Failed to connect to SQLite database at {:?}: {}",
                self.db_path, e
            ))
        })?;

        // Store pool for reuse
        {
            let mut pool_guard = self.pool.write().await;
            *pool_guard = Some(pool.clone());
        }

        Ok(pool)
    }

    /// Convert Value to string for SQL queries
    fn value_to_string(value: &Value) -> Result<String> {
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            _ => Err(RuntimeError::InvalidValue(format!(
                "SQLite backend only supports string, number, and boolean values, got: {:?}",
                value
            ))),
        }
    }
}

#[cfg(feature = "sqlx")]
#[async_trait::async_trait]
impl ListBackend for SqliteBackend {
    async fn contains(&self, list_id: &str, value: &Value) -> Result<bool> {
        let value_str = Self::value_to_string(value)?;
        let pool = self.get_pool().await?;

        // Build query with optional expiration check
        // SQLite uses datetime('now') for current timestamp
        // Use datetime() to normalize timestamp format before comparison
        let query = if let Some(exp_col) = &self.expiration_column {
            format!(
                "SELECT 1 FROM {} WHERE list_id = ?1 AND {} = ?2 AND ({} IS NULL OR datetime({}) > datetime('now')) LIMIT 1",
                self.table, self.value_column, exp_col, exp_col
            )
        } else {
            format!(
                "SELECT 1 FROM {} WHERE list_id = ?1 AND {} = ?2 LIMIT 1",
                self.table, self.value_column
            )
        };

        let result = sqlx::query(&query)
            .bind(list_id)
            .bind(&value_str)
            .fetch_optional(&pool)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("SQLite query failed: {}", e)))?;

        Ok(result.is_some())
    }

    async fn add(&mut self, list_id: &str, value: Value) -> Result<()> {
        let value_str = Self::value_to_string(&value)?;
        let pool = self.get_pool().await?;

        let query = format!(
            "INSERT OR IGNORE INTO {} (list_id, {}, created_at) VALUES (?1, ?2, datetime('now'))",
            self.table, self.value_column
        );

        sqlx::query(&query)
            .bind(list_id)
            .bind(&value_str)
            .execute(&pool)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("SQLite insert failed: {}", e)))?;

        Ok(())
    }

    async fn remove(&mut self, list_id: &str, value: &Value) -> Result<()> {
        let value_str = Self::value_to_string(value)?;
        let pool = self.get_pool().await?;

        let query = format!(
            "DELETE FROM {} WHERE list_id = ?1 AND {} = ?2",
            self.table, self.value_column
        );

        sqlx::query(&query)
            .bind(list_id)
            .bind(&value_str)
            .execute(&pool)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("SQLite delete failed: {}", e)))?;

        Ok(())
    }

    async fn get_all(&self, list_id: &str) -> Result<Vec<Value>> {
        let pool = self.get_pool().await?;

        // Use datetime() to normalize timestamp format before comparison
        let query = if let Some(exp_col) = &self.expiration_column {
            format!(
                "SELECT {} FROM {} WHERE list_id = ?1 AND ({} IS NULL OR datetime({}) > datetime('now'))",
                self.value_column, self.table, exp_col, exp_col
            )
        } else {
            format!(
                "SELECT {} FROM {} WHERE list_id = ?1",
                self.value_column, self.table
            )
        };

        let rows = sqlx::query(&query)
            .bind(list_id)
            .fetch_all(&pool)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("SQLite query failed: {}", e)))?;

        let mut values = Vec::new();
        for row in rows {
            let value_str: String = row.try_get(0).map_err(|e| {
                RuntimeError::InvalidOperation(format!("Failed to read value: {}", e))
            })?;
            values.push(Value::String(value_str));
        }

        Ok(values)
    }
}

#[cfg(not(feature = "sqlx"))]
#[async_trait::async_trait]
impl ListBackend for SqliteBackend {
    async fn contains(&self, _list_id: &str, _value: &Value) -> Result<bool> {
        Err(RuntimeError::InvalidOperation(
            "SQLite backend requires 'sqlx' feature to be enabled".to_string(),
        ))
    }

    async fn add(&mut self, _list_id: &str, _value: Value) -> Result<()> {
        Err(RuntimeError::InvalidOperation(
            "SQLite backend requires 'sqlx' feature to be enabled".to_string(),
        ))
    }

    async fn remove(&mut self, _list_id: &str, _value: &Value) -> Result<()> {
        Err(RuntimeError::InvalidOperation(
            "SQLite backend requires 'sqlx' feature to be enabled".to_string(),
        ))
    }

    async fn get_all(&self, _list_id: &str) -> Result<Vec<Value>> {
        Err(RuntimeError::InvalidOperation(
            "SQLite backend requires 'sqlx' feature to be enabled".to_string(),
        ))
    }
}

#[cfg(all(test, feature = "sqlx"))]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    async fn setup_test_db() -> (NamedTempFile, PathBuf) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Create schema using sqlx
        let db_url = format!("sqlite:{}?mode=rwc", path.display());
        let pool = SqlitePool::connect(&db_url).await.unwrap();

        sqlx::query(
            "CREATE TABLE list_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                list_id TEXT NOT NULL,
                value TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                expires_at TEXT,
                UNIQUE(list_id, value)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        (temp_file, path)
    }

    #[tokio::test]
    async fn test_sqlite_backend_basic() {
        let (_temp_file, path) = setup_test_db().await;
        let mut backend = SqliteBackend::new(path);

        let test_list = "test_list";

        // Test add and contains
        backend
            .add(test_list, Value::String("test1".to_string()))
            .await
            .unwrap();

        assert!(backend
            .contains(test_list, &Value::String("test1".to_string()))
            .await
            .unwrap());

        assert!(!backend
            .contains(test_list, &Value::String("test2".to_string()))
            .await
            .unwrap());

        // Test remove
        backend
            .remove(test_list, &Value::String("test1".to_string()))
            .await
            .unwrap();

        assert!(!backend
            .contains(test_list, &Value::String("test1".to_string()))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_sqlite_backend_expiration() {
        let (_temp_file, path) = setup_test_db().await;
        let backend = SqliteBackend::new(path.clone());

        let test_list = "blocked_users";

        // Insert entries directly using sqlx
        let db_url = format!("sqlite:{}", path.display());
        let pool = SqlitePool::connect(&db_url).await.unwrap();

        // Insert expired entry
        sqlx::query(
            "INSERT INTO list_entries (list_id, value, expires_at) VALUES (?1, ?2, datetime('now', '-1 day'))",
        )
        .bind(test_list)
        .bind("expired_user")
        .execute(&pool)
        .await
        .unwrap();

        // Insert active entry
        sqlx::query(
            "INSERT INTO list_entries (list_id, value, expires_at) VALUES (?1, ?2, datetime('now', '+1 day'))",
        )
        .bind(test_list)
        .bind("active_user")
        .execute(&pool)
        .await
        .unwrap();

        // Insert entry without expiration (never expires)
        sqlx::query(
            "INSERT INTO list_entries (list_id, value, expires_at) VALUES (?1, ?2, NULL)",
        )
        .bind(test_list)
        .bind("permanent_user")
        .execute(&pool)
        .await
        .unwrap();

        // Expired user should NOT be found
        assert!(!backend
            .contains(test_list, &Value::String("expired_user".to_string()))
            .await
            .unwrap());

        // Active user should be found
        assert!(backend
            .contains(test_list, &Value::String("active_user".to_string()))
            .await
            .unwrap());

        // Permanent user should be found
        assert!(backend
            .contains(test_list, &Value::String("permanent_user".to_string()))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_sqlite_backend_get_all() {
        let (_temp_file, path) = setup_test_db().await;
        let mut backend = SqliteBackend::new(path);

        let test_list = "test_list";

        backend
            .add(test_list, Value::String("value1".to_string()))
            .await
            .unwrap();
        backend
            .add(test_list, Value::String("value2".to_string()))
            .await
            .unwrap();
        backend
            .add(test_list, Value::String("value3".to_string()))
            .await
            .unwrap();

        let values = backend.get_all(test_list).await.unwrap();
        assert_eq!(values.len(), 3);
    }
}
