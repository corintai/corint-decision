//! CORINT Decision Engine HTTP Server
//!
//! Provides REST API for executing decision rules.

pub mod api;
pub mod config;
pub mod engine;
pub mod error;

use crate::config::ServerConfig;
use anyhow::Result;
use corint_runtime::observability::otel::{init_opentelemetry, OtelConfig, OtelContext};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing()?;

    // Load configuration
    let config = ServerConfig::load()?;
    info!("Loaded configuration: {:?}", config);

    // Initialize OpenTelemetry
    let otel_ctx = init_otel(&config)?;
    info!("OpenTelemetry initialized");

    // Initialize decision engine
    let engine = engine::init_engine(&config).await?;
    info!("Decision engine initialized");

    // Create router with OTel context
    let app = api::create_router_with_metrics(Arc::new(engine), Arc::new(otel_ctx));

    // Start server
    let addr = format!("{}:{}", config.host, config.port);
    info!("Starting server on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    info!("✓ Server listening on http://{}", addr);
    info!("  Health check: http://{}/health", addr);
    info!("  Decision API: http://{}/v1/decide", addr);
    info!("  Metrics: http://{}/metrics", addr);
    info!("  Reload repository: POST http://{}/v1/repo/reload", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Initialize tracing subscriber
fn init_tracing() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "corint_server=info,corint_sdk=info,corint_runtime=info,tower_http=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize tracing: {}", e))?;

    Ok(())
}

/// Initialize OpenTelemetry
fn init_otel(config: &ServerConfig) -> Result<OtelContext> {
    let otel_config = OtelConfig::new("corint-server")
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_metrics(config.enable_metrics)
        .with_tracing(config.enable_tracing);

    // Configure OTLP endpoint if available
    let otel_config = if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        info!("Using OTLP endpoint: {}", endpoint);
        otel_config.with_otlp_endpoint(endpoint)
    } else {
        otel_config
    };

    init_opentelemetry(otel_config)
}

