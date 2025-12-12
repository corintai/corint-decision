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

pub use metrics::{Metrics, MetricsCollector, Counter, Histogram};
pub use tracing::{Tracer, Span, SpanContext};

#[cfg(feature = "otel")]
pub use otel::{OtelConfig, OtelContext, init_opentelemetry, meter, tracer};
