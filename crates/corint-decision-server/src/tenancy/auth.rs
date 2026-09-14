use super::config::{identifier, read, Scope};
use anyhow::{ensure, Result};
use axum::http::{header::AUTHORIZATION, HeaderMap};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use subtle::ConstantTimeEq;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Decide,
    Inspect,
    Publish,
    Consume,
    Export,
    Manage,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub scope: Scope,
    pub permissions: BTreeSet<Permission>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Principal {
    id: String,
    #[serde(default, skip_serializing)]
    token_env: String,
    #[serde(default)]
    revoked: bool,
    #[serde(default)]
    platform_admin: bool,
    #[serde(default)]
    expires_at_ms: Option<i64>,
    #[serde(default)]
    delegated_by: Option<String>,
    #[serde(default)]
    grants: Vec<Grant>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format_version: String,
    principals: Vec<Principal>,
}
#[derive(Clone, Deserialize, Serialize)]
struct Entry {
    principal: Principal,
    digest: [u8; 32],
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Registry {
    entries: BTreeMap<String, Entry>,
    #[serde(skip, default = "std::time::Instant::now")]
    verified_at: std::time::Instant,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Change {
    Create {
        id: String,
        #[serde(default)]
        platform_admin: bool,
        #[serde(default)]
        expires_at_ms: Option<i64>,
        #[serde(default)]
        delegated_by: Option<String>,
        #[serde(default)]
        grants: Vec<Grant>,
    },
    Rotate {
        id: String,
    },
    Revoke {
        id: String,
    },
}

/// Constructed only by credential verification; HTTP headers/events cannot supply it.
#[derive(Clone)]
pub struct TenantContext {
    pub(crate) scope: Scope,
    principal_id: String,
    actor_id: String,
    request_id: String,
    permissions: BTreeSet<Permission>,
}
impl TenantContext {
    pub(crate) fn request_id(&self) -> &str {
        &self.request_id
    }
    pub(crate) fn permits(&self, scope: &Scope, permission: Permission) -> bool {
        &self.scope == scope && self.permissions.contains(&permission)
    }
    pub(crate) fn audit(&self) -> serde_json::Value {
        serde_json::json!({"tenant_id":self.scope.tenant_id,"environment":self.scope.environment,
            "deployment":self.scope.deployment,"principal_id":self.principal_id,"actor_id":self.actor_id,"request_id":self.request_id})
    }
}
pub struct Identity {
    principal: Principal,
}
impl Identity {
    pub fn id(&self) -> &str {
        &self.principal.id
    }
    pub fn admin(&self) -> bool {
        self.principal.platform_admin
    }
    pub fn context(&self, scope: &Scope, permission: Permission) -> Option<TenantContext> {
        let grant = self
            .principal
            .grants
            .iter()
            .find(|g| &g.scope == scope && g.permissions.contains(&permission))?;
        Some(TenantContext {
            scope: scope.clone(),
            principal_id: self
                .principal
                .delegated_by
                .clone()
                .unwrap_or_else(|| self.principal.id.clone()),
            actor_id: self.principal.id.clone(),
            request_id: uuid::Uuid::new_v4().to_string(),
            permissions: grant.permissions.clone(),
        })
    }
}
impl Registry {
    pub fn load(path: &Path, known: &BTreeSet<Scope>) -> Result<Self> {
        let file: File = serde_json::from_str(&read(path)?)?;
        ensure!(
            file.format_version == "1" && (1..=2048).contains(&file.principals.len()),
            "Invalid credentials document"
        );
        let mut entries = BTreeMap::new();
        for p in file.principals {
            let token = std::env::var(&p.token_env)
                .map_err(|_| anyhow::anyhow!("Missing tenant credential"))?;
            ensure!(
                (32..=1024).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_graphic()),
                "Invalid tenant credential"
            );
            let digest = Sha256::digest(token.as_bytes()).into();
            ensure!(
                entries
                    .insert(
                        p.id.clone(),
                        Entry {
                            principal: p,
                            digest
                        }
                    )
                    .is_none(),
                "Duplicate principal"
            );
        }
        let registry = Self {
            entries,
            verified_at: std::time::Instant::now(),
        };
        registry.validate(known)?;
        Ok(registry)
    }
    pub(super) fn validate(&self, known: &BTreeSet<Scope>) -> Result<()> {
        let entries = &self.entries;
        ensure!(
            (1..=2048).contains(&entries.len()),
            "Invalid credential count"
        );
        let mut digests = BTreeSet::new();
        for (id, e) in entries {
            let p = &e.principal;
            ensure!(
                id == &p.id && identifier(id) && p.grants.len() <= 256,
                "Invalid principal"
            );
            ensure!(
                p.expires_at_ms.is_none_or(|t| t > 0),
                "Invalid credential expiry"
            );
            ensure!(
                digests.insert(e.digest),
                "Credentials must be unique across principals"
            );
            let mut scopes = BTreeSet::new();
            for g in &p.grants {
                g.scope.validate()?;
                ensure!(
                    known.contains(&g.scope)
                        && scopes.insert(g.scope.clone())
                        && !g.permissions.is_empty(),
                    "Invalid tenant grant"
                );
                ensure!(
                    !g.permissions.contains(&Permission::Export)
                        || g.permissions.contains(&Permission::Consume),
                    "Export requires consume permission"
                );
            }
        }
        for e in entries.values() {
            if let Some(parent_id) = &e.principal.delegated_by {
                let parent = &entries
                    .get(parent_id)
                    .ok_or_else(|| anyhow::anyhow!("Unknown delegation parent"))?
                    .principal;
                ensure!(parent.delegated_by.is_none() && !e.principal.platform_admin && e.principal.expires_at_ms.is_some(), "Delegation requires a direct parent, bounded expiry and no platform administration");
                ensure!(
                    parent
                        .expires_at_ms
                        .is_none_or(|t| e.principal.expires_at_ms.unwrap() <= t),
                    "Delegation outlives parent"
                );
                for g in &e.principal.grants {
                    ensure!(
                        parent
                            .grants
                            .iter()
                            .any(|p| p.scope == g.scope && g.permissions.is_subset(&p.permissions)),
                        "Delegation exceeds parent authority"
                    );
                }
            }
        }
        ensure!(
            entries.values().any(|e| !e.principal.revoked
                && e.principal.platform_admin
                && e.principal
                    .expires_at_ms
                    .is_none_or(|t| t > chrono::Utc::now().timestamp_millis())),
            "Require a live platform credential for rotation"
        );
        Ok(())
    }
    pub(super) fn validate_persisted(&self) -> Result<()> {
        // A control database can outlive a host's configured runtime set. Grants
        // for unloaded scopes confer no authority over other scopes.
        self.validate(
            &self
                .entries
                .values()
                .flat_map(|e| e.principal.grants.iter().map(|g| g.scope.clone()))
                .collect(),
        )
    }
    pub(super) fn confirmed_since(&mut self, instant: std::time::Instant) {
        self.verified_at = instant;
    }
    pub(super) fn change(
        &mut self,
        change: Change,
        known: &BTreeSet<Scope>,
    ) -> Result<Option<String>> {
        use rand::RngCore;
        let generate = || {
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        };
        let token = match change {
            Change::Create {
                id,
                platform_admin,
                expires_at_ms,
                delegated_by,
                grants,
            } => {
                ensure!(
                    grants.iter().all(|g| known.contains(&g.scope)),
                    "Unknown tenant scope"
                );
                ensure!(!self.entries.contains_key(&id), "Principal already exists");
                ensure!(
                    expires_at_ms.is_none_or(|t| t > chrono::Utc::now().timestamp_millis()),
                    "Credential already expired"
                );
                let token = generate();
                let principal = Principal {
                    id: id.clone(),
                    token_env: String::new(),
                    revoked: false,
                    platform_admin,
                    expires_at_ms,
                    delegated_by,
                    grants,
                };
                self.entries.insert(
                    id,
                    Entry {
                        principal,
                        digest: Sha256::digest(token.as_bytes()).into(),
                    },
                );
                Some(token)
            }
            Change::Rotate { id } => {
                let e = self
                    .entries
                    .get_mut(&id)
                    .ok_or_else(|| anyhow::anyhow!("Unknown principal"))?;
                ensure!(
                    !e.principal.revoked
                        && e.principal
                            .expires_at_ms
                            .is_none_or(|t| t > chrono::Utc::now().timestamp_millis()),
                    "Credential inactive"
                );
                let token = generate();
                e.digest = Sha256::digest(token.as_bytes()).into();
                Some(token)
            }
            Change::Revoke { id } => {
                self.entries
                    .get_mut(&id)
                    .ok_or_else(|| anyhow::anyhow!("Unknown principal"))?
                    .principal
                    .revoked = true;
                None
            }
        };
        self.validate_persisted()?;
        Ok(token)
    }
    pub fn authenticate(&self, headers: &HeaderMap) -> Option<Identity> {
        // Fail closed if the database has not confirmed this cache recently.
        if self.verified_at.elapsed() >= super::credentials::MAX_CACHE_AGE {
            return None;
        }
        let mut values = headers.get_all(AUTHORIZATION).iter();
        let token = values.next()?.to_str().ok()?.strip_prefix("Bearer ")?;
        if values.next().is_some() || !(32..=1024).contains(&token.len()) {
            return None;
        }
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let now = chrono::Utc::now().timestamp_millis();
        let mut found = None;
        for e in self.entries.values() {
            if bool::from(digest.ct_eq(&e.digest)) {
                found = Some(e);
            }
        }
        let p = &found?.principal;
        if p.revoked || p.expires_at_ms.is_some_and(|t| t <= now) {
            return None;
        }
        if let Some(parent) = &p.delegated_by {
            let parent = &self.entries.get(parent)?.principal;
            if parent.revoked || parent.expires_at_ms.is_some_and(|t| t <= now) {
                return None;
            }
        }
        Some(Identity {
            principal: p.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_age_and_expiry_are_checked_on_every_request() {
        let token = "synthetic-test-credential-at-least-32-chars";
        let principal = Principal {
            id: "admin".into(),
            token_env: String::new(),
            revoked: false,
            platform_admin: true,
            expires_at_ms: None,
            delegated_by: None,
            grants: vec![],
        };
        let mut registry = Registry {
            entries: BTreeMap::from([(
                "admin".into(),
                Entry {
                    principal,
                    digest: Sha256::digest(token.as_bytes()).into(),
                },
            )]),
            verified_at: std::time::Instant::now(),
        };
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        assert!(registry.authenticate(&headers).is_some());
        // A valid cache survives a normal one-minute refresh boundary.
        registry.confirmed_since(std::time::Instant::now() - std::time::Duration::from_secs(61));
        assert!(registry.authenticate(&headers).is_some());
        registry.confirmed_since(std::time::Instant::now() - std::time::Duration::from_secs(121));
        assert!(registry.authenticate(&headers).is_none());
        registry.confirmed_since(std::time::Instant::now());
        registry
            .entries
            .get_mut("admin")
            .unwrap()
            .principal
            .expires_at_ms = Some(1);
        assert!(registry.authenticate(&headers).is_none());
    }
}
