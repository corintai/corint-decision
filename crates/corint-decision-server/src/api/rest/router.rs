//! Router creation and configuration
//!
//! Creates Axum routers for REST API endpoints.

use super::handlers::*;
use super::types::AppState;
use crate::snapshot::{EngineManager, POLICY_HEADER, REVISION_HEADER};
use axum::{
    http::HeaderName,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Create REST API router
pub fn create_router(engine: Arc<EngineManager>) -> Router {
    let state = AppState { engine };

    Router::new()
        .route("/health", get(health))
        .route("/v1/decide", post(decide))
        .route("/v1/repo/reload", post(reload_repository)) // Changed from GET to POST
        .with_state(state)
        .layer(CorsLayer::permissive().expose_headers([
            HeaderName::from_static(REVISION_HEADER),
            HeaderName::from_static(POLICY_HEADER),
        ]))
        .layer(TraceLayer::new_for_http())
}
