//! Condition compilation for pipeline steps.
//!
//! Lower YAML groups and legacy conditions through the shared expression compiler.

use crate::codegen::{ExpressionCompiler, RuleCompiler};
use crate::error::Result;
use corint_decision_model::ast::{Expression, LogicalGroupOp, Operator, WhenBlock};
use corint_decision_model::ir::Instruction;
use corint_decision_model::Value;

/// Produce one boolean, with the same left-to-right short-circuiting as Rules.
pub(super) fn compile_when_block(when: &WhenBlock) -> Result<Vec<Instruction>> {
    let mut conditions = Vec::new();
    if let Some(event_type) = &when.event_type {
        conditions.push(Expression::binary(
            Expression::field_access(vec!["event".into(), "type".into()]),
            Operator::Eq,
            Expression::literal(Value::String(event_type.clone())),
        ));
    }
    if let Some(group) = &when.condition_group {
        conditions.push(RuleCompiler::group_expression(group));
    } else if let Some(legacy) = &when.conditions {
        conditions.extend(legacy.iter().cloned());
    }
    ExpressionCompiler::compile(&Expression::LogicalGroup {
        op: LogicalGroupOp::All,
        conditions,
    })
}
