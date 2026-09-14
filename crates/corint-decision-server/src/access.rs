//! Operator-owned, single-tenant bearer roles shared by HTTP and gRPC.
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct AccessPolicy {
    decision: [u8; 32],
    publisher: [u8; 32],
    pub tenant_id: String,
    database: Option<(
        std::sync::Arc<crate::tenancy::CredentialAuthority>,
        crate::tenancy::Scope,
    )>,
}
impl AccessPolicy {
    pub fn new(decision: &str, publisher: &str, tenant_id: &str) -> anyhow::Result<Self> {
        for token in [decision, publisher] {
            anyhow::ensure!(
                (32..=1024).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_graphic()),
                "Credentials require 32..1024 printable non-space ASCII characters"
            );
        }
        anyhow::ensure!(
            decision != publisher,
            "Credentials must have distinct roles"
        );
        anyhow::ensure!(
            !tenant_id.is_empty()
                && tenant_id.len() <= 128
                && tenant_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
            "Invalid operator tenant ID"
        );
        Ok(Self {
            decision: Sha256::digest(decision.as_bytes()).into(),
            publisher: Sha256::digest(publisher.as_bytes()).into(),
            tenant_id: tenant_id.into(),
            database: None,
        })
    }
    pub fn from_env() -> anyhow::Result<Self> {
        let read = |name| {
            std::env::var(name).map_err(|_| {
                anyhow::anyhow!("Missing operator credential or tenant configuration: {name}")
            })
        };
        let tenant = match std::env::var("CORINT_TENANT_ID") {
            Ok(tenant) => tenant,
            Err(std::env::VarError::NotPresent) => local_tenant(),
            Err(_) => anyhow::bail!("Invalid operator tenant configuration"),
        };
        Self::new(
            &read("CORINT_DECISION_TOKEN")?,
            &read("CORINT_PUBLISHER_TOKEN")?,
            &tenant,
        )
    }
    pub async fn load() -> anyhow::Result<Self> {
        let Some(path) = std::env::var_os("CORINT_AUTH_CONFIG") else {
            return Self::from_env();
        };
        Self::from_database_config(std::path::Path::new(&path)).await
    }
    pub async fn from_database_config(path: &std::path::Path) -> anyhow::Result<Self> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Config {
            scope: crate::tenancy::Scope,
            control_store: crate::tenancy::store::StoreConfig,
            #[serde(default)]
            credentials: Option<std::path::PathBuf>,
        }
        let path = path.canonicalize()?;
        anyhow::ensure!(
            std::fs::metadata(&path)?.len() <= 8 * 1024 * 1024,
            "Authentication config too large"
        );
        let config: Config = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        config.scope.validate()?;
        let authority = crate::tenancy::CredentialAuthority::open(
            path.parent().unwrap(),
            &config.control_store,
            std::collections::BTreeSet::from([config.scope.clone()]),
            config.credentials.as_deref(),
        )
        .await?;
        Ok(Self {
            decision: [0; 32],
            publisher: [0; 32],
            tenant_id: config.scope.tenant_id.clone(),
            database: Some((authority, config.scope)),
        })
    }
    pub async fn drain(&self, timeout: std::time::Duration) -> anyhow::Result<()> {
        if let Some((authority, _)) = &self.database {
            authority.drain(timeout).await?;
        }
        Ok(())
    }
    pub fn credential_router(&self) -> axum::Router {
        self.database
            .as_ref()
            .map(|(authority, _)| authority.router())
            .unwrap_or_default()
    }
    /// Caller must reject multiple authorization fields before calling this method.
    pub async fn permits(&self, authorization: Option<&str>, publisher: bool) -> bool {
        if let Some((authority, scope)) = &self.database {
            let permission = if publisher {
                crate::tenancy::Permission::Publish
            } else {
                crate::tenancy::Permission::Decide
            };
            return authority.permits(authorization, scope, permission).await;
        }
        authorization
            .and_then(|v| v.strip_prefix("Bearer "))
            .filter(|v| v.len() <= 1024)
            .is_some_and(|token| {
                let actual: [u8; 32] = Sha256::digest(token.as_bytes()).into();
                bool::from(actual.ct_eq(if publisher {
                    &self.publisher
                } else {
                    &self.decision
                }))
            })
    }
}

pub fn local_tenant() -> String {
    "local".into()
}
