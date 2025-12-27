//! Condition compilation for pipeline steps
//!
//! Compiles WhenBlock conditions into IR instructions.

use crate::codegen::expression_codegen::ExpressionCompiler;
use crate::error::Result;
use corint_core::ast::rule::{Condition, ConditionGroup};
use corint_core::ast::WhenBlock;
use corint_core::ir::Instruction;

/// Compile a WhenBlock into condition instructions
pub(super) fn compile_when_block(when: &WhenBlock) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::new();

    // Handle condition_group or conditions first (these produce boolean values on stack)
    let condition_instructions = if let Some(ref group) = when.condition_group {
        compile_condition_group(group)?
    } else if let Some(ref conditions) = when.conditions {
        // Legacy format: treat as implicit AND
        compile_legacy_conditions(conditions)?
    } else {
        // No conditions - if event_type was specified, we're done
        // Otherwise, always true
        if when.event_type.is_none() {
            return Ok(vec![Instruction::LoadConst {
                value: corint_core::Value::Bool(true),
            }]);
        } else {
            Vec::new()
        }
    };

    // If we have both event_type and condition_group/conditions:
    // 1. CheckEventType is handled separately (it jumps directly, doesn't return value)
    // 2. We need to compile event_type check as a regular condition expression
    //    and combine it with condition_group using AND
    if let Some(ref event_type) = when.event_type {
        if !condition_instructions.is_empty() {
            // Compile event_type as a condition: event.type == "transaction"
            let event_type_expr = corint_core::ast::Expression::binary(
                corint_core::ast::Expression::field_access(vec![
                    "event".to_string(),
                    "type".to_string(),
                ]),
                corint_core::ast::Operator::Eq,
                corint_core::ast::Expression::literal(corint_core::Value::String(
                    event_type.clone(),
                )),
            );
            let event_type_instructions = ExpressionCompiler::compile(&event_type_expr)?;

            // Combine: event_type_condition AND condition_group
            instructions.extend(event_type_instructions);
            instructions.extend(condition_instructions);
            instructions.push(Instruction::BinaryOp {
                op: corint_core::ast::Operator::And,
            });
        } else {
            // Only event_type, no conditions - use CheckEventType for efficiency
            instructions.push(Instruction::CheckEventType {
                expected: event_type.clone(),
            });
            // CheckEventType doesn't leave a value on stack, so we need to push true
            // if it passes (it will jump if it fails)
            instructions.push(Instruction::LoadConst {
                value: corint_core::Value::Bool(true),
            });
        }
    } else {
        // No event_type, just conditions
        instructions.extend(condition_instructions);
    }

    Ok(instructions)
}

/// Compile condition group (new format)
fn compile_condition_group(group: &ConditionGroup) -> Result<Vec<Instruction>> {
    match group {
        ConditionGroup::All(conditions) => compile_all_conditions(conditions),
        ConditionGroup::Any(conditions) => compile_any_conditions(conditions),
        ConditionGroup::Not(conditions) => compile_not_conditions(conditions),
    }
}

/// Compile ALL (AND) conditions
fn compile_all_conditions(conditions: &[Condition]) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::new();

    // Handle empty conditions: ALL of nothing is true
    if conditions.is_empty() {
        instructions.push(Instruction::LoadConst {
            value: corint_core::Value::Bool(true),
        });
        return Ok(instructions);
    }

    for (i, condition) in conditions.iter().enumerate() {
        let cond_instructions = compile_condition(condition)?;
        instructions.extend(cond_instructions);

        // After first condition, AND with previous result
        if i > 0 {
            instructions.push(Instruction::BinaryOp {
                op: corint_core::ast::Operator::And,
            });
        }
    }

    Ok(instructions)
}

/// Compile ANY (OR) conditions
fn compile_any_conditions(conditions: &[Condition]) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::new();

    // Handle empty conditions: ANY of nothing is false
    if conditions.is_empty() {
        instructions.push(Instruction::LoadConst {
            value: corint_core::Value::Bool(false),
        });
        return Ok(instructions);
    }

    for (i, condition) in conditions.iter().enumerate() {
        let cond_instructions = compile_condition(condition)?;
        instructions.extend(cond_instructions);

        // After first condition, OR with previous result
        if i > 0 {
            instructions.push(Instruction::BinaryOp {
                op: corint_core::ast::Operator::Or,
            });
        }
    }

    Ok(instructions)
}

/// Compile NOT conditions
fn compile_not_conditions(conditions: &[Condition]) -> Result<Vec<Instruction>> {
    // Compile as ALL then negate
    let mut instructions = compile_all_conditions(conditions)?;

    instructions.push(Instruction::UnaryOp {
        op: corint_core::ast::UnaryOperator::Not,
    });

    Ok(instructions)
}

/// Compile a single condition
fn compile_condition(condition: &Condition) -> Result<Vec<Instruction>> {
    match condition {
        Condition::Expression(expr) => ExpressionCompiler::compile(expr),
        Condition::Group(group) => compile_condition_group(group),
    }
}

/// Compile legacy conditions (implicit AND)
fn compile_legacy_conditions(
    conditions: &[corint_core::ast::Expression],
) -> Result<Vec<Instruction>> {
    let mut instructions = Vec::new();

    for (i, expr) in conditions.iter().enumerate() {
        let expr_instructions = ExpressionCompiler::compile(expr)?;
        instructions.extend(expr_instructions);

        // After first expression, AND with previous result
        if i > 0 {
            instructions.push(Instruction::BinaryOp {
                op: corint_core::ast::Operator::And,
            });
        }
    }

    Ok(instructions)
}
