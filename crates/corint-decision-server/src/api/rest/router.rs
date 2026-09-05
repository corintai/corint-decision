//! Router creation and configuration
//!
//! Creates Axum routers for REST API endpoints.

use super::handlers::*;
use super::types::AppState;
use crate::access::AccessPolicy;
use crate::snapshot::{EngineManager, POLICY_HEADER, REVISION_HEADER};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use axum::{
    http::HeaderName,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Create REST API router
pub fn create_router(engine: Arc<EngineManager>, access: AccessPolicy) -> Router {
    let state = AppState {
        engine,
        access: access.clone(),
    };

    Router::new()
        .route("/health", get(health))
        .route("/v1/decide", post(decide))
        .route("/v1/repo/reload", post(reload_repository)) // Changed from GET to POST
        .route_layer(middleware::from_fn_with_state(access, authenticate))
        .with_state(state)
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(CorsLayer::new().expose_headers([
            HeaderName::from_static(REVISION_HEADER),
            HeaderName::from_static(POLICY_HEADER),
        ]))
        .layer(TraceLayer::new_for_http())
}

async fn authenticate(
    State(access): State<AccessPolicy>,
    request: Request,
    next: Next,
) -> Response {
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }
    let headers = request.headers().get_all("authorization");
    let mut values = headers.iter();
    let token = values.next().and_then(|v| v.to_str().ok());
    if values.next().is_some() || !access.permits(token, request.uri().path() == "/v1/repo/reload")
    {
        return (StatusCode::UNAUTHORIZED, "UNAUTHORIZED").into_response();
    }
    next.run(request).await
}
