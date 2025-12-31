//! Pipeline executor
//!
//! Executes IR programs with support for async operations (features, services).

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

use super::operators;
use crate::context::ExecutionContext;
use crate::error::{Result, RuntimeError};
use crate::external_api::ExternalApiClient;
use crate::feature::{FeatureExecutor, FeatureExtractor};
use crate::observability::{Metrics, MetricsCollector};
use crate::result::{DecisionResult, ExecutionResult};
use crate::service::ServiceClient;
use crate::storage::Storage;
use corint_core::ir::{FeatureType, Instruction, Program};
use corint_core::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Pipeline executor for async IR execution
pub struct PipelineExecutor {
    feature_extractor: Option<Arc<FeatureExtractor>>,
    feature_executor: Option<Arc<FeatureExecutor>>,
    service_client: Option<Arc<dyn ServiceClient>>,
    external_api_client: Arc<ExternalApiClient>,
    list_service: Option<Arc<crate::lists::ListService>>,
    metrics: Arc<MetricsCollector>,
}

impl PipelineExecutor {
    /// Create a new pipeline executor without storage
    pub fn new() -> Self {
        Self {
            feature_extractor: None,
            feature_executor: None,
            service_client: None,
            external_api_client: Arc::new(ExternalApiClient::new()),
            list_service: None,
            metrics: Arc::new(MetricsCollector::new()),
        }
    }

    /// Create a pipeline executor with storage backend for features
    pub fn with_storage(storage: Arc<dyn Storage>) -> Self {
        Self {
            feature_extractor: Some(Arc::new(FeatureExtractor::new(storage))),
            feature_executor: None,
            service_client: None,
            external_api_client: Arc::new(ExternalApiClient::new()),
            list_service: None,
            metrics: Arc::new(MetricsCollector::new()),
        }
    }

    /// Set feature executor for lazy feature calculation
    pub fn with_feature_executor(mut self, executor: Arc<FeatureExecutor>) -> Self {
        self.feature_executor = Some(executor);
        self
    }

    /// Set service client
    pub fn with_service_client(mut self, client: Arc<dyn ServiceClient>) -> Self {
        self.service_client = Some(client);
        self
    }

    /// Set external API client
    pub fn with_external_api_client(mut self, client: Arc<ExternalApiClient>) -> Self {
        self.external_api_client = client;
        self
    }

    /// Set list service for list lookup operations
    pub fn with_list_service(mut self, service: Arc<crate::lists::ListService>) -> Self {
        self.list_service = Some(service);
        self
    }

    /// Get metrics collector
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        Arc::clone(&self.metrics)
    }

    /// Execute an IR program with the given event data
    pub async fn execute(
        &self,
        program: &Program,
        event_data: HashMap<String, Value>,
    ) -> Result<DecisionResult> {
        let context_input = crate::ContextInput::new(event_data);
        self.execute_with_result(program, context_input, ExecutionResult::new())
            .await
    }

    /// Execute an IR program with the given context input and existing result state
    pub async fn execute_with_result(
        &self,
        program: &Program,
        context_input: crate::ContextInput,
        existing_result: ExecutionResult,
    ) -> Result<DecisionResult> {
        let start_time = Instant::now();
        self.metrics.counter("executions_total").inc();

        let mut ctx = ExecutionContext::with_result(context_input, existing_result)?;
        let mut pc = 0; // Program Counter

        tracing::debug!("Program has {} instructions", program.instructions.len());
        for (i, inst) in program.instructions.iter().enumerate() {
            tracing::trace!("  [{}]: {:?}", i, inst);
        }
        while pc < program.instructions.len() {
            let instruction = &program.instructions[pc];
            tracing::trace!("Executing pc={}: {:?}", pc, instruction);

            match instruction {
                Instruction::LoadField { path } => {
                    let value = self.handle_load_field(&mut ctx, path).await?;
                    ctx.push(value);
                    pc += 1;
                }

                Instruction::LoadConst { value } => {
                    ctx.push(value.clone());
                    pc += 1;
                }

                Instruction::LoadResult { ruleset_id, field } => {
                    // Load result field from ruleset execution
                    let result_key = match ruleset_id {
                        Some(id) => format!("__ruleset_result__.{}", id),
                        None => "__last_ruleset_result__".to_string(),
                    };

                    let value = match ctx.load_variable(&result_key) {
                        Ok(Value::Object(map)) => {
                            map.get(field).cloned().unwrap_or(Value::Null)
                        }
                        Ok(_) => {
                            tracing::warn!(
                                "Result '{}' is not an object, returning Null",
                                result_key
                            );
                            Value::Null
                        }
                        Err(_) => {
                            tracing::debug!(
                                "Result '{}' not found, returning Null (no ruleset executed yet?)",
                                result_key
                            );
                            Value::Null
                        }
                    };

                    tracing::debug!(
                        "LoadResult: {}.{} = {:?}",
                        ruleset_id.as_deref().unwrap_or("(last)"),
                        field,
                        value
                    );
                    ctx.push(value);
                    pc += 1;
                }

                Instruction::BinaryOp { op } => {
                    let right = ctx.pop()?;
                    let left = ctx.pop()?;
                    let result = operators::execute_binary_op(&left, op, &right)?;
                    ctx.push(result);
                    pc += 1;
                }

                Instruction::Compare { op } => {
                    let right = ctx.pop()?;
                    let left = ctx.pop()?;
                    tracing::trace!("Compare {:?} {:?} {:?}", left, op, right);
                    let result = operators::execute_compare(&left, op, &right)?;
                    tracing::trace!("Compare result: {}", result);
                    ctx.push(Value::Bool(result));
                    pc += 1;
                }

                Instruction::UnaryOp { op } => {
                    let operand = ctx.pop()?;
                    let result = operators::execute_unary_op(&operand, op)?;
                    ctx.push(result);
                    pc += 1;
                }

                Instruction::Jump { offset } => {
                    let new_pc = (pc as isize + offset) as usize;
                    tracing::trace!(
                        "Jump at pc={}, offset={}, jumping to pc={}",
                        pc,
                        offset,
                        new_pc
                    );
                    pc = new_pc;
                }

                Instruction::JumpIfTrue { offset } => {
                    let condition = ctx.pop()?;
                    if Self::is_truthy(&condition) {
                        pc = (pc as isize + offset) as usize;
                    } else {
                        pc += 1;
                    }
                }

                Instruction::JumpIfFalse { offset } => {
                    let condition = ctx.pop()?;
                    tracing::trace!(
                        "JumpIfFalse at pc={}, condition={:?}, offset={}, is_truthy={}",
                        pc,
                        condition,
                        offset,
                        Self::is_truthy(&condition)
                    );
                    if !Self::is_truthy(&condition) {
                        let new_pc = (pc as isize + offset) as usize;
                        tracing::trace!("Jumping to pc={}", new_pc);
                        pc = new_pc;
                    } else {
                        tracing::trace!("Not jumping, pc+=1");
                        pc += 1;
                    }
                }

                Instruction::CheckEventType { expected } => {
                    // Try both event.type and event_type for backward compatibility
                    let event_type = ctx
                        .load_field(&[String::from("event"), String::from("type")])
                        .ok()
                        .filter(|v| *v != Value::Null) // Filter out Null to try fallback
                        .or_else(|| {
                            ctx.load_field(&[String::from("event_type")])
                                .ok()
                                .filter(|v| *v != Value::Null)
                        })
                        .or_else(|| {
                            ctx.load_field(&[String::from("type")])
                                .ok()
                                .filter(|v| *v != Value::Null)
                        }) // Also try "type" directly
                        .unwrap_or(Value::Null);

                    // Get pipeline name and description for logging
                    let pipeline_name = program
                        .metadata
                        .custom
                        .get("name")
                        .map(|s| format!(" - {}", s))
                        .unwrap_or_default();
                    let pipeline_desc = program
                        .metadata
                        .custom
                        .get("description")
                        .map(|s| format!(" ({})", s))
                        .unwrap_or_default();

                    tracing::debug!(
                        "Pipeline [{}]{}{}: CheckEventType: expected={}, actual={:?}",
                        program.metadata.source_id,
                        pipeline_name,
                        pipeline_desc,
                        expected,
                        event_type
                    );

                    if let Value::String(actual) = event_type {
                        if &actual != expected {
                            tracing::info!(
                                "Pipeline [{}]{} skipped: event_type mismatch (expected '{}', got '{}')",
                                program.metadata.source_id,
                                pipeline_name,
                                expected,
                                actual
                            );
                            pc = program.instructions.len();
                            continue;
                        } else {
                            tracing::info!(
                                "Pipeline [{}]{}{} matched: event_type='{}' ✓",
                                program.metadata.source_id,
                                pipeline_name,
                                pipeline_desc,
                                actual
                            );
                        }
                    } else if event_type == Value::Null {
                        // Event type is required but missing
                        return Err(RuntimeError::RuntimeError(
                            "Missing required field: event_type. Please provide event_type in your request.".to_string()
                        ));
                    }
                    pc += 1;
                }

                Instruction::SetScore { value } => {
                    ctx.set_score(*value);
                    pc += 1;
                }

                Instruction::AddScore { value } => {
                    ctx.add_score(*value);
                    pc += 1;
                }

                Instruction::SetSignal { signal } => {
                    tracing::debug!("SetSignal called with signal: {:?}", signal);
                    ctx.set_signal(signal.clone());
                    tracing::trace!(
                        "After set_signal, ctx.result.signal = {:?}",
                        ctx.result.signal
                    );
                    pc += 1;
                }

                Instruction::SetReason { reason } => {
                    tracing::debug!("SetReason called with reason: {}", reason);
                    ctx.set_reason(reason.clone());
                    pc += 1;
                }

                Instruction::SetActions { actions } => {
                    tracing::debug!("SetActions called with actions: {:?}", actions);
                    ctx.set_actions(actions.clone());
                    pc += 1;
                }

                Instruction::MarkRuleTriggered { rule_id } => {
                    ctx.mark_rule_triggered(rule_id.clone());
                    pc += 1;
                }

                Instruction::MarkBranchExecuted {
                    branch_index,
                    condition,
                } => {
                    // Store the executed branch info for tracing
                    ctx.store_variable(
                        "__executed_branch_index__".to_string(),
                        Value::Number(*branch_index as f64),
                    );
                    ctx.store_variable(
                        "__executed_branch_condition__".to_string(),
                        Value::String(condition.clone()),
                    );
                    tracing::debug!(
                        "Branch {} executed: condition={}",
                        branch_index,
                        condition
                    );
                    pc += 1;
                }

                Instruction::MarkStepExecuted {
                    step_id,
                    next_step_id,
                    route_index,
                    is_default_route,
                } => {
                    // Store step execution info for tracing
                    // Build step execution record as JSON
                    let step_record = serde_json::json!({
                        "step_id": step_id,
                        "next_step_id": next_step_id,
                        "route_index": route_index,
                        "is_default_route": is_default_route,
                    });

                    // Get existing array or create new one
                    let mut executed_steps = match ctx.load_variable("__executed_steps__") {
                        Ok(Value::Array(arr)) => arr,
                        _ => Vec::new(),
                    };

                    // Add this step execution record
                    executed_steps.push(Value::String(step_record.to_string()));

                    ctx.store_variable(
                        "__executed_steps__".to_string(),
                        Value::Array(executed_steps),
                    );

                    tracing::debug!(
                        "Step executed: {} -> {:?} (route: {:?}, default: {})",
                        step_id,
                        next_step_id,
                        route_index,
                        is_default_route
                    );
                    pc += 1;
                }

                Instruction::CallRuleset { ruleset_id } => {
                    // Store the ruleset ID in an array to support multiple rulesets
                    // The actual execution will be handled by the DecisionEngine
                    tracing::debug!("CallRuleset: {}", ruleset_id);

                    // Get existing array or create new one
                    let mut rulesets = match ctx.load_variable("__rulesets_to_execute__") {
                        Ok(Value::Array(arr)) => arr,
                        _ => Vec::new(),
                    };

                    // Add this ruleset if not already present
                    let ruleset_value = Value::String(ruleset_id.clone());
                    if !rulesets.contains(&ruleset_value) {
                        rulesets.push(ruleset_value);
                    }

                    ctx.store_variable(
                        "__rulesets_to_execute__".to_string(),
                        Value::Array(rulesets),
                    );

                    // Also keep backward compatibility with single ruleset
                    ctx.store_variable(
                        "__next_ruleset__".to_string(),
                        Value::String(ruleset_id.clone()),
                    );
                    pc += 1;
                }

                Instruction::ListLookup { list_id, negate } => {
                    // Pop the value to check from the stack
                    let value = ctx.pop()?;

                    // Use configured list service if available, otherwise fall back to empty in-memory
                    let contains = if let Some(ref list_service) = self.list_service {
                        list_service.contains(list_id, &value).await?
                    } else {
                        tracing::warn!("List service not configured, treating all lists as empty");
                        false
                    };

                    // Apply negation if needed
                    let result = if *negate { !contains } else { contains };

                    // Push the boolean result onto the stack
                    ctx.push(Value::Bool(result));
                    pc += 1;
                }

                Instruction::Return => {
                    break;
                }

                // Stack operations
                Instruction::Dup => {
                    ctx.dup()?;
                    pc += 1;
                }

                Instruction::Pop => {
                    ctx.pop()?;
                    pc += 1;
                }

                Instruction::Swap => {
                    ctx.swap()?;
                    pc += 1;
                }

                // Variable operations
                Instruction::Store { name } => {
                    let value = ctx.pop()?;
                    tracing::trace!("Storing value at '{}': {:?}", name, value);

                    // Handle nested paths like "api.ipinfo.ip_lookup"
                    if name.contains('.') {
                        let parts: Vec<&str> = name.split('.').collect();
                        Self::store_nested_value(&mut ctx, &parts, value);
                    } else {
                        // Simple variable name
                        ctx.store_variable(name.clone(), value);
                    }
                    pc += 1;
                }

                Instruction::Load { name } => {
                    let value = ctx.load_variable(name)?;
                    ctx.push(value);
                    pc += 1;
                }

                // Feature extraction
                Instruction::CallFeature {
                    feature_type,
                    field,
                    filter: _,
                    time_window,
                } => {
                    let value = if let Some(ref extractor) = self.feature_extractor {
                        // Real feature extraction with storage
                        // TODO: Convert filter expression to EventFilter
                        extractor
                            .extract(feature_type, field, time_window, None)
                            .await?
                    } else {
                        // Fallback: return placeholder
                        Self::placeholder_feature(feature_type)
                    };
                    ctx.push(value);
                    pc += 1;
                }

                // Service calls (internal)
                Instruction::CallService {
                    service,
                    operation,
                    params,
                } => {
                    let service_start = Instant::now();
                    let value = if let Some(ref client) = self.service_client {
                        use crate::service::ServiceRequest;
                        let mut request = ServiceRequest::new(service.clone(), operation.clone());
                        // Convert params to HashMap<String, Value>
                        for (k, v) in params {
                            request = request.with_param(k.clone(), v.clone());
                        }
                        match client.call(request).await {
                            Ok(response) => {
                                self.metrics.counter("service_calls_success").inc();
                                response.data
                            }
                            Err(e) => {
                                self.metrics.counter("service_calls_error").inc();
                                Value::String(format!("Service Error: {}", e))
                            }
                        }
                    } else {
                        Value::Null
                    };
                    self.metrics
                        .record_execution_time("service_call", service_start.elapsed());
                    ctx.push(value);
                    pc += 1;
                }

                // External API calls
                Instruction::CallExternal {
                    api,
                    endpoint,
                    params,
                    timeout,
                    fallback,
                } => {
                    let api_start = Instant::now();

                    // Call external API using the generic client
                    let value = match self
                        .external_api_client
                        .call(api, endpoint, params, *timeout, &ctx)
                        .await
                    {
                        Ok(result) => {
                            tracing::debug!("External API {}::{} succeeded", api, endpoint);
                            result
                        }
                        Err(e) => {
                            tracing::warn!(
                                "External API {}::{} failed: {}, using fallback",
                                api,
                                endpoint,
                                e
                            );
                            if let Some(fallback_val) = fallback {
                                fallback_val.clone()
                            } else {
                                Value::Null
                            }
                        }
                    };

                    self.metrics
                        .record_execution_time("external_api_call", api_start.elapsed());
                    ctx.push(value);
                    pc += 1;
                }
            }
        }

        // Execute decision logic if present
        if let Some(ref decision_instructions) = program.decision_instructions {
            tracing::debug!(
                "Executing {} decision instructions",
                decision_instructions.len()
            );

            let mut decision_pc = 0;
            while decision_pc < decision_instructions.len() {
                let instruction = &decision_instructions[decision_pc];
                tracing::trace!("Decision pc={}: {:?}", decision_pc, instruction);

                match instruction {
                    Instruction::LoadField { path } => {
                        let value = self.handle_load_field(&mut ctx, path).await?;
                        ctx.push(value);
                        decision_pc += 1;
                    }

                    Instruction::LoadConst { value } => {
                        ctx.push(value.clone());
                        decision_pc += 1;
                    }

                    Instruction::LoadResult { ruleset_id, field } => {
                        // Load result field from ruleset execution
                        let result_key = match ruleset_id {
                            Some(id) => format!("__ruleset_result__.{}", id),
                            None => "__last_ruleset_result__".to_string(),
                        };

                        let value = match ctx.load_variable(&result_key) {
                            Ok(Value::Object(map)) => {
                                map.get(field).cloned().unwrap_or(Value::Null)
                            }
                            Ok(_) => {
                                tracing::warn!(
                                    "Result '{}' is not an object, returning Null",
                                    result_key
                                );
                                Value::Null
                            }
                            Err(_) => {
                                tracing::debug!(
                                    "Result '{}' not found, returning Null (no ruleset executed yet?)",
                                    result_key
                                );
                                Value::Null
                            }
                        };

                        tracing::debug!(
                            "Decision LoadResult: {}.{} = {:?}",
                            ruleset_id.as_deref().unwrap_or("(last)"),
                            field,
                            value
                        );
                        ctx.push(value);
                        decision_pc += 1;
                    }

                    Instruction::Compare { op } => {
                        let right = ctx.pop()?;
                        let left = ctx.pop()?;
                        let result = operators::execute_compare(&left, op, &right)?;
                        ctx.push(Value::Bool(result));
                        decision_pc += 1;
                    }

                    Instruction::JumpIfFalse { offset } => {
                        let condition = ctx.pop()?;
                        if !Self::is_truthy(&condition) {
                            decision_pc = (decision_pc as isize + offset) as usize;
                        } else {
                            decision_pc += 1;
                        }
                    }

                    Instruction::Jump { offset } => {
                        decision_pc = (decision_pc as isize + offset) as usize;
                    }

                    Instruction::SetSignal { signal } => {
                        tracing::debug!("Decision: SetSignal {:?}", signal);
                        ctx.set_signal(signal.clone());
                        decision_pc += 1;
                    }

                    Instruction::SetReason { reason } => {
                        tracing::debug!("Decision: SetReason {}", reason);
                        ctx.set_reason(reason.clone());
                        decision_pc += 1;
                    }

                    Instruction::SetActions { actions } => {
                        tracing::debug!("Decision: SetActions {:?}", actions);
                        ctx.set_actions(actions.clone());
                        decision_pc += 1;
                    }

                    Instruction::Return => {
                        break;
                    }

                    _ => {
                        return Err(RuntimeError::RuntimeError(format!(
                            "Unsupported instruction in decision logic: {:?}",
                            instruction
                        )));
                    }
                }
            }
        }

        let duration = start_time.elapsed();
        self.metrics
            .record_execution_time("program_execution", duration);

        Ok(ctx.into_decision_result())
    }

    /// Placeholder feature value for when storage is not available
    fn placeholder_feature(feature_type: &FeatureType) -> Value {
        match feature_type {
            FeatureType::Count
            | FeatureType::CountDistinct
            | FeatureType::Sum
            | FeatureType::Avg
            | FeatureType::Min
            | FeatureType::Max
            | FeatureType::Percentile { .. }
            | FeatureType::StdDev
            | FeatureType::Variance => Value::Number(0.0),
        }
    }

    /// Check if a value is truthy
    fn is_truthy(value: &Value) -> bool {
        match value {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
        }
    }

    /// Store a value at a nested path like ["api", "ipinfo", "ip_lookup"]
    fn store_nested_value(ctx: &mut ExecutionContext, parts: &[&str], value: Value) {
        if parts.is_empty() {
            return;
        }

        if parts.len() == 1 {
            // Base case: store at top level
            ctx.store_variable(parts[0].to_string(), value);
            return;
        }

        // Check if first part is a recognized namespace
        let namespace = parts[0];
        let remaining = &parts[1..];

        ctx.store_in_namespace(namespace, remaining, value);
    }

    /// Handle LoadField instruction with feature resolution
    async fn handle_load_field(
        &self,
        ctx: &mut ExecutionContext,
        path: &[String],
    ) -> Result<Value> {
        // Check if this is a feature namespace access (features.xxx)
        if path.len() == 2 && path[0] == "features" {
            // Explicit feature access: features.xxx
            let feature_name = &path[1];

            // First, check if the feature value was pre-provided in the request
            if let Some(existing_value) = ctx.features.get(feature_name) {
                tracing::debug!(
                    "Using pre-provided feature '{}': {:?}",
                    feature_name,
                    existing_value
                );
                Ok(existing_value.clone())
            } else if let Some(ref feature_executor) = self.feature_executor {
                // Feature not pre-provided, try to calculate it
                if feature_executor.has_feature(feature_name) {
                    tracing::debug!(
                        "Calculating feature '{}' via FeatureExtractor",
                        feature_name
                    );

                    // Calculate feature on-demand
                    match feature_executor.execute_feature(feature_name, ctx).await {
                        Ok(feature_value) => {
                            // Store the result in features namespace
                            ctx.store_feature(feature_name, feature_value.clone());
                            tracing::debug!(
                                "Feature '{}' calculated: {:?}",
                                feature_name,
                                feature_value
                            );
                            Ok(feature_value)
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to calculate feature '{}': {}",
                                feature_name,
                                e
                            );
                            Ok(Value::Null)
                        }
                    }
                } else {
                    Err(RuntimeError::FieldNotFound(format!(
                        "Feature '{}' not found in pre-provided features or FeatureExtractor",
                        feature_name
                    )))
                }
            } else {
                // No pre-provided value and no feature executor
                Err(RuntimeError::FieldNotFound(format!(
                    "Feature '{}' not found: no pre-provided value and FeatureExtractor not available",
                    feature_name
                )))
            }
        } else {
            // Regular field access: event_data, variables, or special fields
            ctx.load_field(path)
        }
    }
}

impl Default for PipelineExecutor {
    fn default() -> Self {
        Self::new()
    }
}

