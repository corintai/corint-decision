//! Platform credential lifecycle. Database commits precede local cache activation.
use super::{
    auth::{Change, Registry},
    error, management_quota,
    quota::Quota,
    store::Store,
    Scope,
};
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::sync::{Mutex, RwLock};

pub(super) const REFRESH_INTERVAL: Duration = Duration::from_secs(60);
pub(super) const MAX_CACHE_AGE: Duration = Duration::from_secs(120);

/// Shared database-backed credentials for tenant HTTP and compatibility HTTP/gRPC.
pub struct CredentialAuthority {
    pub(super) registry: RwLock<Registry>,
    store: Store,
    scopes: BTreeSet<Scope>,
    credential_reload: Arc<Mutex<()>>,
    management: Arc<Quota>,
    pub(super) accepting: AtomicBool,
}
impl CredentialAuthority {
    pub(crate) async fn from_store(
        store: Store,
        scopes: BTreeSet<Scope>,
    ) -> anyhow::Result<Arc<Self>> {
        let (_, registry) = store
            .credentials()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Missing credentials"))?;
        let authority = Arc::new(Self {
            registry: RwLock::new(registry),
            store,
            scopes,
            credential_reload: Arc::new(Mutex::new(())),
            management: management_quota(16),
            accepting: AtomicBool::new(true),
        });
        start_refresh(&authority);
        Ok(authority)
    }
    pub async fn open(
        root: &std::path::Path,
        config: &super::store::StoreConfig,
        scopes: BTreeSet<Scope>,
        seed: Option<&std::path::Path>,
    ) -> anyhow::Result<Arc<Self>> {
        let store = Store::open(root, config, &scopes).await?;
        if store.credentials().await?.is_none() {
            let path = seed.ok_or_else(|| anyhow::anyhow!("Initial credential seed required"))?;
            let path = super::confined(root, path)?;
            let registry = Registry::load(&path, &scopes)?;
            store.bootstrap_credentials(&registry).await?;
        }
        Self::from_store(store, scopes).await
    }
    pub async fn permits(
        &self,
        authorization: Option<&str>,
        scope: &Scope,
        permission: super::Permission,
    ) -> bool {
        let Some(value) = authorization.and_then(|s| s.parse::<axum::http::HeaderValue>().ok())
        else {
            return false;
        };
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(axum::http::header::AUTHORIZATION, value);
        self.registry
            .read()
            .await
            .authenticate(&headers)
            .and_then(|i| i.context(scope, permission))
            .is_some()
    }
    pub(super) fn inflight(&self) -> usize {
        self.management.inflight()
    }
    pub async fn drain(&self, timeout: Duration) -> anyhow::Result<()> {
        self.accepting.store(false, Ordering::Release);
        tokio::time::timeout(timeout, async {
            while self.management.inflight() > 0 {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("Credential operations did not drain"))
    }
    pub fn router(self: &Arc<Self>) -> axum::Router {
        axum::Router::new()
            .route("/v1/tenancy/credentials", axum::routing::post(change))
            .route(
                "/v1/tenancy/credentials/reload",
                axum::routing::post(reload_credentials),
            )
            .with_state(self.clone())
    }
}

pub(super) async fn refresh(host: &CredentialAuthority) -> anyhow::Result<()> {
    let (_, registry) = host
        .store
        .credentials()
        .await?
        .ok_or_else(|| anyhow::anyhow!("Missing credentials"))?;
    *host.registry.write().await = registry;
    Ok(())
}

pub(super) fn start_refresh(host: &Arc<CredentialAuthority>) {
    let weak = Arc::downgrade(host);
    tokio::spawn(async move {
        // Startup already loaded the database; schedule the first refresh one minute later.
        let mut interval = tokio::time::interval_at(
            tokio::time::Instant::now() + REFRESH_INTERVAL,
            REFRESH_INTERVAL,
        );
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(host) = weak.upgrade() else { break };
            let Ok(_lock) = host.credential_reload.try_lock() else {
                continue;
            };
            if tokio::time::timeout(Duration::from_secs(5), refresh(&host))
                .await
                .map_or(true, |r| r.is_err())
            {
                tracing::warn!("Credential cache refresh failed; authentication expires after 120 seconds without confirmation");
            }
        }
    });
}

pub(super) async fn change(
    State(host): State<Arc<CredentialAuthority>>,
    request: Request,
) -> Response {
    let Some(identity) = host.registry.read().await.authenticate(request.headers()) else {
        return error(StatusCode::UNAUTHORIZED, "E_TENANT_UNAUTHORIZED");
    };
    if !identity.admin() {
        return error(StatusCode::FORBIDDEN, "E_TENANT_FORBIDDEN");
    }
    if !host.accepting.load(Ordering::Acquire) {
        return error(StatusCode::SERVICE_UNAVAILABLE, "E_TENANT_DRAINING");
    }
    if request.uri().query().is_some() {
        return error(StatusCode::BAD_REQUEST, "E_CREDENTIAL_REQUEST");
    }
    let Some(permit) = host.management.admit() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "E_TENANT_QUOTA");
    };
    let Ok(lock) = host.credential_reload.clone().try_lock_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "E_CREDENTIAL_RELOAD_BUSY");
    };
    let (parts, body) = request.into_parts();
    let bytes =
        match tokio::time::timeout(Duration::from_secs(10), to_bytes(body, 128 * 1024)).await {
            Ok(Ok(bytes)) => bytes,
            _ => return error(StatusCode::BAD_REQUEST, "E_CREDENTIAL_REQUEST"),
        };
    let change: Change = match serde_json::from_slice(&bytes) {
        Ok(change) => change,
        Err(_) => return error(StatusCode::BAD_REQUEST, "E_CREDENTIAL_REQUEST"),
    };
    // Retain the permit/lock until the commit and cache activation finish, even
    // if the HTTP client disconnects during the database write.
    tokio::spawn(async move {
        let _permit = permit;
        let _lock = lock;
        let result = async {
            let (revision, mut registry) = host
                .store
                .credentials()
                .await?
                .ok_or_else(|| anyhow::anyhow!("Missing credentials"))?;
            // Recheck current DB authority, not just a potentially stale cache.
            let Some(current) = registry.authenticate(&parts.headers) else {
                return Ok(error(StatusCode::UNAUTHORIZED, "E_TENANT_UNAUTHORIZED"));
            };
            if !current.admin() {
                return Ok(error(StatusCode::FORBIDDEN, "E_TENANT_FORBIDDEN"));
            }
            let (action, id) = match &change {
                Change::Create { id, .. } => ("create", id.clone()),
                Change::Rotate { id } => ("rotate", id.clone()),
                Change::Revoke { id } => ("revoke", id.clone()),
            };
            let token = match registry.change(change, &host.scopes) {
                Ok(token) => token,
                Err(_) => {
                    return Ok(error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "E_CREDENTIAL_CONFIG",
                    ))
                }
            };
            if !host
                .store
                .save_credentials(revision, &registry, current.id(), action, &id)
                .await?
            {
                return Ok(error(StatusCode::CONFLICT, "E_CREDENTIAL_REVISION"));
            }
            *host.registry.write().await = registry;
            let mut response = Json(json!({"principal_id":id,"revision":revision+1,"token":token}))
                .into_response();
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
            Ok::<_, anyhow::Error>(response)
        }
        .await;
        result.unwrap_or_else(|_| error(StatusCode::SERVICE_UNAVAILABLE, "E_CREDENTIAL_STORE"))
    })
    .await
    .unwrap_or_else(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "E_TENANT_WORKER"))
}

async fn reload_credentials(
    State(host): State<Arc<CredentialAuthority>>,
    request: Request,
) -> Response {
    let Some(identity) = host.registry.read().await.authenticate(request.headers()) else {
        return error(StatusCode::UNAUTHORIZED, "E_TENANT_UNAUTHORIZED");
    };
    if !identity.admin() {
        return error(StatusCode::FORBIDDEN, "E_TENANT_FORBIDDEN");
    }
    if !host.accepting.load(Ordering::Acquire) {
        return error(StatusCode::SERVICE_UNAVAILABLE, "E_TENANT_DRAINING");
    }
    let Some(permit) = host.management.admit() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "E_TENANT_QUOTA");
    };
    let Ok(lock) = host.credential_reload.clone().try_lock_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "E_CREDENTIAL_RELOAD_BUSY");
    };
    tokio::spawn(async move {
        let _permit = permit;
        let _lock = lock;
        match refresh(&host).await {
            Ok(()) => {
                tracing::info!(actor=%identity.id(), "Tenant credentials reloaded");
                Json(json!({"reloaded":true})).into_response()
            }
            _ => error(StatusCode::UNPROCESSABLE_ENTITY, "E_CREDENTIAL_CONFIG"),
        }
    })
    .await
    .unwrap_or_else(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "E_TENANT_WORKER"))
}
