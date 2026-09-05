//! Compiler-owned locations for observing Core conditions without re-evaluation.
use serde::{Deserialize, Serialize};

pub const CONDITION_MAP: &str = "core_condition_map_v1";
pub const DECISION_CONDITION_MAP: &str = "core_decision_condition_map_v1";
pub const CONDITION_TRACE: &str = "__core_condition_trace__";
pub const TRACE_ENABLED: &str = "__core_trace_enabled__";
pub const TRACE_INVOCATION: &str = "__core_trace_invocation__";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionMap {
    /// Pointer to the source when clause (not to normalized AST children).
    pub field_path: String,
    pub nodes: Vec<ConditionNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionNode {
    /// Path in the normalized boolean tree. Empty denotes the root.
    pub node_path: String,
    /// Inclusive start / exclusive end in the unchanged VM instruction stream.
    pub start: usize,
    pub end: usize,
}
