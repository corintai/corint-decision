//! Example demonstrating OpenTelemetry metrics integration
//!
//! This example shows how to:
//! - Initialize OpenTelemetry with Prometheus exporter
//! - Record custom metrics using OpenTelemetry API
//! - Export metrics in Prometheus format
//!
//! Run with:
//! ```bash
//! cargo run --example opentelemetry_metrics --features otel
//! ```

use corint_runtime::observability::otel::{init_opentelemetry, meter, OtelConfig};
use opentelemetry::KeyValue;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 OpenTelemetry Metrics Example");
    println!("================================\n");

    // Initialize OpenTelemetry with Prometheus exporter
    let config = OtelConfig::new("example-service")
        .with_version("0.1.0")
        .with_metrics(true)
        .with_tracing(false); // Disable tracing for this example

    let otel_ctx = init_opentelemetry(config)?;
    println!("✓ OpenTelemetry initialized with Prometheus exporter\n");

    // Get a meter for recording metrics
    let meter = meter("example-metrics");

    // Create a counter
    let request_counter = meter
        .u64_counter("http_requests_total")
        .with_description("Total HTTP requests")
        .with_unit("requests")
        .build();

    // Create a histogram for latency
    let latency_histogram = meter
        .f64_histogram("http_request_duration_seconds")
        .with_description("HTTP request duration in seconds")
        .with_unit("s")
        .build();

    // Create an up-down counter (gauge)
    let active_connections = meter
        .i64_up_down_counter("active_connections")
        .with_description("Number of active connections")
        .with_unit("connections")
        .build();

    println!("📊 Recording metrics...\n");

    // Simulate some traffic
    for i in 0..10 {
        // Record a request
        request_counter.add(
            1,
            &[
                KeyValue::new("method", "GET"),
                KeyValue::new("endpoint", "/api/data"),
                KeyValue::new("status", "200"),
            ],
        );

        // Record latency (simulate with random values)
        let latency = 0.1 + (i as f64 * 0.05);
        latency_histogram.record(
            latency,
            &[
                KeyValue::new("method", "GET"),
                KeyValue::new("endpoint", "/api/data"),
            ],
        );

        // Update active connections
        active_connections.add(1, &[]);

        if i % 3 == 0 {
            active_connections.add(-1, &[]);
        }

        println!("  Request {}: latency={:.3}s", i + 1, latency);
    }

    println!("\n✓ Recorded 10 requests\n");

    // Get and display metrics
    println!("📈 Prometheus Metrics Output:");
    println!("============================");
    let metrics_text = otel_ctx.metrics()?;
    println!("{}", metrics_text);

    // Cleanup
    otel_ctx.shutdown()?;
    println!("\n✓ OpenTelemetry shutdown complete");

    Ok(())
}
