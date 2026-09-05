//! FFI type definitions

use corint_decision_sdk::DecisionEngine;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Opaque type representing a CORINT decision engine
#[repr(C)]
pub struct CorintEngine {
    pub(crate) engine: Arc<DecisionEngine>,
    pub(crate) runtime: Arc<Runtime>,
}
