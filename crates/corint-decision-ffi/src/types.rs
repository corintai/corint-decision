//! FFI type definitions

use corint_decision_engine::snapshot::EngineManager;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Opaque type representing a CORINT decision engine
#[repr(C)]
pub struct CorintEngine {
    pub(crate) engine: Arc<EngineManager>,
    pub(crate) runtime: Arc<Runtime>,
}
