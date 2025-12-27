//! Pipeline validation and topological sorting
//!
//! Performs validation and topological sorting of pipeline steps.

use crate::error::{CompileError, Result};
use corint_core::ast::pipeline::{Pipeline, PipelineStep, StepNext};
use std::collections::{HashMap, HashSet, VecDeque};

/// Perform topological sort using BFS from entry point
pub(super) fn topological_sort(pipeline: &Pipeline) -> Result<Vec<&PipelineStep>> {
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
        for next_id in get_next_step_ids(step) {
            if !visited.contains(&next_id) {
                queue.push_back(next_id);
            }
        }
    }

    Ok(sorted)
}

/// Get all possible next step IDs from a step
pub(super) fn get_next_step_ids(step: &PipelineStep) -> Vec<String> {
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
        let StepNext::StepId(next_id) = next;
        next_ids.push(next_id.clone());
    }

    next_ids
}

/// Get the next step ID from a step
pub(super) fn get_next_step_id(step: &PipelineStep) -> Option<String> {
    step.next.as_ref().map(|n| {
        let StepNext::StepId(id) = n;
        id.clone()
    })
}
