//! FFI type definitions

use std::sync::Arc;
use corint_sdk::DecisionEngine;
use tokio::runtime::Runtime;

/// Opaque type representing a CORINT decision engine
#[repr(C)]
pub struct CorintEngine {
    pub(crate) engine: Arc<DecisionEngine>,
    pub(crate) runtime: Arc<Runtime>,
}
