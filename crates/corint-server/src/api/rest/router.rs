//! Router creation and configuration
//!
//! Creates Axum routers for REST API endpoints.

use super::handlers::*;
use super::types::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use corint_sdk::DecisionEngine;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Create REST API router
pub fn create_router(engine: Arc<DecisionEngine>) -> Router {
    let state = AppState {
        engine: Arc::new(RwLock::new(
            Arc::try_unwrap(engine).unwrap_or_else(|_arc| {
                // This should not happen during normal initialization
                // If it does, log a warning and create a minimal engine
                tracing::warn!("Arc<DecisionEngine> has multiple references during router creation");
                // We can't clone DecisionEngine, so we panic with a clear message
                panic!("Cannot create router: DecisionEngine Arc has multiple references. This is a programming error.");
            })
        )),
    };

    Router::new()
        .route("/health", get(health))
        .route("/v1/decide", post(decide))
        .route("/v1/repo/reload", post(reload_repository))  // Changed from GET to POST
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
