//! Metrics collection and reporting

use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Counter metric
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    value: Arc<RwLock<u64>>,
    enabled: bool,
    labels: HashMap<String, String>,
}

impl Counter {
    /// Create a new counter
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: Arc::new(RwLock::new(0)),
            enabled: true,
            labels: HashMap::new(),
        }
    }

    /// Create with labels
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Get the counter name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the labels
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }

    /// Increment the counter
    pub fn inc(&self) {
        self.add(1);
    }

    /// Add a value to the counter
    pub fn add(&self, value: u64) {
        if self.enabled {
            let mut count = self.value.write().unwrap();
            *count = count.saturating_add(value);
        }
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

/// Fixed nonnegative observation boundaries; execution durations are in seconds.
const BOUNDS: [f64; 22] = [
    0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5,
    5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0,
];
const MAX_SERIES: usize = 128;
const MAX_NAME_BYTES: usize = 128;

#[derive(Clone)]
struct HistogramState {
    buckets: [u64; BOUNDS.len() + 1],
    count: u64,
    sum: f64,
    min: f64,
    max: f64,
}
impl Default for HistogramState {
    fn default() -> Self {
        Self {
            buckets: [0; BOUNDS.len() + 1],
            count: 0,
            sum: 0.0,
            min: 0.0,
            max: 0.0,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BucketSnapshot {
    /// None represents the final +infinity bucket.
    pub upper_bound: Option<f64>,
    pub count: u64,
}
#[derive(Debug, Serialize)]
pub struct HistogramSnapshot {
    pub name: String,
    pub count: u64,
    pub sum: f64,
    /// Cumulative counts, including the final unbounded bucket.
    pub buckets: Vec<BucketSnapshot>,
}

/// Fixed-space histogram. Finite nonnegative samples are aggregated, never retained.
#[derive(Clone)]
pub struct Histogram {
    name: String,
    state: Arc<RwLock<HistogramState>>,
    labels: HashMap<String, String>,
    enabled: bool,
}
impl std::fmt::Debug for Histogram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Histogram")
            .field("snapshot", &self.snapshot())
            .finish()
    }
}
impl Histogram {
    pub fn new(name: String) -> Self {
        Self {
            name,
            state: Arc::new(RwLock::new(HistogramState::default())),
            labels: HashMap::new(),
            enabled: true,
        }
    }
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }
    /// Ignore negative/nonfinite samples and samples that would overflow the sum.
    pub fn observe(&self, value: f64) {
        if !self.enabled || !value.is_finite() || value < 0.0 {
            return;
        }
        let mut state = self.state.write().unwrap();
        if state.count == u64::MAX || !(state.sum + value).is_finite() {
            return;
        }
        let index = BOUNDS.partition_point(|bound| *bound < value);
        state.buckets[index] += 1;
        if state.count == 0 {
            state.min = value;
            state.max = value;
        }
        state.min = state.min.min(value);
        state.max = state.max.max(value);
        state.count += 1;
        state.sum += value;
    }
    pub fn observe_duration(&self, duration: Duration) {
        self.observe(duration.as_secs_f64());
    }
    pub fn count(&self) -> usize {
        usize::try_from(self.state.read().unwrap().count).unwrap_or(usize::MAX)
    }
    pub fn sum(&self) -> f64 {
        self.state.read().unwrap().sum
    }
    pub fn avg(&self) -> f64 {
        let state = self.state.read().unwrap();
        if state.count == 0 {
            0.0
        } else {
            state.sum / state.count as f64
        }
    }
    /// Approximate percentile by linear interpolation within a bucket. Values in
    /// the final +infinity bucket use its lower bound. p=0/100 return exact extrema.
    /// NaN/infinite percentiles return NaN; finite percentiles are clamped to 0..100.
    pub fn percentile(&self, p: f64) -> f64 {
        if !p.is_finite() {
            return f64::NAN;
        }
        let state = self.state.read().unwrap();
        if state.count == 0 {
            return 0.0;
        }
        if p <= 0.0 {
            return state.min;
        }
        if p >= 100.0 {
            return state.max;
        }
        let rank = p / 100.0 * state.count as f64;
        let mut before = 0u64;
        for (i, count) in state.buckets.iter().copied().enumerate() {
            if count > 0 && rank <= (before + count) as f64 {
                let lower = if i == 0 { 0.0 } else { BOUNDS[i - 1] }.max(state.min);
                let Some(upper) = BOUNDS.get(i) else {
                    return lower;
                };
                return lower
                    + (upper.min(state.max) - lower) * ((rank - before as f64) / count as f64);
            }
            before += count;
        }
        state.max
    }
    pub fn snapshot(&self) -> HistogramSnapshot {
        let state = self.state.read().unwrap();
        let mut cumulative = 0;
        HistogramSnapshot {
            name: self.name.clone(),
            count: state.count,
            sum: state.sum,
            buckets: state
                .buckets
                .iter()
                .enumerate()
                .map(|(i, count)| {
                    cumulative += count;
                    BucketSnapshot {
                        upper_bound: BOUNDS.get(i).copied(),
                        count: cumulative,
                    }
                })
                .collect(),
        }
    }
    pub fn reset(&self) {
        *self.state.write().unwrap() = HistogramState::default();
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
#[derive(Serialize)]
pub struct CounterSnapshot {
    pub name: String,
    pub value: u64,
}
#[derive(Serialize)]
pub struct MetricsSnapshot {
    pub enabled: bool,
    pub rejected_registrations: u64,
    pub counters: Vec<CounterSnapshot>,
    pub histograms: Vec<HistogramSnapshot>,
}

pub struct MetricsCollector {
    enabled: bool,
    rejected_registrations: AtomicU64,
    noop_counter: Arc<Counter>,
    noop_histogram: Arc<Histogram>,
    counters: Arc<RwLock<HashMap<String, Arc<Counter>>>>,
    histograms: Arc<RwLock<HashMap<String, Arc<Histogram>>>>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self::with_enabled(true)
    }
    pub fn with_enabled(enabled: bool) -> Self {
        let mut noop_counter = Counter::new(String::new());
        noop_counter.enabled = false;
        let mut noop_histogram = Histogram::new(String::new());
        noop_histogram.enabled = false;
        Self {
            enabled,
            rejected_registrations: AtomicU64::new(0),
            noop_counter: Arc::new(noop_counter),
            noop_histogram: Arc::new(noop_histogram),
            counters: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
    fn reject_registration(&self) {
        let _ =
            self.rejected_registrations
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                    Some(n.saturating_add(1))
                });
    }
    /// Bounded JSON-ready export; each histogram is internally consistent.
    /// Separate metric series may reflect different instants during concurrent writes.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let mut counters: Vec<_> = self
            .counters
            .read()
            .unwrap()
            .values()
            .map(|c| CounterSnapshot {
                name: c.name().into(),
                value: c.get(),
            })
            .collect();
        let mut histograms: Vec<_> = self
            .histograms
            .read()
            .unwrap()
            .values()
            .map(|h| h.snapshot())
            .collect();
        counters.sort_by(|a, b| a.name.cmp(&b.name));
        histograms.sort_by(|a, b| a.name.cmp(&b.name));
        MetricsSnapshot {
            enabled: self.enabled,
            rejected_registrations: self.rejected_registrations.load(Ordering::Relaxed),
            counters,
            histograms,
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
        self.rejected_registrations.store(0, Ordering::Relaxed);
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
        if !self.enabled {
            return self.noop_counter.clone();
        }
        if name.is_empty() || name.len() > MAX_NAME_BYTES {
            self.reject_registration();
            return self.noop_counter.clone();
        }
        if let Some(counter) = self.counters.read().unwrap().get(name) {
            return counter.clone();
        }
        let mut counters = self.counters.write().unwrap();
        if let Some(counter) = counters.get(name) {
            return counter.clone();
        }
        if counters.len() >= MAX_SERIES {
            self.reject_registration();
            return self.noop_counter.clone();
        }
        let counter = Arc::new(Counter::new(name.into()));
        counters.insert(name.into(), counter.clone());
        counter
    }
    fn histogram(&self, name: &str) -> Arc<Histogram> {
        if !self.enabled {
            return self.noop_histogram.clone();
        }
        if name.is_empty() || name.len() > MAX_NAME_BYTES {
            self.reject_registration();
            return self.noop_histogram.clone();
        }
        if let Some(histogram) = self.histograms.read().unwrap().get(name) {
            return histogram.clone();
        }
        let mut histograms = self.histograms.write().unwrap();
        if let Some(histogram) = histograms.get(name) {
            return histogram.clone();
        }
        if histograms.len() >= MAX_SERIES {
            self.reject_registration();
            return self.noop_histogram.clone();
        }
        let histogram = Arc::new(Histogram::new(name.into()));
        histograms.insert(name.into(), histogram.clone());
        histogram
    }

    fn record_execution_time(&self, operation: &str, duration: Duration) {
        if !self.enabled {
            return;
        }
        if operation.len() > MAX_NAME_BYTES - "_duration".len() {
            self.reject_registration();
            return;
        }
        let hist = self.histogram(&format!("{}_duration", operation));
        hist.observe_duration(duration);
    }

    fn record_error(&self, error_type: &str) {
        if !self.enabled {
            return;
        }
        if error_type.len() > MAX_NAME_BYTES - "errors_".len() {
            self.reject_registration();
            return;
        }
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
    #[test]
    fn concurrent_large_volume_keeps_fixed_storage_and_consistent_exports() {
        let histogram = Arc::new(Histogram::new("load".into()));
        let initial_size = std::mem::size_of_val(&*histogram.state.read().unwrap());
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let histogram = histogram.clone();
                std::thread::spawn(move || {
                    for i in 0..15_000 {
                        histogram.observe(0.001);
                        if i % 1000 == 0 {
                            let snapshot = histogram.snapshot();
                            assert_eq!(snapshot.buckets.last().unwrap().count, snapshot.count);
                            assert!(snapshot
                                .buckets
                                .windows(2)
                                .all(|b| b[0].count <= b[1].count));
                        }
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let snapshot = histogram.snapshot();
        assert_eq!(snapshot.count, 120_000);
        assert!((snapshot.sum - 120.0).abs() < 1e-6);
        assert_eq!(snapshot.buckets.len(), 23);
        assert_eq!(
            std::mem::size_of_val(&*histogram.state.read().unwrap()),
            initial_size
        );
        assert_eq!(snapshot.buckets[3].count, 120_000);
        assert_eq!(snapshot.buckets[2].count, 0);
        assert!(serde_json::to_vec(&snapshot).unwrap().len() < 2048);
        histogram.reset();
        assert_eq!(histogram.count(), 0);
        assert_eq!(histogram.sum(), 0.0);
        assert!(histogram.snapshot().buckets.iter().all(|b| b.count == 0));
    }

    #[test]
    fn histogram_handles_boundaries_invalid_samples_and_percentiles() {
        let histogram = Histogram::new("edges".into());
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            histogram.observe(value);
        }
        assert_eq!(histogram.count(), 0);
        assert_eq!(histogram.percentile(95.0), 0.0);
        for value in [0.0, 0.001, 0.002, 2000.0] {
            histogram.observe(value);
        }
        let snapshot = histogram.snapshot();
        assert_eq!(snapshot.buckets[0].count, 1);
        assert_eq!(snapshot.buckets[3].count, 2);
        assert_eq!(snapshot.buckets[4].count, 3);
        assert_eq!(snapshot.buckets.last().unwrap().count, 4);
        assert_eq!(snapshot.buckets.last().unwrap().upper_bound, None);
        assert_eq!(histogram.percentile(-1.0), 0.0);
        assert_eq!(histogram.percentile(101.0), 2000.0);
        assert_eq!(histogram.percentile(90.0), 1000.0);
        assert!(histogram.percentile(f64::NAN).is_nan());
        assert!((0.001..=0.0025).contains(&histogram.percentile(60.0)));
    }

    #[test]
    fn disabled_metrics_are_noops_even_through_retained_handles() {
        let collector = MetricsCollector::with_enabled(false);
        let counter = collector.counter("requests");
        let histogram = collector.histogram("latency");
        for _ in 0..1000 {
            counter.inc();
            histogram.observe(1.0);
            collector.record_error("test");
            collector.record_execution_time("test", Duration::from_secs(1));
        }
        assert_eq!(counter.get(), 0);
        assert_eq!(histogram.count(), 0);
        let snapshot = collector.snapshot();
        assert!(!snapshot.enabled);
        assert!(snapshot.histograms.is_empty() && snapshot.counters.is_empty());
        assert_eq!(snapshot.rejected_registrations, 0);
    }

    #[test]
    fn dynamic_names_are_bounded_without_disrupting_existing_series() {
        let collector = MetricsCollector::new();
        for i in 0..10_000 {
            collector.counter(&format!("counter_{i}")).inc();
            collector.histogram(&format!("histogram_{i}")).observe(0.1);
        }
        assert_eq!(collector.counter_names().len(), MAX_SERIES);
        assert_eq!(collector.histogram_names().len(), MAX_SERIES);
        assert_eq!(
            collector.snapshot().rejected_registrations,
            2 * (10_000 - MAX_SERIES) as u64
        );
        let existing = collector.counter("counter_0");
        existing.inc();
        assert_eq!(existing.get(), 2);
        let oversized = "x".repeat(MAX_NAME_BYTES + 1);
        assert_eq!(collector.counter(&oversized).name(), "");
        assert_eq!(collector.histogram(&oversized).name(), "");
        collector.record_error(&oversized);
        collector.record_execution_time(&oversized, Duration::ZERO);
        assert_eq!(
            collector.snapshot().rejected_registrations,
            2 * (10_000 - MAX_SERIES) as u64 + 4
        );
        collector.reset_all();
        assert_eq!(collector.snapshot().rejected_registrations, 0);
    }
}
