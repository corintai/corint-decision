//! API endpoint handlers
//!
//! HTTP request handlers for all REST API endpoints.

use super::conversions::{extract_reason_codes, json_to_value, normalize_score, value_to_json};
use super::extractors::JsonExtractor;
use super::types::*;
use crate::error::ServerError;
use crate::snapshot::{EngineSnapshot, EXPECTED_REVISION_HEADER, POLICY_HEADER, REVISION_HEADER};
use axum::{extract::State, http::HeaderMap, Json};
use corint_decision_engine::{DecisionRequest, Signal, Value};
use std::collections::HashMap;
use tracing::{error, info};

/// Health check endpoint
pub(super) async fn health(State(state): State<AppState>) -> (HeaderMap, Json<HealthResponse>) {
    let snapshot = state.engine.snapshot().await;
    (
        snapshot_headers(&snapshot),
        Json(HealthResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
    )
}

/// Decision endpoint
#[axum::debug_handler]
pub(super) async fn decide(
    State(state): State<AppState>,
    JsonExtractor(payload): JsonExtractor<DecideRequestPayload>,
) -> Result<(HeaderMap, Json<DecideResponsePayload>), ServerError> {
    if payload.user.is_some()
        || payload.features.is_some()
        || payload.service.is_some()
        || payload.llm.is_some()
        || payload.vars.is_some()
        || payload.event.contains_key("tenant_id")
    {
        return Err(ServerError::InvalidRequest(
            "Only event data is caller-owned; tenant and computed namespaces are operator-owned"
                .into(),
        ));
    }
    let options = payload.options.unwrap_or_default();
    if options.async_mode {
        return Err(ServerError::InvalidRequest(
            "async is not implemented".into(),
        ));
    }

    info!(
        "Received decision request with {} event fields, enable_trace={}",
        payload.event.len(),
        options.enable_trace
    );

    // Helper function to convert namespace
    let convert_namespace = |ns: HashMap<String, serde_json::Value>| -> HashMap<String, Value> {
        ns.into_iter().map(|(k, v)| (k, json_to_value(v))).collect()
    };

    // Convert event data (required)
    let event_data = convert_namespace(payload.event);

    // Create decision request with multi-namespace support
    let mut request = DecisionRequest::new(event_data).with_vars(HashMap::from([(
        "tenant_id".into(),
        Value::String(state.access.tenant_id.clone()),
    )]));

    // Add user namespace if provided
    if let Some(user) = payload.user {
        request = request.with_vars(convert_namespace(user));
    }

    // Add optional namespaces if provided (legacy/internal)
    if let Some(features) = payload.features {
        request = request.with_features(convert_namespace(features));
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

    let snapshot = state.engine.snapshot().await;
    let response = snapshot.engine.decide(request).await?;

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
    Ok((
        snapshot_headers(&snapshot),
        Json(DecideResponsePayload {
            request_id: response.request_id,
            status: 200,
            process_time_ms: response.processing_time_ms,
            pipeline_id: response
                .pipeline_id
                .unwrap_or_else(|| "default".to_string()),
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
        }),
    ))
}

/// Reload repository endpoint. Empty requests remain valid; callers may supply
/// x-corint-expected-revision to reject stale administrative requests.
pub(super) async fn reload_repository(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<ReloadResponse>), ServerError> {
    let expected = headers
        .get(EXPECTED_REVISION_HEADER)
        .map(|value| {
            value
                .to_str()
                .map_err(|_| ServerError::InvalidRequest("Invalid expected revision".into()))
        })
        .transpose()?;
    let snapshot = state.engine.reload(expected).await.map_err(|error| {
        error!("Failed to reload repository: {}", error);
        ServerError::Reload(error)
    })?;
    Ok((
        snapshot_headers(&snapshot),
        Json(ReloadResponse {
            success: true,
            message: "Repository reloaded successfully".to_string(),
        }),
    ))
}

fn snapshot_headers(snapshot: &EngineSnapshot) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        REVISION_HEADER,
        snapshot.revision.parse().expect("UUID header"),
    );
    headers.insert(
        POLICY_HEADER,
        snapshot.compiled_sha256.parse().expect("SHA256 header"),
    );
    headers
}
