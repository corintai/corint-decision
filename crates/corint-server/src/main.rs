//! CORINT Decision Engine HTTP Server
//!
//! Provides REST API for executing decision rules.

mod api;
mod config;
mod error;

use crate::config::ServerConfig;
use anyhow::Result;
use corint_sdk::DecisionEngineBuilder;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing()?;

    // Load configuration
    let config = ServerConfig::load()?;
    info!("Loaded configuration: {:?}", config);

    // Initialize decision engine
    let engine = init_engine(&config).await?;
    info!("Decision engine initialized");

    // Create router
    let app = api::create_router(Arc::new(engine));

    // Start server
    let addr = format!("{}:{}", config.host, config.port);
    info!("Starting server on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    info!("✓ Server listening on http://{}", addr);
    info!("  Health check: http://{}/health", addr);
    info!("  Decision API: http://{}/v1/decide", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Initialize tracing subscriber
fn init_tracing() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "corint_server=info,corint_sdk=info,corint_runtime=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize tracing: {}", e))?;

    Ok(())
}

/// Initialize decision engine
async fn init_engine(config: &ServerConfig) -> Result<corint_sdk::DecisionEngine> {
    let mut builder = DecisionEngineBuilder::new()
        .enable_metrics(config.enable_metrics)
        .enable_tracing(config.enable_tracing);

    // Load rule files from rules directory
    if config.rules_dir.exists() {
        info!("Loading rules from directory: {:?}", config.rules_dir);
        
        // Read directory and load all YAML files
        let entries = std::fs::read_dir(&config.rules_dir)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                info!("Loading rule file: {:?}", path);
                builder = builder.add_rule_file(path);
            }
        }
    } else {
        error!("Rules directory not found: {:?}", config.rules_dir);
        info!("Server will start without any rules loaded");
    }

    // Build engine
    let engine = builder.build().await?;
    
    Ok(engine)
}
