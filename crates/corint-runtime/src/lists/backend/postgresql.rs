//! PostgreSQL-based list backend

use super::ListBackend;
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::sync::Arc;

#[cfg(feature = "sqlx")]
use sqlx::{PgPool, Row};

/// PostgreSQL list backend
///
/// Stores list entries in a PostgreSQL database.
/// Default table: `list_entries` with columns (list_id, value, created_at, expires_at)
pub struct PostgresBackend {
    /// Database connection pool
    #[cfg(feature = "sqlx")]
    pool: Arc<PgPool>,

    /// Table name for the list
    table: String,

    /// Column name for the value
    value_column: String,

    /// Optional expiration column
    expiration_column: Option<String>,
}

impl PostgresBackend {
    /// Create a new PostgreSQL backend
    #[cfg(feature = "sqlx")]
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            pool,
            table: "list_entries".to_string(),
            value_column: "value".to_string(),
            expiration_column: Some("expires_at".to_string()),
        }
    }

    /// Create a new PostgreSQL backend with custom table
    #[cfg(feature = "sqlx")]
    pub fn new_with_custom_table(
        pool: Arc<PgPool>,
        table: String,
        value_column: String,
        expiration_column: Option<String>,
    ) -> Self {
        Self {
            pool,
            table,
            value_column,
            expiration_column,
        }
    }

    /// Placeholder for when sqlx feature is disabled
    #[cfg(not(feature = "sqlx"))]
    pub fn new() -> Self {
        Self {
            table: "list_entries".to_string(),
            value_column: "value".to_string(),
            expiration_column: Some("expires_at".to_string()),
        }
    }

    /// Convert Value to string for SQL queries
    fn value_to_string(value: &Value) -> Result<String> {
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            _ => Err(RuntimeError::InvalidValue(format!(
                "PostgreSQL backend only supports string, number, and boolean values, got: {:?}",
                value
            ))),
        }
    }
}

#[cfg(feature = "sqlx")]
#[async_trait::async_trait]
impl ListBackend for PostgresBackend {
    async fn contains(&self, list_id: &str, value: &Value) -> Result<bool> {
        let value_str = Self::value_to_string(value)?;

        // Build query with optional expiration check
        let query = if let Some(exp_col) = &self.expiration_column {
            format!(
                "SELECT 1 FROM {} WHERE list_id = $1 AND {} = $2 AND ({} IS NULL OR {} > NOW()) LIMIT 1",
                self.table, self.value_column, exp_col, exp_col
            )
        } else {
            format!(
                "SELECT 1 FROM {} WHERE list_id = $1 AND {} = $2 LIMIT 1",
                self.table, self.value_column
            )
        };

        let result = sqlx::query(&query)
            .bind(list_id)
            .bind(&value_str)
            .fetch_optional(&*self.pool)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("Database query failed: {}", e)))?;

        Ok(result.is_some())
    }

    async fn add(&mut self, list_id: &str, value: Value) -> Result<()> {
        let value_str = Self::value_to_string(&value)?;

        let query = format!(
            "INSERT INTO {} (list_id, {}, created_at) VALUES ($1, $2, NOW())
             ON CONFLICT (list_id, {}) DO NOTHING",
            self.table, self.value_column, self.value_column
        );

        sqlx::query(&query)
            .bind(list_id)
            .bind(&value_str)
            .execute(&*self.pool)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("Database insert failed: {}", e)))?;

        Ok(())
    }

    async fn remove(&mut self, list_id: &str, value: &Value) -> Result<()> {
        let value_str = Self::value_to_string(value)?;

        let query = format!(
            "DELETE FROM {} WHERE list_id = $1 AND {} = $2",
            self.table, self.value_column
        );

        sqlx::query(&query)
            .bind(list_id)
            .bind(&value_str)
            .execute(&*self.pool)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("Database delete failed: {}", e)))?;

        Ok(())
    }

    async fn get_all(&self, list_id: &str) -> Result<Vec<Value>> {
        let query = if let Some(exp_col) = &self.expiration_column {
            format!(
                "SELECT {} FROM {} WHERE list_id = $1 AND ({} IS NULL OR {} > NOW())",
                self.value_column, self.table, exp_col, exp_col
            )
        } else {
            format!(
                "SELECT {} FROM {} WHERE list_id = $1",
                self.value_column, self.table
            )
        };

        let rows = sqlx::query(&query)
            .bind(list_id)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| RuntimeError::InvalidOperation(format!("Database query failed: {}", e)))?;

        let mut values = Vec::new();
        for row in rows {
            let value_str: String = row
                .try_get(0)
                .map_err(|e| RuntimeError::InvalidOperation(format!("Failed to read value: {}", e)))?;
            values.push(Value::String(value_str));
        }

        Ok(values)
    }
}

#[cfg(not(feature = "sqlx"))]
#[async_trait::async_trait]
impl ListBackend for PostgresBackend {
    async fn contains(&self, _list_id: &str, _value: &Value) -> Result<bool> {
        Err(RuntimeError::InvalidOperation(
            "PostgreSQL backend requires 'sqlx' feature to be enabled".to_string(),
        ))
    }

    async fn add(&mut self, _list_id: &str, _value: Value) -> Result<()> {
        Err(RuntimeError::InvalidOperation(
            "PostgreSQL backend requires 'sqlx' feature to be enabled".to_string(),
        ))
    }

    async fn remove(&mut self, _list_id: &str, _value: &Value) -> Result<()> {
        Err(RuntimeError::InvalidOperation(
            "PostgreSQL backend requires 'sqlx' feature to be enabled".to_string(),
        ))
    }

    async fn get_all(&self, _list_id: &str) -> Result<Vec<Value>> {
        Err(RuntimeError::InvalidOperation(
            "PostgreSQL backend requires 'sqlx' feature to be enabled".to_string(),
        ))
    }
}

#[cfg(all(test, feature = "sqlx"))]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    // Note: These tests require a running PostgreSQL database
    // Set DATABASE_URL environment variable to run these tests
    //
    // Example schema:
    // CREATE TABLE list_entries (
    //     id SERIAL PRIMARY KEY,
    //     list_id VARCHAR(255) NOT NULL,
    //     value TEXT NOT NULL,
    //     created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    //     expires_at TIMESTAMP NULL,
    //     UNIQUE(list_id, value)
    // );

    async fn setup_test_pool() -> Option<Arc<PgPool>> {
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            PgPoolOptions::new()
                .max_connections(1)
                .connect(&database_url)
                .await
                .ok()
                .map(Arc::new)
        } else {
            None
        }
    }

    #[tokio::test]
    async fn test_postgres_backend_basic() {
        let Some(pool) = setup_test_pool().await else {
            println!("Skipping test: DATABASE_URL not set");
            return;
        };

        let mut backend = PostgresBackend::new(pool.clone());

        let test_list = format!("test_list_{}", uuid::Uuid::new_v4());

        // Clean up any existing test data
        let _ = backend
            .remove(&test_list, &Value::String("test1".to_string()))
            .await;

        // Test add and contains
        backend
            .add(&test_list, Value::String("test1".to_string()))
            .await
            .unwrap();

        assert!(backend
            .contains(&test_list, &Value::String("test1".to_string()))
            .await
            .unwrap());

        assert!(!backend
            .contains(&test_list, &Value::String("test2".to_string()))
            .await
            .unwrap());

        // Test remove
        backend
            .remove(&test_list, &Value::String("test1".to_string()))
            .await
            .unwrap();

        assert!(!backend
            .contains(&test_list, &Value::String("test1".to_string()))
            .await
            .unwrap());
    }
}
