//! Instruction generation for pipeline steps
//!
//! Generates IR instructions for different types of pipeline steps.

use super::compiler::CompileContext;
use super::condition_compiler::compile_when_block;
use super::validator::get_next_step_id;
use crate::error::{CompileError, Result};
use corint_decision_model::ast::pipeline::{PipelineStep, StepDetails, StepNext};
use corint_decision_model::ir::Instruction;
use std::collections::HashMap;

/// Compile a single pipeline step
pub(super) fn compile_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    validate_step(step)?;
    match step.step_type.as_str() {
        "router" => compile_router_step(step, ctx),
        "ruleset" => compile_ruleset_step(step, ctx),
        "api" => compile_api_step(step, ctx),
        _ => Err(CompileError::UnsupportedFeature(format!(
            "step {}: {}",
            step.id, step.step_type
        ))),
    }
}

/// Reject unsupported semantics before reachability filtering, including dead nodes.
pub(super) fn validate_step(step: &PipelineStep) -> Result<()> {
    use corint_decision_model::ast::pipeline::ApiTarget;
    let reject = |field: &str| {
        CompileError::UnsupportedFeature(format!("step {}: {} is not implemented", step.id, field))
    };
    if step.when.is_some() {
        return Err(reject("when"));
    }
    if step.step_type != "router" && (step.routes.is_some() || step.default.is_some()) {
        return Err(reject("routes/default on a non-router"));
    }
    match (step.step_type.as_str(), &step.details) {
        ("router", StepDetails::Router {}) => {
            if step.next.is_some() {
                return Err(reject("router.next"));
            }
            for route in step.routes.iter().flatten() {
                let instructions = compile_when_block(&route.when)?;
                if instructions.iter().any(|i| match i {
                    Instruction::LoadResult { .. } => true,
                    Instruction::LoadField { path } => path.first().is_some_and(|p| {
                        ["results", "ruleset", "rule", "score", "total_score"].contains(&p.as_str())
                    }),
                    _ => false,
                }) {
                    return Err(reject("result-dependent router (use strict Core)"));
                }
            }
        }
        ("ruleset", StepDetails::Ruleset { ruleset }) if !ruleset.is_empty() => {}
        (
            "api",
            StepDetails::Api {
                api_target,
                params,
                on_error,
                min_success,
                ..
            },
        ) => {
            if !matches!(api_target, ApiTarget::Single { api } if !api.is_empty()) {
                return Err(reject("api.any/all/empty target"));
            }
            if params.is_some() {
                return Err(reject("api.params"));
            }
            if on_error.is_some() {
                return Err(reject("api.on_error"));
            }
            if min_success.is_some() {
                return Err(reject("api.min_success"));
            }
        }
        _ => return Err(reject(&step.step_type)),
    }
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
            ctx.instructions
                .push(Instruction::JumpIfFalse { offset: 0 });

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
    } else {
        ctx.add_pending_jump("end".into());
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
        use corint_decision_model::ast::pipeline::ApiTarget;

        let api_name = match api_target {
            ApiTarget::Single { api } => api.clone(),
            _ => return Err(CompileError::UnsupportedFeature("api.any/all".into())),
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
        ctx.instructions
            .push(Instruction::Store { name: output_var });
    }

    compile_next_jump(step, ctx)
}

/// Compile the unconditional next jump for a step
fn compile_next_jump(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    if let Some(next) = &step.next {
        let StepNext::StepId(next_id) = next;
        ctx.add_pending_jump(next_id.clone());
    } else {
        ctx.add_pending_jump("end".into());
    }
    Ok(())
}
