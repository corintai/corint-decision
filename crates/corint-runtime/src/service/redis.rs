//! Redis service client

use async_trait::async_trait;
use corint_core::Value;
use crate::error::Result;
use crate::service::client::{ServiceClient, ServiceRequest, ServiceResponse};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Redis command
#[derive(Debug, Clone)]
pub struct RedisCommand {
    /// Command name (GET, SET, DEL, etc.)
    pub command: String,

    /// Command arguments
    pub args: Vec<String>,
}

impl RedisCommand {
    /// Create a new Redis command
    pub fn new(command: String) -> Self {
        Self {
            command,
            args: Vec::new(),
        }
    }

    /// Add an argument
    pub fn with_arg(mut self, arg: String) -> Self {
        self.args.push(arg);
        self
    }

    /// Add multiple arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args.extend(args);
        self
    }
}

/// Redis client trait
#[async_trait]
pub trait RedisClient: Send + Sync {
    /// Execute a Redis command
    async fn execute(&self, command: RedisCommand) -> Result<Value>;
}

/// Mock Redis client for testing
pub struct MockRedisClient {
    name: String,
    store: Arc<RwLock<HashMap<String, String>>>,
}

impl MockRedisClient {
    /// Create a new mock Redis client
    pub fn new() -> Self {
        Self {
            name: "mock_redis".to_string(),
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Seed data
    pub fn set(&self, key: String, value: String) {
        self.store.write().unwrap().insert(key, value);
    }

    /// Get data
    pub fn get(&self, key: &str) -> Option<String> {
        self.store.read().unwrap().get(key).cloned()
    }
}

impl Default for MockRedisClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RedisClient for MockRedisClient {
    async fn execute(&self, command: RedisCommand) -> Result<Value> {
        match command.command.to_uppercase().as_str() {
            "GET" => {
                let key = command.args.first().map(|s| s.as_str()).unwrap_or("");
                let value = self.get(key);
                Ok(value.map(Value::String).unwrap_or(Value::Null))
            }
            "SET" => {
                let key = command.args.get(0).cloned().unwrap_or_default();
                let value = command.args.get(1).cloned().unwrap_or_default();
                self.set(key, value);
                Ok(Value::String("OK".to_string()))
            }
            "DEL" => {
                let key = command.args.first().map(|s| s.as_str()).unwrap_or("");
                let existed = self.store.write().unwrap().remove(key).is_some();
                Ok(Value::Number(if existed { 1.0 } else { 0.0 }))
            }
            "EXISTS" => {
                let key = command.args.first().map(|s| s.as_str()).unwrap_or("");
                let exists = self.get(key).is_some();
                Ok(Value::Number(if exists { 1.0 } else { 0.0 }))
            }
            _ => Ok(Value::Null),
        }
    }
}

#[async_trait]
impl ServiceClient for MockRedisClient {
    async fn call(&self, request: ServiceRequest) -> Result<ServiceResponse> {
        let command = RedisCommand::new(request.operation.clone())
            .with_args(
                request.params.values()
                    .filter_map(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect()
            );

        let result = self.execute(command).await?;
        Ok(ServiceResponse::new(result))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_redis_set_get() {
        let client = MockRedisClient::new();

        // Set a value
        let set_cmd = RedisCommand::new("SET".to_string())
            .with_arg("key1".to_string())
            .with_arg("value1".to_string());
        let set_result = client.execute(set_cmd).await.unwrap();
        assert_eq!(set_result, Value::String("OK".to_string()));

        // Get the value
        let get_cmd = RedisCommand::new("GET".to_string())
            .with_arg("key1".to_string());
        let get_result = client.execute(get_cmd).await.unwrap();
        assert_eq!(get_result, Value::String("value1".to_string()));
    }

    #[tokio::test]
    async fn test_redis_del() {
        let client = MockRedisClient::new();

        client.set("key1".to_string(), "value1".to_string());

        // Delete the key
        let del_cmd = RedisCommand::new("DEL".to_string())
            .with_arg("key1".to_string());
        let del_result = client.execute(del_cmd).await.unwrap();
        assert_eq!(del_result, Value::Number(1.0));

        // Verify it's deleted
        let get_cmd = RedisCommand::new("GET".to_string())
            .with_arg("key1".to_string());
        let get_result = client.execute(get_cmd).await.unwrap();
        assert_eq!(get_result, Value::Null);
    }

    #[tokio::test]
    async fn test_redis_exists() {
        let client = MockRedisClient::new();

        client.set("key1".to_string(), "value1".to_string());

        // Check existence
        let exists_cmd = RedisCommand::new("EXISTS".to_string())
            .with_arg("key1".to_string());
        let exists_result = client.execute(exists_cmd).await.unwrap();
        assert_eq!(exists_result, Value::Number(1.0));

        // Check non-existent key
        let exists_cmd2 = RedisCommand::new("EXISTS".to_string())
            .with_arg("nonexistent".to_string());
        let exists_result2 = client.execute(exists_cmd2).await.unwrap();
        assert_eq!(exists_result2, Value::Number(0.0));
    }

    #[tokio::test]
    async fn test_redis_service_client() {
        let client = MockRedisClient::new();

        let request = ServiceRequest::new("redis".to_string(), "SET".to_string())
            .with_param("key".to_string(), Value::String("test_key".to_string()))
            .with_param("value".to_string(), Value::String("test_value".to_string()));

        let response = client.call(request).await.unwrap();
        assert_eq!(response.status, "success");
    }

    #[tokio::test]
    async fn test_redis_command_with_multiple_args() {
        let cmd = RedisCommand::new("MSET".to_string())
            .with_args(vec![
                "key1".to_string(),
                "value1".to_string(),
                "key2".to_string(),
                "value2".to_string(),
            ]);

        assert_eq!(cmd.args.len(), 4);
    }

    #[tokio::test]
    async fn test_redis_unknown_command() {
        let client = MockRedisClient::new();

        let cmd = RedisCommand::new("UNKNOWN".to_string());
        let result = client.execute(cmd).await.unwrap();

        assert_eq!(result, Value::Null);
    }

    #[tokio::test]
    async fn test_redis_client_name() {
        let client = MockRedisClient::new();
        assert_eq!(client.name(), "mock_redis");
    }

    #[tokio::test]
    async fn test_redis_get_nonexistent_key() {
        let client = MockRedisClient::new();

        let cmd = RedisCommand::new("GET".to_string())
            .with_arg("nonexistent".to_string());
        let result = client.execute(cmd).await.unwrap();

        assert_eq!(result, Value::Null);
    }

    #[tokio::test]
    async fn test_redis_del_nonexistent_key() {
        let client = MockRedisClient::new();

        let cmd = RedisCommand::new("DEL".to_string())
            .with_arg("nonexistent".to_string());
        let result = client.execute(cmd).await.unwrap();

        assert_eq!(result, Value::Number(0.0));
    }

    #[tokio::test]
    async fn test_redis_case_insensitive_commands() {
        let client = MockRedisClient::new();

        // Test lowercase command
        let set_cmd = RedisCommand::new("set".to_string())
            .with_arg("key1".to_string())
            .with_arg("value1".to_string());
        let set_result = client.execute(set_cmd).await.unwrap();
        assert_eq!(set_result, Value::String("OK".to_string()));

        // Test mixed case command
        let get_cmd = RedisCommand::new("GeT".to_string())
            .with_arg("key1".to_string());
        let get_result = client.execute(get_cmd).await.unwrap();
        assert_eq!(get_result, Value::String("value1".to_string()));
    }

    #[tokio::test]
    async fn test_redis_update_existing_key() {
        let client = MockRedisClient::new();

        // Set initial value
        client.set("key1".to_string(), "value1".to_string());

        // Update value
        let set_cmd = RedisCommand::new("SET".to_string())
            .with_arg("key1".to_string())
            .with_arg("value2".to_string());
        client.execute(set_cmd).await.unwrap();

        // Verify updated value
        assert_eq!(client.get("key1"), Some("value2".to_string()));
    }
}
