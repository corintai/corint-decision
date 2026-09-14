use super::config::Limits;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct Quota {
    pub slots: Arc<Semaphore>,
    limits: Limits,
    bucket: Mutex<(Instant, f64)>,
    accepted: AtomicU64,
    rejected: AtomicU64,
}
impl Quota {
    pub fn new(limits: Limits) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(limits.max_inflight as usize)),
            bucket: Mutex::new((Instant::now(), f64::from(limits.burst))),
            limits,
            accepted: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
        }
    }
    pub fn admit(&self) -> Option<OwnedSemaphorePermit> {
        let admit = || {
            let permit = self.slots.clone().try_acquire_owned().ok()?;
            let mut bucket = self.bucket.lock().unwrap();
            let now = Instant::now();
            bucket.1 = (bucket.1
                + now.duration_since(bucket.0).as_secs_f64()
                    * f64::from(self.limits.requests_per_second))
            .min(f64::from(self.limits.burst));
            bucket.0 = now;
            if bucket.1 < 1.0 {
                return None;
            }
            bucket.1 -= 1.0;
            Some(permit)
        };
        let result = admit();
        if result.is_some() {
            self.accepted.fetch_add(1, Ordering::Relaxed);
        } else {
            self.rejected.fetch_add(1, Ordering::Relaxed);
        }
        result
    }
    pub fn inflight(&self) -> usize {
        self.limits.max_inflight as usize - self.slots.available_permits()
    }
    pub fn snapshot(&self) -> Value {
        json!({"limits":self.limits,"inflight":self.inflight(),"accepted":self.accepted.load(Ordering::Relaxed),"rejected":self.rejected.load(Ordering::Relaxed)})
    }
}
