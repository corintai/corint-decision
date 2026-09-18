use anyhow::{bail, Context, Result};
use corint_decision_mcp::CorintMcp;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;
use std::{net::SocketAddr, sync::Arc};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("--help")) {
        eprintln!("corint-mcp (--repository PATH | --config PATH) [--http-listen 127.0.0.1:8082]\nDefault: stdio. HTTP mode serves /mcp and /health on a loopback address.\nRepository mode discovers current Pipeline/Ruleset sources. Config mode uses an explicit catalog. See docs/mcp.md.");
        return Ok(());
    }
    let repository = first.as_deref() == Some(std::ffi::OsStr::new("--repository"));
    if !repository && first.as_deref() != Some(std::ffi::OsStr::new("--config")) {
        bail!(
            "Usage: corint-mcp (--repository PATH | --config PATH) [--http-listen 127.0.0.1:8082]"
        );
    }
    let config = PathBuf::from(args.next().context("Source option requires a path")?);
    let address: Option<SocketAddr> = match args.next() {
        None => None,
        Some(flag) if flag == "--http-listen" => Some(
            args.next()
                .context("--http-listen requires an address")?
                .to_str()
                .context("HTTP address must be UTF-8")?
                .parse()?,
        ),
        Some(_) => bail!("Unexpected command-line argument"),
    };
    anyhow::ensure!(args.next().is_none(), "Unexpected command-line argument");
    // stdout belongs exclusively to MCP JSON-RPC. No stdout logging subscriber.
    let server = if repository {
        CorintMcp::from_repository(&config)?
    } else {
        CorintMcp::from_config(&config)?
    };
    if let Some(address) = address {
        anyhow::ensure!(
            address.ip().is_loopback(),
            "MCP HTTP mode requires a loopback listener"
        );
        let shutdown = CancellationToken::new();
        let service = StreamableHttpService::new(
            move || Ok(server.clone()),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default()
                .with_json_response(true)
                .enforce_origin_validation()
                .with_cancellation_token(shutdown.child_token()),
        );
        let app = axum::Router::new().nest_service("/mcp", service).route(
            "/health",
            axum::routing::get(|| async {
                axum::Json(serde_json::json!({"status":"healthy", "service":"corint-mcp"}))
            }),
        );
        let listener = tokio::net::TcpListener::bind(address).await?;
        eprintln!("MCP Server listening on http://{}", listener.local_addr()?);
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                shutdown_signal().await;
                shutdown.cancel();
            })
            .await?;
        return Ok(());
    }
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
