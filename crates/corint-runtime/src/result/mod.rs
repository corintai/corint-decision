//! Execution result types and persistence

mod persistence;
mod result;

pub use persistence::{DecisionRecord, DecisionResultWriter, RuleExecutionRecord};
pub use result::{DecisionResult, ExecutionResult};

