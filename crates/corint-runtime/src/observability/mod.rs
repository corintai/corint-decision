//! Observability module
//!
//! Provides metrics, tracing, and monitoring capabilities.
//!
//! This module includes:
//! - Custom metrics collection (Counter, Histogram)
//! - Custom distributed tracing

pub mod metrics;
pub mod tracing;

pub use metrics::{Counter, Histogram, Metrics, MetricsCollector};
pub use tracing::{Span, SpanContext, Tracer};
