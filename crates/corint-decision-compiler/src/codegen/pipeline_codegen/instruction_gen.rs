//! Instruction generation for pipeline steps
//!
//! Generates IR instructions for different types of pipeline steps.

use super::compiler::CompileContext;
use super::condition_compiler::compile_when_block;
use super::validator::get_next_step_id;
use crate::codegen::expression_codegen::ExpressionCompiler;
use crate::error::{CompileError, Result};
use corint_decision_model::ast::pipeline::{PipelineStep, StepDetails, StepNext};
use corint_decision_model::ir::Instruction;

/// Compile a single pipeline step
pub(super) fn compile_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    if !ctx.strict_core {
        validate_step(step)?;
    }
    if ctx.strict_core {
        if let Some(when) = &step.when {
            ctx.instructions.extend(compile_when_block(when)?);
            let pos = ctx.instructions.len();
            ctx.guard_positions.insert(step.id.clone(), pos);
            ctx.instructions.push(Instruction::JumpIfTrue { offset: 3 });
            let resource_id = match &step.details {
                StepDetails::Ruleset { ruleset } => Some(ruleset.clone()),
                StepDetails::Rule { rule } => Some(rule.clone()),
                StepDetails::SubPipeline { pipeline_id } => Some(pipeline_id.clone()),
                _ => None,
            };
            ctx.instructions.push(Instruction::SkipStep {
                step_id: step.id.clone(),
                resource_id,
            });
            ctx.add_pending_jump(
                get_next_step_id(step)
                    .or_else(|| step.default.clone())
                    .unwrap_or_else(|| "end".into()),
            );
        }
    }
    match step.step_type.as_str() {
        "router" => compile_router_step(step, ctx),
        "ruleset" => compile_ruleset_step(step, ctx),
        "service" if !ctx.strict_core => compile_service_step(step, ctx),
        "rule" | "pipeline" if ctx.strict_core => {
            let resource_id = match &step.details {
                StepDetails::Rule { rule } => rule.clone(),
                StepDetails::SubPipeline { pipeline_id } => pipeline_id.clone(),
                _ => return Err(CompileError::InvalidExpression("Invalid Core call".into())),
            };
            ctx.instructions.push(Instruction::MarkStepExecuted {
                step_id: step.id.clone(),
                next_step_id: get_next_step_id(step),
                route_index: None,
                is_default_route: false,
            });
            ctx.instructions.push(Instruction::CallResource {
                resource_type: step.step_type.clone(),
                resource_id,
            });
            compile_next_jump(step, ctx)
        }
        _ => Err(CompileError::UnsupportedFeature(format!(
            "step {}: {}",
            step.id, step.step_type
        ))),
    }
}

/// Reject unsupported semantics before reachability filtering, including dead nodes.
pub(super) fn validate_step(step: &PipelineStep) -> Result<()> {
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
            "service",
            StepDetails::Service {
                service,
                operation,
                timeout_ms,
                params,
                output,
            },
        ) => {
            if service.trim().is_empty() || operation.trim().is_empty() {
                return Err(reject("service/operation must be non-empty"));
            }
            let output_path = output
                .clone()
                .unwrap_or_else(|| format!("service.{}", step.id));
            if !["service.", "vars."]
                .iter()
                .any(|prefix| output_path.starts_with(prefix))
                || output_path.split('.').any(|part| part.is_empty())
            {
                return Err(reject("output must be a path under service or vars"));
            }
            for (name, expression) in params.iter().flat_map(|params| params.iter()) {
                if name.trim().is_empty() {
                    return Err(reject("parameter names must be non-empty"));
                }
                ExpressionCompiler::compile(expression)?;
            }
            if *timeout_ms == Some(0) {
                return Err(reject("timeout_ms must be positive"));
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

/// Compile a service with deterministic parameter evaluation order.
fn compile_service_step(step: &PipelineStep, ctx: &mut CompileContext) -> Result<()> {
    let (service, operation, params, output, timeout_ms) = match &step.details {
        StepDetails::Service {
            service,
            operation,
            params,
            output,
            timeout_ms,
        } => (
            service.clone(),
            operation.clone(),
            params,
            output
                .clone()
                .unwrap_or_else(|| format!("service.{}", step.id)),
            *timeout_ms,
        ),
        _ => return Err(CompileError::UnsupportedFeature("service target".into())),
    };
    if !["service.", "vars."]
        .iter()
        .any(|prefix| output.starts_with(prefix))
        || output.split('.').any(|part| part.is_empty())
    {
        return Err(CompileError::InvalidExpression(
            "Service output must be a non-empty path under service or vars".into(),
        ));
    }
    ctx.instructions.push(Instruction::MarkStepExecuted {
        step_id: step.id.clone(),
        next_step_id: get_next_step_id(step),
        route_index: None,
        is_default_route: false,
    });
    let mut parameters: Vec<_> = params.iter().flat_map(|p| p.iter()).collect();
    if parameters.iter().any(|(name, _)| name.trim().is_empty()) {
        return Err(CompileError::InvalidExpression(
            "Service parameter names must be non-empty".into(),
        ));
    }
    parameters.sort_by(|a, b| a.0.cmp(b.0));
    for (_, expression) in &parameters {
        ctx.instructions
            .extend(ExpressionCompiler::compile(expression)?);
    }
    ctx.instructions.push(Instruction::InvokeService {
        service,
        operation,
        parameter_names: parameters
            .into_iter()
            .map(|(name, _)| name.clone())
            .collect(),
        timeout_ms,
    });
    ctx.instructions.push(Instruction::Store { name: output });
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
