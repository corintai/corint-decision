//! Instruction generation for pipeline steps
//!
//! Generates IR instructions for different types of pipeline steps.

use crate::error::Result;
use super::compiler::CompileContext;
use super::condition_compiler::compile_when_block;
use super::validator::get_next_step_id;
use corint_core::ast::pipeline::{PipelineStep, StepDetails, StepNext};
use corint_core::ast::WhenBlock;
use corint_core::ir::Instruction;
use std::collections::HashMap;

/// Compile a single pipeline step
pub(super) fn compile_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    // Check step-level when condition if present
    if let Some(when) = &step.when {
        compile_step_when_guard(when, step, ctx)?;
    }

    // Compile based on step type
    match step.step_type.as_str() {
        "router" => compile_router_step(step, ctx),
        "ruleset" => compile_ruleset_step(step, ctx),
        "function" => compile_function_step(step, ctx),
        "service" => compile_service_step(step, ctx),
        "api" => compile_api_step(step, ctx),
        "trigger" => compile_trigger_step(step, ctx),
        "rule" => compile_rule_step(step, ctx),
        "pipeline" => compile_subpipeline_step(step, ctx),
        _ => {
            // Unknown step types are allowed but do nothing
            compile_next_jump(step, ctx)
        }
    }
}

/// Compile step-level when condition as a guard
fn compile_step_when_guard(
    _when: &WhenBlock,
    _step: &PipelineStep,
    _ctx: &mut CompileContext,
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
        for (route_idx, route) in routes.iter().enumerate() {
            // Compile the condition
            let condition_instructions = compile_when_block(&route.when)?;
            ctx.instructions.extend(condition_instructions);

            // If condition is false, skip to next route
            let jump_if_false_pos = ctx.instructions.len();
            ctx.instructions.push(Instruction::JumpIfFalse { offset: 0 });

            // If condition is true:
            // 1. Mark step as executed with the selected route
            ctx.instructions.push(Instruction::MarkStepExecuted {
                step_id: step.id.clone(),
                next_step_id: Some(route.next.clone()),
                route_index: Some(route_idx),
                is_default_route: false,
            });

            // 2. Jump to target step
            ctx.add_pending_jump(route.next.clone());

            // Backfill JumpIfFalse to skip past the MarkStepExecuted and Jump instructions
            if let Instruction::JumpIfFalse { offset } = &mut ctx.instructions[jump_if_false_pos] {
                *offset = 3; // Skip MarkStepExecuted + Jump
            }
        }
    }

    // Default route
    if let Some(default) = &step.default {
        // Mark step as executed with default route
        ctx.instructions.push(Instruction::MarkStepExecuted {
            step_id: step.id.clone(),
            next_step_id: Some(default.clone()),
            route_index: None,
            is_default_route: true,
        });
        ctx.add_pending_jump(default.clone());
    }

    Ok(())
}

/// Compile a ruleset step
fn compile_ruleset_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    // Get next step ID for tracing
    let next_step_id = step.next.as_ref().map(|n| {
        let StepNext::StepId(id) = n;
        id.clone()
    });

    // Mark step as executed
    ctx.instructions.push(Instruction::MarkStepExecuted {
        step_id: step.id.clone(),
        next_step_id: next_step_id.clone(),
        route_index: None,
        is_default_route: false,
    });

    if let StepDetails::Ruleset { ruleset } = &step.details {
        ctx.instructions.push(Instruction::CallRuleset {
            ruleset_id: ruleset.clone(),
        });
    }

    compile_next_jump(step, ctx)
}

/// Compile a function step
fn compile_function_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    let next_step_id = get_next_step_id(step);

    // Mark step as executed
    ctx.instructions.push(Instruction::MarkStepExecuted {
        step_id: step.id.clone(),
        next_step_id: next_step_id.clone(),
        route_index: None,
        is_default_route: false,
    });

    if let StepDetails::Function { function, params: _ } = &step.details {
        // TODO: Implement function call compilation
        // For now, we'll just add a placeholder comment via Store
        ctx.instructions.push(Instruction::Store {
            name: format!("function.{}", function),
        });
    }

    compile_next_jump(step, ctx)
}

/// Compile a service step
fn compile_service_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    let next_step_id = get_next_step_id(step);

    // Mark step as executed
    ctx.instructions.push(Instruction::MarkStepExecuted {
        step_id: step.id.clone(),
        next_step_id: next_step_id.clone(),
        route_index: None,
        is_default_route: false,
    });

    if let StepDetails::Service {
        service,
        query,
        params: _,
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

    compile_next_jump(step, ctx)
}

/// Compile an API step
fn compile_api_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    let next_step_id = get_next_step_id(step);

    // Mark step as executed
    ctx.instructions.push(Instruction::MarkStepExecuted {
        step_id: step.id.clone(),
        next_step_id: next_step_id.clone(),
        route_index: None,
        is_default_route: false,
    });

    if let StepDetails::Api {
        api_target,
        endpoint,
        params: _,
        output,
        timeout,
        on_error: _,
        min_success: _,
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
        let endpoint_name = endpoint.clone().unwrap_or_default();
        let output_var = output.clone().unwrap_or_else(|| {
            if !endpoint_name.is_empty() {
                format!("api.{}.{}", api_name, endpoint_name)
            } else {
                format!("api.{}", api_name)
            }
        });
        ctx.instructions.push(Instruction::Store { name: output_var });
    }

    compile_next_jump(step, ctx)
}

/// Compile a trigger step
fn compile_trigger_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    let next_step_id = get_next_step_id(step);

    // Mark step as executed
    ctx.instructions.push(Instruction::MarkStepExecuted {
        step_id: step.id.clone(),
        next_step_id,
        route_index: None,
        is_default_route: false,
    });

    // Trigger steps don't produce output
    // TODO: Add CallTrigger instruction to IR

    compile_next_jump(step, ctx)
}

/// Compile a rule step (single rule execution)
fn compile_rule_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    let next_step_id = get_next_step_id(step);

    // Mark step as executed
    ctx.instructions.push(Instruction::MarkStepExecuted {
        step_id: step.id.clone(),
        next_step_id,
        route_index: None,
        is_default_route: false,
    });

    // TODO: Implement single rule execution
    // For now, treat it similar to ruleset but with single rule

    compile_next_jump(step, ctx)
}

/// Compile a sub-pipeline step
fn compile_subpipeline_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    let next_step_id = get_next_step_id(step);

    // Mark step as executed
    ctx.instructions.push(Instruction::MarkStepExecuted {
        step_id: step.id.clone(),
        next_step_id,
        route_index: None,
        is_default_route: false,
    });

    // TODO: Implement sub-pipeline call
    // This would require CallPipeline instruction in IR

    compile_next_jump(step, ctx)
}

/// Compile the unconditional next jump for a step
fn compile_next_jump(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    if let Some(next) = &step.next {
        let StepNext::StepId(next_id) = next;
        ctx.add_pending_jump(next_id.clone());
    }
    Ok(())
}
