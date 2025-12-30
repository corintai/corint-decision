//! API endpoint handlers
//!
//! HTTP request handlers for all REST API endpoints.

use super::conversions::{extract_reason_codes, json_to_value, normalize_score, value_to_json};
use super::extractors::JsonExtractor;
use super::types::*;
use crate::error::ServerError;
use axum::{
    extract::State,
    Json,
};
use corint_core::{ast::Signal, Value};
use corint_sdk::DecisionRequest;
use std::collections::HashMap;
use tracing::{error, info};

/// Health check endpoint
pub(super) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Decision endpoint
#[axum::debug_handler]
pub(super) async fn decide(
    State(state): State<AppState>,
    JsonExtractor(payload): JsonExtractor<DecideRequestPayload>,
) -> Result<Json<DecideResponsePayload>, ServerError> {
    let options = payload.options.unwrap_or_default();

    info!(
        "Received decision request with {} event fields, enable_trace={}",
        payload.event.len(),
        options.enable_trace
    );

    // Helper function to convert namespace
    let convert_namespace = |ns: HashMap<String, serde_json::Value>| -> HashMap<String, Value> {
        ns.into_iter()
            .map(|(k, v)| (k, json_to_value(v)))
            .collect()
    };

    // Convert event data (required)
    let event_data = convert_namespace(payload.event);

    // Create decision request with multi-namespace support
    let mut request = DecisionRequest::new(event_data);

    // Add user namespace if provided
    if let Some(user) = payload.user {
        request = request.with_vars(convert_namespace(user));
    }

    // Add optional namespaces if provided (legacy/internal)
    if let Some(features) = payload.features {
        request = request.with_features(convert_namespace(features));
    }
    if let Some(api) = payload.api {
        request = request.with_api(convert_namespace(api));
    }
    if let Some(service) = payload.service {
        request = request.with_service(convert_namespace(service));
    }
    if let Some(llm) = payload.llm {
        request = request.with_llm(convert_namespace(llm));
    }
    if let Some(vars) = payload.vars {
        request = request.with_vars(convert_namespace(vars));
    }

    // Enable tracing if requested
    if options.enable_trace {
        request = request.with_trace();
    }

    // Execute decision (acquire read lock - allows concurrent reads)
    let engine = state.engine.read().await;
    let response = engine.decide(request).await?;
    drop(engine); // Release lock as soon as possible

    // Convert signal to decision result string (lowercase to match test expectations)
    let result_str = response
        .result
        .signal
        .map(|signal| match signal {
            Signal::Approve => "approve",
            Signal::Decline => "decline",
            Signal::Review => "review",
            Signal::Hold => "hold",
            Signal::Pass => "pass",
        })
        .unwrap_or("pass")
        .to_string();

    // Build the response
    Ok(Json(DecideResponsePayload {
        request_id: response.request_id,
        status: 200,
        process_time_ms: response.processing_time_ms,
        pipeline_id: response.pipeline_id.unwrap_or_else(|| "default".to_string()),
        decision: DecisionPayload {
            result: result_str,
            actions: response.result.actions.clone(),
            scores: ScoresPayload {
                canonical: normalize_score(response.result.score),
                raw: response.result.score,
                confidence: None,
            },
            evidence: EvidencePayload {
                triggered_rules: response.result.triggered_rules,
            },
            cognition: CognitionPayload {
                summary: response.result.explanation.clone(),
                reason_codes: extract_reason_codes(&response.result.explanation),
            },
        },
        features: if options.return_features {
            Some(
                response
                    .result
                    .context
                    .into_iter()
                    .map(|(k, v)| (k, value_to_json(v)))
                    .collect(),
            )
        } else {
            None
        },
        trace: response.trace,
    }))
}

/// Reload repository endpoint
pub(super) async fn reload_repository(State(state): State<AppState>) -> Result<Json<ReloadResponse>, ServerError> {
    info!("Received repository reload request");

    // Reload engine using SDK's reload method (acquire write lock - exclusive access)
    {
        let mut engine = state.engine.write().await;
        engine.reload().await.map_err(|e| {
            error!("Failed to reload repository: {}", e);
            ServerError::InternalError(anyhow::anyhow!("Failed to reload repository: {}", e))
        })?;
    }

    info!("Repository reloaded successfully");
    Ok(Json(ReloadResponse {
        success: true,
        message: "Repository reloaded successfully".to_string(),
    }))
}
