//! Operator-owned, single-tenant bearer roles shared by HTTP and gRPC.
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct AccessPolicy {
    decision: [u8; 32],
    publisher: [u8; 32],
    pub tenant_id: String,
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
        })
    }
    pub fn from_env() -> anyhow::Result<Self> {
        let read = |name| {
            std::env::var(name).map_err(|_| {
                anyhow::anyhow!("Missing operator credential or tenant configuration: {name}")
            })
        };
        Self::new(
            &read("CORINT_DECISION_TOKEN")?,
            &read("CORINT_PUBLISHER_TOKEN")?,
            &read("CORINT_TENANT_ID")?,
        )
    }
    /// Caller must reject multiple authorization fields before calling this method.
    pub fn permits(&self, authorization: Option<&str>, publisher: bool) -> bool {
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
