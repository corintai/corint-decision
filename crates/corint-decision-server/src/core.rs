//! Opt-in, single-target Core server. Local operator configuration is the trust
//! root. Policies come only from the configured repository, including on reload.
use crate::{
    evidence::{self, EvidenceConfig},
    journal::{Journal, JournalConfig},
};
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
use corint_decision_engine::{
    decision_host::{DecisionHost, FeatureHostConfig},
    Value,
};
use corint_decision_toolchain::{
    behavior,
    contracts::{CompatibilityReport, TargetContracts},
    package,
    repository::RepositoryIdentity,
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
    #[serde(default = "metrics_enabled_by_default")]
    pub enable_metrics: bool,
    /// Operator-owned JSON FeatureHostConfig, re-read for each candidate reload.
    #[serde(default)]
    pub feature_pipeline: Option<PathBuf>,
    pub listen: SocketAddr,
    pub context: PathBuf,
    pub target: PathBuf,
    pub cases: PathBuf,
    /// Filesystem repository containing published.json and the policy YAML files.
    #[serde(default)]
    pub repository: PathBuf,
    #[serde(default)]
    pub repository_backend: Option<crate::repo_source::BackendConfig>,
    pub decision_token_env: String,
    pub publisher_token_env: String,
    pub approvals: Vec<OperatorApproval>,
    #[serde(default)]
    pub journal: Option<JournalConfig>,
    #[serde(default)]
    pub business_evidence: Option<EvidenceConfig>,
}

/// An explicit local operator allowlist, NOT a portable approval or signature.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorApproval {
    pub policy_sha256: String,
    pub context_sha256: String,
    pub target_sha256: String,
    pub cases_sha256: String,
    #[serde(default)]
    pub feature_binding_sha256: Option<String>,
}

struct Policy {
    host: DecisionHost,
    compatibility: CompatibilityReport,
    repository: RepositoryIdentity,
    subject: serde_json::Value,
}
struct Active {
    revision: String,
    policy: Policy,
}
struct Gate {
    enable_metrics: bool,
    feature_pipeline: Option<PathBuf>,
    runtime: tokio::runtime::Handle,
    root: PathBuf,
    business_evidence: Option<EvidenceConfig>,
    repository: crate::repo_source::Source,
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
    journal: Option<Journal>,
}
#[derive(Clone)]
struct Credential([u8; 32]);

fn metrics_enabled_by_default() -> bool {
    true
}

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
        matches!(config.config_version.as_str(), "2" | "3"),
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
        anyhow::ensure!(
            approval
                .feature_binding_sha256
                .as_ref()
                .is_none_or(|value| is_hash(value)),
            "Invalid feature approval fingerprint"
        );
    }
    anyhow::ensure!(
        config.config_version != "3" || config.journal.is_some(),
        "Core v3 requires a durable journal"
    );
    let (journal, consumer) = if let Some(journal) = &config.journal {
        let consumer = if journal.consumer_token_env.is_empty() {
            None
        } else {
            let token = std::env::var(&journal.consumer_token_env)
                .map_err(|_| anyhow::anyhow!("Missing journal consumer credential"))?;
            anyhow::ensure!(
                (32..=1024).contains(&token.len())
                    && token.bytes().all(|b| b.is_ascii_graphic())
                    && token != decision_token
                    && token != publisher_token,
                "Consumer requires an independent credential"
            );
            Some(Credential(Sha256::digest(token.as_bytes()).into()))
        };
        (Some(Journal::open(root, journal).await?), consumer)
    } else {
        (None, None)
    };
    let context = read(&root.join(&config.context))?;
    let target = read(&root.join(&config.target))?;
    let cases = read(&root.join(&config.cases))?;
    behavior::validate_suite(&cases)?;
    let gate = Arc::new(Gate {
        enable_metrics: config.enable_metrics,
        feature_pipeline: config.feature_pipeline,
        runtime: tokio::runtime::Handle::current(),
        root: root.to_owned(),
        business_evidence: config.business_evidence,
        repository: crate::repo_source::Source::configure(
            root,
            &config.repository,
            config.repository_backend,
        )?,
        contracts: TargetContracts::load(&context, &target)?,
        cases_sha256: hash(cases.yaml.as_bytes()),
        cases,
        approvals: config.approvals,
    });
    let worker_gate = gate.clone();
    let policy = tokio::task::spawn_blocking(move || worker_gate.prepare())
        .await?
        .map_err(|e| anyhow::anyhow!("Core initial policy rejected: {}", e.code))?;
    let state = CoreState {
        journal,
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
        .route("/v1/core/persistence", get(persistence_status))
        .route("/v1/core/metrics", get(metrics))
        .route("/v1/core/repo/reload", post(reload))
        .route_layer(middleware::from_fn_with_state(
            credential(publisher_token),
            authenticate,
        ));
    let outbox = if let Some(credential) = consumer {
        Router::new()
            .route("/v1/core/outbox/claim", post(outbox_claim))
            .route("/v1/core/outbox/ack", post(outbox_ack))
            .route_layer(middleware::from_fn_with_state(credential, authenticate))
    } else {
        Router::new()
    };
    Ok(Router::new()
        .merge(outbox)
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
    fn features(&self) -> Result<Option<FeatureHostConfig>, ApiError> {
        self.feature_pipeline
            .as_ref()
            .map(|path| {
                read(&self.root.join(path))
                    .and_then(|source| Ok(serde_json::from_str(&source.yaml)?))
                    .map_err(|_| {
                        ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "E_FEATURE_CONFIG")
                    })
            })
            .transpose()
    }
    fn check_evidence(
        &self,
        subject: &serde_json::Value,
        resources: &[serde_json::Value],
    ) -> Result<(), ApiError> {
        if let Some(config) = &self.business_evidence {
            evidence::check_with_features(
                &self.root,
                config,
                subject,
                resources,
                chrono::Utc::now().timestamp_millis() as u64,
            )
            .map_err(|_| ApiError::new(StatusCode::FORBIDDEN, "E_PUBLICATION_EVIDENCE"))?;
        }
        Ok(())
    }
    fn prepare(&self) -> Result<Policy, ApiError> {
        let snapshot = match &self.repository {
            crate::repo_source::Source::Filesystem(path) => {
                corint_decision_toolchain::repository::load(path)?
            }
            _ => self.repository.load().map_err(|_| {
                ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "E_REPOSITORY_LOAD")
            })?,
        };
        let bundle = snapshot.closure.bundle();
        let compatibility = self
            .contracts
            .check(&bundle.sources, &bundle.input_schema, None)?;
        let features = self.features()?;
        let feature_binding = features.as_ref().map(FeatureHostConfig::binding_sha256);
        if !self.approvals.iter().any(|approval| {
            approval.policy_sha256 == compatibility.policy_sha256
                && approval.context_sha256 == compatibility.context.sha256
                && approval.target_sha256 == compatibility.target.sha256
                && approval.cases_sha256 == self.cases_sha256
                && approval.feature_binding_sha256 == feature_binding
        }) {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "E_OPERATOR_APPROVAL_REQUIRED",
            ));
        }
        let subject =
            evidence::subject_with_features(&compatibility, feature_binding.as_deref())
                .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "E_CORE_SUBJECT"))?;
        self.check_evidence(
            &subject,
            &features
                .as_ref()
                .map(FeatureHostConfig::resources)
                .unwrap_or_default(),
        )?;
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
        let host = self
            .runtime
            .block_on(DecisionHost::new(
                &bundle.sources,
                parse_core_input_schema(&bundle.input_schema)?,
                features,
                self.enable_metrics,
            ))
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "E_CORE_ENGINE"))?;
        self.repository
            .verify(&snapshot.identity)
            .map_err(|_| ApiError::new(StatusCode::CONFLICT, "E_REPOSITORY_CHANGED"))?;
        if self
            .features()?
            .as_ref()
            .map(FeatureHostConfig::binding_sha256)
            != feature_binding
        {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "E_FEATURE_CONFIG_CHANGED",
            ));
        }
        Ok(Policy {
            host,
            compatibility,
            repository: snapshot.identity,
            subject,
        })
    }
}

#[derive(Serialize)]
struct ActiveReceipt<'a> {
    revision: &'a str,
    subject: &'a serde_json::Value,
    policy_sha256: &'a str,
    target_id: &'a str,
    binding_sha256: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    feature_binding_sha256: Option<&'a str>,
    context_sha256: &'a str,
    target_sha256: &'a str,
    cases_sha256: &'a str,
    repository: &'a RepositoryIdentity,
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
        subject: &active.policy.subject,
        policy_sha256: &report.policy_sha256,
        target_id: &report.target.id,
        binding_sha256: &report.binding_sha256,
        feature_binding_sha256: active.policy.host.feature_binding_sha256(),
        context_sha256: &report.context.sha256,
        target_sha256: &report.target.sha256,
        cases_sha256: &gate.cases_sha256,
        repository: &active.policy.repository,
        scope: "local_repository_reload",
        local_engine_constructed: true,
        server_owned_cases_passed: true,
        operator_allowlist_matched: true,
        business_evaluation: if gate.business_evidence.is_some() {
            "verified_operator_attestation"
        } else {
            "not_performed"
        },
    }
}

async fn target_state(State(state): State<CoreState>) -> Json<serde_json::Value> {
    let active = state.active.read().await.clone();
    Json(json!(receipt(&active, &state.gate)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReloadRequest {
    expected_revision: String,
}
async fn reload(
    State(state): State<CoreState>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let request: ReloadRequest = serde_json::from_slice(&body)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "E_CORE_REQUEST"))?;
    if state.active.read().await.revision != request.expected_revision {
        return Err(ApiError::new(StatusCode::CONFLICT, "E_ACTIVE_REVISION"));
    }
    let permit = state
        .preparation
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::new(StatusCode::TOO_MANY_REQUESTS, "E_RELOAD_BUSY"))?;
    let gate = state.gate.clone();
    // The worker owns the permit even if the HTTP future is cancelled. On
    // success, transfer it back so preparation and snapshot commit share a slot.
    let (policy, _permit) =
        tokio::task::spawn_blocking(move || gate.prepare().map(|policy| (policy, permit)))
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
    #[serde(default)]
    business_event_id: Option<String>,
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
    if state.journal.is_some()
        && !event
            .business_event_id
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty() && s.len() <= 256)
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "E_BUSINESS_EVENT_ID",
        ));
    }
    // Re-read trusted evidence so expiry/revocation also stops new decisions.
    let gate = state.gate.clone();
    let subject = active.policy.subject.clone();
    let resources = active.policy.host.required_resources().to_vec();
    tokio::task::spawn_blocking(move || gate.check_evidence(&subject, &resources))
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "E_CORE_WORKER"))??;
    let now = chrono::Utc::now().timestamp_millis();
    let started = std::time::Instant::now();
    let execution = active
        .policy
        .host
        .decide(event.event, now.div_euclid(1000), event.enable_trace)
        .await;
    let input = execution.input_evidence;
    let result = execution.result;
    let record = if let Some(journal) = &state.journal {
        let id = uuid::Uuid::new_v4().to_string();
        let (signal, pipeline, rules, reasons, actions, error) = match &result {
            Ok(response) => (
                if response.pipeline_id.is_none() { "no_match".into() } else { response.result.signal.as_ref().map(|s| format!("{s:?}").to_lowercase()).unwrap_or_else(|| "pass".into()) },
                response.pipeline_id.clone(), response.result.triggered_rules.clone(),
                if response.result.explanation.trim().is_empty() { vec![] } else { vec![response.result.explanation.clone()] },
                response.result.actions.iter().enumerate().map(|(i,a)| json!({"action_id":format!("{id}:{i}"),"idempotency_key":format!("{}:{id}:{i}",journal.tenant_id),"type":a})).collect::<Vec<_>>(), None),
            Err(corint_decision_engine::EngineError::Core(error)) => ("error".to_owned(), None, vec![], vec![], vec![], Some(error.diagnostic.code.as_str())),
            Err(_) => ("error".to_owned(), None, vec![], vec![], vec![], Some("E_CORE_DECISION")),
        };
        let record = json!({"kind":"corint-decision-record","contract_version":"1","id":id,"revision":"1",
            "provenance":{"producer":"corint-core","reference":active.revision},
            "tenant_id":journal.tenant_id,"decision_id":id,"business_event_id":event.business_event_id,
            "decided_at_ms":now,"subject":active.policy.subject,"engine_version":corint_decision_engine::ENGINE_VERSION,
            "input_evidence":{"reference":format!("journal:{id}"),"sha256":hash(input.to_string().as_bytes())},
            "runtime":{"revision":active.revision,"repository_revision":active.policy.repository.revision,
                "repository_manifest_sha256":active.policy.repository.manifest_sha256,"pipeline_id":pipeline},
            "resources":execution.resources,"triggered_rules":rules,"reasons":reasons,"result":signal,"error_code":error,
            "duration_ms":started.elapsed().as_millis() as u64,"actions":actions});
        journal
            .enqueue(record.clone(), input.clone())
            .map_err(|_| {
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "E_PERSISTENCE_QUEUE_UNAVAILABLE",
                )
            })?;
        Some(record)
    } else {
        None
    };
    let persistence = if record.is_some() {
        "queued"
    } else {
        "disabled"
    };
    match result {
        Ok(response) => Ok(Json(
            json!({"snapshot":receipt(&active, &state.gate),"decision":response,"record":record,"persistence":persistence,"feature_evidence":execution.feature_evidence}),
        )),
        Err(error) => Err(ApiError {
            status: if matches!(&error, corint_decision_engine::EngineError::Core(error) if error.diagnostic.code == "E_HOST_BUSY")
            {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            },
            code: "E_CORE_DECISION".into(),
            diagnostic: Some(
                json!({"record":record,"persistence":persistence,"snapshot":receipt(&active,&state.gate),"feature_evidence":execution.feature_evidence,"cause":match error { corint_decision_engine::EngineError::Core(error) => Some(error.diagnostic), _ => None }}),
            ),
        }),
    }
}

fn journal(state: &CoreState) -> Result<&Journal, ApiError> {
    state
        .journal
        .as_ref()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "E_JOURNAL_DISABLED"))
}
async fn persistence_status(
    State(state): State<CoreState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!(journal(&state)?.persistence_status())))
}

async fn metrics(State(state): State<CoreState>) -> Json<serde_json::Value> {
    let active = state.active.read().await.clone();
    Json(json!({"revision":active.revision,"metrics":active.policy.host.metrics().snapshot()}))
}
async fn outbox_claim(State(state): State<CoreState>) -> Result<Json<serde_json::Value>, ApiError> {
    journal(&state)?
        .claim(chrono::Utc::now().timestamp_millis())
        .await
        .map(Json)
        .map_err(|_| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "E_JOURNAL_UNAVAILABLE"))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ack {
    lease: String,
    idempotency_keys: Vec<String>,
}
async fn outbox_ack(
    State(state): State<CoreState>,
    Json(ack): Json<Ack>,
) -> Result<Json<serde_json::Value>, ApiError> {
    journal(&state)?
        .acknowledge(
            &ack.lease,
            &ack.idempotency_keys,
            chrono::Utc::now().timestamp_millis(),
        )
        .await
        .map_err(|_| ApiError::new(StatusCode::CONFLICT, "E_OUTBOX_LEASE"))?;
    Ok(Json(json!({"acknowledged":true})))
}
