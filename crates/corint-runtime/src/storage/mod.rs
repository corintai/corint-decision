//! Storage layer for historical data queries
//!
//! Provides async interfaces for querying event history to support feature extraction.

use async_trait::async_trait;
use corint_core::Value;
use crate::error::Result;
use std::collections::HashMap;

/// Time range for queries (start, end) in seconds since epoch
pub type TimeRange = (i64, i64);

/// Event data structure
#[derive(Debug, Clone)]
pub struct Event {
    pub timestamp: i64,
    pub data: HashMap<String, Value>,
}

/// Async storage trait for querying historical events
#[async_trait]
pub trait Storage: Send + Sync {
    /// Query events within a time range
    async fn query_events(
        &self,
        time_range: TimeRange,
        filter: Option<EventFilter>,
    ) -> Result<Vec<Event>>;
}

/// Filter for event queries
#[derive(Debug, Clone)]
pub struct EventFilter {
    /// Event type to filter by
    pub event_type: Option<String>,

    /// Additional field filters
    pub field_filters: HashMap<String, Value>,
}

impl EventFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self {
            event_type: None,
            field_filters: HashMap::new(),
        }
    }

    /// Set event type filter
    pub fn with_event_type(mut self, event_type: String) -> Self {
        self.event_type = Some(event_type);
        self
    }

    /// Add field filter
    pub fn with_field_filter(mut self, field: String, value: Value) -> Self {
        self.field_filters.insert(field, value);
        self
    }

    /// Check if an event matches this filter
    pub fn matches(&self, event: &Event) -> bool {
        // Check event type
        if let Some(ref expected_type) = self.event_type {
            if let Some(Value::String(actual_type)) = event.data.get("event_type") {
                if actual_type != expected_type {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check field filters
        for (field, expected_value) in &self.field_filters {
            if let Some(actual_value) = event.data.get(field) {
                if actual_value != expected_value {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory storage implementation for testing
pub struct InMemoryStorage {
    events: Vec<Event>,
}

impl InMemoryStorage {
    /// Create a new in-memory storage
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    /// Add an event to storage
    pub fn add_event(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Add multiple events
    pub fn add_events(&mut self, events: Vec<Event>) {
        self.events.extend(events);
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Storage for InMemoryStorage {
    async fn query_events(
        &self,
        time_range: TimeRange,
        filter: Option<EventFilter>,
    ) -> Result<Vec<Event>> {
        let (start, end) = time_range;

        let mut results: Vec<Event> = self.events
            .iter()
            .filter(|event| event.timestamp >= start && event.timestamp < end)
            .cloned()
            .collect();

        if let Some(ref filter) = filter {
            results.retain(|event| filter.matches(event));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(timestamp: i64, event_type: &str, amount: f64) -> Event {
        let mut data = HashMap::new();
        data.insert("event_type".to_string(), Value::String(event_type.to_string()));
        data.insert("amount".to_string(), Value::Number(amount));

        Event { timestamp, data }
    }

    #[tokio::test]
    async fn test_in_memory_storage_query() {
        let mut storage = InMemoryStorage::new();

        storage.add_event(create_test_event(100, "transaction", 50.0));
        storage.add_event(create_test_event(200, "transaction", 100.0));
        storage.add_event(create_test_event(300, "login", 0.0));

        let results = storage.query_events((150, 350), None).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_event_filter() {
        let mut storage = InMemoryStorage::new();

        storage.add_event(create_test_event(100, "transaction", 50.0));
        storage.add_event(create_test_event(200, "transaction", 100.0));
        storage.add_event(create_test_event(300, "login", 0.0));

        let filter = EventFilter::new().with_event_type("transaction".to_string());
        let results = storage.query_events((0, 400), Some(filter)).await.unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_event_filter_with_field() {
        let mut storage = InMemoryStorage::new();

        storage.add_event(create_test_event(100, "transaction", 50.0));
        storage.add_event(create_test_event(200, "transaction", 100.0));

        let filter = EventFilter::new()
            .with_event_type("transaction".to_string())
            .with_field_filter("amount".to_string(), Value::Number(100.0));

        let results = storage.query_events((0, 400), Some(filter)).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].timestamp, 200);
    }
}
