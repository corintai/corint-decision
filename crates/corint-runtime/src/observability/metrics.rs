//! Metrics collection and reporting

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Counter metric
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    value: Arc<RwLock<u64>>,
    labels: HashMap<String, String>,
}

impl Counter {
    /// Create a new counter
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: Arc::new(RwLock::new(0)),
            labels: HashMap::new(),
        }
    }

    /// Create with labels
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Increment the counter
    pub fn inc(&self) {
        self.add(1);
    }

    /// Add a value to the counter
    pub fn add(&self, value: u64) {
        *self.value.write().unwrap() += value;
    }

    /// Get the current value
    pub fn get(&self) -> u64 {
        *self.value.read().unwrap()
    }

    /// Reset the counter
    pub fn reset(&self) {
        *self.value.write().unwrap() = 0;
    }
}

/// Histogram metric for tracking distributions
#[derive(Debug, Clone)]
pub struct Histogram {
    name: String,
    values: Arc<RwLock<Vec<f64>>>,
    labels: HashMap<String, String>,
}

impl Histogram {
    /// Create a new histogram
    pub fn new(name: String) -> Self {
        Self {
            name,
            values: Arc::new(RwLock::new(Vec::new())),
            labels: HashMap::new(),
        }
    }

    /// Create with labels
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Observe a value
    pub fn observe(&self, value: f64) {
        self.values.write().unwrap().push(value);
    }

    /// Observe a duration
    pub fn observe_duration(&self, duration: Duration) {
        self.observe(duration.as_secs_f64());
    }

    /// Get count of observations
    pub fn count(&self) -> usize {
        self.values.read().unwrap().len()
    }

    /// Get sum of all values
    pub fn sum(&self) -> f64 {
        self.values.read().unwrap().iter().sum()
    }

    /// Get average value
    pub fn avg(&self) -> f64 {
        let values = self.values.read().unwrap();
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        }
    }

    /// Get percentile (0-100)
    pub fn percentile(&self, p: f64) -> f64 {
        let mut values = self.values.read().unwrap().clone();
        if values.is_empty() {
            return 0.0;
        }

        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let index = ((p / 100.0) * (values.len() - 1) as f64).round() as usize;
        values[index]
    }

    /// Reset the histogram
    pub fn reset(&self) {
        self.values.write().unwrap().clear();
    }
}

/// Metrics trait
pub trait Metrics: Send + Sync {
    /// Get a counter
    fn counter(&self, name: &str) -> Arc<Counter>;

    /// Get a histogram
    fn histogram(&self, name: &str) -> Arc<Histogram>;

    /// Record execution time
    fn record_execution_time(&self, operation: &str, duration: Duration);

    /// Record error
    fn record_error(&self, error_type: &str);
}

/// Metrics collector
pub struct MetricsCollector {
    counters: Arc<RwLock<HashMap<String, Arc<Counter>>>>,
    histograms: Arc<RwLock<HashMap<String, Arc<Histogram>>>>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get all counter names
    pub fn counter_names(&self) -> Vec<String> {
        self.counters.read().unwrap().keys().cloned().collect()
    }

    /// Get all histogram names
    pub fn histogram_names(&self) -> Vec<String> {
        self.histograms.read().unwrap().keys().cloned().collect()
    }

    /// Reset all metrics
    pub fn reset_all(&self) {
        for counter in self.counters.read().unwrap().values() {
            counter.reset();
        }
        for histogram in self.histograms.read().unwrap().values() {
            histogram.reset();
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics for MetricsCollector {
    fn counter(&self, name: &str) -> Arc<Counter> {
        self.counters
            .write()
            .unwrap()
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Counter::new(name.to_string())))
            .clone()
    }

    fn histogram(&self, name: &str) -> Arc<Histogram> {
        self.histograms
            .write()
            .unwrap()
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Histogram::new(name.to_string())))
            .clone()
    }

    fn record_execution_time(&self, operation: &str, duration: Duration) {
        let hist = self.histogram(&format!("{}_duration", operation));
        hist.observe_duration(duration);
    }

    fn record_error(&self, error_type: &str) {
        let counter = self.counter(&format!("errors_{}", error_type));
        counter.inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let counter = Counter::new("test_counter".to_string());

        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.add(5);
        assert_eq!(counter.get(), 6);

        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_histogram() {
        let histogram = Histogram::new("test_histogram".to_string());

        histogram.observe(10.0);
        histogram.observe(20.0);
        histogram.observe(30.0);

        assert_eq!(histogram.count(), 3);
        assert_eq!(histogram.sum(), 60.0);
        assert_eq!(histogram.avg(), 20.0);
    }

    #[test]
    fn test_histogram_percentile() {
        let histogram = Histogram::new("test_histogram".to_string());

        for i in 1..=100 {
            histogram.observe(i as f64);
        }

        let p50 = histogram.percentile(50.0);
        // The 50th percentile should be around 50.5 (middle of 1-100)
        assert!((p50 - 50.5).abs() < 2.0);

        let p95 = histogram.percentile(95.0);
        // The 95th percentile should be around 94-95
        assert!((p95 - 94.0).abs() < 2.0);
    }

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new();

        let counter = collector.counter("requests");
        counter.inc();
        counter.inc();

        assert_eq!(counter.get(), 2);

        let histogram = collector.histogram("latency");
        histogram.observe(100.0);
        histogram.observe(200.0);

        assert_eq!(histogram.count(), 2);
        assert_eq!(histogram.avg(), 150.0);
    }

    #[test]
    fn test_record_execution_time() {
        let collector = MetricsCollector::new();

        let duration = Duration::from_millis(100);
        collector.record_execution_time("test_op", duration);

        let histogram = collector.histogram("test_op_duration");
        assert_eq!(histogram.count(), 1);
    }

    #[test]
    fn test_record_error() {
        let collector = MetricsCollector::new();

        collector.record_error("validation");
        collector.record_error("validation");

        let counter = collector.counter("errors_validation");
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn test_reset_all() {
        let collector = MetricsCollector::new();

        collector.counter("c1").inc();
        collector.histogram("h1").observe(10.0);

        collector.reset_all();

        assert_eq!(collector.counter("c1").get(), 0);
        assert_eq!(collector.histogram("h1").count(), 0);
    }
}
