//! CORINT Decision Engine HTTP Server
//!
//! Provides REST API for executing decision rules.

pub mod api;
pub mod config;
pub mod error;
pub mod repository_loader;

use crate::config::ServerConfig;
use crate::repository_loader::load_rules_from_repository;
use anyhow::Result;
use corint_sdk::DecisionEngineBuilder;
use corint_runtime::datasource::{DataSourceClient, DataSourceConfig};
use corint_runtime::feature::{FeatureExecutor, FeatureRegistry};
use corint_runtime::observability::otel::{OtelConfig, OtelContext, init_opentelemetry};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, error, warn};
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
    let engine = init_engine(&config).await?;
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

/// Initialize decision engine
async fn init_engine(config: &ServerConfig) -> Result<corint_sdk::DecisionEngine> {
    let mut builder = DecisionEngineBuilder::new()
        .enable_metrics(config.enable_metrics)
        .enable_tracing(config.enable_tracing);

    // Initialize feature executor (for lazy feature calculation)
    let feature_executor = init_feature_executor().await?;
    if let Some(executor) = feature_executor {
        info!("✓ Feature executor initialized");
        builder = builder.with_feature_executor(Arc::new(executor));
    } else {
        warn!("Feature executor not initialized - features will not be available");
    }

    // Initialize result writer for decision persistence (if database_url is configured)
    #[cfg(feature = "sqlx")]
    {
        // Try to get database URL from config first, then fall back to environment variable
        info!("Checking database configuration...");
        info!("  Config database_url: {:?}", config.database_url);
        
        let database_url = config.database_url.clone()
            .or_else(|| {
                let env_url = std::env::var("DATABASE_URL").ok();
                info!("  Environment DATABASE_URL: {:?}", env_url.is_some());
                env_url
            });
        
        if let Some(database_url) = database_url {
            info!("Initializing decision result writer with database: {}", database_url);
            match sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(&database_url)
                .await
            {
                Ok(pool) => {
                    info!("✓ Database connection pool created");
                    info!("Calling builder.with_result_writer()...");
                    builder = builder.with_result_writer(pool);
                    info!("✓ Decision result persistence enabled");
                }
                Err(e) => {
                    error!("Failed to create database connection pool: {}", e);
                    warn!("Decision result persistence will be disabled");
                }
            }
        } else {
            warn!("Database URL not configured, decision result persistence disabled");
            warn!("  To enable persistence, set 'database_url' in config/server.yaml or DATABASE_URL environment variable");
        }
    }
    #[cfg(not(feature = "sqlx"))]
    {
        info!("sqlx feature not enabled, decision result persistence unavailable");
    }

    // Load rules based on repository configuration
    builder = load_rules_from_repository(builder, &config.repository).await?;

    // Build engine
    let engine = builder.build().await?;
    
    Ok(engine)
}

/// Initialize feature executor with datasources and features
async fn init_feature_executor() -> Result<Option<FeatureExecutor>> {
    // Check if datasource config exists
    let datasource_dir = std::path::Path::new("repository/configs/datasources");
    if !datasource_dir.exists() {
        info!("Datasource directory not found: {:?}", datasource_dir);
        return Ok(None);
    }

    // Check if feature config exists
    let feature_dir = std::path::Path::new("repository/configs/features");
    if !feature_dir.exists() {
        info!("Feature directory not found: {:?}", feature_dir);
        return Ok(None);
    }

    let mut executor = FeatureExecutor::new().with_stats();

    // Load datasource configurations
    info!("Loading datasources from: {:?}", datasource_dir);
    let mut datasource_count = 0;
    
    if let Ok(entries) = std::fs::read_dir(datasource_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        match serde_yaml::from_str::<DataSourceConfig>(&content) {
                            Ok(config) => {
                                let datasource_name = config.name.clone();
                                match DataSourceClient::new(config).await {
                                    Ok(client) => {
                                        executor.add_datasource(&datasource_name, client);
                                        info!("  ✓ Loaded datasource: {}", datasource_name);
                                        datasource_count += 1;
                                        
                                        // Also register as "default" if it's the first datasource
                                        if datasource_count == 1 {
                                            // Need to create another client for "default"
                                            let content_clone = std::fs::read_to_string(&path)?;
                                            let config_clone: DataSourceConfig = serde_yaml::from_str(&content_clone)?;
                                            if let Ok(client_clone) = DataSourceClient::new(config_clone).await {
                                                executor.add_datasource("default", client_clone);
                                                info!("  ✓ Registered as default datasource");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("  ✗ Failed to create datasource {}: {}", datasource_name, e);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("  ✗ Failed to parse datasource config {:?}: {}", path, e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("  ✗ Failed to read datasource config {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    if datasource_count == 0 {
        info!("No datasources loaded");
        return Ok(None);
    }

    // Load feature definitions
    info!("Loading features from: {:?}", feature_dir);
    let mut registry = FeatureRegistry::new();
    let mut feature_file_count = 0;

    if let Ok(entries) = std::fs::read_dir(feature_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                match registry.load_from_file(&path) {
                    Ok(_) => {
                        info!("  ✓ Loaded features from: {:?}", path.file_name());
                        feature_file_count += 1;
                    }
                    Err(e) => {
                        warn!("  ✗ Failed to load features from {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    info!("Loaded {} feature files", feature_file_count);

    // Register features to executor
    for feature in registry.all_features() {
        if let Err(e) = executor.register_feature(feature.clone()) {
            warn!("  ✗ Failed to register feature {}: {}", feature.name, e);
        }
    }

    let feature_count = registry.count();
    if feature_count == 0 {
        info!("No features loaded");
        return Ok(None);
    }

    info!("✓ Loaded {} datasources, {} features", datasource_count, feature_count);
    
    Ok(Some(executor))
}
