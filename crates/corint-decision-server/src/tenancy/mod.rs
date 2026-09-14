//! Tenant-aware shared HTTP host. Pure rule execution remains tenant-independent.
mod auth;
mod config;
mod credentials;
mod decision;
pub(crate) mod queries;
mod quota;
pub mod store;
use crate::{
    core::{self, CoreConfig},
    journal::{Journal, JournalBackend},
    repo_source::BackendConfig,
};
use anyhow::{ensure, Result};
use auth::Registry;
pub use auth::{Permission, TenantContext};
use axum::{
    body::{to_bytes, Body},
    extract::{Path as RoutePath, Request, State},
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{any, get, post},
    Json, Router,
};
pub(crate) use config::confined;
use config::{read, ResourceBudget};
pub use config::{Boundary, Config, DeploymentConfig, Limits, ResourceBinding, Scope};
pub use credentials::CredentialAuthority;
use quota::Quota;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, LazyLock, Weak,
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tower::ServiceExt;
use tracing::Instrument;

struct Loaded {
    app: Router,
    _journal: Journal,
    _loaded: OwnedSemaphorePermit,
    _connections: Arc<config::ResourceLease>,
}
struct Runtime {
    management: Arc<Quota>,
    tenant_management: Arc<Quota>,
    config: DeploymentConfig,
    core: CoreConfig,
    boundary: Arc<Boundary>,
    quota: Arc<Quota>,
    tenant_quota: Arc<Quota>,
    loaded: Mutex<Option<Arc<Loaded>>>,
    last_used: std::sync::Mutex<Instant>,
    completed: AtomicU64,
    failed: AtomicU64,
    timed_out: AtomicU64,
}
struct Host {
    decision_bindings: BTreeMap<String, Scope>,
    request_readers: Semaphore,
    management: Arc<Quota>,
    accepting: AtomicBool,
    runtimes: BTreeMap<Scope, Arc<Runtime>>,
    credentials: Arc<CredentialAuthority>,
    store: store::Store,
    platform: Arc<Quota>,
    loaded_slots: Arc<Semaphore>,
}
static HOSTS: LazyLock<std::sync::Mutex<Vec<Weak<Host>>>> =
    LazyLock::new(|| std::sync::Mutex::new(Vec::new()));
fn management_quota(concurrency: u32) -> Arc<Quota> {
    Arc::new(Quota::new(Limits {
        max_inflight: concurrency,
        requests_per_second: 100,
        burst: 100,
        max_connections: 8,
    }))
}
/// Drain detached work as well as connected HTTP requests before process exit.
pub async fn drain(timeout: Duration) -> Result<()> {
    let hosts: Vec<_> = HOSTS
        .lock()
        .unwrap()
        .iter()
        .filter_map(Weak::upgrade)
        .collect();
    for host in &hosts {
        host.accepting.store(false, Ordering::Release);
        host.credentials.accepting.store(false, Ordering::Release);
    }
    tokio::time::timeout(timeout, async {
        while hosts.iter().any(|h| {
            h.platform.inflight() > 0 || h.management.inflight() > 0 || h.credentials.inflight() > 0
        }) {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("Tenant work did not drain before shutdown"))
}
fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error":code}))).into_response()
}

pub async fn load(path: &Path) -> Result<(SocketAddr, Router)> {
    let path = path.canonicalize()?;
    let config: Config = serde_json::from_str(&read(&path)?)?;
    let address = config.listen;
    let router = create_router(config, path.parent().unwrap()).await?;
    Ok((address, router))
}

pub async fn create_router(config: Config, root: &Path) -> Result<Router> {
    let root = root.canonicalize()?;
    ensure!(
        config.format_version == "1" && config.listen.ip().is_loopback(),
        "Tenant host requires v1 configuration and loopback TLS-edge listener"
    );
    ensure!(
        (1..=256).contains(&config.deployments.len())
            && (1..=256).contains(&config.max_loaded)
            && (1..=16).contains(&config.max_preparations),
        "Invalid tenant runtime capacity"
    );
    config.platform_limits.validate()?;
    let platform = Arc::new(Quota::new(config.platform_limits.clone()));
    let connection_budget = Arc::new(Semaphore::new(
        config.platform_limits.max_connections as usize,
    ));
    let preparations = Arc::new(Semaphore::new(config.max_preparations as usize));
    let mut tenants = BTreeMap::new();
    for (tenant, limits) in &config.tenant_limits {
        ensure!(config::identifier(tenant), "Invalid tenant limits identity");
        limits.validate()?;
        tenants.insert(
            tenant.clone(),
            (
                Arc::new(Quota::new(limits.clone())),
                Arc::new(Semaphore::new(limits.max_connections as usize)),
                management_quota(4),
            ),
        );
    }
    let mut runtimes = BTreeMap::new();
    let mut roots = Vec::new();
    let mut journals = BTreeSet::new();
    for mut deployment in config.deployments {
        deployment.scope.validate()?;
        deployment.limits.validate()?;
        ensure!(
            (100..=120_000).contains(&deployment.timeout_ms)
                && (1..=86400).contains(&deployment.idle_seconds),
            "Invalid runtime time limits"
        );
        let (tenant_quota, tenant_connections, tenant_management) = tenants
            .get(&deployment.scope.tenant_id)
            .ok_or_else(|| anyhow::anyhow!("Missing tenant limits"))?;
        deployment.root = confined(&root, &deployment.root)?;
        ensure!(
            deployment.root.is_dir() && deployment.root != root,
            "A deployment requires its own directory"
        );
        roots.push(deployment.root.clone());
        let mut core: CoreConfig = serde_json::from_str(&read(&confined(
            &deployment.root,
            &deployment.core_config,
        )?)?)?;
        for path in [&mut core.context, &mut core.target, &mut core.cases] {
            *path = confined(&deployment.root, path)?;
        }
        if let Some(path) = &mut core.feature_pipeline {
            *path = confined(&deployment.root, path)?;
        }
        if let Some(e) = &mut core.business_evidence {
            for path in [&mut e.evaluation, &mut e.approval, &mut e.trust] {
                *path = confined(&deployment.root, path)?;
            }
        }
        match &mut core.repository_backend {
            None => {
                core.repository = confined(&deployment.root, &core.repository)?;
            }
            Some(BackendConfig::Sqlite { path }) => {
                *path = confined(&root, &deployment.root.join(&*path))?;
            }
            Some(BackendConfig::Postgres { .. }) => {}
            _ => anyhow::bail!("Tenant repositories require filesystem or scoped SQL"),
        }
        let journal = core
            .journal
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Tenant deployment requires a reliable journal"))?;
        ensure!(
            core.config_version == "3"
                && !journal.best_effort
                && journal.tenant_id == deployment.scope.tenant_id,
            "Tenant Core requires v3, reliable persistence and matching tenant"
        );
        let identity = match &journal.backend {
            JournalBackend::Sqlite {} => {
                let path = deployment.root.join(&journal.path);
                ensure!(
                    !journal.path.as_os_str().is_empty()
                        && path
                            .parent()
                            .unwrap()
                            .canonicalize()?
                            .starts_with(&deployment.root),
                    "Journal escapes deployment root"
                );
                journal.path = path;
                format!("sqlite:{}", journal.path.display())
            }
            JournalBackend::Postgres { url_env, schema } => {
                // Hash the resolved endpoint, so aliases of an environment variable
                // cannot silently share a schema. Persistent scope metadata is the final guard.
                let url = std::env::var(url_env)
                    .map_err(|_| anyhow::anyhow!("Missing journal credential"))?;
                corint_decision_engine::decision_host::canonical_sha256(&(url, schema))
            }
        };
        ensure!(
            journals.insert(identity),
            "Deployments cannot share a journal file/schema"
        );
        let boundary = Arc::new(Boundary {
            scope: deployment.scope.clone(),
            resources: deployment.resources.clone(),
            preparations: preparations.clone(),
            connections: ResourceBudget(vec![
                connection_budget.clone(),
                tenant_connections.clone(),
                Arc::new(Semaphore::new(deployment.limits.max_connections as usize)),
            ]),
        });
        // Validate grants and audience before any request can load an execution host.
        let features = core
            .feature_pipeline
            .as_ref()
            .map(|p| read(p).and_then(|s| Ok(serde_json::from_str(&s)?)))
            .transpose()?;
        boundary.authorize(features.as_ref())?;
        ensure!(!core.approvals.is_empty() && core.approvals.iter().all(|a| a.tenant_scope.as_ref() == Some(&deployment.scope) && a.resource_scope_sha256.as_deref() == Some(boundary.fingerprint().as_str())), "Operator approvals require this tenant/environment/deployment and resource scope fingerprint");
        let scope = deployment.scope.clone();
        let runtime = Arc::new(Runtime {
            management: management_quota(2),
            tenant_management: tenant_management.clone(),
            quota: Arc::new(Quota::new(deployment.limits.clone())),
            tenant_quota: tenant_quota.clone(),
            config: deployment,
            core,
            boundary,
            loaded: Mutex::new(None),
            last_used: std::sync::Mutex::new(Instant::now()),
            completed: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            timed_out: AtomicU64::new(0),
        });
        ensure!(
            runtimes.insert(scope, runtime).is_none(),
            "Duplicate tenant deployment"
        );
    }
    config::validate_roots(&roots)?;
    // Resolve all public tenant routes from operator configuration at startup.
    // Credentials authorize a target; they never choose a different environment.
    let decision_bindings = decision::bindings(&runtimes, config.decision_bindings)?;
    let scopes = runtimes.keys().cloned().collect();
    if let store::StoreConfig::Sqlite { path } = &config.control_store {
        let path = root.join(path);
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid control store path"))?
            .canonicalize()?;
        ensure!(
            !roots.iter().any(|r| parent.starts_with(r)),
            "Platform control store must be outside tenant roots"
        );
    }
    let store = store::Store::open(&root, &config.control_store, &scopes).await?;
    // The file is only an initial seed. Restarts and reloads use persisted hashes.
    if store.credentials().await?.is_none() {
        let path = config.credentials.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Initial credential seed required for an empty control store")
        })?;
        let credentials = confined(&root, path)?;
        ensure!(
            !roots.iter().any(|r| credentials.starts_with(r)),
            "Platform credentials must be outside tenant roots"
        );
        let registry = Registry::load(&credentials, &scopes)?;
        store.bootstrap_credentials(&registry).await?;
    }
    let credentials = CredentialAuthority::from_store(store.clone(), scopes).await?;
    let host = Arc::new(Host {
        decision_bindings,
        request_readers: Semaphore::new(config.platform_limits.max_inflight as usize),
        management: management_quota(16),
        accepting: AtomicBool::new(true),
        runtimes,
        credentials: credentials.clone(),
        store,
        platform,
        loaded_slots: Arc::new(Semaphore::new(config.max_loaded as usize)),
    });
    {
        let mut hosts = HOSTS.lock().unwrap();
        hosts.retain(|h| h.strong_count() > 0);
        hosts.push(Arc::downgrade(&host));
    }
    let weak = Arc::downgrade(&host);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let Some(host) = weak.upgrade() else {
                break;
            };
            host.evict_idle().await;
        }
    });
    Ok(Router::new()
        .route("/v1/decide", post(decision::decide))
        .route("/v1/tenancy/deployments", get(inventory))
        .route(
            "/v1/tenants/:tenant/environments/:environment/deployments/:deployment/*path",
            any(dispatch),
        )
        .with_state(host)
        .merge(credentials.router()))
}

impl Host {
    async fn evict_idle(&self) {
        for runtime in self.runtimes.values() {
            if runtime.quota.inflight() == 0
                && runtime.management.inflight() == 0
                && runtime.last_used.lock().unwrap().elapsed()
                    >= Duration::from_secs(runtime.config.idle_seconds)
            {
                if let Ok(mut loaded) = runtime.loaded.try_lock() {
                    loaded.take();
                }
            }
        }
    }
    async fn loaded(&self, runtime: &Runtime) -> Result<Arc<Loaded>> {
        let mut loaded = runtime.loaded.lock().await;
        if let Some(loaded) = &*loaded {
            return Ok(loaded.clone());
        }
        self.evict_idle().await;
        let slot = self
            .loaded_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("E_LOADED_CAPACITY"))?;
        let connections = runtime.boundary.connections.acquire(
            match runtime.core.journal.as_ref().unwrap().backend {
                JournalBackend::Sqlite {} => 1,
                _ => 8,
            },
        )?;
        let journal = Journal::open_scoped(
            &runtime.config.root,
            runtime.core.journal.as_ref().unwrap(),
            &runtime.config.scope,
        )
        .await?;
        let app = core::create_tenant_router(
            runtime.core.clone(),
            &runtime.config.root,
            runtime.boundary.clone(),
            journal.clone(),
        )
        .await?;
        let value = Arc::new(Loaded {
            app,
            _journal: journal,
            _loaded: slot,
            _connections: connections,
        });
        *loaded = Some(value.clone());
        Ok(value)
    }
}

fn permission(method: &Method, path: &str) -> Option<Permission> {
    match (method.as_str(), path) {
        ("POST", "v1/core/decide") => Some(Permission::Decide),
        (
            "GET",
            "v1/core/target"
            | "v1/core/persistence"
            | "v1/core/metrics"
            | "runtime"
            | "runtime/audit",
        ) => Some(Permission::Inspect),
        ("POST", "v1/core/repo/reload") => Some(Permission::Publish),
        ("POST", "v1/core/outbox/claim" | "v1/core/outbox/ack") => Some(Permission::Consume),
        ("POST", "runtime" | "runtime/unload") => Some(Permission::Manage),
        _ => None,
    }
}
async fn dispatch(
    State(host): State<Arc<Host>>,
    RoutePath((tenant, environment, deployment, path)): RoutePath<(String, String, String, String)>,
    mut request: Request,
) -> Response {
    let path = match path.as_str() {
        "decide" => "v1/core/decide",
        "target" => "v1/core/target",
        "metrics" => "v1/core/metrics",
        "persistence" => "v1/core/persistence",
        "repo/reload" => "v1/core/repo/reload",
        "outbox/claim" => "v1/core/outbox/claim",
        "outbox/ack" => "v1/core/outbox/ack",
        other => other,
    }
    .to_owned();
    let Some(identity) = host
        .credentials
        .registry
        .read()
        .await
        .authenticate(request.headers())
    else {
        return error(StatusCode::UNAUTHORIZED, "E_TENANT_UNAUTHORIZED");
    };
    let scope = Scope {
        tenant_id: tenant,
        environment,
        deployment,
    };
    let Some(permission) = permission(request.method(), &path) else {
        return error(StatusCode::NOT_FOUND, "E_TENANT_ROUTE");
    };
    let Some(context) = identity.context(&scope, permission) else {
        return error(StatusCode::FORBIDDEN, "E_TENANT_FORBIDDEN");
    };
    let Some(runtime) = host.runtimes.get(&scope).cloned() else {
        return error(StatusCode::FORBIDDEN, "E_TENANT_FORBIDDEN");
    };
    // Reject ambiguous/encoded route variants instead of forwarding query/header scope.
    if request.uri().query().is_some() {
        return error(StatusCode::BAD_REQUEST, "E_TENANT_QUERY");
    }
    let mut permits = Vec::new();
    if !host.accepting.load(Ordering::Acquire) {
        return error(StatusCode::SERVICE_UNAVAILABLE, "E_TENANT_DRAINING");
    }
    let quotas = if permission == Permission::Decide {
        [&runtime.quota, &runtime.tenant_quota, &host.platform]
    } else {
        [
            &runtime.management,
            &runtime.tenant_management,
            &host.management,
        ]
    };
    for quota in quotas {
        let Some(permit) = quota.admit() else {
            return error(StatusCode::TOO_MANY_REQUESTS, "E_TENANT_QUOTA");
        };
        permits.push(permit);
    }
    request.extensions_mut().insert(context.clone());
    request
        .headers_mut()
        .remove(axum::http::header::AUTHORIZATION);
    *request.uri_mut() = format!("/{path}")
        .parse::<Uri>()
        .expect("known internal route");
    let timeout = Duration::from_millis(runtime.config.timeout_ms);
    let observed = runtime.clone();
    let request_id = context.request_id().to_owned();
    let span = tracing::info_span!("tenant_request", tenant_id=%scope.tenant_id, environment=%scope.environment,
        deployment=%scope.deployment, actor=%identity.id(), request_id=%request_id);
    // A client disconnect/timeout must not release admission while work survives.
    // The task retains permits and the frozen router through durable completion.
    let task = tokio::spawn(async move {
        let _permits = permits;
        let result = execute(&host, &runtime, &path, &context, request).await;
        let response = match result {
            Ok(r) => r,
            Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "E_TENANT_UNAVAILABLE"),
        };
        runtime.completed.fetch_add(1, Ordering::Relaxed);
        if response.status().is_client_error() || response.status().is_server_error() {
            runtime.failed.fetch_add(1, Ordering::Relaxed);
        }
        *runtime.last_used.lock().unwrap() = Instant::now();
        tracing::info!(tenant_id=%scope.tenant_id, environment=%scope.environment, deployment=%scope.deployment,
            actor=%identity.id(), operation=%path, status=response.status().as_u16(), "Tenant operation completed");
        response
    }.instrument(span));
    let mut response = match tokio::time::timeout(timeout, task).await {
        Ok(Ok(response)) => response,
        Ok(Err(_)) => error(StatusCode::INTERNAL_SERVER_ERROR, "E_TENANT_WORKER"),
        Err(_) => {
            observed.timed_out.fetch_add(1, Ordering::Relaxed);
            error(StatusCode::GATEWAY_TIMEOUT, "E_TENANT_TIMEOUT")
        }
    };
    response.headers_mut().insert(
        "x-corint-request-id",
        request_id.parse().expect("server UUID"),
    );
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeState {
    expected_revision: i64,
    paused: bool,
}
async fn execute(
    host: &Host,
    runtime: &Runtime,
    path: &str,
    context: &TenantContext,
    request: Request,
) -> Result<Response> {
    let scope = &runtime.config.scope;
    let (parts, body) = request.into_parts();
    let body = match tokio::time::timeout(
        Duration::from_millis(runtime.config.timeout_ms.min(10_000)),
        to_bytes(body, 8 * 1024 * 1024),
    )
    .await
    {
        Ok(Ok(body)) => body,
        Ok(Err(_)) => return Ok(error(StatusCode::PAYLOAD_TOO_LARGE, "E_TENANT_BODY")),
        Err(_) => return Ok(error(StatusCode::REQUEST_TIMEOUT, "E_TENANT_BODY_TIMEOUT")),
    };
    let request = Request::from_parts(parts, Body::from(body));
    if path == "runtime" && request.method() == Method::POST {
        let body = to_bytes(request.into_body(), 4096).await?;
        let change: ChangeState = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => return Ok(error(StatusCode::BAD_REQUEST, "E_TENANT_STATE")),
        };
        if !host
            .store
            .set_paused(
                scope,
                change.expected_revision,
                change.paused,
                &context.audit(),
            )
            .await?
        {
            return Ok(error(StatusCode::CONFLICT, "E_TENANT_REVISION"));
        }
        return Ok(Json(json!({"scope":scope,"state":host.store.state(scope).await?,"inflight":runtime.quota.inflight() + runtime.management.inflight().saturating_sub(1)})).into_response());
    }
    if path == "runtime/audit" {
        return Ok(
            Json(json!({"scope":scope,"events":host.store.audit(scope).await?})).into_response(),
        );
    }
    let state = host.store.state(scope).await?;
    if path == "runtime/unload" {
        if !state.paused {
            return Ok(error(StatusCode::CONFLICT, "E_TENANT_PAUSE_REQUIRED"));
        }
        if runtime.quota.inflight() > 0 || runtime.management.inflight() > 1 {
            return Ok(error(StatusCode::CONFLICT, "E_TENANT_DRAINING"));
        }
        runtime.loaded.lock().await.take();
        return Ok(Json(json!({"scope":scope,"unloaded":true,"inflight":runtime.quota.inflight() + runtime.management.inflight().saturating_sub(1)})).into_response());
    }
    if path == "runtime" {
        return Ok(Json(json!({"scope":scope,"state":state,"loaded":runtime.loaded.lock().await.is_some(),"quota":runtime.quota.snapshot(),
            "tenant_quota":runtime.tenant_quota.snapshot(),"management_quota":runtime.management.snapshot(),"completed":runtime.completed.load(Ordering::Relaxed),"failed":runtime.failed.load(Ordering::Relaxed),"timed_out":runtime.timed_out.load(Ordering::Relaxed)})).into_response());
    }
    // Recovery/export must remain usable while policy compilation or features fail.
    if matches!(
        path,
        "v1/core/outbox/claim" | "v1/core/outbox/ack" | "v1/core/persistence"
    ) {
        let cached = runtime.loaded.lock().await.clone();
        let mut _connections = None;
        let journal = if let Some(loaded) = &cached {
            loaded._journal.clone()
        } else {
            _connections = Some(runtime.boundary.connections.acquire(
                match runtime.core.journal.as_ref().unwrap().backend {
                    JournalBackend::Sqlite {} => 1,
                    _ => 8,
                },
            )?);
            Journal::open_scoped(
                &runtime.config.root,
                runtime.core.journal.as_ref().unwrap(),
                scope,
            )
            .await?
        };
        let value = match path {
            "v1/core/persistence" => journal.status().await?,
            "v1/core/outbox/claim" => {
                journal
                    .claim_authorized(
                        chrono::Utc::now().timestamp_millis(),
                        context.permits(scope, Permission::Export),
                    )
                    .await?
            }
            _ => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Ack {
                    lease: String,
                    idempotency_keys: Vec<String>,
                }
                let body = to_bytes(request.into_body(), 64 * 1024).await?;
                let ack: Ack = match serde_json::from_slice(&body) {
                    Ok(v) => v,
                    Err(_) => return Ok(error(StatusCode::BAD_REQUEST, "E_TENANT_ACK")),
                };
                if journal
                    .acknowledge(
                        &ack.lease,
                        &ack.idempotency_keys,
                        chrono::Utc::now().timestamp_millis(),
                    )
                    .await
                    .is_err()
                {
                    return Ok(error(StatusCode::CONFLICT, "E_OUTBOX_LEASE"));
                }
                json!({"acknowledged":true})
            }
        };
        return Ok(Json(value).into_response());
    }
    if state.paused && matches!(path, "v1/core/decide" | "v1/core/repo/reload") {
        return Ok(error(StatusCode::SERVICE_UNAVAILABLE, "E_TENANT_PAUSED"));
    }
    let loaded = host.loaded(runtime).await?;
    // Loading/compilation may take time; a pause committed in the meantime wins.
    if matches!(path, "v1/core/decide" | "v1/core/repo/reload")
        && host.store.state(scope).await?.paused
    {
        return Ok(error(StatusCode::SERVICE_UNAVAILABLE, "E_TENANT_PAUSED"));
    }
    Ok(loaded
        .app
        .clone()
        .oneshot(request)
        .await
        .expect("infallible router"))
}

async fn inventory(State(host): State<Arc<Host>>, request: Request) -> Response {
    let Some(identity) = host
        .credentials
        .registry
        .read()
        .await
        .authenticate(request.headers())
    else {
        return error(StatusCode::UNAUTHORIZED, "E_TENANT_UNAUTHORIZED");
    };
    let Some(_permit) = host.management.admit() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "E_TENANT_QUOTA");
    };
    let scopes: Vec<_> = host
        .runtimes
        .keys()
        .filter(|s| identity.admin() || identity.context(s, Permission::Inspect).is_some())
        .collect();
    Json(json!({"deployments":scopes})).into_response()
}
