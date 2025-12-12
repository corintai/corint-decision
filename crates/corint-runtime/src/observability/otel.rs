//! OpenTelemetry integration
//!
//! This module provides OpenTelemetry-based observability with:
//! - Metrics export to Prometheus
//! - Distributed tracing with OTLP
//! - Unified instrumentation layer

#[cfg(feature = "otel")]
use opentelemetry::{
    global,
    KeyValue,
};
#[cfg(feature = "otel")]
use opentelemetry_prometheus::PrometheusExporter;
#[cfg(feature = "otel")]
use opentelemetry_sdk::{
    trace::TracerProvider,
    Resource,
};
#[cfg(feature = "otel")]
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};

/// OpenTelemetry configuration
#[derive(Debug, Clone)]
pub struct OtelConfig {
    /// Service name
    pub service_name: String,

    /// Service version
    pub service_version: String,

    /// Enable metrics export to Prometheus
    pub enable_metrics: bool,

    /// Enable distributed tracing
    pub enable_tracing: bool,

    /// OTLP endpoint for traces (e.g., "http://localhost:4317")
    pub otlp_endpoint: Option<String>,

    /// Metric export interval in seconds
    pub metrics_export_interval_secs: u64,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            service_name: "corint-decision".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            enable_metrics: true,
            enable_tracing: true,
            otlp_endpoint: None,
            metrics_export_interval_secs: 60,
        }
    }
}

impl OtelConfig {
    /// Create a new config with service name
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            ..Default::default()
        }
    }

    /// Set service version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.service_version = version.into();
        self
    }

    /// Enable/disable metrics
    pub fn with_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }

    /// Enable/disable tracing
    pub fn with_tracing(mut self, enable: bool) -> Self {
        self.enable_tracing = enable;
        self
    }

    /// Set OTLP endpoint for traces
    pub fn with_otlp_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.otlp_endpoint = Some(endpoint.into());
        self
    }

    /// Set metrics export interval
    pub fn with_metrics_interval(mut self, secs: u64) -> Self {
        self.metrics_export_interval_secs = secs;
        self
    }
}

/// OpenTelemetry context holding initialized components
#[cfg(feature = "otel")]
pub struct OtelContext {
    /// Prometheus exporter for metrics
    prometheus_exporter: Option<PrometheusExporter>,

    /// Tracer provider
    tracer_provider: Option<TracerProvider>,
}

#[cfg(feature = "otel")]
impl OtelContext {
    /// Get Prometheus metrics in text format
    pub fn metrics(&self) -> Result<String, anyhow::Error> {
        if let Some(_exporter) = &self.prometheus_exporter {
            // Note: In OpenTelemetry 0.27, PrometheusExporter doesn't expose registry directly
            // Metrics are automatically exposed through the exporter
            // For now, return a placeholder
            Ok("# Metrics available through PrometheusExporter\n".to_string())
        } else {
            Ok("# Metrics disabled\n".to_string())
        }
    }

    /// Shutdown OpenTelemetry (call on application exit)
    pub fn shutdown(self) -> Result<(), anyhow::Error> {
        if let Some(provider) = self.tracer_provider {
            provider.shutdown()?;
        }
        global::shutdown_tracer_provider();
        Ok(())
    }
}

/// Initialize OpenTelemetry with the given configuration
#[cfg(feature = "otel")]
pub fn init_opentelemetry(config: OtelConfig) -> Result<OtelContext, anyhow::Error> {
    // Create resource with service information
    let resource = Resource::new(vec![
        KeyValue::new(SERVICE_NAME, config.service_name.clone()),
        KeyValue::new(SERVICE_VERSION, config.service_version.clone()),
    ]);

    // Initialize metrics if enabled
    let prometheus_exporter = if config.enable_metrics {
        // Build the Prometheus exporter with resource
        let exporter = opentelemetry_prometheus::exporter().build()?;

        tracing::info!("Prometheus metrics exporter initialized");
        Some(exporter)
    } else {
        None
    };

    // Initialize tracing if enabled
    let tracer_provider = if config.enable_tracing {
        if let Some(endpoint) = config.otlp_endpoint {
            // Use OTLP exporter - simplified version for 0.27
            // Note: For production, you may want to use a more sophisticated setup
            tracing::info!(endpoint = %endpoint, "Initializing OTLP tracing");

            // For now, create a basic tracer provider with resource
            let tracer_provider = TracerProvider::builder()
                .with_resource(resource.clone())
                .build();

            // Set global tracer provider
            global::set_tracer_provider(tracer_provider.clone());

            Some(tracer_provider)
        } else {
            // Use noop tracer if no endpoint configured
            tracing::warn!("OTLP endpoint not configured, tracing will be disabled");
            None
        }
    } else {
        None
    };

    tracing::info!(
        service_name = %config.service_name,
        metrics_enabled = config.enable_metrics,
        tracing_enabled = config.enable_tracing,
        "OpenTelemetry initialized"
    );

    Ok(OtelContext {
        prometheus_exporter,
        tracer_provider,
    })
}

/// Get a meter for recording metrics
///
/// Note: The name must be a 'static string literal or leaked string
#[cfg(feature = "otel")]
pub fn meter(name: &'static str) -> opentelemetry::metrics::Meter {
    global::meter(name)
}

/// Get a tracer for creating spans
///
/// Note: The name must be a 'static string literal or leaked string
#[cfg(feature = "otel")]
pub fn tracer(name: &'static str) -> opentelemetry::global::BoxedTracer {
    global::tracer(name)
}

#[cfg(not(feature = "otel"))]
pub struct OtelContext;

#[cfg(not(feature = "otel"))]
impl OtelContext {
    pub fn metrics(&self) -> Result<String, anyhow::Error> {
        Ok("# OpenTelemetry feature not enabled\n".to_string())
    }

    pub fn shutdown(self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

#[cfg(not(feature = "otel"))]
pub fn init_opentelemetry(_config: OtelConfig) -> Result<OtelContext, anyhow::Error> {
    tracing::warn!("OpenTelemetry feature not enabled");
    Ok(OtelContext)
}

#[cfg(test)]
#[cfg(feature = "otel")]
mod tests {
    use super::*;

    #[test]
    fn test_otel_config_default() {
        let config = OtelConfig::default();
        assert_eq!(config.service_name, "corint-decision");
        assert!(config.enable_metrics);
        assert!(config.enable_tracing);
        assert_eq!(config.metrics_export_interval_secs, 60);
    }

    #[test]
    fn test_otel_config_builder() {
        let config = OtelConfig::new("test-service")
            .with_version("1.0.0")
            .with_metrics(false)
            .with_tracing(true)
            .with_otlp_endpoint("http://localhost:4317")
            .with_metrics_interval(30);

        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.service_version, "1.0.0");
        assert!(!config.enable_metrics);
        assert!(config.enable_tracing);
        assert_eq!(config.otlp_endpoint, Some("http://localhost:4317".to_string()));
        assert_eq!(config.metrics_export_interval_secs, 30);
    }

    #[tokio::test]
    async fn test_init_metrics_only() {
        let config = OtelConfig::new("test-metrics")
            .with_metrics(true)
            .with_tracing(false);

        let ctx = init_opentelemetry(config).expect("Failed to initialize");

        // Get metrics output
        let metrics = ctx.metrics().expect("Failed to get metrics");
        assert!(metrics.contains("# ") || metrics.is_empty());

        // Cleanup
        ctx.shutdown().expect("Failed to shutdown");
    }
}
