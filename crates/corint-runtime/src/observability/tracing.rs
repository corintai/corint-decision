//! Distributed tracing support

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Span context for distributed tracing
#[derive(Debug, Clone)]
pub struct SpanContext {
    /// Trace ID
    pub trace_id: String,

    /// Span ID
    pub span_id: String,

    /// Parent span ID
    pub parent_id: Option<String>,
}

impl SpanContext {
    /// Create a new span context
    pub fn new(trace_id: String, span_id: String) -> Self {
        Self {
            trace_id,
            span_id,
            parent_id: None,
        }
    }

    /// Create with parent
    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }
}

/// Trace span
#[derive(Debug, Clone)]
pub struct Span {
    /// Span context
    pub context: SpanContext,

    /// Operation name
    pub operation: String,

    /// Start time
    pub start_time: Instant,

    /// End time
    pub end_time: Option<Instant>,

    /// Span attributes/tags
    pub attributes: Arc<RwLock<HashMap<String, String>>>,

    /// Span events
    pub events: Arc<RwLock<Vec<SpanEvent>>>,
}

/// Span event
#[derive(Debug, Clone)]
pub struct SpanEvent {
    /// Event name
    pub name: String,

    /// Event timestamp
    pub timestamp: Instant,

    /// Event attributes
    pub attributes: HashMap<String, String>,
}

impl Span {
    /// Create a new span
    pub fn new(context: SpanContext, operation: String) -> Self {
        Self {
            context,
            operation,
            start_time: Instant::now(),
            end_time: None,
            attributes: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Set an attribute
    pub fn set_attribute(&self, key: String, value: String) {
        self.attributes.write().unwrap().insert(key, value);
    }

    /// Add an event
    pub fn add_event(&self, name: String) {
        let event = SpanEvent {
            name,
            timestamp: Instant::now(),
            attributes: HashMap::new(),
        };
        self.events.write().unwrap().push(event);
    }

    /// Add an event with attributes
    pub fn add_event_with_attributes(&self, name: String, attributes: HashMap<String, String>) {
        let event = SpanEvent {
            name,
            timestamp: Instant::now(),
            attributes,
        };
        self.events.write().unwrap().push(event);
    }

    /// End the span
    pub fn end(&mut self) {
        self.end_time = Some(Instant::now());
    }

    /// Get span duration
    pub fn duration(&self) -> Option<Duration> {
        self.end_time.map(|end| end.duration_since(self.start_time))
    }

    /// Check if span is finished
    pub fn is_finished(&self) -> bool {
        self.end_time.is_some()
    }
}

/// Tracer for creating and managing spans
pub trait Tracer: Send + Sync {
    /// Start a new span
    fn start_span(&self, operation: String) -> Span;

    /// Start a child span
    fn start_child_span(&self, parent: &Span, operation: String) -> Span;

    /// Record a span
    fn record_span(&self, span: Span);
}

/// In-memory tracer for testing and development
pub struct InMemoryTracer {
    trace_id_counter: Arc<RwLock<u64>>,
    span_id_counter: Arc<RwLock<u64>>,
    spans: Arc<RwLock<Vec<Span>>>,
}

impl InMemoryTracer {
    /// Create a new in-memory tracer
    pub fn new() -> Self {
        Self {
            trace_id_counter: Arc::new(RwLock::new(0)),
            span_id_counter: Arc::new(RwLock::new(0)),
            spans: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get all recorded spans
    pub fn get_spans(&self) -> Vec<Span> {
        self.spans.read().unwrap().clone()
    }

    /// Clear all spans
    pub fn clear(&self) {
        self.spans.write().unwrap().clear();
    }

    /// Generate trace ID
    fn next_trace_id(&self) -> String {
        let mut counter = self.trace_id_counter.write().unwrap();
        *counter += 1;
        format!("trace-{:016x}", *counter)
    }

    /// Generate span ID
    fn next_span_id(&self) -> String {
        let mut counter = self.span_id_counter.write().unwrap();
        *counter += 1;
        format!("span-{:016x}", *counter)
    }
}

impl Default for InMemoryTracer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tracer for InMemoryTracer {
    fn start_span(&self, operation: String) -> Span {
        let context = SpanContext::new(self.next_trace_id(), self.next_span_id());
        Span::new(context, operation)
    }

    fn start_child_span(&self, parent: &Span, operation: String) -> Span {
        let context = SpanContext::new(parent.context.trace_id.clone(), self.next_span_id())
            .with_parent(parent.context.span_id.clone());

        Span::new(context, operation)
    }

    fn record_span(&self, span: Span) {
        self.spans.write().unwrap().push(span);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_context() {
        let context = SpanContext::new("trace-1".to_string(), "span-1".to_string())
            .with_parent("span-0".to_string());

        assert_eq!(context.trace_id, "trace-1");
        assert_eq!(context.span_id, "span-1");
        assert_eq!(context.parent_id, Some("span-0".to_string()));
    }

    #[test]
    fn test_span_attributes() {
        let context = SpanContext::new("trace-1".to_string(), "span-1".to_string());
        let span = Span::new(context, "test_operation".to_string());

        span.set_attribute("key1".to_string(), "value1".to_string());
        span.set_attribute("key2".to_string(), "value2".to_string());

        let attributes = span.attributes.read().unwrap();
        assert_eq!(attributes.len(), 2);
        assert_eq!(attributes.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_span_events() {
        let context = SpanContext::new("trace-1".to_string(), "span-1".to_string());
        let span = Span::new(context, "test_operation".to_string());

        span.add_event("event1".to_string());
        span.add_event("event2".to_string());

        let events = span.events.read().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].name, "event1");
    }

    #[test]
    fn test_span_duration() {
        let context = SpanContext::new("trace-1".to_string(), "span-1".to_string());
        let mut span = Span::new(context, "test_operation".to_string());

        assert!(!span.is_finished());
        assert!(span.duration().is_none());

        std::thread::sleep(Duration::from_millis(10));
        span.end();

        assert!(span.is_finished());
        assert!(span.duration().is_some());
        assert!(span.duration().unwrap().as_millis() >= 10);
    }

    #[test]
    fn test_tracer() {
        let tracer = InMemoryTracer::new();

        let mut span = tracer.start_span("parent_operation".to_string());
        span.set_attribute("attr1".to_string(), "value1".to_string());
        span.end();

        tracer.record_span(span.clone());

        let spans = tracer.get_spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].operation, "parent_operation");
    }

    #[test]
    fn test_child_span() {
        let tracer = InMemoryTracer::new();

        let parent = tracer.start_span("parent_operation".to_string());
        let child = tracer.start_child_span(&parent, "child_operation".to_string());

        assert_eq!(child.context.trace_id, parent.context.trace_id);
        assert_eq!(
            child.context.parent_id,
            Some(parent.context.span_id.clone())
        );
    }

    #[test]
    fn test_tracer_clear() {
        let tracer = InMemoryTracer::new();

        let mut span = tracer.start_span("test".to_string());
        span.end();
        tracer.record_span(span);

        assert_eq!(tracer.get_spans().len(), 1);

        tracer.clear();
        assert_eq!(tracer.get_spans().len(), 0);
    }
}
