//! Pipeline executor
//!
//! Executes IR programs with support for async operations (features, LLM, services).

use corint_core::ast::Operator;
use corint_core::ir::{FeatureType, Instruction, Program};
use corint_core::Value;
use crate::context::ExecutionContext;
use crate::error::{RuntimeError, Result};
use crate::external_api::ExternalApiClient;
use crate::feature::{FeatureExtractor, FeatureExecutor};
use crate::llm::LLMClient;
use crate::observability::{Metrics, MetricsCollector};
use crate::result::{DecisionResult, ExecutionResult};
use crate::service::ServiceClient;
use crate::storage::Storage;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Pipeline executor for async IR execution
pub struct PipelineExecutor {
    feature_extractor: Option<Arc<FeatureExtractor>>,
    feature_executor: Option<Arc<FeatureExecutor>>,
    llm_client: Option<Arc<dyn LLMClient>>,
    service_client: Option<Arc<dyn ServiceClient>>,
    external_api_client: Arc<ExternalApiClient>,
    metrics: Arc<MetricsCollector>,
}

impl PipelineExecutor {
    /// Create a new pipeline executor without storage
    pub fn new() -> Self {
        Self {
            feature_extractor: None,
            feature_executor: None,
            llm_client: None,
            service_client: None,
            external_api_client: Arc::new(ExternalApiClient::new()),
            metrics: Arc::new(MetricsCollector::new()),
        }
    }

    /// Create a pipeline executor with storage backend for features
    pub fn with_storage(storage: Arc<dyn Storage>) -> Self {
        Self {
            feature_extractor: Some(Arc::new(FeatureExtractor::new(storage))),
            feature_executor: None,
            llm_client: None,
            service_client: None,
            external_api_client: Arc::new(ExternalApiClient::new()),
            metrics: Arc::new(MetricsCollector::new()),
        }
    }

    /// Set feature executor for lazy feature calculation
    pub fn with_feature_executor(mut self, executor: Arc<FeatureExecutor>) -> Self {
        self.feature_executor = Some(executor);
        self
    }

    /// Set LLM client
    pub fn with_llm_client(mut self, client: Arc<dyn LLMClient>) -> Self {
        self.llm_client = Some(client);
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

    /// Get metrics collector
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        self.metrics.clone()
    }

    /// Execute an IR program with the given event data
    pub async fn execute(
        &self,
        program: &Program,
        event_data: HashMap<String, Value>,
    ) -> Result<DecisionResult> {
        self.execute_with_result(program, event_data, ExecutionResult::new()).await
    }

    /// Execute an IR program with the given event data and existing result state
    pub async fn execute_with_result(
        &self,
        program: &Program,
        event_data: HashMap<String, Value>,
        existing_result: ExecutionResult,
    ) -> Result<DecisionResult> {
        let start_time = Instant::now();
        self.metrics.counter("executions_total").inc();

        let mut ctx = ExecutionContext::with_result(event_data, existing_result);
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
                    // Try to load field from event_data or variables
                    let value_result = ctx.load_field(path);
                    
                    let value = match value_result {
                        Ok(v) => v,
                        Err(RuntimeError::FieldNotFound(ref field_name)) => {
                            // Field not found - check if it's a registered feature
                            if path.len() == 1 {
                                let field_name = &path[0];
                                
                                // Check if this is a registered feature
                                if let Some(ref feature_executor) = self.feature_executor {
                                    // Check if feature is registered
                                    if feature_executor.has_feature(field_name) {
                                        tracing::debug!("Feature '{}' not in event_data, calculating from data source", field_name);
                                        
                                        // Calculate feature on-demand
                                        match feature_executor.execute_feature(field_name, &ctx).await {
                                            Ok(feature_value) => {
                                                // Cache the result in event_data for subsequent accesses
                                                ctx.event_data.insert(field_name.clone(), feature_value.clone());
                                                tracing::debug!("Feature '{}' calculated: {:?}", field_name, feature_value);
                                                feature_value
                                            }
                                            Err(e) => {
                                                tracing::warn!("Failed to calculate feature '{}': {}", field_name, e);
                                                Value::Null
                                            }
                                        }
                                    } else {
                                        // Not a feature, return error
                                        return Err(RuntimeError::FieldNotFound(field_name.clone()));
                                    }
                                } else {
                                    // No feature executor, return error
                                    return Err(RuntimeError::FieldNotFound(field_name.clone()));
                                }
                            } else {
                                // Multi-path field access, return error
                                return Err(RuntimeError::FieldNotFound(path.join(".")));
                            }
                        }
                        Err(e) => return Err(e),
                    };
                    
                    ctx.push(value);
                    pc += 1;
                }

                Instruction::LoadConst { value } => {
                    ctx.push(value.clone());
                    pc += 1;
                }

                Instruction::BinaryOp { op } => {
                    let right = ctx.pop()?;
                    let left = ctx.pop()?;
                    let result = Self::execute_binary_op(&left, op, &right)?;
                    ctx.push(result);
                    pc += 1;
                }

                Instruction::Compare { op } => {
                    let right = ctx.pop()?;
                    let left = ctx.pop()?;
                    tracing::trace!("Compare {:?} {:?} {:?}", left, op, right);
                    let result = Self::execute_compare(&left, op, &right)?;
                    tracing::trace!("Compare result: {}", result);
                    ctx.push(Value::Bool(result));
                    pc += 1;
                }

                Instruction::UnaryOp { op } => {
                    let operand = ctx.pop()?;
                    let result = Self::execute_unary_op(&operand, op)?;
                    ctx.push(result);
                    pc += 1;
                }

                Instruction::Jump { offset } => {
                    let new_pc = (pc as isize + offset) as usize;
                    tracing::trace!("Jump at pc={}, offset={}, jumping to pc={}", pc, offset, new_pc);
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
                    tracing::trace!("JumpIfFalse at pc={}, condition={:?}, offset={}, is_truthy={}",
                             pc, condition, offset, Self::is_truthy(&condition));
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
                    // Check event_type field (only support event_type format)
                    let event_type = ctx
                        .load_field(&[String::from("event_type")])
                        .unwrap_or(Value::Null);

                    // Get pipeline name and description for logging
                    let pipeline_name = program.metadata.custom.get("name")
                        .map(|s| format!(" - {}", s))
                        .unwrap_or_default();
                    let pipeline_desc = program.metadata.custom.get("description")
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

                Instruction::SetAction { action } => {
                    tracing::debug!("SetAction called with action: {:?}", action);
                    ctx.set_action(action.clone());
                    tracing::trace!("After set_action, ctx.result.action = {:?}", ctx.result.action);
                    pc += 1;
                }

                Instruction::MarkRuleTriggered { rule_id } => {
                    ctx.mark_rule_triggered(rule_id.clone());
                    pc += 1;
                }

                Instruction::CallRuleset { ruleset_id } => {
                    // Store the ruleset ID to be executed
                    // The actual execution will be handled by the DecisionEngine
                    tracing::debug!("CallRuleset: {}", ruleset_id);
                    ctx.store_variable("__next_ruleset__".to_string(), Value::String(ruleset_id.clone()));
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

                    // Handle nested paths like "context.ip_info"
                    if name.contains('.') {
                        let parts: Vec<&str> = name.split('.').collect();
                        if parts.len() == 2 {
                            // Two-level nesting: context.ip_info
                            let root = parts[0];
                            let key = parts[1];

                            // Get or create the root object
                            let mut root_obj = match ctx.load_variable(root) {
                                Ok(val) => {
                                    if let Value::Object(map) = val {
                                        map
                                    } else {
                                        HashMap::new()
                                    }
                                }
                                Err(_) => HashMap::new(),
                            };

                            // Set the nested value
                            root_obj.insert(key.to_string(), value);
                            ctx.store_variable(root.to_string(), Value::Object(root_obj));
                        } else {
                            // For deeper nesting, just use the full path as key for now
                            ctx.store_variable(name.clone(), value);
                        }
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
                        extractor.extract(feature_type, field, time_window, None).await?
                    } else {
                        // Fallback: return placeholder
                        Self::placeholder_feature(feature_type)
                    };
                    ctx.push(value);
                    pc += 1;
                }

                // LLM calls
                Instruction::CallLLM { prompt, model, .. } => {
                    let llm_start = Instant::now();
                    let value = if let Some(ref client) = self.llm_client {
                        use crate::llm::LLMRequest;
                        let request = LLMRequest::new(prompt.clone(), model.clone());
                        match client.call(request).await {
                            Ok(response) => {
                                self.metrics.counter("llm_calls_success").inc();
                                Value::String(response.content)
                            }
                            Err(e) => {
                                self.metrics.counter("llm_calls_error").inc();
                                Value::String(format!("LLM Error: {}", e))
                            }
                        }
                    } else {
                        Value::String("LLM not configured".to_string())
                    };
                    self.metrics.record_execution_time("llm_call", llm_start.elapsed());
                    ctx.push(value);
                    pc += 1;
                }

                // Service calls (internal)
                Instruction::CallService { service, operation, params } => {
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
                    self.metrics.record_execution_time("service_call", service_start.elapsed());
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
                    let value = match self.external_api_client.call(api, endpoint, params, *timeout, &ctx).await {
                        Ok(result) => {
                            tracing::debug!("External API {}::{} succeeded", api, endpoint);
                            result
                        }
                        Err(e) => {
                            tracing::warn!("External API {}::{} failed: {}, using fallback", api, endpoint, e);
                            if let Some(fallback_val) = fallback {
                                fallback_val.clone()
                            } else {
                                Value::Null
                            }
                        }
                    };

                    self.metrics.record_execution_time("external_api_call", api_start.elapsed());
                    ctx.push(value);
                    pc += 1;
                }
            }
        }

        let duration = start_time.elapsed();
        self.metrics.record_execution_time("program_execution", duration);

        Ok(ctx.into_decision_result())
    }

    /// Placeholder feature value for when storage is not available
    fn placeholder_feature(feature_type: &FeatureType) -> Value {
        match feature_type {
            FeatureType::Count | FeatureType::CountDistinct | FeatureType::Sum
            | FeatureType::Avg | FeatureType::Min | FeatureType::Max
            | FeatureType::Percentile { .. } | FeatureType::StdDev | FeatureType::Variance => {
                Value::Number(0.0)
            }
        }
    }

    /// Execute a binary operation
    fn execute_binary_op(left: &Value, op: &Operator, right: &Value) -> Result<Value> {
        match (left, op, right) {
            // Arithmetic operations
            (Value::Number(l), Operator::Add, Value::Number(r)) => Ok(Value::Number(l + r)),
            (Value::Number(l), Operator::Sub, Value::Number(r)) => Ok(Value::Number(l - r)),
            (Value::Number(l), Operator::Mul, Value::Number(r)) => Ok(Value::Number(l * r)),
            (Value::Number(l), Operator::Div, Value::Number(r)) => {
                if *r == 0.0 {
                    Err(RuntimeError::DivisionByZero)
                } else {
                    Ok(Value::Number(l / r))
                }
            }
            (Value::Number(l), Operator::Mod, Value::Number(r)) => {
                if *r == 0.0 {
                    Err(RuntimeError::DivisionByZero)
                } else {
                    Ok(Value::Number(l % r))
                }
            }

            // Logical operations
            (Value::Bool(l), Operator::And, Value::Bool(r)) => Ok(Value::Bool(*l && *r)),
            (Value::Bool(l), Operator::Or, Value::Bool(r)) => Ok(Value::Bool(*l || *r)),

            // String operations
            (Value::String(l), Operator::Contains, Value::String(r)) => {
                Ok(Value::Bool(l.contains(r)))
            }
            (Value::String(l), Operator::StartsWith, Value::String(r)) => {
                Ok(Value::Bool(l.starts_with(r)))
            }
            (Value::String(l), Operator::EndsWith, Value::String(r)) => {
                Ok(Value::Bool(l.ends_with(r)))
            }

            // Array operations
            (Value::Array(arr), Operator::Contains, val) => {
                Ok(Value::Bool(arr.iter().any(|v| v == val)))
            }

            // In operator
            (val, Operator::In, Value::Array(arr)) => {
                Ok(Value::Bool(arr.iter().any(|v| v == val)))
            }
            (val, Operator::NotIn, Value::Array(arr)) => {
                Ok(Value::Bool(!arr.iter().any(|v| v == val)))
            }

            _ => Err(RuntimeError::InvalidOperation(format!(
                "Cannot apply {:?} to {:?} and {:?}",
                op, left, right
            ))),
        }
    }

    /// Execute a comparison operation
    fn execute_compare(left: &Value, op: &Operator, right: &Value) -> Result<bool> {
        match (left, op, right) {
            (Value::Number(l), Operator::Eq, Value::Number(r)) => Ok(l == r),
            (Value::Number(l), Operator::Ne, Value::Number(r)) => Ok(l != r),
            (Value::Number(l), Operator::Gt, Value::Number(r)) => Ok(l > r),
            (Value::Number(l), Operator::Ge, Value::Number(r)) => Ok(l >= r),
            (Value::Number(l), Operator::Lt, Value::Number(r)) => Ok(l < r),
            (Value::Number(l), Operator::Le, Value::Number(r)) => Ok(l <= r),

            (Value::String(l), Operator::Eq, Value::String(r)) => Ok(l == r),
            (Value::String(l), Operator::Ne, Value::String(r)) => Ok(l != r),

            (Value::Bool(l), Operator::Eq, Value::Bool(r)) => Ok(l == r),
            (Value::Bool(l), Operator::Ne, Value::Bool(r)) => Ok(l != r),

            _ => Err(RuntimeError::InvalidOperation(format!(
                "Cannot compare {:?} and {:?} with {:?}",
                left, right, op
            ))),
        }
    }

    /// Execute a unary operation
    fn execute_unary_op(operand: &Value, op: &corint_core::ast::UnaryOperator) -> Result<Value> {
        use corint_core::ast::UnaryOperator;

        match (op, operand) {
            (UnaryOperator::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnaryOperator::Negate, Value::Number(n)) => Ok(Value::Number(-n)),
            _ => Err(RuntimeError::InvalidOperation(format!(
                "Cannot apply {:?} to {:?}",
                op, operand
            ))),
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
}

impl Default for PipelineExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corint_core::ast::Action;
    use corint_core::ir::ProgramMetadata;
    use crate::storage::InMemoryStorage;

    #[tokio::test]
    async fn test_execute_simple_program() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::SetScore { value: 50 },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.score, 50);
    }

    #[tokio::test]
    async fn test_execute_with_arithmetic() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::LoadConst {
                value: Value::Number(20.0),
            },
            Instruction::BinaryOp { op: Operator::Add },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        // Result should be on stack but not reflected in score
        // This tests the stack operations work correctly
        assert_eq!(result.score, 0);
    }

    #[tokio::test]
    async fn test_execute_with_comparison() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::LoadConst {
                value: Value::Number(5.0),
            },
            Instruction::Compare { op: Operator::Gt },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.score, 0);
    }

    #[tokio::test]
    async fn test_execute_with_jump() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::LoadConst {
                value: Value::Bool(false),
            },
            Instruction::JumpIfFalse { offset: 2 },
            Instruction::SetScore { value: 100 },
            Instruction::SetScore { value: 50 },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.score, 50);
    }

    #[tokio::test]
    async fn test_execute_with_action() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::SetAction {
                action: Action::Approve,
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.action, Some(Action::Approve));
    }

    #[tokio::test]
    async fn test_execute_mark_rule_triggered() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::MarkRuleTriggered {
                rule_id: "test_rule".to_string(),
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        assert_eq!(result.triggered_rules.len(), 1);
        assert!(result.triggered_rules.contains(&"test_rule".to_string()));
    }

    #[tokio::test]
    async fn test_division_by_zero() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::LoadConst {
                value: Value::Number(10.0),
            },
            Instruction::LoadConst {
                value: Value::Number(0.0),
            },
            Instruction::BinaryOp { op: Operator::Div },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RuntimeError::DivisionByZero));
    }

    #[tokio::test]
    async fn test_feature_extraction_without_storage() {
        use corint_core::ir::TimeWindow;

        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::CallFeature {
                feature_type: FeatureType::Count,
                field: vec!["transaction".to_string(), "amount".to_string()],
                filter: None,
                time_window: TimeWindow::Last24Hours,
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        // Without storage, should return placeholder
        assert_eq!(result.score, 0);
    }

    #[tokio::test]
    async fn test_feature_extraction_with_storage() {
        use crate::storage::Event;
        use corint_core::ir::TimeWindow;

        // Create storage with test events
        let mut storage = InMemoryStorage::new();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        for i in 0..5 {
            let mut data = HashMap::new();
            data.insert("amount".to_string(), Value::Number((i + 1) as f64 * 10.0));
            storage.add_event(Event {
                timestamp: now - 100 + i,
                data,
            });
        }

        let executor = PipelineExecutor::with_storage(Arc::new(storage));

        let instructions = vec![
            Instruction::CallFeature {
                feature_type: FeatureType::Sum,
                field: vec!["amount".to_string()],
                filter: None,
                time_window: TimeWindow::Last1Hour,
            },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        let result = executor.execute(&program, HashMap::new()).await.unwrap();

        // Sum of 10, 20, 30, 40, 50 = 150
        assert_eq!(result.score, 0); // Score is separate from stack value
    }

    #[tokio::test]
    async fn test_llm_integration() {
        use crate::llm::MockProvider;

        let llm_client = Arc::new(MockProvider::with_response("Risk detected".to_string()));
        let executor = PipelineExecutor::new().with_llm_client(llm_client);

        // We need to check CallLLM instruction structure first
        // For now, just test that the executor can be created with LLM client
        assert!(executor.llm_client.is_some());
    }

    #[tokio::test]
    async fn test_service_integration() {
        use crate::service::http::MockHttpClient;

        let service_client = Arc::new(MockHttpClient::new());
        let executor = PipelineExecutor::new().with_service_client(service_client);

        assert!(executor.service_client.is_some());
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let executor = PipelineExecutor::new();

        let instructions = vec![
            Instruction::SetScore { value: 10 },
            Instruction::Return,
        ];

        let program = Program::new(
            instructions,
            ProgramMetadata::for_rule("test".to_string()),
        );

        executor.execute(&program, HashMap::new()).await.unwrap();

        let metrics = executor.metrics();
        let executions = metrics.counter("executions_total");
        assert_eq!(executions.get(), 1);

        let duration_hist = metrics.histogram("program_execution_duration");
        assert_eq!(duration_hist.count(), 1);
    }
}
