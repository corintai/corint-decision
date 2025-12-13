//! Execution result types and persistence

mod persistence;
#[allow(clippy::module_inception)]
mod result;

pub use persistence::{DecisionRecord, DecisionResultWriter, RuleExecutionRecord};
pub use result::{DecisionResult, ExecutionResult};
