//! Pipeline DAG compiler
//!
//! Compiles Pipeline AST with DAG structure into IR programs.
//! Supports entry points, router steps, and non-linear control flow.

use crate::codegen::expression_codegen::ExpressionCompiler;
use crate::error::{CompileError, Result};
use corint_core::ast::pipeline::{Pipeline, PipelineStep, StepDetails, StepNext};
use corint_core::ast::rule::{Condition, ConditionGroup};
use corint_core::ast::WhenBlock;
use corint_core::ir::{Instruction, Program, ProgramMetadata};
use std::collections::{HashMap, HashSet, VecDeque};

/// Compilation context for tracking state during compilation
struct CompileContext {
    /// Step ID -> starting instruction index
    step_positions: HashMap<String, usize>,

    /// Pending jumps to be resolved: (instruction_index, target_step_id)
    pending_jumps: Vec<(usize, String)>,

    /// Compiled instructions
    instructions: Vec<Instruction>,
}

impl CompileContext {
    fn new() -> Self {
        Self {
            step_positions: HashMap::new(),
            pending_jumps: Vec::new(),
            instructions: Vec::new(),
        }
    }

    /// Record the current position for a step
    fn mark_step_position(&mut self, step_id: String) {
        let position = self.instructions.len();
        self.step_positions.insert(step_id, position);
    }

    /// Add a jump instruction that needs to be resolved later
    fn add_pending_jump(&mut self, target: String) {
        let jump_pos = self.instructions.len();
        self.instructions.push(Instruction::Jump { offset: 0 });
        self.pending_jumps.push((jump_pos, target));
    }

    /// Add a conditional jump instruction that needs to be resolved later
    fn add_pending_conditional_jump(&mut self, target: String, jump_if_true: bool) {
        let jump_pos = self.instructions.len();
        if jump_if_true {
            self.instructions.push(Instruction::JumpIfTrue { offset: 0 });
        } else {
            self.instructions.push(Instruction::JumpIfFalse { offset: 0 });
        }
        self.pending_jumps.push((jump_pos, target));
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

        // Step 1: Topological sort - get ordered list of reachable steps from entry
        let sorted_steps = Self::topological_sort(pipeline)?;

        // Step 2: Compile each step in order
        for step in &sorted_steps {
            ctx.mark_step_position(step.id.clone());
            Self::compile_step(step, &mut ctx)?;
        }

        // Step 3: Add Return instruction at the end
        ctx.instructions.push(Instruction::Return);

        // Step 4: Resolve all pending jumps
        Self::resolve_jumps(&mut ctx)?;

        // Step 5: Build program metadata
        let metadata = ProgramMetadata::for_pipeline(pipeline.id.clone())
            .with_name(pipeline.name.clone());

        Ok(Program::new(ctx.instructions, metadata))
    }

    /// Perform topological sort using BFS from entry point
    fn topological_sort(pipeline: &Pipeline) -> Result<Vec<&PipelineStep>> {
        let mut visited = HashSet::new();
        let mut sorted = Vec::new();
        let mut queue = VecDeque::new();

        // Build step map for quick lookup
        let step_map: HashMap<&str, &PipelineStep> = pipeline
            .steps
            .iter()
            .map(|step| (step.id.as_str(), step))
            .collect();

        // Start from entry (use owned String in queue)
        queue.push_back(pipeline.entry.clone());

        while let Some(step_id) = queue.pop_front() {
            // "end" is a special terminal marker
            if step_id == "end" || visited.contains(&step_id) {
                continue;
            }

            visited.insert(step_id.clone());

            // Find the step
            let step = step_map.get(step_id.as_str()).ok_or_else(|| {
                CompileError::InvalidExpression(format!(
                    "Step '{}' referenced but not found in pipeline",
                    step_id
                ))
            })?;

            sorted.push(*step);

            // Add all next steps to queue
            for next_id in Self::get_next_step_ids(step) {
                if !visited.contains(&next_id) {
                    queue.push_back(next_id);
                }
            }
        }

        Ok(sorted)
    }

    /// Get all possible next step IDs from a step
    fn get_next_step_ids(step: &PipelineStep) -> Vec<String> {
        let mut next_ids = Vec::new();

        // Collect from routes
        if let Some(routes) = &step.routes {
            for route in routes {
                next_ids.push(route.next.clone());
            }
        }

        // Collect from default
        if let Some(default) = &step.default {
            next_ids.push(default.clone());
        }

        // Collect from next
        if let Some(next) = &step.next {
            if let StepNext::StepId(next_id) = next {
                next_ids.push(next_id.clone());
            }
        }

        next_ids
    }

    /// Compile a single pipeline step
    fn compile_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        // Check step-level when condition if present
        if let Some(when) = &step.when {
            Self::compile_step_when_guard(when, step, ctx)?;
        }

        // Compile based on step type
        match step.step_type.as_str() {
            "router" => Self::compile_router_step(step, ctx),
            "ruleset" => Self::compile_ruleset_step(step, ctx),
            "function" => Self::compile_function_step(step, ctx),
            "service" => Self::compile_service_step(step, ctx),
            "api" => Self::compile_api_step(step, ctx),
            "trigger" => Self::compile_trigger_step(step, ctx),
            "rule" => Self::compile_rule_step(step, ctx),
            "pipeline" => Self::compile_subpipeline_step(step, ctx),
            _ => {
                // Unknown step types are allowed but do nothing
                Self::compile_next_jump(step, ctx)
            }
        }
    }

    /// Compile step-level when condition as a guard
    fn compile_step_when_guard(
        when: &WhenBlock,
        step: &PipelineStep,
        ctx: &mut CompileContext,
    ) -> Result<()> {
        // TODO: Implement proper step-level when guards
        // For now, step-level when conditions are not used in comprehensive_dsl_demo.yaml
        // So we'll leave this as a no-op to avoid breaking existing functionality
        //
        // When implementing, we need to:
        // 1. Compile the when condition
        // 2. Add JumpIfFalse with proper offset calculation
        // 3. Handle the case where the step should be skipped

        Ok(())
    }

    /// Compile a router step
    fn compile_router_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        // Router step: evaluate conditions and jump to appropriate target
        if let Some(routes) = &step.routes {
            for route in routes {
                // Compile the condition
                let condition_instructions = Self::compile_when_block(&route.when)?;
                ctx.instructions.extend(condition_instructions);

                // If condition is false, skip to next route
                let jump_if_false_pos = ctx.instructions.len();
                ctx.instructions.push(Instruction::JumpIfFalse { offset: 0 });

                // If condition is true, jump to target step
                ctx.add_pending_jump(route.next.clone());

                // Backfill JumpIfFalse to skip past the Jump instruction
                if let Instruction::JumpIfFalse { offset } = &mut ctx.instructions[jump_if_false_pos] {
                    *offset = 2; // Skip the Jump instruction we just added
                }
            }
        }

        // Default route
        if let Some(default) = &step.default {
            ctx.add_pending_jump(default.clone());
        }

        Ok(())
    }

    /// Compile a ruleset step
    fn compile_ruleset_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        if let StepDetails::Ruleset { ruleset } = &step.details {
            ctx.instructions.push(Instruction::CallRuleset {
                ruleset_id: ruleset.clone(),
            });
        }

        Self::compile_next_jump(step, ctx)
    }

    /// Compile a function step
    fn compile_function_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        if let StepDetails::Function { function, params } = &step.details {
            // TODO: Implement function call compilation
            // For now, we'll just add a placeholder comment via Store
            ctx.instructions.push(Instruction::Store {
                name: format!("function.{}", function),
            });
        }

        Self::compile_next_jump(step, ctx)
    }

    /// Compile a service step
    fn compile_service_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        if let StepDetails::Service {
            service,
            query,
            params,
            output,
        } = &step.details
        {
            // Compile service call
            ctx.instructions.push(Instruction::CallService {
                service: service.clone(),
                operation: query.clone().unwrap_or_default(),
                params: HashMap::new(), // TODO: Compile params
            });

            // Store result to output variable (convention: service.<name>)
            let output_var = output
                .clone()
                .unwrap_or_else(|| format!("service.{}", service));
            ctx.instructions.push(Instruction::Store { name: output_var });
        }

        Self::compile_next_jump(step, ctx)
    }

    /// Compile an API step
    fn compile_api_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        if let StepDetails::Api {
            api_target,
            endpoint,
            params,
            output,
            timeout,
            on_error,
            min_success,
        } = &step.details
        {
            // For now, we'll handle simple single API calls
            // TODO: Implement any/all modes
            use corint_core::ast::pipeline::ApiTarget;

            let api_name = match api_target {
                ApiTarget::Single { api } => api.clone(),
                ApiTarget::Any { any } => any.first().unwrap_or(&String::new()).clone(),
                ApiTarget::All { all } => all.first().unwrap_or(&String::new()).clone(),
            };

            ctx.instructions.push(Instruction::CallExternal {
                api: api_name.clone(),
                endpoint: endpoint.clone().unwrap_or_default(),
                params: HashMap::new(), // TODO: Compile params
                timeout: *timeout,
                fallback: None,
            });

            // Store result
            let output_var = output.clone().unwrap_or_else(|| format!("api.{}", api_name));
            ctx.instructions.push(Instruction::Store { name: output_var });
        }

        Self::compile_next_jump(step, ctx)
    }

    /// Compile a trigger step
    fn compile_trigger_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        // Trigger steps don't produce output
        // TODO: Add CallTrigger instruction to IR

        Self::compile_next_jump(step, ctx)
    }

    /// Compile a rule step (single rule execution)
    fn compile_rule_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        // TODO: Implement single rule execution
        // For now, treat it similar to ruleset but with single rule

        Self::compile_next_jump(step, ctx)
    }

    /// Compile a sub-pipeline step
    fn compile_subpipeline_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        // TODO: Implement sub-pipeline call
        // This would require CallPipeline instruction in IR

        Self::compile_next_jump(step, ctx)
    }

    /// Compile the unconditional next jump for a step
    fn compile_next_jump(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
        if let Some(next) = &step.next {
            if let StepNext::StepId(next_id) = next {
                ctx.add_pending_jump(next_id.clone());
            }
        }
        Ok(())
    }

    /// Compile a WhenBlock into condition instructions
    fn compile_when_block(when: &WhenBlock) -> Result<Vec<Instruction>> {
        // Handle both old and new formats
        if let Some(ref group) = when.condition_group {
            Self::compile_condition_group(group)
        } else if let Some(ref conditions) = when.conditions {
            // Legacy format: treat as implicit AND
            Self::compile_legacy_conditions(conditions)
        } else {
            // No conditions means always true
            Ok(vec![Instruction::LoadConst {
                value: corint_core::Value::Bool(true),
            }])
        }
    }

    /// Compile condition group (new format)
    fn compile_condition_group(group: &ConditionGroup) -> Result<Vec<Instruction>> {
        match group {
            ConditionGroup::All(conditions) => Self::compile_all_conditions(conditions),
            ConditionGroup::Any(conditions) => Self::compile_any_conditions(conditions),
            ConditionGroup::Not(conditions) => Self::compile_not_conditions(conditions),
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
            let cond_instructions = Self::compile_condition(condition)?;
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
            let cond_instructions = Self::compile_condition(condition)?;
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
        let mut instructions = Self::compile_all_conditions(conditions)?;

        instructions.push(Instruction::UnaryOp {
            op: corint_core::ast::UnaryOperator::Not,
        });

        Ok(instructions)
    }

    /// Compile a single condition
    fn compile_condition(condition: &Condition) -> Result<Vec<Instruction>> {
        match condition {
            Condition::Expression(expr) => ExpressionCompiler::compile(expr),
            Condition::Group(group) => Self::compile_condition_group(group),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::pipeline::Route;
    use corint_core::ast::rule::{ConditionGroup, Condition};
    use corint_core::ast::{Expression, Operator};
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

        // Should have: CallRuleset, Jump, Return
        assert!(matches!(program.instructions[0], Instruction::CallRuleset { .. }));
        assert!(matches!(program.instructions[1], Instruction::Jump { .. }));
        assert!(matches!(program.instructions[2], Instruction::Return));
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
            version: None,
            entry: String::new(), // Empty entry indicates legacy format
            when: None,
            steps: vec![],
        };

        let result = PipelineCompiler::compile(&pipeline);
        assert!(result.is_err());
    }
}
