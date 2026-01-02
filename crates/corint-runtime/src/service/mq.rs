//! Message Queue service client

use crate::error::Result;
use crate::service::client::{ServiceClient, ServiceRequest, ServiceResponse};
use async_trait::async_trait;
use corint_core::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Message Queue driver type
#[derive(Debug, Clone, PartialEq)]
pub enum MqDriver {
    Kafka,
    RabbitMQ,
}

/// Message to be published
#[derive(Debug, Clone)]
pub struct MqMessage {
    /// Topic name
    pub topic: String,
    /// Message key (optional)
    pub key: Option<String>,
    /// Message value (payload)
    pub value: Value,
}

/// Message Queue client trait
#[async_trait]
pub trait MqClient: Send + Sync {
    /// Publish a message to a topic
    async fn publish(&self, topic: String, key: Option<String>, value: Value) -> Result<()>;

    /// Get driver type
    fn driver(&self) -> MqDriver;
}

/// Published message record for testing
#[derive(Debug, Clone)]
pub struct PublishedMessage {
    pub topic: String,
    pub key: Option<String>,
    pub value: Value,
    pub timestamp: std::time::SystemTime,
}

/// Mock Message Queue client for testing
pub struct MockMqClient {
    name: String,
    driver: MqDriver,
    /// Published messages (for verification in tests)
    published_messages: Arc<Mutex<Vec<PublishedMessage>>>,
}

impl MockMqClient {
    /// Create a new mock MQ client
    pub fn new() -> Self {
        Self {
            name: "mock_mq".to_string(),
            driver: MqDriver::Kafka,
            published_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create with custom driver
    pub fn with_driver(driver: MqDriver) -> Self {
        Self {
            name: "mock_mq".to_string(),
            driver,
            published_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create with custom name
    pub fn with_name(name: String) -> Self {
        Self {
            name,
            driver: MqDriver::Kafka,
            published_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all published messages (for testing)
    pub fn get_published_messages(&self) -> Vec<PublishedMessage> {
        self.published_messages.lock().unwrap().clone()
    }

    /// Clear published messages (for testing)
    pub fn clear_published_messages(&self) {
        self.published_messages.lock().unwrap().clear();
    }

    /// Get message count (for testing)
    pub fn message_count(&self) -> usize {
        self.published_messages.lock().unwrap().len()
    }
}

impl Default for MockMqClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MqClient for MockMqClient {
    async fn publish(&self, topic: String, key: Option<String>, value: Value) -> Result<()> {
        // Record the published message
        let message = PublishedMessage {
            topic,
            key,
            value,
            timestamp: std::time::SystemTime::now(),
        };

        self.published_messages.lock().unwrap().push(message);

        Ok(())
    }

    fn driver(&self) -> MqDriver {
        self.driver.clone()
    }
}

#[async_trait]
impl ServiceClient for MockMqClient {
    async fn call(&self, request: ServiceRequest) -> Result<ServiceResponse> {
        // Extract topic from operation (operation should be the topic name)
        let topic = request.operation.clone();

        // Extract key and value from params
        let key = request
            .params
            .get("key")
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            });

        let value = request
            .params
            .get("value")
            .cloned()
            .unwrap_or(Value::Object(HashMap::new()));

        // Publish the message
        self.publish(topic.clone(), key.clone(), value.clone())
            .await?;

        // Return success response
        let mut metadata = HashMap::new();
        metadata.insert("topic".to_string(), Value::String(topic));
        if let Some(k) = key {
            metadata.insert("key".to_string(), Value::String(k));
        }
        metadata.insert("driver".to_string(), Value::String(format!("{:?}", self.driver)));
        metadata.insert("status".to_string(), Value::String("published".to_string()));

        Ok(ServiceResponse {
            data: Value::Object(HashMap::new()),
            status: "published".to_string(),
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_mq_client_new() {
        let client = MockMqClient::new();
        assert_eq!(client.name, "mock_mq");
        assert_eq!(client.driver(), MqDriver::Kafka);
        assert_eq!(client.message_count(), 0);
    }

    #[tokio::test]
    async fn test_mock_mq_client_with_driver() {
        let client = MockMqClient::with_driver(MqDriver::RabbitMQ);
        assert_eq!(client.driver(), MqDriver::RabbitMQ);
    }

    #[tokio::test]
    async fn test_mock_mq_client_publish() {
        let client = MockMqClient::new();

        let mut message_value = HashMap::new();
        message_value.insert("user_id".to_string(), Value::String("user123".to_string()));
        message_value.insert("event_type".to_string(), Value::String("login".to_string()));

        client
            .publish(
                "risk_decisions".to_string(),
                Some("user123".to_string()),
                Value::Object(message_value),
            )
            .await
            .unwrap();

        assert_eq!(client.message_count(), 1);

        let messages = client.get_published_messages();
        assert_eq!(messages[0].topic, "risk_decisions");
        assert_eq!(messages[0].key, Some("user123".to_string()));
    }

    #[tokio::test]
    async fn test_mock_mq_client_publish_without_key() {
        let client = MockMqClient::new();

        let message_value = Value::String("test message".to_string());

        client
            .publish("test_topic".to_string(), None, message_value)
            .await
            .unwrap();

        assert_eq!(client.message_count(), 1);

        let messages = client.get_published_messages();
        assert_eq!(messages[0].topic, "test_topic");
        assert_eq!(messages[0].key, None);
    }

    #[tokio::test]
    async fn test_mock_mq_client_multiple_messages() {
        let client = MockMqClient::new();

        for i in 0..5 {
            client
                .publish(
                    "test_topic".to_string(),
                    Some(format!("key{}", i)),
                    Value::Number(i as f64),
                )
                .await
                .unwrap();
        }

        assert_eq!(client.message_count(), 5);

        let messages = client.get_published_messages();
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].key, Some("key0".to_string()));
        assert_eq!(messages[4].key, Some("key4".to_string()));
    }

    #[tokio::test]
    async fn test_mock_mq_client_clear() {
        let client = MockMqClient::new();

        client
            .publish(
                "test_topic".to_string(),
                None,
                Value::String("test".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(client.message_count(), 1);

        client.clear_published_messages();
        assert_eq!(client.message_count(), 0);
    }

    #[tokio::test]
    async fn test_service_client_implementation() {
        let client = MockMqClient::new();

        let mut params = HashMap::new();
        params.insert("key".to_string(), Value::String("user123".to_string()));

        let mut value_data = HashMap::new();
        value_data.insert("event_type".to_string(), Value::String("login".to_string()));
        params.insert("value".to_string(), Value::Object(value_data));

        let request = ServiceRequest {
            service: "event_bus".to_string(),
            operation: "risk_decisions".to_string(),
            params,
        };

        let result = client.call(request).await.unwrap();

        // Verify response metadata
        assert_eq!(
            result.metadata.get("topic"),
            Some(&Value::String("risk_decisions".to_string()))
        );
        assert_eq!(
            result.metadata.get("key"),
            Some(&Value::String("user123".to_string()))
        );
        assert_eq!(
            result.metadata.get("status"),
            Some(&Value::String("published".to_string()))
        );

        // Verify message was published
        assert_eq!(client.message_count(), 1);
        let messages = client.get_published_messages();
        assert_eq!(messages[0].topic, "risk_decisions");
    }

    #[tokio::test]
    async fn test_mock_mq_client_with_name() {
        let client = MockMqClient::with_name("custom_mq".to_string());
        assert_eq!(client.name, "custom_mq");
    }

    #[tokio::test]
    async fn test_mock_mq_client_default() {
        let client = MockMqClient::default();
        assert_eq!(client.name, "mock_mq");
        assert_eq!(client.driver(), MqDriver::Kafka);
    }

    #[tokio::test]
    async fn test_mq_driver_equality() {
        assert_eq!(MqDriver::Kafka, MqDriver::Kafka);
        assert_eq!(MqDriver::RabbitMQ, MqDriver::RabbitMQ);
        assert_ne!(MqDriver::Kafka, MqDriver::RabbitMQ);
    }
}
