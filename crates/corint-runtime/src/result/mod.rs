//! Execution result types and persistence

mod persistence;
#[allow(clippy::module_inception)]
mod result;
mod trace;

pub use persistence::{DecisionRecord, DecisionResultWriter, RuleExecutionRecord};
pub use result::{DecisionResult, ExecutionResult};
pub use trace::{
    ConclusionTrace, ConditionTrace, DecisionLogicTrace, ExecutionTrace, PipelineTrace, RuleTrace,
    RulesetTrace, StepTrace,
};
