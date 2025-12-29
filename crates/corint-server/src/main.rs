//! CORINT Decision Engine HTTP Server
//!
//! Provides REST API for executing decision rules.

pub mod api;
pub mod config;
pub mod engine;
pub mod error;

use crate::api::grpc::pb::decision_service_server::DecisionServiceServer;
use crate::api::grpc::DecisionGrpcService;
use crate::config::ServerConfig;
use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tonic::transport::Server as TonicServer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing()?;

    // Load configuration
    let config = ServerConfig::load()?;
    info!("Loaded configuration: {:?}", config);

    // Initialize decision engine
    let engine = engine::init_engine(&config).await?;
    info!("Decision engine initialized");

    // Create router
    let app = api::create_router(Arc::new(engine));

    // Start HTTP server
    let http_addr = format!("{}:{}", config.host, config.port);
    info!("Starting HTTP server on {}", http_addr);

    let listener = TcpListener::bind(&http_addr).await?;
    info!("✓ HTTP Server listening on http://{}", http_addr);
    info!("  Health check: http://{}/health", http_addr);
    info!("  Decision API: http://{}/v1/decide", http_addr);
    info!("  Reload repository: POST http://{}/v1/repo/reload", http_addr);

    // Start gRPC server if configured
    if let Some(grpc_port) = config.grpc_port {
        let grpc_addr = format!("{}:{}", config.host, grpc_port).parse()?;

        // Reinitialize engine for gRPC server
        let grpc_engine = engine::init_engine(&config).await?;
        let grpc_service = DecisionGrpcService::new(Arc::new(RwLock::new(grpc_engine)));

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
                "corint_server=info,corint_sdk=info,corint_runtime=info,tower_http=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize tracing: {}", e))?;

    Ok(())
}

