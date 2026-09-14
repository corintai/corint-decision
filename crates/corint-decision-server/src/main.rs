//! CORINT Decision Engine HTTP Server
//!
//! Provides REST API for executing decision rules.

pub mod access;
pub mod api;
pub mod config;
pub mod core;
pub mod engine;
pub mod error;
pub mod evidence;
pub mod journal;
pub mod repo_source;
pub mod snapshot;
pub mod tenancy;

use crate::api::grpc::pb::decision_service_server::DecisionServiceServer;
use crate::api::grpc::DecisionGrpcService;
use crate::config::ServerConfig;
use crate::snapshot::EngineManager;
use anyhow::Result;
use corint_decision_engine::background::shutdown_background_writes;
use std::sync::Arc;
use tokio::net::TcpListener;
use tonic::transport::Server as TonicServer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|v| v == "--replay-journal") {
        anyhow::ensure!(
            args.len() == 4,
            "usage: corint-decision-server --replay-journal BUNDLE_JSON EXPORT_ITEM_JSON"
        );
        fn read(path: &std::ffi::OsStr) -> Result<String> {
            use std::io::Read;
            let file = std::fs::File::open(path)?;
            anyhow::ensure!(
                file.metadata()?.is_file(),
                "Replay input must be a regular file"
            );
            let mut bytes = Vec::new();
            file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
            anyhow::ensure!(bytes.len() <= 8 * 1024 * 1024, "Replay input exceeds 8 MiB");
            Ok(String::from_utf8(bytes)?)
        }
        let bundle = corint_decision_toolchain::transfer::read_bundle(
            &corint_decision_compiler::core::CoreSource {
                path: "bundle".into(),
                yaml: read(&args[2])?,
            },
        )?;
        let export = serde_json::from_str(&read(&args[3])?)
            .map_err(|_| anyhow::anyhow!("Invalid journal export JSON"))?;
        let report = corint_decision_toolchain::replay::replay_journal(
            &bundle.sources,
            &bundle.input_schema,
            &export,
        )
        .await?;
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    // Initialize tracing
    init_tracing()?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let notify_shutdown = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = notify_shutdown.send(true);
    });

    // Explicit isolated mode: errors never fall back to compatibility loading.
    // Do not load/log legacy datasource configuration or start a second gRPC engine.
    anyhow::ensure!(
        std::env::var_os("CORINT_CORE_CONFIG").is_none()
            || std::env::var_os("CORINT_TENANT_CONFIG").is_none(),
        "Choose Core or tenant configuration, not both"
    );
    if let Some(path) = std::env::var_os("CORINT_TENANT_CONFIG") {
        let (address, app) = tenancy::load(std::path::Path::new(&path)).await?;
        let listener = TcpListener::bind(address).await?;
        info!(
            "Tenant decision host listening on {}",
            listener.local_addr()?
        );
        let result = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_requested(shutdown_rx))
            .await;
        tenancy::drain(std::time::Duration::from_secs(120)).await?;
        shutdown_background_writes(std::time::Duration::from_secs(30))
            .await
            .map_err(anyhow::Error::msg)?;
        result?;
        return Ok(());
    }
    if let Some(path) = std::env::var_os("CORINT_CORE_CONFIG") {
        let (address, app) = core::load(std::path::Path::new(&path)).await?;
        let listener = TcpListener::bind(address).await?;
        info!(
            "Experimental strict Core server listening on {}",
            listener.local_addr()?
        );
        let result = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_requested(shutdown_rx))
            .await;
        shutdown_background_writes(std::time::Duration::from_secs(30))
            .await
            .map_err(anyhow::Error::msg)?;
        result?;
        return Ok(());
    }

    // Load configuration
    let config = ServerConfig::load()?;
    let access = access::AccessPolicy::load().await?;
    anyhow::ensure!(
        config
            .server
            .host
            .parse::<std::net::IpAddr>()?
            .is_loopback(),
        "Compatibility mode requires a loopback listener; terminate TLS at an authenticated edge"
    );
    info!("Loaded server configuration");

    // Initialize decision engine
    let engine = engine::init_engine(&config).await.map_err(|_| {
        anyhow::anyhow!("Decision engine initialization failed; check operator configuration")
    })?;
    info!("Decision engine initialized");

    // Create router
    let manager = Arc::new(EngineManager::new(Arc::new(engine))?);
    let app = api::create_router(manager.clone(), access.clone());

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
    let grpc_task = if let Some(grpc_port) = config.server.grpc_port {
        let grpc_addr = format!("{}:{}", config.server.host, grpc_port).parse()?;

        let grpc_service = DecisionGrpcService::new(manager.clone(), access.clone());

        info!("Starting gRPC server on {}", grpc_addr);

        // Build reflection service
        let file_descriptor_set =
            include_bytes!(concat!(env!("OUT_DIR"), "/decision_descriptor.bin"));
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(file_descriptor_set)
            .build_v1()
            .unwrap();

        // Spawn gRPC server in background
        let grpc_shutdown = shutdown_rx.clone();
        let task = tokio::spawn(async move {
            TonicServer::builder()
                .add_service(DecisionServiceServer::new(grpc_service))
                .add_service(reflection_service)
                .serve_with_shutdown(grpc_addr, shutdown_requested(grpc_shutdown))
                .await
        });

        info!("✓ gRPC Server listening on {}", grpc_addr);
        info!("  gRPC Decision API: {}:Decide", grpc_addr);
        info!("  gRPC Health check: {}:HealthCheck", grpc_addr);
        info!("  gRPC Reflection API enabled");
        Some(task)
    } else {
        None
    };

    // Run HTTP server
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_requested(shutdown_rx))
        .await;
    let _ = shutdown_tx.send(true);
    if let Some(task) = grpc_task {
        task.await??;
    }
    access.drain(std::time::Duration::from_secs(30)).await?;
    shutdown_background_writes(std::time::Duration::from_secs(30))
        .await
        .map_err(anyhow::Error::msg)?;
    result?;
    Ok(())
}

async fn shutdown_requested(mut signal: tokio::sync::watch::Receiver<bool>) {
    if !*signal.borrow() {
        let _ = signal.changed().await;
    }
}
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
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
