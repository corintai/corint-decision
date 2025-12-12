//! Database service client

use async_trait::async_trait;
use corint_core::Value;
use crate::error::Result;
use crate::service::client::{ServiceClient, ServiceRequest, ServiceResponse};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Database query
#[derive(Debug, Clone)]
pub struct DatabaseQuery {
    /// SQL query or command
    pub query: String,

    /// Query parameters
    pub params: Vec<Value>,
}

impl DatabaseQuery {
    /// Create a new database query
    pub fn new(query: String) -> Self {
        Self {
            query,
            params: Vec::new(),
        }
    }

    /// Add a parameter
    pub fn with_param(mut self, param: Value) -> Self {
        self.params.push(param);
        self
    }
}

/// Database client trait
#[async_trait]
pub trait DatabaseClient: Send + Sync {
    /// Execute a query
    async fn query(&self, query: DatabaseQuery) -> Result<Vec<HashMap<String, Value>>>;

    /// Execute a command (insert, update, delete)
    async fn execute(&self, query: DatabaseQuery) -> Result<u64>;
}

/// Mock database client for testing
pub struct MockDatabaseClient {
    name: String,
    data: Arc<RwLock<HashMap<String, Vec<HashMap<String, Value>>>>>,
}

impl MockDatabaseClient {
    /// Create a new mock database client
    pub fn new() -> Self {
        Self {
            name: "mock_db".to_string(),
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Seed data for a table
    pub fn seed_table(&self, table: String, rows: Vec<HashMap<String, Value>>) {
        self.data.write().unwrap().insert(table, rows);
    }
}

impl Default for MockDatabaseClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DatabaseClient for MockDatabaseClient {
    async fn query(&self, query: DatabaseQuery) -> Result<Vec<HashMap<String, Value>>> {
        // Simple mock: extract table name from query
        let table_name = query.query
            .split_whitespace()
            .skip_while(|w| !w.eq_ignore_ascii_case("from"))
            .nth(1)
            .unwrap_or("unknown");

        Ok(self.data.read().unwrap()
            .get(table_name)
            .cloned()
            .unwrap_or_default())
    }

    async fn execute(&self, _query: DatabaseQuery) -> Result<u64> {
        // Mock execution always succeeds with 1 row affected
        Ok(1)
    }
}

#[async_trait]
impl ServiceClient for MockDatabaseClient {
    async fn call(&self, request: ServiceRequest) -> Result<ServiceResponse> {
        match request.operation.as_str() {
            "query" => {
                let query_str = request.params.get("query")
                    .and_then(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                let query = DatabaseQuery::new(query_str);
                let results = self.query(query).await?;

                Ok(ServiceResponse::new(Value::Array(
                    results.into_iter()
                        .map(|row| Value::Object(row))
                        .collect()
                )))
            }
            "execute" => {
                let query_str = request.params.get("query")
                    .and_then(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                let query = DatabaseQuery::new(query_str);
                let rows_affected = self.execute(query).await?;

                Ok(ServiceResponse::new(Value::Number(rows_affected as f64)))
            }
            _ => Ok(ServiceResponse::new(Value::Null)),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_query() {
        let client = MockDatabaseClient::new();

        let mut row1 = HashMap::new();
        row1.insert("id".to_string(), Value::Number(1.0));
        row1.insert("name".to_string(), Value::String("Alice".to_string()));

        let mut row2 = HashMap::new();
        row2.insert("id".to_string(), Value::Number(2.0));
        row2.insert("name".to_string(), Value::String("Bob".to_string()));

        client.seed_table("users".to_string(), vec![row1, row2]);

        let query = DatabaseQuery::new("SELECT * FROM users".to_string());
        let results = client.query(query).await.unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_database_execute() {
        let client = MockDatabaseClient::new();

        let query = DatabaseQuery::new("INSERT INTO users VALUES (1, 'Alice')".to_string());
        let rows_affected = client.execute(query).await.unwrap();

        assert_eq!(rows_affected, 1);
    }

    #[tokio::test]
    async fn test_database_service_client() {
        let client = MockDatabaseClient::new();

        let mut row = HashMap::new();
        row.insert("count".to_string(), Value::Number(5.0));
        client.seed_table("transactions".to_string(), vec![row]);

        let request = ServiceRequest::new("database".to_string(), "query".to_string())
            .with_param("query".to_string(), Value::String("SELECT COUNT(*) FROM transactions".to_string()));

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_database_query_with_params() {
        let query = DatabaseQuery::new("SELECT * FROM users WHERE id = ?".to_string())
            .with_param(Value::Number(1.0));

        assert_eq!(query.params.len(), 1);
        assert_eq!(query.params[0], Value::Number(1.0));
    }

    #[tokio::test]
    async fn test_database_query_empty_table() {
        let client = MockDatabaseClient::new();

        // Query non-existent table
        let query = DatabaseQuery::new("SELECT * FROM nonexistent".to_string());
        let results = client.query(query).await.unwrap();

        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_database_multiple_params() {
        let query = DatabaseQuery::new("SELECT * FROM users WHERE name = ? AND age > ?".to_string())
            .with_param(Value::String("Alice".to_string()))
            .with_param(Value::Number(18.0));

        assert_eq!(query.params.len(), 2);
    }

    #[tokio::test]
    async fn test_database_service_execute() {
        let client = MockDatabaseClient::new();

        let request = ServiceRequest::new("database".to_string(), "execute".to_string())
            .with_param("query".to_string(), Value::String("DELETE FROM users WHERE id = 1".to_string()));

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
        assert_eq!(response.data, Value::Number(1.0));
    }

    #[tokio::test]
    async fn test_database_service_unknown_operation() {
        let client = MockDatabaseClient::new();

        let request = ServiceRequest::new("database".to_string(), "unknown".to_string());
        let response = client.call(request).await.unwrap();

        // Unknown operation should return Null
        assert_eq!(response.data, Value::Null);
    }

    #[tokio::test]
    async fn test_database_client_name() {
        let client = MockDatabaseClient::new();
        assert_eq!(client.name(), "mock_db");
    }

    #[tokio::test]
    async fn test_database_seed_multiple_tables() {
        let client = MockDatabaseClient::new();

        // Seed users table
        let mut user1 = HashMap::new();
        user1.insert("id".to_string(), Value::Number(1.0));
        user1.insert("name".to_string(), Value::String("Alice".to_string()));
        client.seed_table("users".to_string(), vec![user1]);

        // Seed orders table
        let mut order1 = HashMap::new();
        order1.insert("id".to_string(), Value::Number(100.0));
        order1.insert("user_id".to_string(), Value::Number(1.0));
        client.seed_table("orders".to_string(), vec![order1]);

        // Query both tables
        let users = client.query(DatabaseQuery::new("SELECT * FROM users".to_string())).await.unwrap();
        let orders = client.query(DatabaseQuery::new("SELECT * FROM orders".to_string())).await.unwrap();

        assert_eq!(users.len(), 1);
        assert_eq!(orders.len(), 1);
    }
}
