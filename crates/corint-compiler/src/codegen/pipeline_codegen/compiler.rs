//! Core pipeline compiler
//!
//! Main compilation logic for pipelines with DAG structure.

use crate::error::{CompileError, Result};
use super::condition_compiler::compile_when_block;
use super::instruction_gen::compile_step;
use super::metadata_builder::build_steps_metadata;
use super::validator::topological_sort;
use corint_core::ast::pipeline::Pipeline;
use corint_core::ir::{Instruction, Program, ProgramMetadata};
use std::collections::HashMap;

/// Compilation context for tracking state during compilation
pub(super) struct CompileContext {
    /// Step ID -> starting instruction index
    pub(super) step_positions: HashMap<String, usize>,

    /// Pending jumps to be resolved: (instruction_index, target_step_id)
    pub(super) pending_jumps: Vec<(usize, String)>,

    /// Compiled instructions
    pub(super) instructions: Vec<Instruction>,

    /// Position of pipeline when guard jump instruction (if present)
    pub(super) pipeline_when_guard_pos: Option<usize>,
}

impl CompileContext {
    pub(super) fn new() -> Self {
        Self {
            step_positions: HashMap::new(),
            pending_jumps: Vec::new(),
            instructions: Vec::new(),
            pipeline_when_guard_pos: None,
        }
    }

    /// Record the current position for a step
    pub(super) fn mark_step_position(&mut self, step_id: String) {
        let position = self.instructions.len();
        self.step_positions.insert(step_id, position);
    }

    /// Add a jump instruction that needs to be resolved later
    pub(super) fn add_pending_jump(&mut self, target: String) {
        let jump_pos = self.instructions.len();
        self.instructions.push(Instruction::Jump { offset: 0 });
        self.pending_jumps.push((jump_pos, target));
    }

    /// Add a conditional jump instruction that needs to be resolved later
    pub(super) fn add_pending_conditional_jump(&mut self, target: String, jump_if_true: bool) {
        let jump_pos = self.instructions.len();
        if jump_if_true {
            self.instructions.push(Instruction::JumpIfTrue { offset: 0 });
        } else {
            self.instructions.push(Instruction::JumpIfFalse { offset: 0 });
        }
        self.pending_jumps.push((jump_pos, target));
    }

    /// Mark the position of pipeline when guard jump instruction
    pub(super) fn mark_pipeline_when_guard(&mut self, pos: usize) {
        self.pipeline_when_guard_pos = Some(pos);
    }
}

/// Pipeline compiler
pub struct PipelineCompiler;

impl PipelineCompiler {
    /// Compile a pipeline into an IR program
    pub fn compile(pipeline: &Pipeline) -> Result<Program> {
        // Check if this is the new DAG format
        if pipeline.entry.is_empty() {
            return Err(CompileError::UnsupportedFeature(
                "Legacy pipeline format (without entry point) is not supported. Please use new pipeline format with entry point.".to_string()
            ));
        }

        let mut ctx = CompileContext::new();

        // Step 0: Compile pipeline-level when condition if present
        // This acts as a guard - if condition fails, skip entire pipeline
        if let Some(ref when) = pipeline.when {
            let condition_instructions = compile_when_block(when)?;
            ctx.instructions.extend(condition_instructions);

            // If condition is false, jump to end (skip entire pipeline)
            let jump_if_false_pos = ctx.instructions.len();
            ctx.instructions.push(Instruction::JumpIfFalse { offset: 0 });

            // Mark the position where we'll jump to (after all steps)
            // We'll backfill this offset after we know the total instruction count
            ctx.mark_pipeline_when_guard(jump_if_false_pos);
        }

        // Step 1: Topological sort - get ordered list of reachable steps from entry
        let sorted_steps = topological_sort(pipeline)?;

        // Step 2: Compile each step in order
        for step in &sorted_steps {
            ctx.mark_step_position(step.id.clone());
            compile_step(step, &mut ctx)?;
        }

        // Step 3: Add Return instruction at the end
        ctx.instructions.push(Instruction::Return);

        // Step 4: Resolve pipeline when guard jump (if present)
        // Jump to Return instruction if when condition fails
        if let Some(guard_pos) = ctx.pipeline_when_guard_pos {
            let end_pos = ctx.instructions.len() - 1; // Position of Return instruction
            let offset = (end_pos as isize) - (guard_pos as isize);
            if let Instruction::JumpIfFalse { offset: ref mut o } = ctx.instructions[guard_pos] {
                *o = offset;
            }
        }

        // Step 5: Resolve all pending jumps
        resolve_jumps(&mut ctx)?;

        // Step 6: Build program metadata
        let mut metadata = ProgramMetadata::for_pipeline(pipeline.id.clone())
            .with_name(pipeline.name.clone());

        // Step 7: Add step information to metadata for tracing
        let steps_json = build_steps_metadata(&sorted_steps);
        metadata = metadata.with_custom("steps_json".to_string(), steps_json);

        Ok(Program::new(ctx.instructions, metadata))
    }
}

/// Resolve all pending jumps by calculating offsets
fn resolve_jumps(ctx: &mut CompileContext) -> Result<()> {
    let return_pos = ctx.instructions.len() - 1; // Position of Return instruction

    for (jump_pos, target_id) in &ctx.pending_jumps {
        // "end" is special - jump to Return instruction
        let target_pos = if target_id == "end" {
            return_pos
        } else {
            *ctx.step_positions.get(target_id).ok_or_else(|| {
                CompileError::InvalidExpression(format!(
                    "Unknown step target: '{}'",
                    target_id
                ))
            })?
        };

        // Calculate relative offset
        let offset = (target_pos as isize) - (*jump_pos as isize);

        // Update the jump instruction
        match &mut ctx.instructions[*jump_pos] {
            Instruction::Jump { offset: o } => *o = offset,
            Instruction::JumpIfTrue { offset: o } => *o = offset,
            Instruction::JumpIfFalse { offset: o } => *o = offset,
            _ => {
                return Err(CompileError::InvalidExpression(
                    "Expected jump instruction during backfill".to_string(),
                ))
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::pipeline::{PipelineStep, Route};
    use corint_core::ast::rule::{Condition, ConditionGroup};
    use corint_core::ast::{Expression, Operator, WhenBlock};
    use corint_core::Value;

    #[test]
    fn test_compile_simple_pipeline() {
        // Create a simple pipeline with entry -> ruleset -> end
        let pipeline = Pipeline::new(
            "test_pipeline".to_string(),
            "Test Pipeline".to_string(),
            "step1".to_string(),
        )
        .add_step(
            PipelineStep::ruleset(
                "step1".to_string(),
                "Run Rules".to_string(),
                "fraud_rules".to_string(),
            )
            .with_next("end".to_string()),
        );

        let program = PipelineCompiler::compile(&pipeline).unwrap();

        assert_eq!(program.metadata.source_type, "pipeline");
        assert!(!program.instructions.is_empty());

        // Should have: MarkStepExecuted, CallRuleset, Jump, Return
        assert!(matches!(program.instructions[0], Instruction::MarkStepExecuted { .. }));
        assert!(matches!(program.instructions[1], Instruction::CallRuleset { .. }));
        assert!(matches!(program.instructions[2], Instruction::Jump { .. }));
        assert!(matches!(program.instructions[3], Instruction::Return));
    }

    #[test]
    fn test_compile_router_pipeline() {
        // Create pipeline with router
        let when_block = WhenBlock {
            event_type: None,
            condition_group: Some(ConditionGroup::All(vec![Condition::Expression(
                Expression::binary(
                    Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
                    Operator::Gt,
                    Expression::literal(Value::Number(10000.0)),
                ),
            )])),
            conditions: None,
        };

        let route = Route::new("high_risk".to_string(), when_block);

        let pipeline = Pipeline::new(
            "fraud_detection".to_string(),
            "Fraud Detection".to_string(),
            "router1".to_string(),
        )
        .add_step(
            PipelineStep::router("router1".to_string(), "Risk Router".to_string())
                .with_routes(vec![route])
                .with_default("low_risk".to_string()),
        )
        .add_step(
            PipelineStep::ruleset(
                "high_risk".to_string(),
                "High Risk Rules".to_string(),
                "high_risk_rules".to_string(),
            )
            .with_next("end".to_string()),
        )
        .add_step(
            PipelineStep::ruleset(
                "low_risk".to_string(),
                "Low Risk Rules".to_string(),
                "low_risk_rules".to_string(),
            )
            .with_next("end".to_string()),
        );

        let program = PipelineCompiler::compile(&pipeline).unwrap();

        assert!(!program.instructions.is_empty());
        // Should contain LoadField, LoadConst, Compare, JumpIfFalse, Jump instructions
        assert!(program.instructions.iter().any(|i| matches!(i, Instruction::LoadField { .. })));
        assert!(program.instructions.iter().any(|i| matches!(i, Instruction::Compare { .. })));
        assert!(program.instructions.iter().any(|i| matches!(i, Instruction::JumpIfFalse { .. })));
    }

    #[test]
    fn test_legacy_pipeline_format_not_supported() {
        let pipeline = Pipeline {
            id: "legacy".to_string(),
            name: "Legacy".to_string(),
            description: None,
            entry: String::new(), // Empty entry indicates legacy format
            when: None,
            steps: vec![],
            metadata: None,
        };

        let result = PipelineCompiler::compile(&pipeline);
        assert!(result.is_err());
    }
}
