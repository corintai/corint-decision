//! Pipeline semantic analyzer
//!
//! Implements validation rules E001-E007 and W001-W003 for Pipeline DSL as specified in mypipeline.yml

use crate::error::{CompileError, Result};
use corint_core::ast::pipeline::{Pipeline, PipelineStep};
use std::collections::{HashMap, HashSet, VecDeque};

/// Warning diagnostic
#[derive(Debug, Clone)]
pub struct Warning {
    pub code: String,
    pub message: String,
}

/// Pipeline analyzer result
#[derive(Debug)]
pub struct PipelineAnalysisResult {
    pub errors: Vec<CompileError>,
    pub warnings: Vec<Warning>,
}

impl PipelineAnalysisResult {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn add_error(&mut self, error: CompileError) {
        self.errors.push(error);
    }

    fn add_warning(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(Warning {
            code: code.into(),
            message: message.into(),
        });
    }

    fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    fn into_result(self) -> Result<()> {
        if self.has_errors() {
            // Return the first error
            Err(self.errors.into_iter().next().unwrap())
        } else {
            Ok(())
        }
    }
}

/// Analyze a pipeline for the new format with validation rules
pub fn analyze_new_pipeline(pipeline: &Pipeline) -> Result<Vec<Warning>> {
    let mut result = PipelineAnalysisResult::new();

    // E001: entry field is required
    if pipeline.entry.is_empty() {
        result.add_error(CompileError::InvalidExpression(
            "[E001] Pipeline entry point is required for DAG compilation".to_string(),
        ));
        // Fatal error - cannot continue without entry
        let warnings = result.warnings.clone();
        return result.into_result().map(|_| warnings);
    }

    // Build step ID map
    let mut step_map = HashMap::new();
    let mut step_ids = HashSet::new();

    for step in &pipeline.steps {
        // E003: step.id must be unique
        if !step_ids.insert(&step.id) {
            result.add_error(CompileError::InvalidExpression(format!(
                "[E003] Duplicate step ID: '{}'",
                step.id
            )));
            continue;
        }

        step_map.insert(step.id.clone(), step);
    }

    // E002: entry must match a step.id
    if !step_map.contains_key(&pipeline.entry) {
        result.add_error(CompileError::InvalidExpression(format!(
            "[E002] Pipeline entry '{}' does not match any step ID",
            pipeline.entry
        )));
    }

    // Validate each step
    for step in &pipeline.steps {
        validate_step(step, &step_map, &mut result);
    }

    // E007: Check for circular dependencies
    if let Err(e) = check_circular_dependencies(pipeline, &step_map) {
        result.add_error(e);
    }

    // W001: Check for unreachable steps
    let reachable = find_reachable_steps(pipeline, &step_map);
    for step_id in step_ids {
        if !reachable.contains(step_id) {
            result.add_warning(
                "W001",
                format!("Unreachable step '{}' (not in execution path from entry)", step_id),
            );
        }
    }

    // W002: Check for dead ends (steps without next/routes/default, except if they're terminal)
    check_dead_ends(pipeline, &step_map, &mut result);

    // W003: Check for unused routes (this requires more complex analysis)
    // For now, we'll add a placeholder - full implementation would require flow analysis
    // check_unused_routes(pipeline, &step_map, &mut result);

    // Return result
    let warnings = result.warnings.clone();
    result.into_result().map(|_| warnings)
}

/// Validate a single step
fn validate_step(
    step: &PipelineStep,
    step_map: &HashMap<String, &PipelineStep>,
    result: &mut PipelineAnalysisResult,
) {
    // E004 & E005: Router step validations
    if step.step_type == "router" {
        // E004: Router step cannot have next field
        if step.next.is_some() {
            result.add_error(CompileError::InvalidExpression(format!(
                "[E004] Router step '{}' cannot have 'next' field (use routes/default instead)",
                step.id
            )));
        }

        // E005: Router step must have routes or default
        if step.routes.is_none() && step.default.is_none() {
            result.add_error(CompileError::InvalidExpression(format!(
                "[E005] Router step '{}' must have either 'routes' or 'default' field",
                step.id
            )));
        }
    }

    // E006: Validate all route.next references
    if let Some(routes) = &step.routes {
        for route in routes {
            validate_step_reference(&route.next, &step.id, step_map, result);
        }
    }

    // E006: Validate default reference
    if let Some(default) = &step.default {
        validate_step_reference(default, &step.id, step_map, result);
    }

    // E006: Validate next reference
    if let Some(next) = &step.next {
        // next is StepNext::StepId(String)
        use corint_core::ast::pipeline::StepNext;
        let StepNext::StepId(next_id) = next;
        validate_step_reference(next_id, &step.id, step_map, result);
    }
}

/// Validate a step reference (next, default, route.next)
fn validate_step_reference(
    target: &str,
    source_step_id: &str,
    step_map: &HashMap<String, &PipelineStep>,
    result: &mut PipelineAnalysisResult,
) {
    // "end" is a valid terminal marker
    if target == "end" {
        return;
    }

    // E006: route.next must be valid (step id or "end")
    if !step_map.contains_key(target) {
        result.add_error(CompileError::InvalidExpression(format!(
            "[E006] Step '{}' references non-existent step '{}' (must be a valid step ID or 'end')",
            source_step_id, target
        )));
    }
}

/// E007: Check for circular dependencies in step flow
fn check_circular_dependencies(
    pipeline: &Pipeline,
    step_map: &HashMap<String, &PipelineStep>,
) -> Result<()> {
    // Use DFS with a recursion stack to detect cycles
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    fn dfs(
        step_id: &str,
        step_map: &HashMap<String, &PipelineStep>,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> Result<()> {
        visited.insert(step_id.to_string());
        rec_stack.insert(step_id.to_string());
        path.push(step_id.to_string());

        if step_id == "end" {
            path.pop();
            rec_stack.remove(step_id);
            return Ok(());
        }

        if let Some(step) = step_map.get(step_id) {
            // Collect all possible next steps
            let mut next_steps = Vec::new();

            if let Some(routes) = &step.routes {
                for route in routes {
                    next_steps.push(route.next.as_str());
                }
            }

            if let Some(default) = &step.default {
                next_steps.push(default.as_str());
            }

            if let Some(next) = &step.next {
                use corint_core::ast::pipeline::StepNext;
                let StepNext::StepId(next_id) = next;
                next_steps.push(next_id.as_str());
            }

            // Visit each next step
            for next_step_id in next_steps {
                if next_step_id == "end" {
                    continue;
                }

                if rec_stack.contains(next_step_id) {
                    // Cycle detected!
                    return Err(CompileError::InvalidExpression(format!(
                        "[E007] Circular dependency detected in pipeline: {} -> {} (path: {})",
                        step_id,
                        next_step_id,
                        path.join(" -> ")
                    )));
                }

                if !visited.contains(next_step_id) {
                    dfs(next_step_id, step_map, visited, rec_stack, path)?;
                }
            }
        }

        path.pop();
        rec_stack.remove(step_id);
        Ok(())
    }

    // Start DFS from entry point
    let mut path = Vec::new();
    dfs(&pipeline.entry, step_map, &mut visited, &mut rec_stack, &mut path)
}

/// Find all reachable steps from the entry point (for W001)
fn find_reachable_steps(
    pipeline: &Pipeline,
    step_map: &HashMap<String, &PipelineStep>,
) -> HashSet<String> {
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::new();

    queue.push_back(pipeline.entry.clone());

    while let Some(step_id) = queue.pop_front() {
        if step_id == "end" || reachable.contains(&step_id) {
            continue;
        }

        reachable.insert(step_id.clone());

        if let Some(step) = step_map.get(&step_id) {
            // Add all possible next steps to the queue
            if let Some(routes) = &step.routes {
                for route in routes {
                    if !reachable.contains(&route.next) {
                        queue.push_back(route.next.clone());
                    }
                }
            }

            if let Some(default) = &step.default {
                if !reachable.contains(default) {
                    queue.push_back(default.clone());
                }
            }

            if let Some(next) = &step.next {
                use corint_core::ast::pipeline::StepNext;
                let StepNext::StepId(next_id) = next;
                if !reachable.contains(next_id) {
                    queue.push_back(next_id.clone());
                }
            }
        }
    }

    reachable
}

/// W002: Check for dead ends (steps without next/routes/default)
fn check_dead_ends(
    pipeline: &Pipeline,
    _step_map: &HashMap<String, &PipelineStep>,
    result: &mut PipelineAnalysisResult,
) {
    for step in &pipeline.steps {
        // Check if step has any exit path
        let has_next = step.next.is_some();
        let has_routes = step.routes.is_some();
        let has_default = step.default.is_some();

        // If no exit path, it's a dead end (unless it's explicitly allowed by type)
        if !has_next && !has_routes && !has_default {
            // Some step types are allowed to be terminal (e.g., trigger)
            if step.step_type != "trigger" {
                result.add_warning(
                    "W002",
                    format!(
                        "Dead end detected in step '{}' (no next, routes, or default defined)",
                        step.id
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::pipeline::{PipelineStep, Route, StepDetails, StepNext};
    use corint_core::ast::WhenBlock;

    fn create_test_pipeline(entry: &str, steps: Vec<PipelineStep>) -> Pipeline {
        Pipeline {
            id: "test_pipeline".to_string(),
            name: "Test Pipeline".to_string(),
            description: None,
            entry: entry.to_string(),
            when: None,
            steps,
            decision: None,
            metadata: None,
        }
    }

    fn create_test_step(id: &str, step_type: &str) -> PipelineStep {
        PipelineStep {
            id: id.to_string(),
            name: format!("Step {}", id),
            step_type: step_type.to_string(),
            routes: None,
            default: None,
            next: None,
            when: None,
            details: StepDetails::Unknown {},
        }
    }

    #[test]
    fn test_e001_missing_entry() {
        let pipeline = Pipeline {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            entry: String::new(), // Empty entry
            when: None,
            steps: vec![],
            decision: None,
            metadata: None,
        };

        let result = analyze_new_pipeline(&pipeline);
        assert!(result.is_err());
    }

    #[test]
    fn test_e002_entry_not_matching_step() {
        let step = create_test_step("step1", "function");
        let pipeline = create_test_pipeline("nonexistent", vec![step]);

        let result = analyze_new_pipeline(&pipeline);
        assert!(result.is_err());
    }

    #[test]
    fn test_e003_duplicate_step_ids() {
        let step1 = create_test_step("step1", "function");
        let step2 = create_test_step("step1", "function"); // Duplicate
        let pipeline = create_test_pipeline("step1", vec![step1, step2]);

        let result = analyze_new_pipeline(&pipeline);
        assert!(result.is_err());
    }

    #[test]
    fn test_e004_router_with_next() {
        let mut step = create_test_step("router1", "router");
        step.next = Some(StepNext::StepId("end".to_string())); // Invalid for router
        let pipeline = create_test_pipeline("router1", vec![step]);

        let result = analyze_new_pipeline(&pipeline);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_pipeline() {
        let mut step1 = create_test_step("step1", "function");
        step1.next = Some(StepNext::StepId("end".to_string()));
        let pipeline = create_test_pipeline("step1", vec![step1]);

        let result = analyze_new_pipeline(&pipeline);
        assert!(result.is_ok());
    }
}
