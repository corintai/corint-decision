use anyhow::{ensure, Result};
use corint_decision_engine::{
    decision_host::{canonical_sha256, FeatureHostConfig},
    DataAccessScope, DataSourceType,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub tenant_id: String,
    pub environment: String,
    pub deployment: String,
}
pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
impl Scope {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            [&self.tenant_id, &self.environment, &self.deployment]
                .into_iter()
                .all(|s| identifier(s)),
            "Invalid tenant scope"
        );
        Ok(())
    }
    pub fn key(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_inflight: u32,
    pub requests_per_second: u32,
    pub burst: u32,
    pub max_connections: u32,
}
impl Limits {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=65536).contains(&self.max_inflight)
                && (1..=1_000_000).contains(&self.requests_per_second)
                && (1..=1_000_000).contains(&self.burst)
                && (8..=65536).contains(&self.max_connections),
            "Invalid tenant limits"
        );
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBinding {
    pub datasource: String,
    pub revision: String,
    pub config_sha256: String,
    pub entity: String,
    pub tenant_column: String,
    pub environment_column: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentConfig {
    pub scope: Scope,
    pub root: PathBuf,
    pub core_config: PathBuf,
    pub limits: Limits,
    pub timeout_ms: u64,
    pub idle_seconds: u64,
    #[serde(default)]
    pub resources: Vec<ResourceBinding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub format_version: String,
    pub listen: SocketAddr,
    /// First-start seed; persisted credentials are authoritative afterwards.
    #[serde(default)]
    pub credentials: Option<PathBuf>,
    pub control_store: super::store::StoreConfig,
    pub platform_limits: Limits,
    pub tenant_limits: BTreeMap<String, Limits>,
    pub max_loaded: u32,
    pub max_preparations: u32,
    pub deployments: Vec<DeploymentConfig>,
    /// Optional operator routing for tenants with multiple internal runtimes.
    /// A tenant with one runtime needs no explicit binding.
    #[serde(default)]
    pub decision_bindings: BTreeMap<String, Scope>,
}

/// Acquire all tiers without waiting. Permits follow the resource lifetime,
/// including old snapshots retained by in-flight requests during reloads.
#[derive(Clone)]
pub struct ResourceBudget(pub Vec<Arc<Semaphore>>);
pub struct ResourceLease {
    _permits: Vec<OwnedSemaphorePermit>,
}
impl ResourceBudget {
    pub fn acquire(&self, count: u32) -> Result<Arc<ResourceLease>> {
        let mut permits = Vec::new();
        for tier in &self.0 {
            permits.push(
                tier.clone()
                    .try_acquire_many_owned(count)
                    .map_err(|_| anyhow::anyhow!("E_CONNECTION_BUDGET"))?,
            );
        }
        Ok(Arc::new(ResourceLease { _permits: permits }))
    }
}

#[derive(Clone)]
pub struct Boundary {
    pub scope: Scope,
    pub resources: Vec<ResourceBinding>,
    pub connections: ResourceBudget,
    pub preparations: Arc<Semaphore>,
}
impl Boundary {
    pub fn fingerprint(&self) -> String {
        canonical_sha256(&serde_json::json!({"scope":self.scope, "resources":self.resources}))
    }
    pub fn authorize(
        &self,
        features: Option<&FeatureHostConfig>,
    ) -> Result<BTreeMap<String, DataAccessScope>> {
        let Some(features) = features else {
            ensure!(self.resources.is_empty(), "Unused tenant resource bindings");
            return Ok(BTreeMap::new());
        };
        ensure!(
            features.datasources.len() == self.resources.len(),
            "Tenant datasource grant set differs"
        );
        let mut scopes = BTreeMap::new();
        for grant in &self.resources {
            let binding = features
                .datasources
                .get(&grant.datasource)
                .ok_or_else(|| anyhow::anyhow!("Unowned datasource"))?;
            ensure!(
                binding.revision == grant.revision
                    && canonical_sha256(&binding.config) == grant.config_sha256,
                "Tenant datasource binding changed without authorization"
            );
            ensure!(
                matches!(binding.config.source_type, DataSourceType::SQL(_)),
                "Tenant feature resources require SQL row isolation"
            );
            ensure!(
                grant.tenant_column != grant.environment_column,
                "Tenant/environment columns must differ"
            );
            let scope = DataAccessScope {
                namespace: canonical_sha256(&(self.scope.clone(), grant)),
                entity: grant.entity.clone(),
                equalities: BTreeMap::from([
                    (grant.tenant_column.clone(), self.scope.tenant_id.clone()),
                    (
                        grant.environment_column.clone(),
                        self.scope.environment.clone(),
                    ),
                ]),
            };
            scope.validate()?;
            ensure!(
                scopes.insert(grant.datasource.clone(), scope).is_none(),
                "Duplicate datasource grant"
            );
        }
        Ok(scopes)
    }
    pub fn reserve_host(&self, features: Option<&FeatureHostConfig>) -> Result<Arc<ResourceLease>> {
        let mut total = 0u32;
        if let Some(features) = features {
            for source in features.datasources.values() {
                let DataSourceType::SQL(sql) = &source.config.source_type else {
                    anyhow::bail!("Unsupported tenant datasource");
                };
                let count = match sql.options.get("max_connections") {
                    Some(value) => value.parse::<u32>()?,
                    None => source.config.pool_size,
                };
                ensure!(
                    (1..=1024).contains(&count),
                    "Invalid tenant datasource pool size"
                );
                total = total
                    .checked_add(count)
                    .ok_or_else(|| anyhow::anyhow!("Connection budget overflow"))?;
            }
        }
        self.connections.acquire(total)
    }
}

pub fn read(path: &Path) -> Result<String> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "Expected operator configuration file"
    );
    let mut bytes = Vec::new();
    file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 8 * 1024 * 1024,
        "Tenant configuration exceeds 8 MiB"
    );
    Ok(String::from_utf8(bytes)?)
}
pub fn confined(root: &Path, path: &Path) -> Result<PathBuf> {
    let resolved = root.join(path).canonicalize()?;
    ensure!(
        resolved.starts_with(root),
        "Tenant file escapes deployment root"
    );
    Ok(resolved)
}
pub fn validate_roots(roots: &[PathBuf]) -> Result<()> {
    let unique: BTreeSet<_> = roots.iter().collect();
    ensure!(
        unique.len() == roots.len(),
        "Deployments cannot share an operator root"
    );
    for (i, a) in roots.iter().enumerate() {
        for b in &roots[i + 1..] {
            ensure!(
                !a.starts_with(b) && !b.starts_with(a),
                "Deployment roots cannot overlap"
            );
        }
    }
    Ok(())
}
