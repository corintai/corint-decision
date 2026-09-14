//! Bounded, volatile background persistence. Admission is not a durable commit.
use serde::Serialize;
use std::{
    future::Future,
    sync::{Arc, LazyLock, Mutex, Weak},
    time::Duration,
};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

const MAX_PENDING: usize = 64;
const MAX_BYTES: usize = 32 * 1024 * 1024;
static WRITERS: LazyLock<Mutex<Vec<Weak<Inner>>>> = LazyLock::new(|| Mutex::new(Vec::new()));

#[derive(Clone, Debug, Serialize)]
pub struct PersistenceStatus {
    pub accepting: bool,
    pub pending: usize,
    pub written: u64,
    pub failed: u64,
    pub retries: u64,
    pub last_failed_id: Option<String>,
}
struct Inner {
    status: Mutex<PersistenceStatus>,
    slots: Arc<Semaphore>,
    bytes: Arc<Semaphore>,
    notify: Notify,
}
#[derive(Clone)]
pub struct BackgroundWrites(Arc<Inner>);
impl Default for BackgroundWrites {
    fn default() -> Self {
        Self::with_limits(MAX_PENDING, MAX_BYTES)
    }
}
impl BackgroundWrites {
    fn with_limits(capacity: usize, bytes: usize) -> Self {
        let inner = Arc::new(Inner {
            status: Mutex::new(PersistenceStatus {
                accepting: true,
                pending: 0,
                written: 0,
                failed: 0,
                retries: 0,
                last_failed_id: None,
            }),
            slots: Arc::new(Semaphore::new(capacity)),
            bytes: Arc::new(Semaphore::new(bytes)),
            notify: Notify::new(),
        });
        let mut writers = WRITERS.lock().expect("persistence registry poisoned");
        writers.retain(|writer| writer.strong_count() > 0);
        writers.push(Arc::downgrade(&inner));
        Self(inner)
    }
    pub fn status(&self) -> PersistenceStatus {
        self.0
            .status
            .lock()
            .expect("persistence status poisoned")
            .clone()
    }
    /// Admit work without waiting for database I/O. Buffers are limited by both
    /// record count and serialized bytes; callers must reject failed admission.
    pub fn submit<F, Fut>(&self, id: String, bytes: usize, write: F) -> Result<(), String>
    where
        F: FnMut() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.submit_with_attempts(id, bytes, 3, write)
    }
    /// Use for writes without retry idempotency: an uncertain commit must not
    /// cause automatic duplicate side effects.
    pub fn submit_once<F, Fut>(&self, id: String, bytes: usize, write: F) -> Result<(), String>
    where
        F: FnMut() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.submit_with_attempts(id, bytes, 1, write)
    }
    fn submit_with_attempts<F, Fut>(
        &self,
        id: String,
        bytes: usize,
        attempts: usize,
        mut write: F,
    ) -> Result<(), String>
    where
        F: FnMut() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| "Background persistence requires a running Tokio runtime")?;
        let slots = self
            .0
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Persistence queue full")?;
        let bytes = u32::try_from(bytes.max(1)).map_err(|_| "Persistence record too large")?;
        let bytes = self
            .0
            .bytes
            .clone()
            .try_acquire_many_owned(bytes)
            .map_err(|_| "Persistence byte budget exhausted")?;
        let mut status = self.0.status.lock().expect("persistence status poisoned");
        if !status.accepting {
            return Err("Persistence is shutting down".into());
        }
        status.pending += 1;
        let mut completion = Completion {
            inner: self.0.clone(),
            id,
            success: false,
            _slots: slots,
            _bytes: bytes,
        };
        runtime.spawn(async move {
            for attempt in 0..attempts {
                match write().await {
                    Ok(()) => { completion.success = true; drop(completion); return; }
                    Err(error) if attempt + 1 < attempts => {
                        completion.inner.status.lock().expect("persistence status poisoned").retries += 1;
                        tracing::warn!(decision_id=%completion.id, attempt=attempt+1, error=%error, "Background decision write failed; retrying");
                        tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
                    }
                    Err(error) => {
                        tracing::error!(decision_id=%completion.id, error=%error, "Background decision write exhausted retries");
                    }
                }
            }
        });
        Ok(())
    }
    /// Stop accepting new work, then wait for all accepted writes to finish.
    pub async fn shutdown(&self, timeout: Duration) -> Result<(), String> {
        self.close();
        tokio::time::timeout(timeout, self.wait())
            .await
            .map_err(|_| "Timed out draining background decisions".to_string())
    }
    fn close(&self) {
        self.0
            .status
            .lock()
            .expect("persistence status poisoned")
            .accepting = false;
    }
    async fn wait(&self) {
        loop {
            let notified = self.0.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.status().pending == 0 {
                return;
            }
            notified.await;
        }
    }
}
struct Completion {
    inner: Arc<Inner>,
    id: String,
    success: bool,
    _slots: OwnedSemaphorePermit,
    _bytes: OwnedSemaphorePermit,
}
impl Drop for Completion {
    fn drop(&mut self) {
        let mut status = self
            .inner
            .status
            .lock()
            .expect("persistence status poisoned");
        status.pending -= 1;
        if self.success {
            status.written += 1;
        } else {
            status.failed += 1;
            status.last_failed_id = Some(self.id.clone());
            tracing::error!(decision_id=%self.id, "Accepted decision was not persisted");
        }
        self.inner.notify.notify_waiters();
    }
}
/// After stopping request admission, drain every live writer (including old
/// policy snapshots). Forced process termination can still lose buffered work.
pub async fn shutdown_background_writes(timeout: Duration) -> Result<(), String> {
    let writers: Vec<_> = WRITERS
        .lock()
        .expect("persistence registry poisoned")
        .iter()
        .filter_map(Weak::upgrade)
        .map(BackgroundWrites)
        .collect();
    for writer in &writers {
        writer.close();
    }
    tokio::time::timeout(timeout, async {
        for writer in &writers {
            writer.wait().await;
        }
    })
    .await
    .map_err(|_| "Timed out draining background decisions".to_string())?;
    let failed: u64 = writers.iter().map(|writer| writer.status().failed).sum();
    if failed > 0 {
        return Err(format!("{failed} background decision writes failed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn admission_returns_before_io_and_bounds_pending_bytes_and_count() {
        let writer = BackgroundWrites::with_limits(2, 10);
        let gate = Arc::new(Semaphore::new(0));
        let captured = gate.clone();
        writer
            .submit("first".into(), 6, move || {
                let gate = captured.clone();
                async move {
                    gate.acquire().await.unwrap().forget();
                    Ok(())
                }
            })
            .unwrap();
        assert_eq!(writer.status().pending, 1);
        assert!(writer
            .submit("too-big".into(), 5, || async { Ok(()) })
            .is_err());
        let captured = gate.clone();
        writer
            .submit("second".into(), 4, move || {
                let gate = captured.clone();
                async move {
                    gate.acquire().await.unwrap().forget();
                    Ok(())
                }
            })
            .unwrap();
        assert!(writer
            .submit("full".into(), 1, || async { Ok(()) })
            .is_err());
        gate.add_permits(2);
        writer.shutdown(Duration::from_secs(1)).await.unwrap();
        assert_eq!(writer.status().written, 2);
        assert!(writer
            .submit("closed".into(), 1, || async { Ok(()) })
            .is_err());
    }
    #[tokio::test(start_paused = true)]
    async fn transient_errors_retry_and_terminal_failures_are_visible() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let writer = BackgroundWrites::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let copy = calls.clone();
        writer
            .submit("retry".into(), 1, move || {
                let n = copy.fetch_add(1, Ordering::Relaxed);
                async move {
                    if n < 2 {
                        Err("temporarily unavailable".into())
                    } else {
                        Ok(())
                    }
                }
            })
            .unwrap();
        writer
            .submit("failed".into(), 1, || async { Err("unavailable".into()) })
            .unwrap();
        writer.shutdown(Duration::from_secs(5)).await.unwrap();
        let status = writer.status();
        assert_eq!(calls.load(Ordering::Relaxed), 3);
        assert_eq!(status.pending, 0);
        assert_eq!(status.written, 1);
        assert_eq!(status.failed, 1);
        assert_eq!(status.retries, 4);
        assert_eq!(status.last_failed_id.as_deref(), Some("failed"));
    }
    #[tokio::test]
    async fn panicking_jobs_release_capacity_and_report_failure() {
        let writer = BackgroundWrites::default();
        writer
            .submit("panic".into(), 1, || async {
                panic!("simulated failure");
                #[allow(unreachable_code)]
                Ok(())
            })
            .unwrap();
        writer.shutdown(Duration::from_secs(1)).await.unwrap();
        assert_eq!(writer.status().failed, 1);
        assert_eq!(writer.status().pending, 0);
    }
}
