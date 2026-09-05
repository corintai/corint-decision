//! Opt-in, single-target Core server. Local operator configuration is the trust
//! root; caller declarations and historical reports never authorize activation.
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use corint_decision_compiler::core::{parse_core_input_schema, CoreError, CoreSource};
use corint_decision_engine::{DecisionEngine, DecisionRequest, Value};
use corint_decision_toolchain::{
    behavior,
    contracts::{CompatibilityReport, TargetContracts},
    package, transfer,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::Read,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use subtle::ConstantTimeEq;
use tokio::sync::{RwLock, Semaphore};

const MAX_BYTES: usize = 8 * 1024 * 1024;

/// Read only from operator-owned local files, never from HTTP request data.
/// Configuration and secrets are immutable until process restart.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreConfig {
    pub config_version: String,
    pub listen: SocketAddr,
    pub context: PathBuf,
    pub target: PathBuf,
    pub cases: PathBuf,
    pub initial_bundle: PathBuf,
    pub decision_token_env: String,
    pub publisher_token_env: String,
    pub approvals: Vec<OperatorApproval>,
}

/// An explicit local operator allowlist, NOT a portable approval or signature.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorApproval {
    pub policy_sha256: String,
    pub context_sha256: String,
    pub target_sha256: String,
    pub cases_sha256: String,
}

struct Policy {
    engine: DecisionEngine,
    compatibility: CompatibilityReport,
}
struct Active {
    revision: String,
    policy: Policy,
}
struct Gate {
    contracts: TargetContracts,
    cases: CoreSource,
    approvals: Vec<OperatorApproval>,
    cases_sha256: String,
}
#[derive(Clone)]
struct CoreState {
    gate: Arc<Gate>,
    active: Arc<RwLock<Arc<Active>>>,
    preparation: Arc<Semaphore>,
}
#[derive(Clone)]
struct Credential([u8; 32]);

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn read(path: &Path) -> anyhow::Result<CoreSource> {
    let file = std::fs::File::open(path)?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "Expected a regular operator file"
    );
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= MAX_BYTES, "Operator file exceeds 8 MiB");
    Ok(CoreSource {
        path: path.display().to_string(),
        yaml: String::from_utf8(bytes)?,
    })
}

pub async fn load(path: &Path) -> anyhow::Result<(SocketAddr, Router)> {
    let config: CoreConfig = serde_json::from_str(&read(path)?.yaml)?;
    let address = config.listen;
    let decision_token = std::env::var(&config.decision_token_env)
        .map_err(|_| anyhow::anyhow!("Missing Core decision credential"))?;
    let publisher_token = std::env::var(&config.publisher_token_env)
        .map_err(|_| anyhow::anyhow!("Missing Core publisher credential"))?;
    let app = create_router(
        config,
        path.parent().unwrap_or(Path::new(".")),
        &decision_token,
        &publisher_token,
    )
    .await?;
    Ok((address, app))
}

/// Construct the isolated router for an operator-owned configuration. This is
/// also the in-process integration boundary; it does not start a listener.
pub async fn create_router(
    config: CoreConfig,
    root: &Path,
    decision_token: &str,
    publisher_token: &str,
) -> anyhow::Result<Router> {
    anyhow::ensure!(
        config.config_version == "1",
        "Unsupported Core server config version"
    );
    // This increment is deliberately local-only: no unauthenticated plaintext
    // service on external interfaces, and no claim of a multi-tenant auth system.
    anyhow::ensure!(
        config.listen.ip().is_loopback(),
        "Core mode requires a loopback listener"
    );
    for token in [decision_token, publisher_token] {
        anyhow::ensure!(
            token.len() >= 32 && token.len() <= 1024 && token.bytes().all(|b| b.is_ascii_graphic()),
            "Core tokens require 32..1024 printable non-space ASCII characters"
        );
    }
    anyhow::ensure!(
        decision_token != publisher_token,
        "Decision and publisher credentials must differ"
    );
    anyhow::ensure!(
        !config.approvals.is_empty() && config.approvals.len() <= 128,
        "Require 1..128 explicit operator approvals"
    );
    for approval in &config.approvals {
        anyhow::ensure!(
            [
                &approval.policy_sha256,
                &approval.context_sha256,
                &approval.target_sha256,
                &approval.cases_sha256,
            ]
            .iter()
            .all(|value| is_hash(value)),
            "Invalid operator approval fingerprint"
        );
    }
    let context = read(&root.join(&config.context))?;
    let target = read(&root.join(&config.target))?;
    let cases = read(&root.join(&config.cases))?;
    behavior::validate_suite(&cases)?;
    let initial = read(&root.join(&config.initial_bundle))?;
    let gate = Arc::new(Gate {
        contracts: TargetContracts::load(&context, &target)?,
        cases_sha256: hash(cases.yaml.as_bytes()),
        cases,
        approvals: config.approvals,
    });
    let worker_gate = gate.clone();
    let policy = tokio::task::spawn_blocking(move || worker_gate.prepare(&initial))
        .await?
        .map_err(|e| anyhow::anyhow!("Core initial policy rejected: {}", e.code))?;
    let state = CoreState {
        gate,
        active: Arc::new(RwLock::new(Arc::new(Active {
            revision: uuid::Uuid::new_v4().to_string(),
            policy,
        }))),
        preparation: Arc::new(Semaphore::new(1)),
    };
    let credential = |token: &str| Credential(Sha256::digest(token.as_bytes()).into());
    let decisions = Router::new()
        .route("/v1/core/decide", post(decide))
        .route_layer(middleware::from_fn_with_state(
            credential(decision_token),
            authenticate,
        ));
    let control = Router::new()
        .route("/v1/core/target", get(target_state))
        .route("/v1/core/policies/activate", post(activate))
        .route_layer(middleware::from_fn_with_state(
            credential(publisher_token),
            authenticate,
        ));
    Ok(Router::new()
        .merge(decisions)
        .merge(control)
        .with_state(state)
        .layer(DefaultBodyLimit::max(MAX_BYTES)))
}

async fn authenticate(
    State(expected): State<Credential>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers().get_all(AUTHORIZATION);
    let mut values = headers.iter();
    let token = values
        .next()
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let allowed = if values.next().is_none() {
        token.filter(|t| t.len() <= 1024).is_some_and(|t| {
            let actual: [u8; 32] = Sha256::digest(t.as_bytes()).into();
            bool::from(actual.ct_eq(&expected.0))
        })
    } else {
        false
    };
    if !allowed {
        return ApiError::new(StatusCode::UNAUTHORIZED, "E_CORE_UNAUTHORIZED").into_response();
    }
    next.run(request).await
}

struct ApiError {
    status: StatusCode,
    code: String,
    diagnostic: Option<serde_json::Value>,
}
impl ApiError {
    fn new(status: StatusCode, code: &str) -> Self {
        Self {
            status,
            code: code.into(),
            diagnostic: None,
        }
    }
}
impl From<CoreError> for ApiError {
    fn from(error: CoreError) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: error.diagnostic.code.clone(),
            diagnostic: Some(json!(error.diagnostic)),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":self.code, "diagnostic":self.diagnostic})),
        )
            .into_response()
    }
}

impl Gate {
    fn prepare(&self, stored: &CoreSource) -> Result<Policy, ApiError> {
        let bundle = transfer::read_bundle(stored)?;
        let compatibility = self
            .contracts
            .check(&bundle.sources, &bundle.input_schema, None)?;
        if !self.approvals.iter().any(|approval| {
            approval.policy_sha256 == compatibility.policy_sha256
                && approval.context_sha256 == compatibility.context.sha256
                && approval.target_sha256 == compatibility.target.sha256
                && approval.cases_sha256 == self.cases_sha256
        }) {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "E_OPERATOR_APPROVAL_REQUIRED",
            ));
        }
        // This worker owns a synchronous test runtime. Never use caller cases,
        // report booleans or imported historical package evidence here.
        let (package, _) = package::prepare(&bundle.sources, &bundle.input_schema, &self.cases)?;
        if package.is_none() {
            // Do not disclose private operator test inputs/expected outputs.
            return Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "E_CORE_BEHAVIOR_REJECTED",
            ));
        }
        let engine = DecisionEngine::from_core(
            &bundle.sources,
            parse_core_input_schema(&bundle.input_schema)?,
        )
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "E_CORE_ENGINE"))?;
        Ok(Policy {
            engine,
            compatibility,
        })
    }
}

#[derive(Serialize)]
struct ActiveReceipt<'a> {
    revision: &'a str,
    policy_sha256: &'a str,
    target_id: &'a str,
    binding_sha256: &'a str,
    context_sha256: &'a str,
    target_sha256: &'a str,
    cases_sha256: &'a str,
    scope: &'static str,
    local_engine_constructed: bool,
    server_owned_cases_passed: bool,
    operator_allowlist_matched: bool,
    business_evaluation: &'static str,
}
fn receipt<'a>(active: &'a Active, gate: &'a Gate) -> ActiveReceipt<'a> {
    let report = &active.policy.compatibility;
    ActiveReceipt {
        revision: &active.revision,
        policy_sha256: &report.policy_sha256,
        target_id: &report.target.id,
        binding_sha256: &report.binding_sha256,
        context_sha256: &report.context.sha256,
        target_sha256: &report.target.sha256,
        cases_sha256: &gate.cases_sha256,
        scope: "local_operator_activation",
        local_engine_constructed: true,
        server_owned_cases_passed: true,
        operator_allowlist_matched: true,
        business_evaluation: "not_performed",
    }
}

async fn target_state(State(state): State<CoreState>) -> Json<serde_json::Value> {
    let active = state.active.read().await.clone();
    Json(json!(receipt(&active, &state.gate)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActivateRequest {
    expected_revision: String,
    bundle: transfer::SourceBundle,
}
async fn activate(
    State(state): State<CoreState>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let request: ActivateRequest = serde_json::from_slice(&body)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "E_CORE_REQUEST"))?;
    if state.active.read().await.revision != request.expected_revision {
        return Err(ApiError::new(StatusCode::CONFLICT, "E_ACTIVE_REVISION"));
    }
    let permit = state
        .preparation
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::new(StatusCode::TOO_MANY_REQUESTS, "E_ACTIVATION_BUSY"))?;
    let gate = state.gate.clone();
    let policy = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        gate.prepare(&CoreSource {
            path: "<candidate>".into(),
            yaml: serde_json::to_string(&request.bundle).expect("bundle JSON"),
        })
    })
    .await
    .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "E_CORE_WORKER"))??;
    let mut active = state.active.write().await;
    // Compare again after compilation/testing; never overwrite a newer snapshot.
    if active.revision != request.expected_revision {
        return Err(ApiError::new(StatusCode::CONFLICT, "E_ACTIVE_REVISION"));
    }
    *active = Arc::new(Active {
        revision: uuid::Uuid::new_v4().to_string(),
        policy,
    });
    Ok(Json(json!(receipt(&active, &state.gate))))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventRequest {
    event: HashMap<String, Value>,
    #[serde(default)]
    enable_trace: bool,
}
async fn decide(
    State(state): State<CoreState>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let event: EventRequest = serde_json::from_slice(&body)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "E_CORE_REQUEST"))?;
    // Keep the engine and identity from ONE snapshot; no lock during evaluation.
    let active = state.active.read().await.clone();
    let mut request = DecisionRequest::new(event.event);
    if event.enable_trace {
        request = request.with_trace();
    }
    let response = active
        .policy
        .engine
        .decide(request)
        .await
        .map_err(|_| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "E_CORE_DECISION"))?;
    Ok(Json(
        json!({"snapshot":receipt(&active, &state.gate), "decision":response}),
    ))
}
