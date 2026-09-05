//! One compatibility policy pointer shared by all transports in a server process.
//! The strict Core server keeps its separate, stronger admission gate.

use corint_decision_engine::{DecisionEngine, EngineError};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;

pub const REVISION_HEADER: &str = "x-corint-revision";
pub const POLICY_HEADER: &str = "x-corint-compiled-sha256";
pub const EXPECTED_REVISION_HEADER: &str = "x-corint-expected-revision";

/// Engine and identity travel together for the entire request.
pub struct EngineSnapshot {
    pub engine: Arc<DecisionEngine>,
    pub revision: String,
    /// Hash of compiled policy only, not a source digest or connector version.
    pub compiled_sha256: String,
}

impl EngineSnapshot {
    fn new(engine: Arc<DecisionEngine>) -> Result<Self, EngineError> {
        let mut policy = engine.compiled_policy()?;
        // Stable even if another workspace crate enables serde_json/preserve_order.
        policy.sort_all_objects();
        let bytes = serde_json::to_vec(&policy)
            .map_err(|e| EngineError::Config(format!("Cannot fingerprint policy: {e}")))?;
        Ok(Self {
            engine,
            revision: Uuid::new_v4().to_string(),
            compiled_sha256: format!("{:x}", Sha256::digest(bytes)),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReloadError {
    #[error("Repository reload already in progress")]
    Busy,
    #[error("Engine revision changed; read the current revision before retrying")]
    Stale,
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("Repository preparation worker failed: {0}")]
    Worker(String),
}

pub struct EngineManager {
    active: RwLock<Arc<EngineSnapshot>>,
    preparation: Arc<Semaphore>,
}

impl EngineManager {
    pub fn new(engine: Arc<DecisionEngine>) -> Result<Self, EngineError> {
        Ok(Self {
            active: RwLock::new(Arc::new(EngineSnapshot::new(engine)?)),
            preparation: Arc::new(Semaphore::new(1)),
        })
    }

    /// Release the version lock before executing any user code or I/O.
    pub async fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.active.read().await.clone()
    }

    /// An absent precondition preserves existing empty-body reload clients.
    /// Concurrent work is rejected, never queued to overwrite a later publication.
    pub async fn reload(&self, expected: Option<&str>) -> Result<Arc<EngineSnapshot>, ReloadError> {
        let base = self.snapshot().await;
        if expected.is_some_and(|expected| expected != base.revision) {
            return Err(ReloadError::Stale);
        }
        let permit = self
            .preparation
            .clone()
            .try_acquire_owned()
            .map_err(|_| ReloadError::Busy)?;
        let runtime = tokio::runtime::Handle::current();
        let engine = base.engine.clone();
        // Compilation is CPU work. Keep it off the async request executor as well
        // as outside the pointer lock. A cancelled caller cannot free the permit
        // while its worker is still preparing a candidate.
        let (candidate, _permit) = tokio::task::spawn_blocking(move || {
            runtime.block_on(async move {
                let engine = engine.prepare_reload().await?;
                EngineSnapshot::new(Arc::new(engine)).map(|snapshot| (Arc::new(snapshot), permit))
            })
        })
        .await
        .map_err(|error| ReloadError::Worker(error.to_string()))??;
        self.commit(&base.revision, candidate).await
    }

    async fn commit(
        &self,
        expected: &str,
        candidate: Arc<EngineSnapshot>,
    ) -> Result<Arc<EngineSnapshot>, ReloadError> {
        let mut active = self.active.write().await;
        if active.revision != expected {
            return Err(ReloadError::Stale);
        }
        let retired = std::mem::replace(&mut *active, candidate.clone());
        drop(active);
        // Destruction of an unused old engine must also happen outside the lock.
        drop(retired);
        Ok(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_decision_engine::EngineConfig;

    async fn manager() -> Arc<EngineManager> {
        let engine = DecisionEngine::new(EngineConfig::new()).await.unwrap();
        Arc::new(EngineManager::new(Arc::new(engine)).unwrap())
    }

    #[tokio::test]
    async fn simultaneous_candidates_cannot_overwrite_each_other() {
        let manager = manager().await;
        let original = manager.snapshot().await;
        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let mut tasks = Vec::new();
        for _ in 0..2 {
            let manager = manager.clone();
            let barrier = barrier.clone();
            let original = original.clone();
            tasks.push(tokio::spawn(async move {
                let candidate = Arc::new(EngineSnapshot::new(original.engine.clone()).unwrap());
                barrier.wait().await;
                manager.commit(&original.revision, candidate).await
            }));
        }
        barrier.wait().await;
        let mut winners = Vec::new();
        let mut stale = 0;
        for task in tasks {
            match task.await.unwrap() {
                Ok(snapshot) => winners.push(snapshot),
                Err(ReloadError::Stale) => stale += 1,
                _ => panic!("unexpected commit error"),
            }
        }
        assert_eq!(winners.len(), 1);
        assert_eq!(stale, 1);
        assert_eq!(manager.snapshot().await.revision, winners[0].revision);
        // A request retaining the previous snapshot neither blocks nor changes it.
        assert_ne!(original.revision, winners[0].revision);
    }

    #[tokio::test]
    async fn occupied_preparation_slot_rejects_reload_without_queueing() {
        let manager = manager().await;
        let original = manager.snapshot().await;
        let permit = manager.preparation.clone().acquire_owned().await.unwrap();
        assert!(matches!(manager.reload(None).await, Err(ReloadError::Busy)));
        assert_eq!(manager.snapshot().await.revision, original.revision);
        drop(permit);
        // A non-repository engine is rejected, and the failed worker releases its slot.
        assert!(matches!(
            manager.reload(None).await,
            Err(ReloadError::Engine(_))
        ));
        assert_eq!(manager.preparation.available_permits(), 1);
    }
}
