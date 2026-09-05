//! CORINT Decision Engine HTTP Server
//!
//! Provides REST API for executing decision rules.

pub mod api;
pub mod config;
pub mod core;
pub mod engine;
pub mod error;
pub mod snapshot;

use crate::api::grpc::pb::decision_service_server::DecisionServiceServer;
use crate::api::grpc::DecisionGrpcService;
use crate::config::ServerConfig;
use crate::snapshot::EngineManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tonic::transport::Server as TonicServer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing()?;

    // Explicit isolated mode: errors never fall back to compatibility loading.
    // Do not load/log legacy datasource configuration or start a second gRPC engine.
    if let Some(path) = std::env::var_os("CORINT_CORE_CONFIG") {
        let (address, app) = core::load(std::path::Path::new(&path)).await?;
        let listener = TcpListener::bind(address).await?;
        info!(
            "Experimental strict Core server listening on {}",
            listener.local_addr()?
        );
        axum::serve(listener, app).await?;
        return Ok(());
    }

    // Load configuration
    let config = ServerConfig::load()?;
    info!("Loaded configuration: {:?}", config);

    // Initialize decision engine
    let engine = engine::init_engine(&config).await?;
    info!("Decision engine initialized");

    // Create router
    let manager = Arc::new(EngineManager::new(Arc::new(engine))?);
    let app = api::create_router(manager.clone());

    // Start HTTP server
    let http_addr = format!("{}:{}", config.server.host, config.server.port);
    info!("Starting HTTP server on {}", http_addr);

    let listener = TcpListener::bind(&http_addr).await?;
    info!("✓ HTTP Server listening on http://{}", http_addr);
    info!("  Health check: http://{}/health", http_addr);
    info!("  Decision API: http://{}/v1/decide", http_addr);
    info!(
        "  Reload repository: POST http://{}/v1/repo/reload",
        http_addr
    );

    // Start gRPC server if configured
    if let Some(grpc_port) = config.server.grpc_port {
        let grpc_addr = format!("{}:{}", config.server.host, grpc_port).parse()?;

        let grpc_service = DecisionGrpcService::new(manager.clone());

        info!("Starting gRPC server on {}", grpc_addr);

        // Build reflection service
        let file_descriptor_set = include_bytes!("../proto/decision_descriptor.bin");
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(file_descriptor_set)
            .build_v1()
            .unwrap();

        // Spawn gRPC server in background
        tokio::spawn(async move {
            TonicServer::builder()
                .add_service(DecisionServiceServer::new(grpc_service))
                .add_service(reflection_service)
                .serve(grpc_addr)
                .await
                .expect("gRPC server failed");
        });

        info!("✓ gRPC Server listening on {}", grpc_addr);
        info!("  gRPC Decision API: {}:Decide", grpc_addr);
        info!("  gRPC Health check: {}:HealthCheck", grpc_addr);
        info!("  gRPC Reflection API enabled");
    }

    // Run HTTP server
    axum::serve(listener, app).await?;

    Ok(())
}

/// Initialize tracing subscriber
fn init_tracing() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "corint_decision_server=info,corint_decision_engine=info,corint_decision_runtime=info,tower_http=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize tracing: {}", e))?;

    Ok(())
}
