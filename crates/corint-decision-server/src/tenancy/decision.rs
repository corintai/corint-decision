//! Public decision API: tenant is request metadata; policy routing stays in Registry.
use super::{config::identifier, dispatch, error, Host, Runtime, Scope};
use anyhow::{ensure, Result};
use axum::{
    body::{to_bytes, Body},
    extract::{Path as RoutePath, Request, State},
    http::{header::CONTENT_LENGTH, StatusCode},
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

pub(super) fn bindings(
    runtimes: &BTreeMap<Scope, Arc<Runtime>>,
    configured: BTreeMap<String, Scope>,
) -> Result<BTreeMap<String, Scope>> {
    for (tenant, scope) in &configured {
        ensure!(
            tenant == &scope.tenant_id && runtimes.contains_key(scope),
            "Invalid tenant decision binding"
        );
    }
    let mut routes = configured;
    for scope in runtimes.keys() {
        if routes.contains_key(&scope.tenant_id) {
            continue;
        }
        ensure!(
            runtimes
                .keys()
                .filter(|s| s.tenant_id == scope.tenant_id)
                .count()
                == 1,
            "Tenant has multiple runtimes; configure one operator decision_binding"
        );
        routes.insert(scope.tenant_id.clone(), scope.clone());
    }
    Ok(routes)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DecisionRequest {
    #[serde(skip_serializing)]
    tenant_id: String,
    event: serde_json::Map<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    business_event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    idempotency_key: Option<String>,
    #[serde(default)]
    enable_trace: bool,
}

pub(super) async fn decide(State(host): State<Arc<Host>>, request: Request) -> Response {
    // Authentication precedes body reading. Authorization for the requested
    // tenant follows bounded parsing and is repeated in the shared dispatcher.
    if host
        .credentials
        .registry
        .read()
        .await
        .authenticate(request.headers())
        .is_none()
    {
        return error(StatusCode::UNAUTHORIZED, "E_TENANT_UNAUTHORIZED");
    }
    if request.uri().query().is_some() {
        return error(StatusCode::BAD_REQUEST, "E_TENANT_QUERY");
    }
    let Ok(reader) = host.request_readers.try_acquire() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "E_TENANT_QUOTA");
    };
    let (mut parts, body) = request.into_parts();
    let bytes = match tokio::time::timeout(Duration::from_secs(10), to_bytes(body, 8 * 1024 * 1024))
        .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(_)) => return error(StatusCode::PAYLOAD_TOO_LARGE, "E_TENANT_BODY"),
        Err(_) => return error(StatusCode::REQUEST_TIMEOUT, "E_TENANT_BODY_TIMEOUT"),
    };
    // Struct deserialization also rejects duplicate tenant_id and unknown scope overrides.
    let input: DecisionRequest = match serde_json::from_slice(&bytes) {
        Ok(input) => input,
        Err(_) => return error(StatusCode::BAD_REQUEST, "E_TENANT_REQUEST"),
    };
    if !identifier(&input.tenant_id) {
        return error(StatusCode::BAD_REQUEST, "E_TENANT_REQUEST");
    }
    let Some(scope) = host.decision_bindings.get(&input.tenant_id).cloned() else {
        return error(StatusCode::FORBIDDEN, "E_TENANT_FORBIDDEN");
    };
    // Internal forwarding removes only transport identity; business fields and
    // idempotency options reach the existing Core validation without overrides.
    let body = serde_json::to_vec(&input).expect("JSON decision request");
    drop(input);
    drop(bytes);
    parts.headers.remove(CONTENT_LENGTH);
    let request = Request::from_parts(parts, Body::from(body));
    drop(reader);
    dispatch(
        State(host),
        RoutePath((
            scope.tenant_id,
            scope.environment,
            scope.deployment,
            "v1/core/decide".into(),
        )),
        request,
    )
    .await
}
