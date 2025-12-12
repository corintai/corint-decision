//! Feature extractor
//!
//! Extracts statistical features from historical data.

use corint_core::ir::{FeatureType, TimeWindow};
use corint_core::Value;
use crate::error::{RuntimeError, Result};
use crate::storage::{Event, EventFilter, Storage, TimeRange};
use std::collections::HashSet;
use std::sync::Arc;

/// Feature extractor for computing statistical features
pub struct FeatureExtractor {
    storage: Arc<dyn Storage>,
}

impl FeatureExtractor {
    /// Create a new feature extractor with storage backend
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// Extract a feature value asynchronously
    pub async fn extract(
        &self,
        feature_type: &FeatureType,
        field: &[String],
        time_window: &TimeWindow,
        filter: Option<EventFilter>,
    ) -> Result<Value> {
        let time_range = self.calculate_time_range(time_window);
        let events = self.storage.query_events(time_range, filter).await?;

        match feature_type {
            FeatureType::Count => self.compute_count(&events),
            FeatureType::CountDistinct => self.compute_count_distinct(&events, field),
            FeatureType::Sum => self.compute_sum(&events, field),
            FeatureType::Avg => self.compute_avg(&events, field),
            FeatureType::Min => self.compute_min(&events, field),
            FeatureType::Max => self.compute_max(&events, field),
            FeatureType::Percentile { p } => self.compute_percentile(&events, field, *p),
            FeatureType::StdDev => self.compute_stddev(&events, field),
            FeatureType::Variance => self.compute_variance(&events, field),
        }
    }

    /// Calculate time range for a time window
    ///
    /// Returns (start_timestamp, end_timestamp) in seconds since epoch.
    pub fn calculate_time_range(&self, time_window: &TimeWindow) -> TimeRange {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        match time_window {
            TimeWindow::Last1Hour => (now - 3600, now),
            TimeWindow::Last24Hours => (now - 86400, now),
            TimeWindow::Last7Days => (now - 604800, now),
            TimeWindow::Last30Days => (now - 2592000, now),
            TimeWindow::Custom { seconds } => (now - (*seconds as i64), now),
        }
    }

    // Computation methods

    fn compute_count(&self, events: &[Event]) -> Result<Value> {
        Ok(Value::Number(events.len() as f64))
    }

    fn compute_count_distinct(&self, events: &[Event], field: &[String]) -> Result<Value> {
        let mut seen = HashSet::new();

        for event in events {
            if let Some(value) = self.get_field_value(&event.data, field) {
                seen.insert(format!("{:?}", value));
            }
        }

        Ok(Value::Number(seen.len() as f64))
    }

    fn compute_sum(&self, events: &[Event], field: &[String]) -> Result<Value> {
        let mut sum = 0.0;

        for event in events {
            if let Some(Value::Number(n)) = self.get_field_value(&event.data, field) {
                sum += n;
            }
        }

        Ok(Value::Number(sum))
    }

    fn compute_avg(&self, events: &[Event], field: &[String]) -> Result<Value> {
        if events.is_empty() {
            return Ok(Value::Number(0.0));
        }

        let mut sum = 0.0;
        let mut count = 0;

        for event in events {
            if let Some(Value::Number(n)) = self.get_field_value(&event.data, field) {
                sum += n;
                count += 1;
            }
        }

        if count == 0 {
            Ok(Value::Number(0.0))
        } else {
            Ok(Value::Number(sum / count as f64))
        }
    }

    fn compute_min(&self, events: &[Event], field: &[String]) -> Result<Value> {
        let mut min = f64::INFINITY;

        for event in events {
            if let Some(Value::Number(n)) = self.get_field_value(&event.data, field) {
                if n < min {
                    min = n;
                }
            }
        }

        Ok(Value::Number(if min.is_infinite() { 0.0 } else { min }))
    }

    fn compute_max(&self, events: &[Event], field: &[String]) -> Result<Value> {
        let mut max = f64::NEG_INFINITY;

        for event in events {
            if let Some(Value::Number(n)) = self.get_field_value(&event.data, field) {
                if n > max {
                    max = n;
                }
            }
        }

        Ok(Value::Number(if max.is_infinite() { 0.0 } else { max }))
    }

    fn compute_percentile(&self, events: &[Event], field: &[String], p: f64) -> Result<Value> {
        if !(0.0..=100.0).contains(&p) {
            return Err(RuntimeError::InvalidOperation(
                "Percentile must be between 0 and 100".to_string(),
            ));
        }

        let mut values: Vec<f64> = Vec::new();

        for event in events {
            if let Some(Value::Number(n)) = self.get_field_value(&event.data, field) {
                // Filter out NaN and Infinity values
                if n.is_finite() {
                    values.push(n);
                }
            }
        }

        if values.is_empty() {
            return Ok(Value::Number(0.0));
        }

        // Safe sort: NaN values have been filtered out above
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let index = (p / 100.0 * (values.len() - 1) as f64).round() as usize;
        Ok(Value::Number(values[index]))
    }

    fn compute_stddev(&self, events: &[Event], field: &[String]) -> Result<Value> {
        let variance = match self.compute_variance(events, field)? {
            Value::Number(v) => v,
            _ => return Ok(Value::Number(0.0)),
        };

        Ok(Value::Number(variance.sqrt()))
    }

    fn compute_variance(&self, events: &[Event], field: &[String]) -> Result<Value> {
        if events.is_empty() {
            return Ok(Value::Number(0.0));
        }

        let mut values: Vec<f64> = Vec::new();

        for event in events {
            if let Some(Value::Number(n)) = self.get_field_value(&event.data, field) {
                values.push(n);
            }
        }

        if values.is_empty() {
            return Ok(Value::Number(0.0));
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;

        Ok(Value::Number(variance))
    }

    /// Get field value from event data using path
    fn get_field_value(&self, data: &std::collections::HashMap<String, Value>, field: &[String]) -> Option<Value> {
        if field.is_empty() {
            return None;
        }

        let mut current = data.get(&field[0])?;

        for segment in &field[1..] {
            match current {
                Value::Object(map) => {
                    current = map.get(segment)?;
                }
                _ => return None,
            }
        }

        Some(current.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;
    use std::collections::HashMap;

    fn create_test_event(timestamp: i64, amount: f64) -> Event {
        let mut data = HashMap::new();
        data.insert("amount".to_string(), Value::Number(amount));

        Event { timestamp, data }
    }

    fn create_storage_with_events(events: Vec<Event>) -> Arc<dyn Storage> {
        let mut storage = InMemoryStorage::new();
        storage.add_events(events);
        Arc::new(storage)
    }

    #[tokio::test]
    async fn test_extract_count() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let events = vec![
            create_test_event(now - 300, 10.0),
            create_test_event(now - 200, 20.0),
            create_test_event(now - 100, 30.0),
        ];

        let storage = create_storage_with_events(events);
        let extractor = FeatureExtractor::new(storage);

        let result = extractor
            .extract(
                &FeatureType::Count,
                &[],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await
            .unwrap();

        assert_eq!(result, Value::Number(3.0));
    }

    #[tokio::test]
    async fn test_extract_sum() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let events = vec![
            create_test_event(now - 300, 10.0),
            create_test_event(now - 200, 20.0),
            create_test_event(now - 100, 30.0),
        ];

        let storage = create_storage_with_events(events);
        let extractor = FeatureExtractor::new(storage);

        let result = extractor
            .extract(
                &FeatureType::Sum,
                &["amount".to_string()],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await
            .unwrap();

        assert_eq!(result, Value::Number(60.0));
    }

    #[tokio::test]
    async fn test_extract_avg() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let events = vec![
            create_test_event(now - 300, 10.0),
            create_test_event(now - 200, 20.0),
            create_test_event(now - 100, 30.0),
        ];

        let storage = create_storage_with_events(events);
        let extractor = FeatureExtractor::new(storage);

        let result = extractor
            .extract(
                &FeatureType::Avg,
                &["amount".to_string()],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await
            .unwrap();

        assert_eq!(result, Value::Number(20.0));
    }

    #[tokio::test]
    async fn test_extract_min_max() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let events = vec![
            create_test_event(now - 300, 10.0),
            create_test_event(now - 200, 20.0),
            create_test_event(now - 100, 5.0),
        ];

        let storage = create_storage_with_events(events);
        let extractor = FeatureExtractor::new(storage);

        let min_result = extractor
            .extract(
                &FeatureType::Min,
                &["amount".to_string()],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await
            .unwrap();

        assert_eq!(min_result, Value::Number(5.0));

        let max_result = extractor
            .extract(
                &FeatureType::Max,
                &["amount".to_string()],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await
            .unwrap();

        assert_eq!(max_result, Value::Number(20.0));
    }

    #[tokio::test]
    async fn test_extract_percentile() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let events = vec![
            create_test_event(now - 300, 10.0),
            create_test_event(now - 200, 20.0),
            create_test_event(now - 100, 30.0),
            create_test_event(now - 50, 40.0),
            create_test_event(now - 10, 50.0),
        ];

        let storage = create_storage_with_events(events);
        let extractor = FeatureExtractor::new(storage);

        let result = extractor
            .extract(
                &FeatureType::Percentile { p: 50.0 },
                &["amount".to_string()],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await
            .unwrap();

        assert_eq!(result, Value::Number(30.0));
    }

    #[tokio::test]
    async fn test_extract_percentile_invalid() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let events = vec![create_test_event(now - 300, 10.0)];

        let storage = create_storage_with_events(events);
        let extractor = FeatureExtractor::new(storage);

        let result = extractor
            .extract(
                &FeatureType::Percentile { p: 150.0 },
                &["amount".to_string()],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_extract_count_distinct() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut data1 = HashMap::new();
        data1.insert("user_id".to_string(), Value::String("user1".to_string()));

        let mut data2 = HashMap::new();
        data2.insert("user_id".to_string(), Value::String("user2".to_string()));

        let mut data3 = HashMap::new();
        data3.insert("user_id".to_string(), Value::String("user1".to_string()));

        let events = vec![
            Event { timestamp: now - 300, data: data1 },
            Event { timestamp: now - 200, data: data2 },
            Event { timestamp: now - 100, data: data3 },
        ];

        let storage = create_storage_with_events(events);
        let extractor = FeatureExtractor::new(storage);

        let result = extractor
            .extract(
                &FeatureType::CountDistinct,
                &["user_id".to_string()],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await
            .unwrap();

        assert_eq!(result, Value::Number(2.0));
    }

    #[tokio::test]
    async fn test_extract_variance_stddev() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let events = vec![
            create_test_event(now - 300, 10.0),
            create_test_event(now - 200, 20.0),
            create_test_event(now - 100, 30.0),
        ];

        let storage = create_storage_with_events(events);
        let extractor = FeatureExtractor::new(storage);

        let variance = extractor
            .extract(
                &FeatureType::Variance,
                &["amount".to_string()],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await
            .unwrap();

        // Variance of [10, 20, 30] = mean is 20, variance = ((10-20)^2 + (20-20)^2 + (30-20)^2) / 3 = 66.67
        if let Value::Number(v) = variance {
            assert!((v - 66.666666).abs() < 0.01);
        } else {
            panic!("Expected Number value");
        }

        let stddev = extractor
            .extract(
                &FeatureType::StdDev,
                &["amount".to_string()],
                &TimeWindow::Custom { seconds: 1000 },
                None,
            )
            .await
            .unwrap();

        // StdDev should be sqrt of variance
        if let Value::Number(s) = stddev {
            assert!((s - 8.165).abs() < 0.01);
        } else {
            panic!("Expected Number value");
        }
    }

    #[tokio::test]
    async fn test_calculate_time_range() {
        let storage = Arc::new(InMemoryStorage::new());
        let extractor = FeatureExtractor::new(storage);

        let (start, end) = extractor.calculate_time_range(&TimeWindow::Last1Hour);
        assert_eq!(end - start, 3600);

        let (start, end) = extractor.calculate_time_range(&TimeWindow::Last24Hours);
        assert_eq!(end - start, 86400);

        let (start, end) = extractor.calculate_time_range(&TimeWindow::Last7Days);
        assert_eq!(end - start, 604800);

        let (start, end) = extractor.calculate_time_range(&TimeWindow::Last30Days);
        assert_eq!(end - start, 2592000);

        let (start, end) = extractor.calculate_time_range(&TimeWindow::Custom { seconds: 1000 });
        assert_eq!(end - start, 1000);
    }
}
