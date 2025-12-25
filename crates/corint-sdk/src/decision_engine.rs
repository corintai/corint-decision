//! DecisionEngine - Main API for executing decisions

use crate::config::EngineConfig;
use crate::error::{Result, SdkError};
use corint_compiler::{Compiler, CompilerOptions as CompilerOpts};
use corint_core::ast::{Condition, ConditionGroup, Expression, Operator, PipelineRegistry, Signal, WhenBlock};
use corint_core::ir::Program;
use corint_core::Value;
use corint_parser::{PipelineParser, RegistryParser, RuleParser, RulesetParser};
use corint_runtime::{
    ApiConfig, ConclusionTrace, ConditionTrace, ContextInput, DecisionResult, ExecutionTrace,
    ExternalApiClient, MetricsCollector, PipelineExecutor, PipelineTrace, RuleTrace, RulesetTrace,
    StepTrace,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Decision request options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionOptions {
    /// Enable detailed execution tracing
    #[serde(default)]
    pub enable_trace: bool,
}

/// Decision request (supports Phase 5 multi-namespace format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRequest {
    /// Event data (required)
    pub event_data: HashMap<String, Value>,

    /// Feature computation results (optional)
    #[serde(default)]
    pub features: Option<HashMap<String, Value>>,

    /// External API results (optional)
    #[serde(default)]
    pub api: Option<HashMap<String, Value>>,

    /// Service call results (optional)
    #[serde(default)]
    pub service: Option<HashMap<String, Value>>,

    /// LLM analysis results (optional)
    #[serde(default)]
    pub llm: Option<HashMap<String, Value>>,

    /// Variables (optional)
    #[serde(default)]
    pub vars: Option<HashMap<String, Value>>,

    /// Request metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Request options (including trace enablement)
    #[serde(default)]
    pub options: DecisionOptions,
}

impl DecisionRequest {
    /// Create a new decision request
    pub fn new(event_data: HashMap<String, Value>) -> Self {
        Self {
            event_data,
            features: None,
            api: None,
            service: None,
            llm: None,
            vars: None,
            metadata: HashMap::new(),
            options: DecisionOptions::default(),
        }
    }

    /// Enable execution tracing
    pub fn with_trace(mut self) -> Self {
        self.options.enable_trace = true;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Add features
    pub fn with_features(mut self, features: HashMap<String, Value>) -> Self {
        self.features = Some(features);
        self
    }

    /// Add API results
    pub fn with_api(mut self, api: HashMap<String, Value>) -> Self {
        self.api = Some(api);
        self
    }

    /// Add service results
    pub fn with_service(mut self, service: HashMap<String, Value>) -> Self {
        self.service = Some(service);
        self
    }

    /// Add LLM results
    pub fn with_llm(mut self, llm: HashMap<String, Value>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Add variables
    pub fn with_vars(mut self, vars: HashMap<String, Value>) -> Self {
        self.vars = Some(vars);
        self
    }

    /// Convert to ContextInput for runtime execution
    pub(crate) fn to_context_input(&self) -> ContextInput {
        let mut input = ContextInput::new(self.event_data.clone());

        if let Some(features) = &self.features {
            input = input.with_features(features.clone());
        }
        if let Some(api) = &self.api {
            input = input.with_api(api.clone());
        }
        if let Some(service) = &self.service {
            input = input.with_service(service.clone());
        }
        if let Some(llm) = &self.llm {
            input = input.with_llm(llm.clone());
        }
        if let Some(vars) = &self.vars {
            input = input.with_vars(vars.clone());
        }

        input
    }
}

/// Decision response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResponse {
    /// Request ID (for tracking and correlation)
    pub request_id: String,

    /// Pipeline ID that processed this request
    pub pipeline_id: Option<String>,

    /// Decision result
    pub result: DecisionResult,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Request metadata (echoed back)
    pub metadata: HashMap<String, String>,

    /// Execution trace (only present if enable_trace was set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<ExecutionTrace>,
}

/// Main decision engine
pub struct DecisionEngine {
    /// Compiled programs (one per rule/ruleset/pipeline)
    programs: Vec<Program>,

    /// Mapping of ruleset ID to compiled program (for pipeline routing)
    ruleset_map: HashMap<String, Program>,

    /// Mapping of rule ID to compiled program
    rule_map: HashMap<String, Program>,

    /// Mapping of pipeline ID to compiled program
    pipeline_map: HashMap<String, Program>,

    /// Optional pipeline registry for event routing
    registry: Option<PipelineRegistry>,

    /// Pipeline executor
    executor: Arc<PipelineExecutor>,

    /// Metrics collector
    metrics: Arc<MetricsCollector>,

    /// Configuration
    config: EngineConfig,

    /// Optional decision result writer for persisting decision results
    pub(crate) result_writer: Option<Arc<corint_runtime::DecisionResultWriter>>,

    // Reload support: save builder state for reloading
    /// Repository configuration (if any) for reloading
    pub(crate) repository_config: Option<corint_repository::RepositoryConfig>,

    /// Feature executor (for reload)
    pub(crate) feature_executor: Option<Arc<corint_runtime::feature::FeatureExecutor>>,

    /// List service (for reload)
    pub(crate) list_service: Option<Arc<corint_runtime::lists::ListService>>,
}

impl DecisionEngine {
    /// Generate a unique request ID
    /// Format: req_YYYYMMDDHHmmss_xxxxxx
    /// Example: req_20231209143052_a3f2e1
    ///
    /// Uses chrono for timestamp and rand for truly random suffix
    fn generate_request_id() -> String {
        use chrono::Utc;
        use rand::Rng;

        // Get current UTC time and format it directly - this correctly handles
        // leap years, variable month lengths, and all date edge cases
        let now = Utc::now();
        let datetime_str = now.format("%Y%m%d%H%M%S").to_string();

        // Generate truly random suffix using thread_rng
        let random: u32 = rand::thread_rng().gen_range(0..0xFFFFFF);

        format!("req_{}_{:06x}", datetime_str, random)
    }

    /// Create a new decision engine from configuration
    pub async fn new(config: EngineConfig) -> Result<Self> {
        Self::new_with_feature_executor(config, None, None).await
    }

    /// Create a new decision engine with optional feature executor and list service
    pub async fn new_with_feature_executor(
        config: EngineConfig,
        feature_executor: Option<Arc<corint_runtime::feature::FeatureExecutor>>,
        list_service: Option<Arc<corint_runtime::lists::ListService>>,
    ) -> Result<Self> {
        let mut programs = Vec::new();

        // Compile all rule files
        let compiler_opts = CompilerOpts {
            enable_semantic_analysis: config.compiler_options.enable_semantic_analysis,
            enable_constant_folding: config.compiler_options.enable_constant_folding,
            enable_dead_code_elimination: true, // FIXED: Bug with default actions resolved - now uses proper CFG analysis
            library_base_path: "repository".to_string(),
        };

        let mut compiler = Compiler::with_options(compiler_opts);

        // Compile rule files
        for rule_file in &config.rule_files {
            programs.extend(Self::load_and_compile_rules(rule_file, &mut compiler).await?);
        }

        // Compile rule contents (from repository)
        for (id, content) in &config.rule_contents {
            programs.extend(Self::compile_rules_from_content(id, content, &mut compiler).await?);
        }

        // Build ruleset_map, rule_map, and pipeline_map for routing
        let mut ruleset_map = HashMap::new();
        let mut rule_map = HashMap::new();
        let mut pipeline_map = HashMap::new();
        for program in &programs {
            match program.metadata.source_type.as_str() {
                "ruleset" => {
                    ruleset_map.insert(program.metadata.source_id.clone(), program.clone());
                }
                "rule" => {
                    rule_map.insert(program.metadata.source_id.clone(), program.clone());
                }
                "pipeline" => {
                    pipeline_map.insert(program.metadata.source_id.clone(), program.clone());
                }
                _ => {}
            }
        }

        // Load optional registry file
        let registry = if let Some(registry_content) = &config.registry_content {
            // Load registry from content string using RegistryParser
            match RegistryParser::parse(registry_content) {
                Ok(reg) => {
                    tracing::info!(
                        "✓ Loaded pipeline registry from content: {} entries",
                        reg.registry.len()
                    );
                    Some(reg)
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse registry content: {}. Continuing without registry.",
                        e
                    );
                    None
                }
            }
        } else if let Some(registry_file) = &config.registry_file {
            // Fall back to loading from file
            match Self::load_registry(registry_file).await {
                Ok(reg) => {
                    tracing::info!(
                        "✓ Loaded pipeline registry from file: {} entries",
                        reg.registry.len()
                    );
                    Some(reg)
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load registry file: {}. Continuing without registry.",
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        // Load external API configurations
        let mut api_client = ExternalApiClient::new();

        // Load API configs from repository/configs/apis directory
        let api_config_dir = Path::new("repository/configs/apis");
        if api_config_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(api_config_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(api_config) = serde_yaml::from_str::<ApiConfig>(&content) {
                                tracing::info!("Loaded API config: {}", api_config.name);
                                api_client.register_api(api_config);
                            }
                        }
                    }
                }
            }
        }

        // Create executor with API client
        let mut pipeline_executor =
            PipelineExecutor::new().with_external_api_client(Arc::new(api_client));

        // Clone feature_executor and list_service before using them (they will be moved)
        let feature_executor_clone = feature_executor.clone();
        let list_service_clone = list_service.clone();

        // Set feature executor if provided (for lazy feature calculation)
        if let Some(feature_executor) = feature_executor {
            pipeline_executor = pipeline_executor.with_feature_executor(feature_executor);
        }

        // Set list service if provided (for list lookups)
        if let Some(list_service) = list_service {
            pipeline_executor = pipeline_executor.with_list_service(list_service);
        }

        let executor = Arc::new(pipeline_executor);
        let metrics = executor.metrics();

        Ok(Self {
            programs,
            ruleset_map,
            rule_map,
            pipeline_map,
            registry,
            executor,
            metrics,
            config,
            result_writer: None,
            repository_config: None,
            feature_executor: feature_executor_clone,
            list_service: list_service_clone,
        })
    }

    /// Load registry from file
    async fn load_registry(path: &Path) -> Result<PipelineRegistry> {
        let content = tokio::fs::read_to_string(path).await?;
        let registry = RegistryParser::parse(&content)?;
        Ok(registry)
    }

    /// Evaluate a when block against event data
    fn evaluate_when_block(when: &WhenBlock, event_data: &HashMap<String, Value>) -> bool {
        tracing::debug!("evaluate_when_block: when={:?}, event_data={:?}", when, event_data);

        // Check event_type if specified
        // Note: event_type field in WhenBlock corresponds to event.type in YAML,
        // which is stored as "type" key in event_data HashMap
        if let Some(ref expected_type) = when.event_type {
            if let Some(Value::String(actual)) = event_data.get("type") {
                if actual != expected_type {
                    tracing::debug!("event_type mismatch: expected={}, actual={}", expected_type, actual);
                    return false; // Event type mismatch
                }
            } else {
                tracing::debug!("No type field in event data");
                return false; // No type field in event data or type is not a string
            }
        }

        // Evaluate condition_group (new format: all/any/not)
        if let Some(ref condition_group) = when.condition_group {
            let result = Self::evaluate_condition_group(condition_group, event_data);
            tracing::debug!("condition_group evaluation result: {}", result);
            return result;
        }

        // Evaluate all conditions (legacy format - AND logic)
        if let Some(ref conditions) = when.conditions {
            for condition in conditions {
                if !Self::evaluate_expression(condition, event_data) {
                    return false; // Condition failed
                }
            }
        }

        true // All checks passed
    }

    /// Evaluate a condition group (all/any/not)
    fn evaluate_condition_group(
        group: &ConditionGroup,
        event_data: &HashMap<String, Value>,
    ) -> bool {
        match group {
            ConditionGroup::All(conditions) => {
                // All conditions must be true (AND logic)
                for condition in conditions {
                    let result = Self::evaluate_condition(condition, event_data);
                    tracing::debug!("Evaluating condition in All group: {:?}, result={}", condition, result);
                    if !result {
                        return false;
                    }
                }
                true
            }
            ConditionGroup::Any(conditions) => {
                // At least one condition must be true (OR logic)
                for condition in conditions {
                    if Self::evaluate_condition(condition, event_data) {
                        return true;
                    }
                }
                false
            }
            ConditionGroup::Not(conditions) => {
                // None of the conditions should be true (NOT logic)
                for condition in conditions {
                    if Self::evaluate_condition(condition, event_data) {
                        return false;
                    }
                }
                true
            }
        }
    }

    /// Evaluate a single condition (expression or nested group)
    fn evaluate_condition(
        condition: &Condition,
        event_data: &HashMap<String, Value>,
    ) -> bool {
        match condition {
            Condition::Expression(expr) => Self::evaluate_expression(expr, event_data),
            Condition::Group(group) => Self::evaluate_condition_group(group, event_data),
        }
    }

    /// Evaluate a condition group with tracing support
    fn evaluate_condition_group_with_trace(
        group: &ConditionGroup,
        event_data: &HashMap<String, Value>,
    ) -> (bool, Vec<ConditionTrace>) {
        match group {
            ConditionGroup::All(conditions) => {
                let mut traces = Vec::new();
                let mut all_true = true;

                for condition in conditions {
                    let (result, mut cond_traces) = Self::evaluate_condition_with_trace(condition, event_data);
                    traces.append(&mut cond_traces);
                    if !result {
                        all_true = false;
                    }
                }

                (all_true, traces)
            }
            ConditionGroup::Any(conditions) => {
                let mut traces = Vec::new();
                let mut any_true = false;

                for condition in conditions {
                    let (result, mut cond_traces) = Self::evaluate_condition_with_trace(condition, event_data);
                    traces.append(&mut cond_traces);
                    if result {
                        any_true = true;
                    }
                }

                (any_true, traces)
            }
            ConditionGroup::Not(conditions) => {
                let mut traces = Vec::new();
                let mut all_true = true;

                for condition in conditions {
                    let (result, mut cond_traces) = Self::evaluate_condition_with_trace(condition, event_data);
                    traces.append(&mut cond_traces);
                    if !result {
                        all_true = false;
                    }
                }

                // NOT inverts the result
                (!all_true, traces)
            }
        }
    }

    /// Evaluate a single condition with tracing support
    fn evaluate_condition_with_trace(
        condition: &Condition,
        event_data: &HashMap<String, Value>,
    ) -> (bool, Vec<ConditionTrace>) {
        match condition {
            Condition::Expression(expr) => {
                // For expression, evaluate it and create a trace
                let result = Self::evaluate_expression(expr, event_data);
                let expr_string = Self::expression_to_string(expr);
                let trace = ConditionTrace::new(expr_string, result);
                (result, vec![trace])
            }
            Condition::Group(group) => {
                // For nested group, recursively evaluate
                Self::evaluate_condition_group_with_trace(group, event_data)
            }
        }
    }

    /// Evaluate an expression against event data
    fn evaluate_expression(expr: &Expression, event_data: &HashMap<String, Value>) -> bool {
        match expr {
            Expression::Literal(val) => {
                // Literal is truthy if non-zero, non-empty, non-null
                Self::is_truthy(val)
            }
            Expression::FieldAccess(path) => {
                // Field access - get value and check if truthy
                if let Some(val) = Self::get_field_value(event_data, path) {
                    Self::is_truthy(&val)
                } else {
                    false
                }
            }
            Expression::Binary { left, op, right } => {
                Self::evaluate_binary_expression(left, op, right, event_data)
            }
            Expression::Unary { .. } => {
                // Unary not supported yet in this simple evaluator
                false
            }
            Expression::FunctionCall { .. } => {
                // Function calls not supported in this simple evaluator
                false
            }
            Expression::Ternary { .. } => {
                // Ternary expressions not supported in this simple evaluator
                false
            }
            Expression::LogicalGroup { op, conditions } => {
                // Evaluate logical group (any/all)
                use corint_core::ast::LogicalGroupOp;
                match op {
                    LogicalGroupOp::Any => {
                        // OR logic: return true if ANY condition is true
                        conditions
                            .iter()
                            .any(|cond| Self::evaluate_expression(cond, event_data))
                    }
                    LogicalGroupOp::All => {
                        // AND logic: return true if ALL conditions are true
                        conditions
                            .iter()
                            .all(|cond| Self::evaluate_expression(cond, event_data))
                    }
                }
            }
            Expression::ListReference { .. } => {
                // List references cannot be directly evaluated to boolean in this simple evaluator
                // They are only used in conjunction with InList/NotInList operators
                false
            }
            Expression::ResultAccess { .. } => {
                // Result access requires runtime context, not supported in this simple evaluator
                false
            }
        }
    }

    /// Evaluate a binary expression
    fn evaluate_binary_expression(
        left: &Expression,
        op: &Operator,
        right: &Expression,
        event_data: &HashMap<String, Value>,
    ) -> bool {
        let left_val = Self::expression_to_value(left, event_data);
        let right_val = Self::expression_to_value(right, event_data);

        match op {
            Operator::Eq => left_val == right_val,
            Operator::Ne => left_val != right_val,
            Operator::Lt => {
                Self::compare_values(&left_val, &right_val) == Some(std::cmp::Ordering::Less)
            }
            Operator::Gt => {
                Self::compare_values(&left_val, &right_val) == Some(std::cmp::Ordering::Greater)
            }
            Operator::Le => matches!(
                Self::compare_values(&left_val, &right_val),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            ),
            Operator::Ge => matches!(
                Self::compare_values(&left_val, &right_val),
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            ),
            Operator::And => Self::is_truthy(&left_val) && Self::is_truthy(&right_val),
            Operator::Or => Self::is_truthy(&left_val) || Self::is_truthy(&right_val),
            Operator::In => {
                // Check if left value is in right array
                if let Value::Array(arr) = &right_val {
                    arr.contains(&left_val)
                } else {
                    false
                }
            }
            Operator::InList | Operator::NotInList => {
                // List membership operators are not supported in this simple evaluator
                // They require runtime list lookup which is handled by the VM
                false
            }
            _ => false,
        }
    }

    /// Convert expression to value
    fn expression_to_value(expr: &Expression, event_data: &HashMap<String, Value>) -> Value {
        match expr {
            Expression::Literal(val) => val.clone(),
            Expression::FieldAccess(path) => {
                Self::get_field_value(event_data, path).unwrap_or(Value::Null)
            }
            Expression::Binary { left, op, right } => {
                let result = Self::evaluate_binary_expression(left, op, right, event_data);
                Value::Bool(result)
            }
            Expression::Unary { .. } => Value::Null,
            Expression::FunctionCall { .. } => Value::Null,
            Expression::Ternary { .. } => Value::Null,
            Expression::LogicalGroup { op, conditions } => {
                // Convert logical group to boolean value
                use corint_core::ast::LogicalGroupOp;
                let result = match op {
                    LogicalGroupOp::Any => conditions
                        .iter()
                        .any(|cond| Self::evaluate_expression(cond, event_data)),
                    LogicalGroupOp::All => conditions
                        .iter()
                        .all(|cond| Self::evaluate_expression(cond, event_data)),
                };
                Value::Bool(result)
            }
            Expression::ListReference { .. } => {
                // List references cannot be directly evaluated to a value in this context
                // They are only used in conjunction with InList/NotInList operators
                Value::Null
            }
            Expression::ResultAccess { .. } => {
                // Result access requires runtime context, not supported in this simple evaluator
                Value::Null
            }
        }
    }

    /// Get field value from nested path
    fn get_field_value(event_data: &HashMap<String, Value>, path: &[String]) -> Option<Value> {
        if path.is_empty() {
            return None;
        }

        // Special case: if path starts with "event", skip it since event_data IS the event
        let actual_path = if path.len() > 0 && path[0] == "event" {
            &path[1..]
        } else {
            path
        };

        if actual_path.is_empty() {
            return None;
        }

        let mut current = event_data.get(&actual_path[0])?;

        for key in &actual_path[1..] {
            match current {
                Value::Object(map) => {
                    current = map.get(key)?;
                }
                _ => return None,
            }
        }

        Some(current.clone())
    }

    /// Compare two values
    fn compare_values(left: &Value, right: &Value) -> Option<std::cmp::Ordering> {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => a.partial_cmp(b),
            (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
            _ => None,
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

    // =========================================================================
    // TRACE-ENABLED EVALUATION FUNCTIONS
    // These functions are prepared for detailed condition-level tracing.
    // They will be used when we integrate more detailed trace collection.
    // =========================================================================

    /// Convert an Expression to a string representation for tracing
    #[allow(dead_code)]
    fn expression_to_string(expr: &Expression) -> String {
        match expr {
            Expression::Literal(val) => match val {
                Value::String(s) => format!("\"{}\"", s),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                Value::Array(arr) => format!(
                    "[{}]",
                    arr.iter()
                        .map(|v| match v {
                            Value::String(s) => format!("\"{}\"", s),
                            Value::Number(n) => n.to_string(),
                            _ => format!("{:?}", v),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Value::Object(_) => "{...}".to_string(),
            },
            Expression::FieldAccess(path) => path.join("."),
            Expression::Binary { left, op, right } => {
                let op_str = match op {
                    Operator::Eq => "==",
                    Operator::Ne => "!=",
                    Operator::Lt => "<",
                    Operator::Gt => ">",
                    Operator::Le => "<=",
                    Operator::Ge => ">=",
                    Operator::And => "&&",
                    Operator::Or => "||",
                    Operator::Add => "+",
                    Operator::Sub => "-",
                    Operator::Mul => "*",
                    Operator::Div => "/",
                    Operator::Mod => "%",
                    Operator::In => "in",
                    Operator::NotIn => "not in",
                    Operator::Contains => "contains",
                    Operator::StartsWith => "starts_with",
                    Operator::EndsWith => "ends_with",
                    Operator::Regex => "=~",
                    Operator::InList => "in list",
                    Operator::NotInList => "not in list",
                };
                format!(
                    "{} {} {}",
                    Self::expression_to_string(left),
                    op_str,
                    Self::expression_to_string(right)
                )
            }
            Expression::Unary { op, operand } => {
                format!("{:?} {}", op, Self::expression_to_string(operand))
            }
            Expression::FunctionCall { name, args } => {
                let args_str = args
                    .iter()
                    .map(|a| Self::expression_to_string(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, args_str)
            }
            Expression::Ternary { .. } => "?:".to_string(),
            Expression::LogicalGroup { op, .. } => {
                use corint_core::ast::LogicalGroupOp;
                match op {
                    LogicalGroupOp::Any => "any:[...]".to_string(),
                    LogicalGroupOp::All => "all:[...]".to_string(),
                }
            }
            Expression::ListReference { list_id } => {
                format!("list.{}", list_id)
            }
            Expression::ResultAccess { ruleset_id, field } => {
                match ruleset_id {
                    Some(id) => format!("result.{}.{}", id, field),
                    None => format!("result.{}", field),
                }
            }
        }
    }

    /// Convert Operator to string for tracing
    #[allow(dead_code)]
    fn operator_to_string(op: &Operator) -> &'static str {
        match op {
            Operator::Eq => "==",
            Operator::Ne => "!=",
            Operator::Lt => "<",
            Operator::Gt => ">",
            Operator::Le => "<=",
            Operator::Ge => ">=",
            Operator::And => "&&",
            Operator::Or => "||",
            Operator::Add => "+",
            Operator::Sub => "-",
            Operator::Mul => "*",
            Operator::Div => "/",
            Operator::Mod => "%",
            Operator::In => "in",
            Operator::NotIn => "not in",
            Operator::Contains => "contains",
            Operator::StartsWith => "starts_with",
            Operator::EndsWith => "ends_with",
            Operator::Regex => "=~",
            Operator::InList => "in list",
            Operator::NotInList => "not in list",
        }
    }

    /// Evaluate an expression with tracing enabled
    #[allow(dead_code)]
    fn evaluate_expression_with_trace(
        expr: &Expression,
        event_data: &HashMap<String, Value>,
    ) -> (bool, ConditionTrace) {
        match expr {
            Expression::Literal(val) => {
                let result = Self::is_truthy(val);
                let trace = ConditionTrace::new(Self::expression_to_string(expr), result);
                (result, trace)
            }
            Expression::FieldAccess(path) => {
                let val = Self::get_field_value(event_data, path).unwrap_or(Value::Null);
                let result = Self::is_truthy(&val);
                let mut trace = ConditionTrace::new(path.join("."), result);
                trace.left_value = Some(val);
                (result, trace)
            }
            Expression::Binary { left, op, right } => {
                let left_val = Self::expression_to_value(left, event_data);
                let right_val = Self::expression_to_value(right, event_data);
                let result = Self::evaluate_binary_expression(left, op, right, event_data);

                let trace = ConditionTrace::binary(
                    Self::expression_to_string(expr),
                    left_val,
                    Self::operator_to_string(op),
                    right_val,
                    result,
                );
                (result, trace)
            }
            Expression::LogicalGroup { op, conditions } => {
                use corint_core::ast::LogicalGroupOp;

                let mut nested_traces = Vec::new();
                let result = match op {
                    LogicalGroupOp::Any => {
                        let mut any_true = false;
                        for cond in conditions {
                            let (r, t) = Self::evaluate_expression_with_trace(cond, event_data);
                            nested_traces.push(t);
                            if r {
                                any_true = true;
                            }
                        }
                        any_true
                    }
                    LogicalGroupOp::All => {
                        let mut all_true = true;
                        for cond in conditions {
                            let (r, t) = Self::evaluate_expression_with_trace(cond, event_data);
                            nested_traces.push(t);
                            if !r {
                                all_true = false;
                            }
                        }
                        all_true
                    }
                };

                let group_type = match op {
                    LogicalGroupOp::Any => "any",
                    LogicalGroupOp::All => "all",
                };
                let trace = ConditionTrace::group(group_type, nested_traces, result);
                (result, trace)
            }
            _ => {
                // Unary, FunctionCall, Ternary - not fully supported
                let result = Self::evaluate_expression(expr, event_data);
                let trace = ConditionTrace::new(Self::expression_to_string(expr), result);
                (result, trace)
            }
        }
    }

    /// Evaluate a when block with tracing enabled
    #[allow(dead_code)]
    fn evaluate_when_block_with_trace(
        when: &WhenBlock,
        event_data: &HashMap<String, Value>,
    ) -> (bool, Vec<ConditionTrace>) {
        let mut traces = Vec::new();

        // Check event_type if specified
        if let Some(ref expected_type) = when.event_type {
            let actual = event_data
                .get("type")
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            let matched = &actual == expected_type;

            traces.push(ConditionTrace::binary(
                format!("event.type == \"{}\"", expected_type),
                Value::String(actual),
                "==",
                Value::String(expected_type.clone()),
                matched,
            ));

            if !matched {
                return (false, traces);
            }
        }

        // Evaluate condition_group (new format: all/any/not)
        if let Some(ref condition_group) = when.condition_group {
            let (result, group_traces) =
                Self::evaluate_condition_group_with_trace(condition_group, event_data);
            traces.extend(group_traces);
            return (result, traces);
        }

        // Evaluate all conditions (legacy format - AND logic)
        if let Some(ref conditions) = when.conditions {
            for condition in conditions {
                let (result, trace) = Self::evaluate_expression_with_trace(condition, event_data);
                traces.push(trace);
                if !result {
                    return (false, traces);
                }
            }
        }

        (true, traces)
    }

    /// Convert conditions JSON string to Vec<ConditionTrace>
    /// The JSON format is an array of structured condition objects from expression_to_json
    fn json_to_condition_traces(
        conditions_json: &str,
        triggered: bool,
        event_data: &HashMap<String, Value>,
    ) -> Vec<ConditionTrace> {
        // Parse the JSON string
        let conditions: Vec<serde_json::Value> = match serde_json::from_str(conditions_json) {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        conditions
            .into_iter()
            .map(|cond| Self::json_value_to_condition_trace(&cond, triggered, event_data))
            .collect()
    }

    /// Convert condition group JSON string to Vec<ConditionTrace>
    /// The JSON format is a ConditionGroup enum: {"all": [...]} or {"any": [...]} or {"not": [...]}
    fn condition_group_json_to_traces(
        condition_group_json: &str,
        _triggered: bool,
        event_data: &HashMap<String, Value>,
    ) -> Vec<ConditionTrace> {
        // Parse the JSON string
        let group: serde_json::Value = match serde_json::from_str(condition_group_json) {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        // Determine the group type and get conditions
        let (group_type, conditions) = if let Some(all_conditions) = group.get("all") {
            ("all", all_conditions.as_array())
        } else if let Some(any_conditions) = group.get("any") {
            ("any", any_conditions.as_array())
        } else if let Some(not_conditions) = group.get("not") {
            ("not", not_conditions.as_array())
        } else {
            return vec![];
        };

        let conditions = match conditions {
            Some(arr) => arr,
            None => return vec![],
        };

        // Convert each condition to a trace
        let nested: Vec<ConditionTrace> = conditions
            .iter()
            .map(|cond| Self::condition_to_trace(cond, false, event_data))
            .collect();

        // Calculate actual group result based on nested results
        let group_result = match group_type {
            "all" => nested.iter().all(|c| c.result),
            "any" => nested.iter().any(|c| c.result),
            "not" => !nested.iter().all(|c| c.result),
            _ => false,
        };

        // Return a single group trace containing all nested conditions
        vec![ConditionTrace::group(group_type, nested, group_result)]
    }

    /// Convert a single condition (Expression or nested Group) to ConditionTrace
    fn condition_to_trace(
        cond: &serde_json::Value,
        _triggered: bool,
        event_data: &HashMap<String, Value>,
    ) -> ConditionTrace {
        // Check if it's a nested group (has "all", "any", or "not" key)
        if let Some(all_conditions) = cond.get("all") {
            if let Some(arr) = all_conditions.as_array() {
                let nested: Vec<ConditionTrace> = arr
                    .iter()
                    .map(|c| Self::condition_to_trace(c, false, event_data))
                    .collect();
                // "all" is true only if all nested conditions are true
                let group_result = nested.iter().all(|c| c.result);
                return ConditionTrace::group("all", nested, group_result);
            }
        }
        if let Some(any_conditions) = cond.get("any") {
            if let Some(arr) = any_conditions.as_array() {
                let nested: Vec<ConditionTrace> = arr
                    .iter()
                    .map(|c| Self::condition_to_trace(c, false, event_data))
                    .collect();
                // "any" is true if any nested condition is true
                let group_result = nested.iter().any(|c| c.result);
                return ConditionTrace::group("any", nested, group_result);
            }
        }
        if let Some(not_conditions) = cond.get("not") {
            if let Some(arr) = not_conditions.as_array() {
                let nested: Vec<ConditionTrace> = arr
                    .iter()
                    .map(|c| Self::condition_to_trace(c, false, event_data))
                    .collect();
                // "not" is true if all nested conditions are false (negation of "all")
                let group_result = !nested.iter().all(|c| c.result);
                return ConditionTrace::group("not", nested, group_result);
            }
        }

        // It's an expression - convert to string representation
        Self::expression_json_to_trace(cond, false, event_data)
    }

    /// Convert an expression JSON to ConditionTrace
    fn expression_json_to_trace(
        expr: &serde_json::Value,
        _triggered: bool,
        event_data: &HashMap<String, Value>,
    ) -> ConditionTrace {
        // Try to build a human-readable expression string
        let expression = Self::expr_json_to_string(expr);

        // Default result - will be calculated for Binary expressions
        let mut result = false;
        let mut left_value_for_display: Option<Value> = None;
        let mut right_value_for_display: Option<Value> = None;

        // For Binary expressions, extract left/right values and calculate actual result
        if let Some(binary) = expr.get("Binary") {
            if let Some(obj) = binary.as_object() {
                let left_expr = obj.get("left");
                let right_expr = obj.get("right");
                let op = obj.get("op").and_then(|o| o.as_str()).unwrap_or("");

                // Extract actual values for evaluation
                let left_val = left_expr.and_then(|e| Self::extract_value_from_expr_json(e, event_data));
                let right_val = right_expr.and_then(|e| Self::extract_value_from_expr_json(e, event_data));

                // Set display values (only for non-constants)
                // Always show the value for FieldAccess, even if null (to indicate field not found)
                if let Some(left_e) = left_expr {
                    if Self::should_display_value(left_e) {
                        left_value_for_display = Some(left_val.clone().unwrap_or(Value::Null));
                    }
                }
                if let Some(right_e) = right_expr {
                    if Self::should_display_value(right_e) {
                        right_value_for_display = Some(right_val.clone().unwrap_or(Value::Null));
                    }
                }

                // Calculate actual result
                if let (Some(lv), Some(rv)) = (&left_val, &right_val) {
                    result = Self::evaluate_comparison(lv, op, rv);
                }
            }
        }

        let mut trace = ConditionTrace::new(expression, result);
        trace.left_value = left_value_for_display;
        trace.right_value = right_value_for_display;
        trace
    }

    /// Evaluate a comparison between two values
    fn evaluate_comparison(left: &Value, op: &str, right: &Value) -> bool {
        match (left, right) {
            (Value::Number(l), Value::Number(r)) => {
                match op {
                    // Enum-style names
                    "Gt" | ">" => l > r,
                    "Ge" | ">=" => l >= r,
                    "Lt" | "<" => l < r,
                    "Le" | "<=" => l <= r,
                    "Eq" | "==" => (l - r).abs() < f64::EPSILON,
                    "Ne" | "!=" => (l - r).abs() >= f64::EPSILON,
                    _ => false,
                }
            }
            (Value::String(l), Value::String(r)) => {
                match op {
                    "Eq" | "==" => l == r,
                    "Ne" | "!=" => l != r,
                    "Gt" | ">" => l > r,
                    "Ge" | ">=" => l >= r,
                    "Lt" | "<" => l < r,
                    "Le" | "<=" => l <= r,
                    "Contains" | "contains" => l.contains(r.as_str()),
                    "StartsWith" | "starts_with" => l.starts_with(r.as_str()),
                    "EndsWith" | "ends_with" => l.ends_with(r.as_str()),
                    _ => false,
                }
            }
            (Value::Bool(l), Value::Bool(r)) => {
                match op {
                    "Eq" | "==" => l == r,
                    "Ne" | "!=" => l != r,
                    "And" | "&&" => *l && *r,
                    "Or" | "||" => *l || *r,
                    _ => false,
                }
            }
            // Handle cross-type comparisons for equality
            _ => {
                match op {
                    "Eq" | "==" => left == right,
                    "Ne" | "!=" => left != right,
                    _ => false,
                }
            }
        }
    }

    /// Check if an expression's value should be displayed in trace
    /// Returns true for FieldAccess and complex expressions, false for Literal, ListReference, and boolean values
    fn should_display_value(expr: &serde_json::Value) -> bool {
        // Skip Literal values (constants)
        if let Some(literal) = expr.get("Literal") {
            // Also skip boolean literals
            if literal.is_boolean() {
                return false;
            }
            return false;
        }

        // Skip ListReference (lists)
        if expr.get("ListReference").is_some() {
            return false;
        }

        // Skip direct boolean values
        if expr.is_boolean() {
            return false;
        }

        // FieldAccess should be displayed
        if expr.get("FieldAccess").is_some() {
            return true;
        }

        // FunctionCall should be displayed
        if expr.get("FunctionCall").is_some() {
            return true;
        }

        // Binary expressions (complex calculations) should be displayed
        if expr.get("Binary").is_some() {
            return true;
        }

        // Unary expressions should be displayed
        if expr.get("Unary").is_some() {
            return true;
        }

        false
    }

    /// Extract actual value from expression JSON (new format with Binary/FieldAccess keys)
    fn extract_value_from_expr_json(
        expr: &serde_json::Value,
        event_data: &HashMap<String, Value>,
    ) -> Option<Value> {
        // Handle FieldAccess: {"FieldAccess": ["event", "transaction", "amount"]} or {"FieldAccess": ["features", "transaction_sum_7d"]}
        if let Some(field_access) = expr.get("FieldAccess") {
            if let Some(arr) = field_access.as_array() {
                let path: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                // Strip namespace prefixes since event_data contains merged data
                // Field paths may be:
                // - ["event", "transaction", "amount"] - strip "event" prefix
                // - ["features", "transaction_sum_7d"] - strip "features" prefix
                // - ["api", "device_fingerprint", "score"] - strip "api" prefix
                let effective_path: Vec<String> =
                    if let Some(first) = path.first() {
                        match first.as_str() {
                            // Known namespaces - strip the prefix since trace_data is a flat merge
                            "event" | "features" | "api" | "service" | "llm" | "vars" => {
                                path.into_iter().skip(1).collect()
                            }
                            _ => path,
                        }
                    } else {
                        path
                    };
                let result = Self::get_field_value(event_data, &effective_path);
                if result.is_none() {
                    tracing::debug!("Failed to get field value for path {:?}, event_data keys: {:?}", effective_path, event_data.keys().collect::<Vec<_>>());
                }
                return result;
            }
        }

        // Handle Literal: {"Literal": 10000.0} or nested like {"Literal": {"Number": 70}}
        if let Some(literal) = expr.get("Literal") {
            // Try direct conversion first
            if let Some(val) = Self::json_to_core_value(literal) {
                return Some(val);
            }
            // Handle nested Value format: {"Literal": {"Number": 70}}
            if let Some(obj) = literal.as_object() {
                if let Some(num) = obj.get("Number") {
                    return num.as_f64().map(Value::Number);
                }
                if let Some(s) = obj.get("String") {
                    return s.as_str().map(|s| Value::String(s.to_string()));
                }
                if let Some(b) = obj.get("Bool") {
                    return b.as_bool().map(Value::Bool);
                }
                if obj.contains_key("Null") {
                    return Some(Value::Null);
                }
            }
            return None;
        }

        // Handle Binary expression (calculate the result)
        if let Some(binary) = expr.get("Binary") {
            if let Some(obj) = binary.as_object() {
                let left = obj.get("left")?;
                let right = obj.get("right")?;
                let op = obj.get("op").and_then(|o| o.as_str())?;

                let left_val = Self::extract_value_from_expr_json(left, event_data)?;
                let right_val = Self::extract_value_from_expr_json(right, event_data)?;

                // Handle numeric operations
                if let (Value::Number(l), Value::Number(r)) = (&left_val, &right_val) {
                    let result = match op {
                        "Add" => l + r,
                        "Sub" => l - r,
                        "Mul" => l * r,
                        "Div" => if *r != 0.0 { l / r } else { return None },
                        "Mod" => l % r,
                        _ => return None,
                    };
                    return Some(Value::Number(result));
                }
            }
        }

        // Handle FunctionCall - cannot extract value without runtime context
        if expr.get("FunctionCall").is_some() {
            return None;
        }

        None
    }

    /// Convert expression JSON to readable string
    fn expr_json_to_string(expr: &serde_json::Value) -> String {
        // Check for Binary expression: {"Binary": {"left": ..., "op": "Gt", "right": ...}}
        if let Some(binary) = expr.get("Binary") {
            if let Some(obj) = binary.as_object() {
                let left = obj.get("left").map(|l| Self::expr_json_to_string(l)).unwrap_or_default();
                let op = obj.get("op").map(|o| Self::operator_to_symbol(o)).unwrap_or("?".to_string());
                let right = obj.get("right").map(|r| Self::expr_json_to_string(r)).unwrap_or_default();
                return format!("{} {} {}", left, op, right);
            }
        }

        // Check for FieldAccess: {"FieldAccess": ["event", "transaction", "amount"]}
        if let Some(field_access) = expr.get("FieldAccess") {
            if let Some(arr) = field_access.as_array() {
                let parts: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                return parts.join(".");
            }
        }

        // Check for Literal: {"Literal": 10000.0} or {"Literal": "USD"} or {"Literal": true}
        if let Some(literal) = expr.get("Literal") {
            return Self::value_to_string(literal);
        }

        // Check for Unary: {"Unary": {"op": "Not", "operand": ...}}
        if let Some(unary) = expr.get("Unary") {
            if let Some(obj) = unary.as_object() {
                let op = obj.get("op").and_then(|o| o.as_str()).unwrap_or("!");
                let operand = obj.get("operand").map(|o| Self::expr_json_to_string(o)).unwrap_or_default();
                let op_symbol = if op == "Not" { "!" } else if op == "Negate" { "-" } else { op };
                return format!("{}{}", op_symbol, operand);
            }
        }

        // Check for ListReference: {"ListReference": {"list_id": "..."}}
        if let Some(list_ref) = expr.get("ListReference") {
            if let Some(obj) = list_ref.as_object() {
                if let Some(list_id) = obj.get("list_id").and_then(|v| v.as_str()) {
                    return format!("list.{}", list_id);
                }
            }
        }

        // Check for FunctionCall: {"FunctionCall": {"name": "count", "args": [...]}}
        if let Some(func) = expr.get("FunctionCall") {
            if let Some(obj) = func.as_object() {
                let name = obj.get("name").and_then(|n| n.as_str()).unwrap_or("func");
                let args = obj.get("args")
                    .and_then(|a| a.as_array())
                    .map(|arr| arr.iter().map(|a| Self::expr_json_to_string(a)).collect::<Vec<_>>().join(", "))
                    .unwrap_or_default();
                return format!("{}({})", name, args);
            }
        }

        // Fallback: try to format nicely
        if let Some(s) = expr.as_str() {
            return s.to_string();
        }
        if let Some(n) = expr.as_f64() {
            return format!("{}", n);
        }
        if let Some(b) = expr.as_bool() {
            return format!("{}", b);
        }
        if expr.is_null() {
            return "null".to_string();
        }

        // Last resort: serialize as JSON
        expr.to_string()
    }

    /// Convert operator JSON to readable symbol
    fn operator_to_symbol(op: &serde_json::Value) -> String {
        let op_str = op.as_str().unwrap_or("");
        match op_str {
            "Eq" => "==".to_string(),
            "Ne" => "!=".to_string(),
            "Gt" => ">".to_string(),
            "Ge" => ">=".to_string(),
            "Lt" => "<".to_string(),
            "Le" => "<=".to_string(),
            "Add" => "+".to_string(),
            "Sub" => "-".to_string(),
            "Mul" => "*".to_string(),
            "Div" => "/".to_string(),
            "Mod" => "%".to_string(),
            "And" => "&&".to_string(),
            "Or" => "||".to_string(),
            "Contains" => "contains".to_string(),
            "StartsWith" => "starts_with".to_string(),
            "EndsWith" => "ends_with".to_string(),
            "Regex" => "regex".to_string(),
            "In" => "in".to_string(),
            "NotIn" => "not in".to_string(),
            "InList" => "in".to_string(),
            "NotInList" => "not in".to_string(),
            _ => op_str.to_string(),
        }
    }

    /// Convert JSON value to readable string representation
    fn value_to_string(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Bool(b) => format!("{}", b),
            serde_json::Value::Number(n) => format!("{}", n),
            serde_json::Value::String(s) => format!("\"{}\"", s),
            serde_json::Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(Self::value_to_string).collect();
                format!("[{}]", items.join(", "))
            }
            serde_json::Value::Object(obj) => {
                // Check if it's a Value enum variant
                if let Some(v) = obj.get("String") {
                    return format!("\"{}\"", v.as_str().unwrap_or(""));
                }
                if let Some(v) = obj.get("Number") {
                    return format!("{}", v);
                }
                if let Some(v) = obj.get("Bool") {
                    return format!("{}", v);
                }
                if obj.contains_key("Null") {
                    return "null".to_string();
                }
                // Default: just serialize
                format!("{}", value)
            }
        }
    }

    /// Convert a single JSON value to ConditionTrace
    fn json_value_to_condition_trace(
        json: &serde_json::Value,
        _triggered: bool,
        event_data: &HashMap<String, Value>,
    ) -> ConditionTrace {
        let expr_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let expression = json
            .get("expression")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        match expr_type {
            "group" => {
                // Logical group (any/all)
                let group_type = json
                    .get("group_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("all");
                let nested_conditions: Vec<ConditionTrace> = json
                    .get("conditions")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|c| Self::json_value_to_condition_trace(c, false, event_data))
                            .collect()
                    })
                    .unwrap_or_default();

                // Calculate actual result based on nested results
                let result = match group_type {
                    "all" => nested_conditions.iter().all(|c| c.result),
                    "any" => nested_conditions.iter().any(|c| c.result),
                    "not" => !nested_conditions.iter().all(|c| c.result),
                    _ => false,
                };

                ConditionTrace::group(group_type, nested_conditions, result)
            }
            "binary" => {
                // Check right side type
                let right_json = json.get("right");
                let right_type = right_json
                    .and_then(|r| r.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Check if right side is a boolean literal - if so, skip both left and right values
                let is_boolean_literal = right_type == "literal"
                    && right_json
                        .and_then(|r| r.get("value"))
                        .map(|v| v.is_boolean())
                        .unwrap_or(false);

                // Check if right side is a simple literal (not boolean)
                let is_simple_literal = right_type == "literal" && !is_boolean_literal;

                // Extract left value from event data
                let left_val = json.get("left")
                    .and_then(|left| Self::extract_value_from_json_expr(left, event_data));

                // Extract right value
                let right_val = right_json.and_then(|right| {
                    Self::extract_value_from_json_expr(right, event_data)
                });

                // Calculate the actual result
                let operator = json.get("operator").and_then(|v| v.as_str()).unwrap_or("");
                let result = if let (Some(ref lv), Some(ref rv)) = (&left_val, &right_val) {
                    Self::evaluate_comparison(lv, operator, rv)
                } else {
                    false
                };

                // For display: skip values for boolean literals and simple literals
                let left_value_display = if is_boolean_literal {
                    None
                } else {
                    left_val
                };

                let right_value_display = if is_boolean_literal || is_simple_literal {
                    None
                } else {
                    right_val
                };

                ConditionTrace {
                    expression,
                    left_value: left_value_display,
                    operator: None, // Operator is already visible in expression string
                    right_value: right_value_display,
                    result,
                    nested: None,
                    group_type: None,
                }
            }
            _ => {
                // Literal, field access, or other - evaluate to determine result
                let result = if let Some(val) = Self::extract_value_from_json_expr(json, event_data) {
                    Self::is_truthy(&val)
                } else {
                    false
                };
                ConditionTrace::new(expression, result)
            }
        }
    }

    /// Extract the actual value from a JSON expression (field access, literal, or binary)
    fn extract_value_from_json_expr(
        json: &serde_json::Value,
        event_data: &HashMap<String, Value>,
    ) -> Option<Value> {
        let expr_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match expr_type {
            "field" => {
                // Field access - extract path and look up value
                let path: Vec<String> = json
                    .get("path")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                // Handle namespace prefixes
                // event_data contains both event data and computed features/vars merged together
                // Field paths may be:
                // - ["event", "transaction", "amount"] - strip "event" prefix
                // - ["features", "transaction_sum_7d"] - strip "features" prefix (features are flat in trace_data)
                // - ["api", "device_fingerprint", "score"] - strip "api" prefix
                // - ["user_id"] - no prefix, look up directly
                let effective_path: Vec<String> =
                    if let Some(first) = path.first() {
                        match first.as_str() {
                            // Known namespaces - strip the prefix since trace_data is a flat merge
                            "event" | "features" | "api" | "service" | "llm" | "vars" => {
                                path.into_iter().skip(1).collect()
                            }
                            _ => path,
                        }
                    } else {
                        path
                    };

                Self::get_field_value(event_data, &effective_path)
            }
            "literal" => {
                // Literal value - convert from serde_json::Value to corint_core::Value
                json.get("value").and_then(Self::json_to_core_value)
            }
            "binary" => {
                // Binary expression (e.g., event.user.average_transaction * 3)
                // Recursively evaluate left and right, then apply operator
                let left = json.get("left")?;
                let right = json.get("right")?;
                let operator = json.get("operator").and_then(|v| v.as_str())?;

                let left_val = Self::extract_value_from_json_expr(left, event_data)?;
                let right_val = Self::extract_value_from_json_expr(right, event_data)?;

                // Only handle numeric operations for now
                if let (Value::Number(l), Value::Number(r)) = (&left_val, &right_val) {
                    let result = match operator {
                        "+" | "Add" => l + r,
                        "-" | "Sub" => l - r,
                        "*" | "Mul" => l * r,
                        "/" | "Div" => {
                            if *r != 0.0 {
                                l / r
                            } else {
                                return None;
                            }
                        }
                        "%" | "Mod" => {
                            if *r != 0.0 {
                                l % r
                            } else {
                                return None;
                            }
                        }
                        _ => return None, // Comparison operators return bool, skip
                    };
                    Some(Value::Number(result))
                } else {
                    // For string concatenation
                    if operator == "+" || operator == "Add" {
                        if let (Value::String(l), Value::String(r)) = (&left_val, &right_val) {
                            return Some(Value::String(format!("{}{}", l, r)));
                        }
                    }
                    None
                }
            }
            _ => None,
        }
    }

    /// Convert serde_json::Value to corint_core::Value
    fn json_to_core_value(json: &serde_json::Value) -> Option<Value> {
        match json {
            serde_json::Value::Null => Some(Value::Null),
            serde_json::Value::Bool(b) => Some(Value::Bool(*b)),
            serde_json::Value::Number(n) => n.as_f64().map(Value::Number),
            serde_json::Value::String(s) => Some(Value::String(s.clone())),
            serde_json::Value::Array(arr) => {
                let values: Vec<Value> = arr.iter().filter_map(Self::json_to_core_value).collect();
                Some(Value::Array(values))
            }
            serde_json::Value::Object(obj) => {
                let map: HashMap<String, Value> = obj
                    .iter()
                    .filter_map(|(k, v)| Self::json_to_core_value(v).map(|val| (k.clone(), val)))
                    .collect();
                Some(Value::Object(map))
            }
        }
    }

    /// Load and compile rules from a file
    async fn load_and_compile_rules(path: &Path, compiler: &mut Compiler) -> Result<Vec<Program>> {
        use corint_parser::YamlParser;

        // Read file
        let content = tokio::fs::read_to_string(path).await?;

        tracing::debug!("Loading file: {}", path.display());

        let mut programs = Vec::new();
        let mut has_pipeline = false;
        let mut pipeline_count = 0;

        // Parse multi-document YAML (supports files with --- separators)
        let documents = YamlParser::parse_multi_document(&content)?;

        // Try to parse each document
        for doc in documents.iter() {
            // Try rule first
            if let Ok(rule) = RuleParser::parse_from_yaml(doc) {
                let prog = compiler.compile_rule(&rule)?;
                programs.push(prog);
                continue;
            }

            // Try ruleset
            if let Ok(ruleset) = RulesetParser::parse_from_yaml(doc) {
                let prog = compiler.compile_ruleset(&ruleset)?;
                programs.push(prog);
                continue;
            }

            // Try pipeline
            if let Ok(pipeline) = PipelineParser::parse_from_yaml(doc) {
                has_pipeline = true;
                pipeline_count += 1;

                // Validate: Pipeline must have when condition
                if pipeline.when.is_none() {
                    return Err(SdkError::InvalidRuleFile(format!(
                        "Pipeline '{}' in file '{}' is missing mandatory 'when' condition. \
                         All pipelines must specify when conditions to filter events.",
                        &pipeline.id,
                        path.display()
                    )));
                }

                tracing::debug!(
                    "Parsed pipeline: when={:?}, steps={}",
                    pipeline.when,
                    pipeline.steps.len()
                );
                let prog = compiler.compile_pipeline(&pipeline)?;
                tracing::debug!(
                    "Compiled pipeline: {} instructions",
                    prog.instructions.len()
                );
                programs.push(prog);
                continue;
            }

            // Skip documents that don't match any known type (e.g., metadata sections)
        }

        // If no valid documents were found, return error
        if programs.is_empty() {
            return Err(SdkError::InvalidRuleFile(format!(
                "File does not contain a valid rule, ruleset, or pipeline: {}",
                path.display()
            )));
        }

        // Validate: File must contain at least one pipeline
        if !has_pipeline {
            return Err(SdkError::InvalidRuleFile(format!(
                "Rule file '{}' must contain at least one pipeline definition. \
                 Pipelines are the entry points for rule execution and must have 'when' conditions. \
                 Rules and rulesets cannot be used as top-level entry points.",
                path.display()
            )));
        }

        tracing::info!(
            "✓ Loaded file '{}': {} pipeline(s), {} total definitions",
            path.display(),
            pipeline_count,
            programs.len()
        );

        Ok(programs)
    }

    /// Compile rules from content string (from repository)
    async fn compile_rules_from_content(
        id: &str,
        content: &str,
        compiler: &mut Compiler,
    ) -> Result<Vec<Program>> {
        use corint_parser::YamlParser;

        tracing::debug!("Compiling content from: {}", id);

        let mut programs = Vec::new();
        let mut has_pipeline = false;
        let mut pipeline_count = 0;

        // First, try to parse as a pipeline with imports (most common case for repository content)
        if let Ok(document) = corint_parser::PipelineParser::parse_with_imports(content) {
            has_pipeline = true;
            pipeline_count += 1;

            // Validate: Pipeline must have when condition
            if document.definition.when.is_none() {
                return Err(SdkError::InvalidRuleFile(format!(
                    "Pipeline '{}' from '{}' is missing mandatory 'when' condition. \
                     All pipelines must specify when conditions to filter events.",
                    &document.definition.id,
                    id
                )));
            }

            tracing::debug!(
                "Parsed pipeline with imports: when={:?}, steps={}, imports={:?}",
                document.definition.when,
                document.definition.steps.len(),
                document.imports.is_some()
            );

            // Resolve imports and compile dependencies
            let resolved = compiler
                .import_resolver_mut()
                .resolve_imports(&document)
                .map_err(SdkError::CompileError)?;

            tracing::debug!(
                "Resolved imports: {} rules, {} rulesets",
                resolved.rules.len(),
                resolved.rulesets.len()
            );

            // Compile all resolved rules first
            for rule in &resolved.rules {
                let rule_prog = compiler.compile_rule(rule)?;
                programs.push(rule_prog);
            }

            // Compile all resolved rulesets
            for ruleset in &resolved.rulesets {
                let ruleset_prog = compiler.compile_ruleset(ruleset)?;
                programs.push(ruleset_prog);
            }

            // Finally compile the pipeline itself
            let prog = compiler.compile_pipeline(&document.definition)?;
            tracing::debug!(
                "Compiled pipeline: {} instructions",
                prog.instructions.len()
            );
            programs.push(prog);

            // IMPORTANT: Also parse inline rules and rulesets from the same YAML file
            // This supports the format where pipeline, rules, and rulesets are in the same file
            let documents = YamlParser::parse_multi_document(content)?;
            if documents.len() > 1 {
                tracing::debug!(
                    "Found {} documents in file, checking for inline rules/rulesets",
                    documents.len()
                );

                use corint_parser::{RuleParser, RulesetParser};

                for (doc_idx, doc) in documents.iter().enumerate() {
                    // Skip the first document (already processed as pipeline metadata)
                    if doc_idx == 0 {
                        continue;
                    }

                    // Skip if it's the pipeline definition (already compiled above)
                    if doc.get("pipeline").is_some() {
                        continue;
                    }

                    // Try to parse as rule
                    if let Ok(rule) = RuleParser::parse_from_yaml(doc) {
                        tracing::debug!("Found inline rule: {}", rule.id);
                        let rule_prog = compiler.compile_rule(&rule)?;
                        programs.push(rule_prog);
                        continue;
                    }

                    // Try to parse as ruleset
                    if let Ok(ruleset) = RulesetParser::parse_from_yaml(doc) {
                        tracing::debug!("Found inline ruleset: {}", ruleset.id);
                        let ruleset_prog = compiler.compile_ruleset(&ruleset)?;
                        programs.push(ruleset_prog);
                        continue;
                    }
                }
            }
        } else {
            // Fallback: Parse as multi-document YAML for individual rules/rulesets
            let documents = YamlParser::parse_multi_document(content)?;

            // Try to parse each document
            for doc in documents.iter() {
                // Try rule first
                if let Ok(rule) = RuleParser::parse_from_yaml(doc) {
                    let prog = compiler.compile_rule(&rule)?;
                    programs.push(prog);
                    continue;
                }

                // Try ruleset
                if let Ok(ruleset) = RulesetParser::parse_from_yaml(doc) {
                    let prog = compiler.compile_ruleset(&ruleset)?;
                    programs.push(prog);
                    continue;
                }

                // Try pipeline (without imports, since parse_with_imports already failed above)
                if let Ok(pipeline) = PipelineParser::parse_from_yaml(doc) {
                    has_pipeline = true;
                    pipeline_count += 1;

                    // Validate: Pipeline must have when condition
                    if pipeline.when.is_none() {
                        return Err(SdkError::InvalidRuleFile(format!(
                            "Pipeline '{}' from '{}' is missing mandatory 'when' condition. \
                         All pipelines must specify when conditions to filter events.",
                            &pipeline.id,
                            id
                        )));
                    }

                    let prog = compiler.compile_pipeline(&pipeline)?;
                    tracing::debug!(
                        "Compiled pipeline (no imports): {} instructions",
                        prog.instructions.len()
                    );
                    programs.push(prog);
                    continue;
                }

                // Skip documents that don't match any known type (e.g., metadata sections)
            }
        } // Close the else block

        // If no valid documents were found, return error
        if programs.is_empty() {
            return Err(SdkError::InvalidRuleFile(format!(
                "Content from '{}' does not contain a valid rule, ruleset, or pipeline",
                id
            )));
        }

        // Validate: Content must contain at least one pipeline
        if !has_pipeline {
            return Err(SdkError::InvalidRuleFile(format!(
                "Content from '{}' must contain at least one pipeline definition. \
                 Pipelines are the entry points for rule execution and must have 'when' conditions. \
                 Rules and rulesets cannot be used as top-level entry points.",
                id
            )));
        }

        tracing::info!(
            "✓ Loaded content '{}': {} pipeline(s), {} total definitions",
            id,
            pipeline_count,
            programs.len()
        );

        Ok(programs)
    }

    /// Execute a decision request
    /// Reload rules and configurations from repository
    ///
    /// This method reloads all content from the configured repository and recompiles
    /// all rules, rulesets, and pipelines. It preserves the feature executor,
    /// list service, and result writer configurations.
    ///
    /// # Returns
    ///
    /// Returns an error if the repository is not configured or if reloading fails.
    pub async fn reload(&mut self) -> Result<()> {
        use corint_repository::RepositoryLoader;

        // Check if repository is configured
        let repo_config = self.repository_config.as_ref().ok_or_else(|| {
            SdkError::Config("Repository not configured. Cannot reload.".to_string())
        })?;

        tracing::info!("Reloading repository content...");

        // Load content from repository
        let loader = RepositoryLoader::new(repo_config.clone());
        let content = loader.load_all().await.map_err(|e| {
            SdkError::Config(format!("Failed to load repository: {}", e))
        })?;

        // Create a new config with repository content merged
        let mut new_config = self.config.clone();
        
        // Clear existing rule contents and files (they will be replaced by repository content)
        new_config.rule_contents.clear();
        new_config.rule_files.clear();
        new_config.registry_content = None;
        new_config.registry_file = None;

        // Merge repository content into config
        if let Some(registry) = content.registry {
            new_config.registry_content = Some(registry);
        }

        // Add all pipelines as rule content
        for (id, yaml) in content.pipelines {
            new_config.rule_contents.push((id, yaml));
        }

        // Recompile all programs
        let mut programs = Vec::new();
        let compiler_opts = CompilerOpts {
            enable_semantic_analysis: new_config.compiler_options.enable_semantic_analysis,
            enable_constant_folding: new_config.compiler_options.enable_constant_folding,
            enable_dead_code_elimination: true,
            library_base_path: "repository".to_string(),
        };

        let mut compiler = Compiler::with_options(compiler_opts);

        // Compile rule contents (from repository)
        for (id, content_str) in &new_config.rule_contents {
            programs.extend(Self::compile_rules_from_content(id, content_str, &mut compiler).await?);
        }

        // Build ruleset_map, rule_map, and pipeline_map for routing
        let mut ruleset_map = HashMap::new();
        let mut rule_map = HashMap::new();
        let mut pipeline_map = HashMap::new();
        for program in &programs {
            match program.metadata.source_type.as_str() {
                "ruleset" => {
                    ruleset_map.insert(program.metadata.source_id.clone(), program.clone());
                }
                "rule" => {
                    rule_map.insert(program.metadata.source_id.clone(), program.clone());
                }
                "pipeline" => {
                    pipeline_map.insert(program.metadata.source_id.clone(), program.clone());
                }
                _ => {}
            }
        }

        // Load optional registry
        let registry = if let Some(registry_content) = &new_config.registry_content {
            match RegistryParser::parse(registry_content) {
                Ok(reg) => {
                    tracing::info!(
                        "✓ Reloaded pipeline registry: {} entries",
                        reg.registry.len()
                    );
                    Some(reg)
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse registry content: {}. Continuing without registry.",
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        // Update engine state
        self.programs = programs;
        self.ruleset_map = ruleset_map;
        self.rule_map = rule_map;
        self.pipeline_map = pipeline_map;
        self.registry = registry;
        self.config = new_config;

        tracing::info!("✓ Repository reloaded successfully");

        Ok(())
    }

    pub async fn decide(&self, mut request: DecisionRequest) -> Result<DecisionResponse> {
        use corint_runtime::result::ExecutionResult;

        let start = std::time::Instant::now();

        // Track rule executions for persistence
        let mut rule_executions: Vec<corint_runtime::RuleExecutionRecord> = Vec::new();

        // Generate request ID from metadata or create new one
        // Generate at the very beginning of the request (only once)
        // Store it in metadata so it's available throughout the request lifecycle
        let request_id = if let Some(existing_id) = request.metadata.get("request_id") {
            existing_id.clone()
        } else {
            let new_id = Self::generate_request_id();
            request
                .metadata
                .insert("request_id".to_string(), new_id.clone());
            tracing::debug!("Generated new request_id: {}", new_id);
            new_id
        };

        // Create initial execution result
        let mut execution_result = ExecutionResult::new();

        let mut combined_result = DecisionResult {
            signal: None,
            actions: Vec::new(),
            score: 0,
            triggered_rules: Vec::new(),
            explanation: String::new(),
            context: HashMap::new(),
        };

        // Track whether pipeline routing handled rule execution
        let mut pipeline_handled_rules = false;
        // Track whether any pipeline actually matched (when 条件命中)
        let mut pipeline_matched = false;
        // Track which pipeline was matched for this request
        let mut matched_pipeline_id: Option<String> = None;
        // Track branch execution info from pipeline (preserved before rules overwrite context)
        let mut executed_branch_index: Option<usize> = None;
        let mut executed_branch_condition: Option<String> = None;

        // PRIORITY 1: Use Registry-based routing if available
        if let Some(ref registry) = self.registry {
            tracing::debug!(
                "Using registry-based routing with {} entries",
                registry.registry.len()
            );

            // Find the first matching registry entry (top-to-bottom order)
            for (idx, entry) in registry.registry.iter().enumerate() {
                tracing::debug!(
                    "Checking registry entry {}: pipeline={}, when={:?}",
                    idx,
                    entry.pipeline,
                    entry.when
                );

                // Evaluate when block against event data
                if Self::evaluate_when_block(&entry.when, &request.event_data) {
                    tracing::info!(
                        "✓ Registry matched entry {}: pipeline={}",
                        idx,
                        entry.pipeline
                    );

                    // Record the matched pipeline ID
                    matched_pipeline_id = Some(entry.pipeline.clone());

                    // Get the pipeline program
                    if let Some(pipeline_program) = self.pipeline_map.get(&entry.pipeline) {
                        // Log pipeline execution at INFO level
                        tracing::info!(
                            "🚀 Executing pipeline: {} (request_id={})",
                            entry.pipeline,
                            request_id
                        );

                        // Execute the matched pipeline
                        let result = self
                            .executor
                            .execute_with_result(
                                pipeline_program,
                                request.to_context_input(),
                                execution_result.clone(),
                            )
                            .await?;

                        pipeline_matched = true;

                        // Update execution_result with pipeline context
                        execution_result.variables = result.context.clone();

                        // Preserve branch execution info from pipeline execution
                        if let Some(Value::Number(branch_idx)) =
                            result.context.get("__executed_branch_index__")
                        {
                            executed_branch_index = Some(*branch_idx as usize);
                            if let Some(Value::String(branch_cond)) =
                                result.context.get("__executed_branch_condition__")
                            {
                                executed_branch_condition = Some(branch_cond.clone());
                            }
                        }

                        // Get list of rulesets to execute - prefer array, fall back to single
                        let rulesets_to_execute: Vec<String> = if let Some(Value::Array(arr)) =
                            result.context.get("__rulesets_to_execute__")
                        {
                            arr.iter()
                                .filter_map(|v| {
                                    if let Value::String(s) = v {
                                        Some(s.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        } else if let Some(Value::String(ruleset_id)) =
                            result.context.get("__next_ruleset__")
                        {
                            vec![ruleset_id.clone()]
                        } else {
                            vec![]
                        };

                        if !rulesets_to_execute.is_empty() {
                            tracing::debug!(
                                "Pipeline routing to {} rulesets: {:?}",
                                rulesets_to_execute.len(),
                                rulesets_to_execute
                            );

                            // Execute ALL rulesets in order
                            for ruleset_id in &rulesets_to_execute {
                                if let Some(ruleset_program) = self.ruleset_map.get(ruleset_id) {
                                    // Execute rules first
                                    if let Some(rules_str) =
                                        ruleset_program.metadata.custom.get("rules")
                                    {
                                        let mut seen = std::collections::HashSet::new();
                                        let rule_ids: Vec<&str> = rules_str
                                            .split(',')
                                            .filter(|rid| seen.insert(*rid))
                                            .collect();

                                        for rule_id in rule_ids {
                                            if let Some(rule_program) = self.rule_map.get(rule_id)
                                            {
                                                tracing::info!(
                                                    "Executing rule (via ruleset {}): {}",
                                                    ruleset_id,
                                                    rule_id
                                                );

                                                let rule_start = std::time::Instant::now();
                                                let prev_score = execution_result.score;

                                                let rule_result = self
                                                    .executor
                                                    .execute_with_result(
                                                        rule_program,
                                                        request.to_context_input(),
                                                        execution_result.clone(),
                                                    )
                                                    .await?;

                                                let rule_time_ms =
                                                    rule_start.elapsed().as_millis() as u64;
                                                let rule_score = rule_result.score - prev_score;
                                                let triggered = rule_result
                                                    .triggered_rules
                                                    .contains(&rule_id.to_string());

                                                // Extract rule metadata for trace
                                                let rule_name = rule_program.metadata.name.as_deref();
                                                let rule_conditions = rule_program
                                                    .metadata
                                                    .custom
                                                    .get("conditions")
                                                    .cloned();
                                                let conditions_json = rule_program
                                                    .metadata
                                                    .custom
                                                    .get("conditions_json")
                                                    .cloned();
                                                let condition_group_json = rule_program
                                                    .metadata
                                                    .custom
                                                    .get("condition_group_json")
                                                    .cloned();

                                                // Record rule execution
                                                rule_executions.push(
                                                    Self::create_rule_execution_record(
                                                        &request_id,
                                                        Some(ruleset_id.as_str()),
                                                        rule_id,
                                                        rule_name,
                                                        triggered,
                                                        rule_score,
                                                        rule_time_ms,
                                                        rule_conditions,
                                                        conditions_json,
                                                        condition_group_json,
                                                    ),
                                                );

                                                execution_result.score = rule_result.score;
                                                execution_result.triggered_rules =
                                                    rule_result.triggered_rules;
                                                // Merge computed features and variables into execution context
                                                execution_result.variables.extend(rule_result.context);
                                            }
                                        }
                                    }

                                    // Execute ruleset decision logic
                                    let ruleset_result = self
                                        .executor
                                        .execute_with_result(
                                            ruleset_program,
                                            request.to_context_input(),
                                            execution_result.clone(),
                                        )
                                        .await?;

                                    // Store ruleset result in context for pipeline decision logic
                                    let mut result_map = std::collections::HashMap::new();
                                    if let Some(ref signal) = ruleset_result.signal {
                                        let signal_str = match signal {
                                            Signal::Approve => "approve",
                                            Signal::Decline => "decline",
                                            Signal::Review => "review",
                                            Signal::Hold => "hold",
                                            Signal::Pass => "pass",
                                        };
                                        result_map.insert(
                                            "signal".to_string(),
                                            Value::String(signal_str.to_string()),
                                        );
                                    }
                                    result_map.insert(
                                        "score".to_string(),
                                        Value::Number(execution_result.score as f64),
                                    );
                                    result_map.insert(
                                        "total_score".to_string(),
                                        Value::Number(execution_result.score as f64),
                                    );
                                    if !ruleset_result.explanation.is_empty() {
                                        result_map.insert(
                                            "explanation".to_string(),
                                            Value::String(ruleset_result.explanation.clone()),
                                        );
                                        result_map.insert(
                                            "reason".to_string(),
                                            Value::String(ruleset_result.explanation.clone()),
                                        );
                                    }

                                    // Store conclusion_json from program metadata for trace building
                                    if let Some(conclusion_json) = ruleset_program.metadata.custom.get("conclusion_json") {
                                        result_map.insert(
                                            "conclusion_json".to_string(),
                                            Value::String(conclusion_json.clone()),
                                        );
                                    }

                                    // Store result with ruleset ID for result.ruleset_id.field access
                                    execution_result
                                        .variables
                                        .insert(format!("__ruleset_result__.{}", ruleset_id), Value::Object(result_map.clone()));

                                    // Store as last result for result.field access (without ruleset_id)
                                    execution_result
                                        .variables
                                        .insert("__last_ruleset_result__".to_string(), Value::Object(result_map));

                                    if ruleset_result.signal.is_some() {
                                        combined_result.signal = ruleset_result.signal;
                                    }
                                    combined_result.explanation = ruleset_result.explanation;
                                    combined_result.score = execution_result.score;
                                    combined_result.triggered_rules =
                                        execution_result.triggered_rules.clone();
                                }
                            }
                        }

                        // Update state from pipeline execution
                        if result.signal.is_some() {
                            combined_result.signal = result.signal;
                        }

                        // First match wins - stop evaluating registry
                        break;
                    } else {
                        tracing::warn!(
                            "Registry entry {} references unknown pipeline: {}",
                            idx,
                            entry.pipeline
                        );
                    }
                } else {
                    tracing::trace!("Registry entry {} did not match", idx);
                }
            }

            if !pipeline_matched {
                tracing::debug!("No registry entry matched the event");
            }
        } else {
            // PRIORITY 2: Fallback to old pipeline-based routing if no registry
            tracing::debug!("No registry configured, using legacy pipeline routing");

            // Separate programs into rules, rulesets, and pipelines
            let mut rule_programs = Vec::new();
            let mut ruleset_programs = Vec::new();
            let mut pipeline_programs = Vec::new();

            for program in &self.programs {
                match program.metadata.source_type.as_str() {
                    "rule" => rule_programs.push(program),
                    "ruleset" => ruleset_programs.push(program),
                    "pipeline" => pipeline_programs.push(program),
                    _ => {}
                }
            }

            // IMPORTANT: Execute pipelines FIRST to set up context (e.g., external API calls)
            // Rules may depend on context variables set by pipelines
            // Only execute the first pipeline whose event_type matches (pre-filter by metadata)
            if !pipeline_programs.is_empty() {
                // Extract request event_type
                let req_event_type = request.event_data.get("event_type").and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                });

                // Find the first matching pipeline by event_type hint (if present)
                let mut selected_pipeline: Option<&Program> = None;
                for pipeline_program in &pipeline_programs {
                    if let Some(evt) = pipeline_program.metadata.custom.get("event_type") {
                        if let Some(ref req_evt) = req_event_type {
                            if evt != req_evt {
                                continue; // event_type 不匹配，跳过
                            }
                        }
                    }
                    selected_pipeline = Some(*pipeline_program);
                    break;
                }

                if let Some(pipeline_program) = selected_pipeline {
                    // Record the matched pipeline ID
                    matched_pipeline_id = Some(pipeline_program.metadata.source_id.clone());

                    // Log pipeline execution at INFO level
                    tracing::info!(
                        "🚀 Executing pipeline: {} (request_id={})",
                        pipeline_program.metadata.source_id,
                        request_id
                    );

                    // 记录执行前的状态，用于判断是否匹配
                    let before_score = execution_result.score;
                    let before_triggers_len = execution_result.triggered_rules.len();

                    // Execute the pipeline
                    let result = self
                        .executor
                        .execute_with_result(
                            pipeline_program,
                            request.to_context_input(),
                            execution_result.clone(),
                        )
                        .await?;

                    // 判断该 pipeline 是否匹配了当前事件（when 条件命中）
                    let matched = result.score != before_score
                        || result.triggered_rules.len() != before_triggers_len
                        || result.signal.is_some()
                        || !result.context.is_empty();

                    if matched {
                        pipeline_matched = true;
                    }

                    // Update execution_result with pipeline context (important for subsequent rules)
                    execution_result.variables = result.context.clone();

                    // Preserve branch execution info before rules overwrite context
                    if let Some(Value::Number(branch_idx)) =
                        result.context.get("__executed_branch_index__")
                    {
                        executed_branch_index = Some(*branch_idx as usize);
                        if let Some(Value::String(branch_cond)) =
                            result.context.get("__executed_branch_condition__")
                        {
                            executed_branch_condition = Some(branch_cond.clone());
                        }
                    }

                    // Get list of rulesets to execute - prefer array, fall back to single
                    let rulesets_to_execute: Vec<String> =
                        if let Some(Value::Array(arr)) = result.context.get("__rulesets_to_execute__")
                        {
                            arr.iter()
                                .filter_map(|v| {
                                    if let Value::String(s) = v {
                                        Some(s.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        } else if let Some(Value::String(ruleset_id)) =
                            result.context.get("__next_ruleset__")
                        {
                            vec![ruleset_id.clone()]
                        } else {
                            vec![]
                        };

                    if !rulesets_to_execute.is_empty() {
                        tracing::debug!(
                            "Pipeline routing to {} rulesets: {:?}",
                            rulesets_to_execute.len(),
                            rulesets_to_execute
                        );
                        pipeline_matched = true;

                        // Execute ALL rulesets in order
                        for ruleset_id in &rulesets_to_execute {
                            if let Some(ruleset_program) = self.ruleset_map.get(ruleset_id) {
                                // IMPORTANT: Execute rules FIRST before decision logic
                                // Get the list of rules from ruleset metadata
                                if let Some(rules_str) =
                                    ruleset_program.metadata.custom.get("rules")
                                {
                                    // Dedup rule IDs to avoid accidental double execution
                                    let mut seen = std::collections::HashSet::new();
                                    let rule_ids: Vec<&str> = rules_str
                                        .split(',')
                                        .filter(|rid| seen.insert(*rid))
                                        .collect();
                                    tracing::debug!(
                                        "Executing {} rules for ruleset {}: {:?}",
                                        rule_ids.len(),
                                        ruleset_id,
                                        rule_ids
                                    );

                                    // Execute each rule and accumulate results
                                    for rule_id in rule_ids {
                                        if let Some(rule_program) = self.rule_map.get(rule_id) {
                                            tracing::info!(
                                                "Executing rule (via ruleset {}): {}",
                                                ruleset_id,
                                                rule_id
                                            );

                                            let rule_start = std::time::Instant::now();
                                            let prev_score = execution_result.score;

                                            let rule_result = self
                                                .executor
                                                .execute_with_result(
                                                    rule_program,
                                                    request.to_context_input(),
                                                    execution_result.clone(),
                                                )
                                                .await?;

                                            let rule_time_ms =
                                                rule_start.elapsed().as_millis() as u64;
                                            let rule_score = rule_result.score - prev_score;
                                            let triggered = rule_result
                                                .triggered_rules
                                                .contains(&rule_id.to_string());

                                            // Extract rule metadata for trace
                                            let rule_name = rule_program.metadata.name.as_deref();
                                            let rule_conditions = rule_program
                                                .metadata
                                                .custom
                                                .get("conditions")
                                                .cloned();
                                            let conditions_json = rule_program
                                                .metadata
                                                .custom
                                                .get("conditions_json")
                                                .cloned();
                                            let condition_group_json = rule_program
                                                .metadata
                                                .custom
                                                .get("condition_group_json")
                                                .cloned();

                                            // Record rule execution
                                            rule_executions.push(
                                                Self::create_rule_execution_record(
                                                    &request_id,
                                                    Some(ruleset_id.as_str()),
                                                    rule_id,
                                                    rule_name,
                                                    triggered,
                                                    rule_score,
                                                    rule_time_ms,
                                                    rule_conditions,
                                                    conditions_json,
                                                    condition_group_json,
                                                ),
                                            );

                                            // Update execution_result with the returned state
                                            execution_result.score = rule_result.score;
                                            execution_result.triggered_rules =
                                                rule_result.triggered_rules;
                                            // Merge computed features and variables into execution context
                                            execution_result.variables.extend(rule_result.context);
                                        }
                                    }

                                    // Mark that pipeline routing handled rule execution
                                    pipeline_handled_rules = true;
                                }

                                // NOW execute the ruleset's decision logic with accumulated results
                                let ruleset_result = self
                                    .executor
                                    .execute_with_result(
                                        ruleset_program,
                                        request.to_context_input(),
                                        execution_result.clone(),
                                    )
                                    .await?;

                                // Store ruleset result in context for pipeline decision logic
                                let mut result_map = std::collections::HashMap::new();
                                if let Some(ref action) = ruleset_result.signal {
                                    let action_str = match action {
                                        Signal::Approve => "approve",
                                        Signal::Decline => "decline",
                                        Signal::Review => "review",
                                        Signal::Hold => "hold",
                                        Signal::Pass => "pass",
                                    };
                                    // Store as both "action" and "signal" for compatibility
                                    result_map.insert(
                                        "action".to_string(),
                                        Value::String(action_str.to_string()),
                                    );
                                    result_map.insert(
                                        "signal".to_string(),
                                        Value::String(action_str.to_string()),
                                    );
                                }
                                result_map.insert(
                                    "score".to_string(),
                                    Value::Number(execution_result.score as f64),
                                );
                                result_map.insert(
                                    "total_score".to_string(),
                                    Value::Number(execution_result.score as f64),
                                );
                                if !ruleset_result.explanation.is_empty() {
                                    result_map.insert(
                                        "explanation".to_string(),
                                        Value::String(ruleset_result.explanation.clone()),
                                    );
                                    result_map.insert(
                                        "reason".to_string(),
                                        Value::String(ruleset_result.explanation.clone()),
                                    );
                                }

                                // Store conclusion_json from program metadata for trace building
                                // Try "conclusion_json" first (new format), then "decision_logic_json" (legacy)
                                if let Some(conclusion_json) = ruleset_program.metadata.custom.get("conclusion_json") {
                                    result_map.insert(
                                        "conclusion_json".to_string(),
                                        Value::String(conclusion_json.clone()),
                                    );
                                }

                                // Store result with ruleset ID for result.ruleset_id.field access
                                execution_result
                                    .variables
                                    .insert(format!("__ruleset_result__.{}", ruleset_id), Value::Object(result_map.clone()));

                                // Store as last result for result.field access (without ruleset_id)
                                execution_result
                                    .variables
                                    .insert("__last_ruleset_result__".to_string(), Value::Object(result_map));

                                // Update combined result with ruleset decision
                                if ruleset_result.signal.is_some() {
                                    combined_result.signal = ruleset_result.signal;
                                }
                                combined_result.explanation = ruleset_result.explanation;
                                combined_result.score = execution_result.score;
                                combined_result.triggered_rules =
                                    execution_result.triggered_rules.clone();
                            }
                        }
                    }

                    // Update state from pipeline execution
                    if result.signal.is_some() {
                        combined_result.signal = result.signal;
                    }
                }
            }

            // Execute all rule programs ONLY if pipeline routing didn't handle them
            // This prevents double execution of rules
            if !pipeline_handled_rules && !pipeline_matched {
                // Deduplicate rule programs by source_id to prevent accidental repeats
                let mut seen_rules = std::collections::HashSet::new();
                for program in &rule_programs {
                    if !seen_rules.insert(program.metadata.source_id.clone()) {
                        continue;
                    }
                    tracing::info!("Executing rule (global): {}", program.metadata.source_id);

                    let rule_start = std::time::Instant::now();
                    let prev_score = execution_result.score;

                    let result = self
                        .executor
                        .execute_with_result(
                            program,
                            request.to_context_input(),
                            execution_result.clone(),
                        )
                        .await?;

                    let rule_time_ms = rule_start.elapsed().as_millis() as u64;
                    let rule_score = result.score - prev_score;
                    let rule_id = &program.metadata.source_id;
                    let triggered = result.triggered_rules.contains(rule_id);

                    // Extract rule metadata for trace
                    let rule_name = program.metadata.name.as_deref();
                    let rule_conditions = program.metadata.custom.get("conditions").cloned();
                    let conditions_json =
                        program.metadata.custom.get("conditions_json").cloned();
                    let condition_group_json =
                        program.metadata.custom.get("condition_group_json").cloned();

                    // Record rule execution (global rules have no ruleset)
                    rule_executions.push(Self::create_rule_execution_record(
                        &request_id,
                        None,
                        rule_id,
                        rule_name,
                        triggered,
                        rule_score,
                        rule_time_ms,
                        rule_conditions,
                        conditions_json,
                        condition_group_json,
                    ));

                    // Accumulate state
                    execution_result.score = result.score;
                    execution_result.triggered_rules = result.triggered_rules;
                    execution_result.signal = result.signal;
                    execution_result.variables = result.context;
                }

                // Update combined result with rule execution results
                combined_result.score = execution_result.score;
                combined_result.triggered_rules = execution_result.triggered_rules.clone();
                combined_result.context = execution_result.variables.clone();
            }

            // If no pipelines, execute rulesets sequentially (old behavior)
            if pipeline_programs.is_empty() && !ruleset_programs.is_empty() {
                for program in &ruleset_programs {
                    let result = self
                        .executor
                        .execute_with_result(
                            program,
                            request.to_context_input(),
                            execution_result.clone(),
                        )
                        .await?;

                    // Update combined result with decision from ruleset
                    if result.signal.is_some() {
                        combined_result.signal = result.signal;
                    }
                }
            }
        }

        let processing_time_ms = start.elapsed().as_millis() as u64;

        // Persist decision result asynchronously if result writer is configured
        tracing::debug!("Checking result_writer in DecisionEngine.decide()...");
        tracing::debug!(
            "  Engine has result_writer: {}",
            self.result_writer.is_some()
        );

        if let Some(ref result_writer) = self.result_writer {
            tracing::debug!("Result writer is configured, preparing to persist decision result");

            // Extract request_id and event_id from metadata
            // request_id was already generated at the beginning of decide(), so just retrieve it
            let request_id = request
                .metadata
                .get("request_id")
                .cloned()
                .expect("request_id should have been generated at the start of decide()");
            let event_id = request.metadata.get("event_id").cloned();

            tracing::debug!("Request ID: {}, Event ID: {:?}", request_id, event_id);

            // Determine pipeline_id (use first matched pipeline or default)
            let pipeline_id = if let Some(ref registry) = self.registry {
                // Find the matched pipeline from registry
                registry
                    .registry
                    .iter()
                    .find(|entry| Self::evaluate_when_block(&entry.when, &request.event_data))
                    .map(|entry| entry.pipeline.clone())
                    .unwrap_or("unknown".to_string())
            } else if !self.pipeline_map.is_empty() {
                // Use first pipeline ID
                self.pipeline_map
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or("unknown".to_string())
            } else {
                "unknown".to_string()
            };

            tracing::debug!("Pipeline ID: {}", pipeline_id);

            // Create decision record
            let decision_record = corint_runtime::DecisionRecord::from_decision_result(
                request_id.clone(),
                event_id,
                pipeline_id,
                &combined_result,
                processing_time_ms,
                rule_executions.clone(), // Clone for trace usage later
            );

            tracing::info!(
                "Queuing decision record for persistence: request_id={}, score={}, action={:?}",
                request_id,
                combined_result.score,
                combined_result.signal
            );

            // Write asynchronously (non-blocking)
            match result_writer.write_decision(decision_record) {
                Ok(()) => {
                    tracing::info!(
                        "Decision record queued successfully for request_id: {}",
                        request_id
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to queue decision record for request_id {}: {}",
                        request_id,
                        e
                    );
                }
            }
        } else {
            tracing::debug!("Result writer not configured, skipping persistence");
        }

        // Build trace if enabled
        let trace = if request.options.enable_trace {
            // Build execution trace from collected rule executions
            let mut pipeline_trace = if let Some(ref pid) = matched_pipeline_id {
                PipelineTrace::new(pid.clone())
            } else {
                PipelineTrace::new("unknown".to_string())
            };

            // Add pipeline when_conditions from metadata
            if let Some(ref pid) = matched_pipeline_id {
                if let Some(pipeline_program) = self.pipeline_map.get(pid) {
                    if let Some(when_conditions_str) =
                        pipeline_program.metadata.custom.get("when_conditions")
                    {
                        // Create a ConditionTrace for the when conditions
                        let condition_trace =
                            ConditionTrace::new(when_conditions_str.clone(), true);
                        pipeline_trace.when_conditions.push(condition_trace);
                    }
                }
            }

            // Add step traces from pipeline metadata
            if let Some(ref pid) = matched_pipeline_id {
                if let Some(pipeline_program) = self.pipeline_map.get(pid) {
                    if let Some(steps_json_str) = pipeline_program.metadata.custom.get("steps_json") {
                        let step_traces = Self::build_step_traces_from_json(
                            steps_json_str,
                            &execution_result.variables,
                        );
                        for step_trace in step_traces {
                            pipeline_trace.push_step(step_trace);
                        }
                    }
                }
            }

            // Add branch execution info from preserved variables (captured before rules overwrite context)
            if let Some(branch_idx) = executed_branch_index {
                pipeline_trace.executed_branch = Some(branch_idx);

                // Get the branch condition and add to trace
                if let Some(ref branch_condition) = executed_branch_condition {
                    let condition_trace = ConditionTrace::new(branch_condition.clone(), true);
                    pipeline_trace.branch_conditions.push(condition_trace);
                }
            }

            // Convert rule_executions to RulesetTraces
            // Group rules by ruleset_id
            let mut rulesets_map: std::collections::HashMap<String, Vec<&corint_runtime::RuleExecutionRecord>> =
                std::collections::HashMap::new();
            for rule_exec in &rule_executions {
                let ruleset_id = rule_exec.ruleset_id.clone().unwrap_or_else(|| "global".to_string());
                rulesets_map.entry(ruleset_id).or_default().push(rule_exec);
            }

            // Merge event_data with features and execution variables for trace generation
            // This ensures that both pre-provided features and computed values are available
            let mut trace_data = request.event_data.clone();

            // First, merge pre-provided features from the request
            if let Some(ref features) = request.features {
                tracing::debug!("Merging {} pre-provided features into trace_data", features.len());
                for (key, value) in features {
                    tracing::debug!("  Feature: {} = {:?}", key, value);
                    trace_data.insert(key.clone(), value.clone());
                }
            }

            // Then merge computed variables from execution (may override features)
            for (key, value) in &execution_result.variables {
                // Only merge top-level keys that don't start with "__" (system variables)
                if !key.starts_with("__") {
                    trace_data.insert(key.clone(), value.clone());
                }
            }

            tracing::debug!("Final trace_data has {} keys", trace_data.len());

            // Build ruleset traces
            for (ruleset_id, rules) in rulesets_map {
                let mut ruleset_trace = RulesetTrace::new(ruleset_id);
                let mut ruleset_score = 0;

                for rule_exec in rules {
                    let mut rule_trace = RuleTrace::new(rule_exec.rule_id.clone());
                    rule_trace.triggered = rule_exec.triggered;
                    rule_trace.score = rule_exec.score;
                    rule_trace.execution_time_ms = rule_exec.execution_time_ms;

                    // Add rule conditions from metadata - prefer condition_group_json (new format)
                    if let Some(ref condition_group_json_str) = rule_exec.condition_group_json {
                        // Use condition group JSON for all/any format
                        let condition_traces = Self::condition_group_json_to_traces(
                            condition_group_json_str,
                            rule_exec.triggered,
                            &trace_data,
                        );
                        rule_trace.conditions.extend(condition_traces);
                    } else if let Some(ref conditions_json_str) = rule_exec.conditions_json {
                        // Use structured JSON to create nested condition traces with actual values
                        let condition_traces = Self::json_to_condition_traces(
                            conditions_json_str,
                            rule_exec.triggered,
                            &trace_data,
                        );
                        rule_trace.conditions.extend(condition_traces);
                    } else if let Some(ref conditions_json) = rule_exec.rule_conditions {
                        // Fallback to legacy string format
                        if let serde_json::Value::String(conditions_str) = conditions_json {
                            let condition_trace =
                                ConditionTrace::new(conditions_str.clone(), rule_exec.triggered);
                            rule_trace.conditions.push(condition_trace);
                        }
                    }

                    if let Some(score) = rule_exec.score {
                        ruleset_score += score;
                    }
                    ruleset_trace = ruleset_trace.add_rule(rule_trace);
                }

                // Get ruleset-specific signal and conclusion from execution variables
                let result_key = format!("__ruleset_result__.{}", ruleset_trace.ruleset_id);
                if let Some(Value::Object(result_map)) = execution_result.variables.get(&result_key) {
                    let signal_str = result_map
                        .get("signal")
                        .and_then(|v| if let Value::String(s) = v { Some(s.as_str()) } else { None });

                    // Build conclusion trace from stored JSON
                    if let Some(Value::String(conclusion_json)) = result_map.get("conclusion_json") {
                        let conclusion_traces = Self::build_decision_logic_traces(
                            conclusion_json,
                            signal_str,
                            ruleset_score,
                            &request.event_data,
                        );
                        ruleset_trace.conclusion = conclusion_traces;
                    }
                } else if let Some(ref action) = combined_result.signal {
                    // Fallback to combined result if ruleset-specific result not found
                    let action_str = match action {
                        Signal::Approve => "approve",
                        Signal::Decline => "decline",
                        Signal::Review => "review",
                        Signal::Hold => "hold",
                        Signal::Pass => "pass",
                    };
                    // Build conclusion trace from combined result (if conclusion_json is available)
                    // Note: This is a fallback case, conclusion_json might not be available
                }

                pipeline_trace = pipeline_trace.add_ruleset(ruleset_trace);
            }

            Some(
                ExecutionTrace::new()
                    .with_pipeline(pipeline_trace)
                    .with_time(processing_time_ms),
            )
        } else {
            None
        };

        Ok(DecisionResponse {
            request_id,
            pipeline_id: matched_pipeline_id,
            result: combined_result,
            processing_time_ms,
            metadata: request.metadata,
            trace,
        })
    }

    /// Get metrics collector
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        self.metrics.clone()
    }

    /// Build decision_logic traces from JSON
    fn build_decision_logic_traces(
        decision_logic_json: &str,
        matched_action: Option<&str>,
        total_score: i32,
        _event_data: &HashMap<String, Value>,
    ) -> Vec<ConclusionTrace> {
        let mut traces = Vec::new();

        // Parse the decision_logic JSON
        let decision_rules: Vec<serde_json::Value> = match serde_json::from_str(decision_logic_json) {
            Ok(rules) => rules,
            Err(e) => {
                tracing::warn!("Failed to parse decision_logic_json: {}", e);
                return traces;
            }
        };

        let mut matched_found = false;

        for rule in decision_rules {
            let is_default = rule.get("default").and_then(|v| v.as_bool()).unwrap_or(false);
            let condition = rule.get("condition").and_then(|v| v.as_str()).map(|s| s.to_string());
            // Read signal from JSON (compiled format uses "signal" field with uppercase values)
            let signal_upper = rule.get("signal").and_then(|v| v.as_str()).map(|s| s.to_string());
            let reason = rule.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());

            // Normalize signal to lowercase for comparison
            let signal_lower = signal_upper.as_ref().map(|s| s.to_lowercase());

            // Build condition string for display
            let condition_str = if is_default {
                "default".to_string()
            } else {
                condition.clone().unwrap_or_else(|| "unknown".to_string())
            };

            // Determine if this rule matched
            // A rule is matched if:
            // 1. matched_action matches this rule's signal AND we haven't found a match yet
            // 2. This is a default rule and no previous rule matched
            let matched = if !matched_found {
                if let Some(ref rule_signal) = signal_lower {
                    if matched_action == Some(rule_signal.as_str()) {
                        // This could be the matched rule - check condition if present
                        if is_default {
                            // Default rule matches if we reach it
                            true
                        } else if let Some(ref _cond) = condition {
                            // Try to evaluate the condition (simplified - just check if signal matches)
                            // In practice, we trust the signal match since the VM already evaluated it
                            true
                        } else {
                            true
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if matched {
                matched_found = true;
            }

            let mut trace = ConclusionTrace::new(condition_str, matched);
            // Use signal_lower (normalized) for trace
            trace.signal = signal_lower;
            trace.reason = reason;
            // Add total_score for matched conclusion rules
            if matched {
                trace.total_score = Some(total_score);
            }

            traces.push(trace);

            // If this rule matched and has terminate, stop processing
            if matched {
                let terminate = rule.get("terminate").and_then(|v| v.as_bool()).unwrap_or(false);
                if terminate {
                    break;
                }
            }
        }

        traces
    }

    /// Build step traces from the steps_json metadata and executed steps list
    fn build_step_traces_from_json(
        steps_json_str: &str,
        execution_variables: &HashMap<String, Value>,
    ) -> Vec<StepTrace> {
        let mut step_traces = Vec::new();

        // Parse the steps JSON (step definitions from compilation)
        let steps: Vec<serde_json::Value> = match serde_json::from_str(steps_json_str) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to parse steps_json: {}", e);
                return step_traces;
            }
        };

        // Parse the executed steps from runtime (actual execution path)
        let executed_steps: Vec<serde_json::Value> = execution_variables
            .get("__executed_steps__")
            .and_then(|v| {
                if let Value::Array(arr) = v {
                    Some(
                        arr.iter()
                            .filter_map(|item| {
                                if let Value::String(s) = item {
                                    serde_json::from_str(s).ok()
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // Build a map of step_id -> execution info for quick lookup
        let executed_map: HashMap<String, &serde_json::Value> = executed_steps
            .iter()
            .filter_map(|exec| {
                exec.get("step_id")
                    .and_then(|v| v.as_str())
                    .map(|id| (id.to_string(), exec))
            })
            .collect();

        for step_json in steps {
            let step_id = step_json
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let step_type = step_json
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let step_name = step_json
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut step_trace = StepTrace::new(step_id.clone(), step_type.clone());

            if let Some(name) = step_name {
                step_trace = step_trace.with_name(name);
            }

            // Add ruleset ID if present
            if let Some(ruleset_id) = step_json.get("ruleset").and_then(|v| v.as_str()) {
                step_trace = step_trace.with_ruleset(ruleset_id.to_string());
            }

            // Check if this step was actually executed and get execution details
            if let Some(exec_info) = executed_map.get(&step_id) {
                step_trace = step_trace.mark_executed();

                // Get next_step_id from execution info
                if let Some(next_step) = exec_info.get("next_step_id").and_then(|v| v.as_str()) {
                    step_trace = step_trace.with_next_step(next_step.to_string());
                }

                // Get route info for router steps - add condition trace for the matched route
                if let Some(route_idx) = exec_info.get("route_index").and_then(|v| v.as_u64()) {
                    // Get the route condition from step definition
                    let route_condition = step_json
                        .get("routes")
                        .and_then(|v| v.as_array())
                        .and_then(|routes| routes.get(route_idx as usize))
                        .and_then(|route| route.get("when"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    // Add condition trace for the matched route
                    if let Some(cond) = route_condition {
                        let condition_trace = ConditionTrace::new(cond, true);
                        step_trace = step_trace.add_condition(condition_trace);
                    }
                }

                // Check if default route was taken
                if exec_info
                    .get("is_default_route")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    step_trace = step_trace.with_default_route();
                }
            } else {
                // Step was not executed - use step definition's next for reference
                if let Some(next) = step_json.get("next").and_then(|v| v.as_str()) {
                    step_trace = step_trace.with_next_step(next.to_string());
                }
            }

            step_traces.push(step_trace);
        }

        step_traces
    }

    /// Create a rule execution record
    fn create_rule_execution_record(
        request_id: &str,
        ruleset_id: Option<&str>,
        rule_id: &str,
        rule_name: Option<&str>,
        triggered: bool,
        score: i32,
        execution_time_ms: u64,
        rule_conditions: Option<String>,
        conditions_json: Option<String>,
        condition_group_json: Option<String>,
    ) -> corint_runtime::RuleExecutionRecord {
        // Convert conditions string to JSON value for storage
        let rule_conditions_json = rule_conditions.map(|s| serde_json::Value::String(s));

        corint_runtime::RuleExecutionRecord {
            request_id: request_id.to_string(),
            ruleset_id: ruleset_id.map(|s| s.to_string()),
            rule_id: rule_id.to_string(),
            rule_name: rule_name.map(|s| s.to_string()),
            triggered,
            score: if triggered { Some(score) } else { None },
            execution_time_ms: Some(execution_time_ms),
            feature_values: None,
            rule_conditions: rule_conditions_json,
            conditions_json,
            condition_group_json,
        }
    }

    /// Get configuration
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_request() {
        let mut event_data = HashMap::new();
        event_data.insert("user_id".to_string(), Value::String("123".to_string()));

        let request = DecisionRequest::new(event_data.clone())
            .with_metadata("request_id".to_string(), "req-123".to_string());

        assert_eq!(request.event_data.len(), 1);
        assert_eq!(request.metadata.len(), 1);
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let config = EngineConfig::new();

        let engine = DecisionEngine::new(config).await;
        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn test_ruleset_execution() {
        use crate::builder::DecisionEngineBuilder;
        use corint_core::ast::Signal;

        // Create a temporary YAML file with pipeline
        let yaml_content = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  when:
    event.type: test
  steps:
    - include:
        ruleset: test_execution

---

ruleset:
  id: test_execution
  name: Test Execution
  rules: []
  conclusion:
    - when: amount > 100
      signal: review
    - default: true
      signal: approve
"#;
        let temp_file = "/tmp/test_ruleset_exec.yaml";
        std::fs::write(temp_file, yaml_content).unwrap();

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(temp_file)
            .build()
            .await
            .unwrap();

        let mut event_data = HashMap::new();
        event_data.insert("type".to_string(), Value::String("test".to_string()));
        event_data.insert("amount".to_string(), Value::Number(150.0));

        let request = DecisionRequest::new(event_data);
        let response = engine.decide(request).await.unwrap();

        println!("Test ruleset execution:");
        println!("  Action: {:?}", response.result.signal);
        println!("  Score: {}", response.result.score);

        // Should be Review because 150 > 100
        assert!(response.result.signal.is_some());
        assert!(matches!(response.result.signal, Some(Signal::Review)));
    }

    #[tokio::test]
    async fn test_fraud_detection_ruleset() {
        use crate::builder::DecisionEngineBuilder;
        use corint_core::ast::Signal;

        // Create a temporary YAML file matching fraud_detection.yaml
        let yaml_content = r#"
pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline
  when:
    event.type: transaction
  steps:
    - include:
        ruleset: fraud_detection

---

ruleset:
  id: fraud_detection
  name: Fraud Detection Ruleset
  description: Simple fraud detection based on transaction amount
  rules: []
  conclusion:
    - when: transaction_amount > 10000
      signal: decline
      reason: Extremely high value transaction
      terminate: true
    - when: transaction_amount > 1000
      signal: review
      reason: High value transaction
    - when: transaction_amount > 100
      signal: review
      reason: Elevated transaction amount
    - default: true
      signal: approve
"#;
        let temp_file = "/tmp/test_fraud_detection.yaml";
        std::fs::write(temp_file, yaml_content).unwrap();

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(temp_file)
            .build()
            .await
            .unwrap();

        // Test Case 1: Normal transaction (50.0) - should approve
        let mut event_data = HashMap::new();
        event_data.insert("type".to_string(), Value::String("transaction".to_string()));
        event_data.insert("transaction_amount".to_string(), Value::Number(50.0));
        let request = DecisionRequest::new(event_data.clone());
        let response = engine.decide(request).await.unwrap();
        println!("Test Case 1 (50.0):");
        println!("  Action: {:?}", response.result.signal);
        println!("  Score: {}", response.result.score);
        println!("  Triggered Rules: {:?}", response.result.triggered_rules);
        assert!(
            matches!(response.result.signal, Some(Signal::Approve)),
            "Expected Some(Approve) but got {:?}",
            response.result.signal
        );

        // Test Case 2: High value (5000.0) - should review
        event_data.insert("transaction_amount".to_string(), Value::Number(5000.0));
        let request = DecisionRequest::new(event_data.clone());
        let response = engine.decide(request).await.unwrap();
        println!("Test Case 2 (5000.0): Action: {:?}", response.result.signal);
        assert!(matches!(response.result.signal, Some(Signal::Review)));

        // Test Case 3: Very high value (15000.0) - should decline
        event_data.insert("transaction_amount".to_string(), Value::Number(15000.0));
        let request = DecisionRequest::new(event_data);
        let response = engine.decide(request).await.unwrap();
        println!(
            "Test Case 3 (15000.0): Action: {:?}",
            response.result.signal
        );
        assert!(matches!(response.result.signal, Some(Signal::Decline)));
    }

    #[tokio::test]
    async fn test_decide_request_id_generation() {
        use crate::builder::DecisionEngineBuilder;

        let yaml_content = r#"
pipeline:
  id: simple_pipeline
  name: Simple Pipeline
  when:
    event.type: test
  steps:
    - include:
        ruleset: simple_ruleset

---

ruleset:
  id: simple_ruleset
  name: Simple Ruleset
  rules: []
  conclusion:
    - default: true
      signal: approve
"#;
        let temp_file = "/tmp/test_request_id.yaml";
        std::fs::write(temp_file, yaml_content).unwrap();

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(temp_file)
            .build()
            .await
            .unwrap();

        // Test Case 1: Auto-generated request ID
        let mut event_data = HashMap::new();
        event_data.insert("type".to_string(), Value::String("test".to_string()));

        let request = DecisionRequest::new(event_data.clone());
        let response = engine.decide(request).await.unwrap();

        println!("Auto-generated request_id: {}", response.request_id);
        assert!(response.request_id.starts_with("req_"));
        assert!(response.request_id.len() > 10);

        // Test Case 2: Custom request ID
        let request = DecisionRequest::new(event_data)
            .with_metadata("request_id".to_string(), "custom_req_123".to_string());
        let response = engine.decide(request).await.unwrap();

        println!("Custom request_id: {}", response.request_id);
        assert_eq!(response.request_id, "custom_req_123");
    }

    #[tokio::test]
    async fn test_decide_metadata_handling() {
        use crate::builder::DecisionEngineBuilder;

        let yaml_content = r#"
pipeline:
  id: metadata_pipeline
  name: Metadata Pipeline
  when:
    event.type: test
  steps:
    - include:
        ruleset: metadata_ruleset

---

ruleset:
  id: metadata_ruleset
  name: Metadata Ruleset
  rules: []
  conclusion:
    - default: true
      signal: approve
"#;
        let temp_file = "/tmp/test_metadata.yaml";
        std::fs::write(temp_file, yaml_content).unwrap();

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(temp_file)
            .build()
            .await
            .unwrap();

        let mut event_data = HashMap::new();
        event_data.insert("type".to_string(), Value::String("test".to_string()));

        let request = DecisionRequest::new(event_data)
            .with_metadata("event_id".to_string(), "evt_123".to_string())
            .with_metadata("source".to_string(), "mobile_app".to_string())
            .with_metadata("user_agent".to_string(), "iOS/14.5".to_string());

        let response = engine.decide(request).await.unwrap();

        // Verify metadata is preserved in response
        assert_eq!(
            response.metadata.get("event_id"),
            Some(&"evt_123".to_string())
        );
        assert_eq!(
            response.metadata.get("source"),
            Some(&"mobile_app".to_string())
        );
        assert_eq!(
            response.metadata.get("user_agent"),
            Some(&"iOS/14.5".to_string())
        );
        // Note: request_id may also be added to metadata, so length could be 3 or 4
        assert!(response.metadata.len() >= 3);

        println!("Metadata preserved: {:?}", response.metadata);
    }

    #[tokio::test]
    async fn test_decide_processing_time() {
        use crate::builder::DecisionEngineBuilder;

        let yaml_content = r#"
pipeline:
  id: timing_pipeline
  name: Timing Pipeline
  when:
    event.type: test
  steps:
    - include:
        ruleset: timing_ruleset

---

ruleset:
  id: timing_ruleset
  name: Timing Ruleset
  rules: []
  conclusion:
    - when: value > 100
      signal: review
    - default: true
      signal: approve
"#;
        let temp_file = "/tmp/test_timing.yaml";
        std::fs::write(temp_file, yaml_content).unwrap();

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(temp_file)
            .build()
            .await
            .unwrap();

        let mut event_data = HashMap::new();
        event_data.insert("type".to_string(), Value::String("test".to_string()));
        event_data.insert("value".to_string(), Value::Number(150.0));

        let request = DecisionRequest::new(event_data);
        let response = engine.decide(request).await.unwrap();

        // Verify processing time is recorded and reasonable
        // Note: May be 0 for very fast operations, which is acceptable
        assert!(response.processing_time_ms < 1000); // Should complete in less than 1 second

        println!("Processing time: {}ms", response.processing_time_ms);
    }

    #[tokio::test]
    async fn test_decide_with_missing_fields() {
        use crate::builder::DecisionEngineBuilder;
        use corint_core::ast::Signal;

        let yaml_content = r#"
pipeline:
  id: missing_field_pipeline
  name: Missing Field Pipeline
  when:
    event.type: test
  steps:
    - include:
        ruleset: missing_field_ruleset

---

ruleset:
  id: missing_field_ruleset
  name: Missing Field Ruleset
  rules: []
  conclusion:
    - when: optional_field > 100
      signal: review
    - default: true
      signal: approve
"#;
        let temp_file = "/tmp/test_missing_field.yaml";
        std::fs::write(temp_file, yaml_content).unwrap();

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(temp_file)
            .build()
            .await
            .unwrap();

        // Test with missing optional_field - should use default action
        let mut event_data = HashMap::new();
        event_data.insert("type".to_string(), Value::String("test".to_string()));
        // Note: optional_field is not provided

        let request = DecisionRequest::new(event_data);
        let response = engine.decide(request).await.unwrap();

        println!("Missing field test:");
        println!("  Action: {:?}", response.result.signal);

        // Should approve because condition fails (Null > 100 is false)
        assert!(matches!(response.result.signal, Some(Signal::Approve)));
    }

    #[tokio::test]
    async fn test_decide_with_content_api() {
        use crate::builder::DecisionEngineBuilder;
        use corint_core::ast::Signal;

        // Simulate loading from repository/API
        let rule_content = r#"
pipeline:
  id: content_pipeline
  name: Content Pipeline
  when:
    event.type: api_test
  steps:
    - include:
        ruleset: content_ruleset

---

ruleset:
  id: content_ruleset
  name: Content Ruleset
  rules: []
  conclusion:
    - when: risk_score > 50
      signal: decline
    - default: true
      signal: approve
"#;

        let engine = DecisionEngineBuilder::new()
            .add_rule_content("content_pipeline", rule_content)
            .build()
            .await
            .unwrap();

        let mut event_data = HashMap::new();
        event_data.insert("type".to_string(), Value::String("api_test".to_string()));
        event_data.insert("risk_score".to_string(), Value::Number(75.0));

        let request = DecisionRequest::new(event_data);
        let response = engine.decide(request).await.unwrap();

        println!("Content API test:");
        println!("  Action: {:?}", response.result.signal);

        assert!(matches!(response.result.signal, Some(Signal::Decline)));
    }

    #[tokio::test]
    async fn test_builder_config_options() {
        use crate::builder::DecisionEngineBuilder;

        let yaml_content = r#"
pipeline:
  id: config_test_pipeline
  name: Config Test Pipeline
  when:
    event.type: test
  steps:
    - include:
        ruleset: config_test_ruleset

---

ruleset:
  id: config_test_ruleset
  name: Config Test Ruleset
  rules: []
  conclusion:
    - default: true
      signal: approve
"#;
        let temp_file = "/tmp/test_config.yaml";
        std::fs::write(temp_file, yaml_content).unwrap();

        // Test with various configuration options
        let engine = DecisionEngineBuilder::new()
            .add_rule_file(temp_file)
            .enable_metrics(true)
            .enable_tracing(false)
            .enable_semantic_analysis(true)
            .enable_constant_folding(true)
            .enable_dead_code_elimination(true)
            .build()
            .await
            .unwrap();

        // Verify configuration is applied
        let config = engine.config();
        assert!(config.enable_metrics);
        assert!(!config.enable_tracing);
        assert!(config.compiler_options.enable_semantic_analysis);
        assert!(config.compiler_options.enable_constant_folding);
        assert!(config.compiler_options.enable_dead_code_elimination);

        // Test execution still works
        let mut event_data = HashMap::new();
        event_data.insert("type".to_string(), Value::String("test".to_string()));

        let request = DecisionRequest::new(event_data);
        let response = engine.decide(request).await.unwrap();

        assert!(response.request_id.starts_with("req_"));
        println!(
            "Config test passed with request_id: {}",
            response.request_id
        );
    }

    #[tokio::test]
    async fn test_rules_in_ruleset() {
        use crate::builder::DecisionEngineBuilder;
        use corint_core::ast::Signal;

        // Test that rules defined in the same file are correctly loaded and executed
        let yaml_content = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  when:
    event.type: test
  steps:
    - include:
        ruleset: test_ruleset

---

rule:
  id: high_risk
  name: High Risk Rule
  when:
    conditions:
      - event.risk_level > 80
  score: 100

---

ruleset:
  id: test_ruleset
  rules:
    - high_risk
  conclusion:
    - when: total_score >= 100
      signal: decline
    - default: true
      signal: approve
"#;
        let temp_file = "/tmp/test_rules_in_ruleset.yaml";
        std::fs::write(temp_file, yaml_content).unwrap();

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(temp_file)
            .build()
            .await
            .unwrap();

        let mut event_data = HashMap::new();
        event_data.insert("type".to_string(), Value::String("test".to_string()));
        event_data.insert("risk_level".to_string(), Value::Number(90.0));

        let request = DecisionRequest::new(event_data).with_trace();
        let response = engine.decide(request).await.unwrap();

        println!("Score: {}", response.result.score);
        println!("Triggered rules: {:?}", response.result.triggered_rules);
        println!("Action: {:?}", response.result.signal);

        assert_eq!(response.result.score, 100, "Rule should add 100 points");
        assert!(
            response.result.triggered_rules.contains(&"high_risk".to_string()),
            "high_risk rule should be triggered"
        );
        assert!(matches!(response.result.signal, Some(Signal::Decline)));
    }
}
