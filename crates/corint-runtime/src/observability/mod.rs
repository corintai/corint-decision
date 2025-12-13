//! Observability module
//!
//! Provides metrics, tracing, and monitoring capabilities.
//!
//! This module includes:
//! - Custom metrics collection (Counter, Histogram)
//! - Custom distributed tracing
//! - OpenTelemetry integration (with `otel` feature)

pub mod metrics;
pub mod tracing;

#[cfg(feature = "otel")]
pub mod otel;

pub use metrics::{Counter, Histogram, Metrics, MetricsCollector};
pub use tracing::{Span, SpanContext, Tracer};

#[cfg(feature = "otel")]
pub use otel::{init_opentelemetry, meter, tracer, OtelConfig, OtelContext};
