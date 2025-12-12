//! DecisionEngine - Main API for executing decisions

use crate::config::EngineConfig;
use crate::error::{Result, SdkError};
use corint_compiler::{Compiler, CompilerOptions as CompilerOpts};
use corint_core::ir::Program;
use corint_core::Value;
use corint_core::ast::{PipelineRegistry, WhenBlock, Expression, Operator};
use corint_parser::{RuleParser, RulesetParser, PipelineParser, RegistryParser};
use corint_runtime::{DecisionResult, PipelineExecutor, MetricsCollector, ExternalApiClient, ApiConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Decision request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRequest {
    /// Event data
    pub event_data: HashMap<String, Value>,

    /// Request metadata
    pub metadata: HashMap<String, String>,
}

impl DecisionRequest {
    /// Create a new decision request
    pub fn new(event_data: HashMap<String, Value>) -> Self {
        Self {
            event_data,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
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
}

impl DecisionEngine {
    /// Generate a unique request ID
    /// Format: req_YYYYMMDDHHmmss_xxx
    /// Example: req_20231209143052_a3f
    /// 
    /// Uses chrono to correctly handle leap years and variable month lengths
    fn generate_request_id() -> String {
        use chrono::Utc;
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        // Get current UTC time and format it directly - this correctly handles
        // leap years, variable month lengths, and all date edge cases
        let now = Utc::now();
        let datetime_str = now.format("%Y%m%d%H%M%S").to_string();

        // Generate random suffix using atomic counter
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let random = ((now.timestamp() as u32) ^ counter) & 0xFFFFFF; // 6 hex digits (24 bits)

        format!("req_{}_{:06x}", datetime_str, random)
    }

    /// Create a new decision engine from configuration
    pub async fn new(config: EngineConfig) -> Result<Self> {
        Self::new_with_feature_executor(config, None).await
    }

    /// Create a new decision engine with optional feature executor
    pub async fn new_with_feature_executor(
        config: EngineConfig,
        feature_executor: Option<Arc<corint_runtime::feature::FeatureExecutor>>,
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
                    tracing::info!("✓ Loaded pipeline registry from content: {} entries", reg.registry.len());
                    Some(reg)
                }
                Err(e) => {
                    tracing::warn!("Failed to parse registry content: {}. Continuing without registry.", e);
                    None
                }
            }
        } else if let Some(registry_file) = &config.registry_file {
            // Fall back to loading from file
            match Self::load_registry(registry_file).await {
                Ok(reg) => {
                    tracing::info!("✓ Loaded pipeline registry from file: {} entries", reg.registry.len());
                    Some(reg)
                }
                Err(e) => {
                    tracing::warn!("Failed to load registry file: {}. Continuing without registry.", e);
                    None
                }
            }
        } else {
            None
        };

        // Load external API configurations
        let mut api_client = ExternalApiClient::new();

        // Load API configs from examples/configs/apis directory
        let api_config_dir = Path::new("examples/configs/apis");
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
        let mut pipeline_executor = PipelineExecutor::new().with_external_api_client(Arc::new(api_client));
        
        // Set feature executor if provided (for lazy feature calculation)
        if let Some(feature_executor) = feature_executor {
            pipeline_executor = pipeline_executor.with_feature_executor(feature_executor);
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
        // Check event_type if specified
        // Note: event_type field in WhenBlock corresponds to event.type in YAML,
        // which is stored as "type" key in event_data HashMap
        if let Some(ref expected_type) = when.event_type {
            if let Some(actual_type) = event_data.get("type") {
                if let Value::String(actual) = actual_type {
                    if actual != expected_type {
                        return false; // Event type mismatch
                    }
                } else {
                    return false; // Event type is not a string
                }
            } else {
                return false; // No type field in event data
            }
        }

        // Evaluate all conditions (AND logic)
        for condition in &when.conditions {
            if !Self::evaluate_expression(condition, event_data) {
                return false; // Condition failed
            }
        }

        true // All checks passed
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
            Operator::Lt => Self::compare_values(&left_val, &right_val) == Some(std::cmp::Ordering::Less),
            Operator::Gt => Self::compare_values(&left_val, &right_val) == Some(std::cmp::Ordering::Greater),
            Operator::Le => matches!(Self::compare_values(&left_val, &right_val), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)),
            Operator::Ge => matches!(Self::compare_values(&left_val, &right_val), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)),
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
        }
    }

    /// Get field value from nested path
    fn get_field_value(event_data: &HashMap<String, Value>, path: &[String]) -> Option<Value> {
        if path.is_empty() {
            return None;
        }

        let mut current = event_data.get(&path[0])?;

        for key in &path[1..] {
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

    /// Load and compile rules from a file
    async fn load_and_compile_rules(
        path: &Path,
        compiler: &mut Compiler,
    ) -> Result<Vec<Program>> {
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
        for (_doc_idx, doc) in documents.iter().enumerate() {
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
                        pipeline.id.as_ref().unwrap_or(&"<unnamed>".to_string()),
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
                    document.definition.id.as_ref().unwrap_or(&"<unnamed>".to_string()),
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
            let resolved = compiler.import_resolver_mut().resolve_imports(&document)
                .map_err(|e| SdkError::CompileError(e))?;

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
        for (_doc_idx, doc) in documents.iter().enumerate() {
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
                        pipeline.id.as_ref().unwrap_or(&"<unnamed>".to_string()),
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
            request.metadata.insert("request_id".to_string(), new_id.clone());
            tracing::debug!("Generated new request_id: {}", new_id);
            new_id
        };

        // Create initial execution result
        let mut execution_result = ExecutionResult::new();

        let mut combined_result = DecisionResult {
            action: None,
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

        // PRIORITY 1: Use Registry-based routing if available
        if let Some(ref registry) = self.registry {
            tracing::debug!("Using registry-based routing with {} entries", registry.registry.len());

            // Find the first matching registry entry (top-to-bottom order)
            for (idx, entry) in registry.registry.iter().enumerate() {
                tracing::debug!("Checking registry entry {}: pipeline={}", idx, entry.pipeline);

                // Evaluate when block against event data
                if Self::evaluate_when_block(&entry.when, &request.event_data) {
                    tracing::info!("✓ Registry matched entry {}: pipeline={}", idx, entry.pipeline);

                    // Record the matched pipeline ID
                    matched_pipeline_id = Some(entry.pipeline.clone());

                    // Get the pipeline program
                    if let Some(pipeline_program) = self.pipeline_map.get(&entry.pipeline) {
                        // Log pipeline execution at INFO level
                        tracing::info!("🚀 Executing pipeline: {} (request_id={})", entry.pipeline, request_id);

                        // Execute the matched pipeline
                        let result = self.executor.execute_with_result(
                            pipeline_program,
                            request.event_data.clone(),
                            execution_result.clone()
                        ).await?;

                        pipeline_matched = true;

                        // Update execution_result with pipeline context
                        execution_result.variables = result.context.clone();

                        // Check if pipeline set a __next_ruleset__ variable
                        if let Some(Value::String(ruleset_id)) = result.context.get("__next_ruleset__") {
                            tracing::debug!("Pipeline routing to ruleset: {}", ruleset_id);

                            // Execute the specified ruleset
                            if let Some(ruleset_program) = self.ruleset_map.get(ruleset_id) {
                                // Execute rules first
                                if let Some(rules_str) = ruleset_program.metadata.custom.get("rules") {
                                    let mut seen = std::collections::HashSet::new();
                                    let rule_ids: Vec<&str> = rules_str
                                        .split(',')
                                        .filter(|rid| seen.insert(*rid))
                                        .collect();

                                    for rule_id in rule_ids {
                                        if let Some(rule_program) = self.rule_map.get(rule_id) {
                                            tracing::info!("Executing rule (via ruleset {}): {}", ruleset_id, rule_id);

                                            let rule_start = std::time::Instant::now();
                                            let prev_score = execution_result.score;

                                            let rule_result = self.executor.execute_with_result(
                                                rule_program,
                                                request.event_data.clone(),
                                                execution_result.clone()
                                            ).await?;

                                            let rule_time_ms = rule_start.elapsed().as_millis() as u64;
                                            let rule_score = rule_result.score - prev_score;
                                            let triggered = rule_result.triggered_rules.contains(&rule_id.to_string());

                                            // Record rule execution
                                            rule_executions.push(Self::create_rule_execution_record(
                                                &request_id,
                                                rule_id,
                                                triggered,
                                                rule_score,
                                                rule_time_ms,
                                            ));

                                            execution_result.score = rule_result.score;
                                            execution_result.triggered_rules = rule_result.triggered_rules;
                                        }
                                    }
                                }

                                // Execute ruleset decision logic
                                let ruleset_result = self.executor.execute_with_result(
                                    ruleset_program,
                                    request.event_data.clone(),
                                    execution_result.clone()
                                ).await?;

                                if ruleset_result.action.is_some() {
                                    combined_result.action = ruleset_result.action;
                                }
                                combined_result.explanation = ruleset_result.explanation;
                                combined_result.score = execution_result.score;
                                combined_result.triggered_rules = execution_result.triggered_rules.clone();
                            }
                        }

                        // Update state from pipeline execution
                        if result.action.is_some() {
                            combined_result.action = result.action;
                        }

                        // First match wins - stop evaluating registry
                        break;
                    } else {
                        tracing::warn!("Registry entry {} references unknown pipeline: {}", idx, entry.pipeline);
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
            let req_event_type = request
                .event_data
                .get("event_type")
                .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None });

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
                tracing::info!("🚀 Executing pipeline: {} (request_id={})",
                    pipeline_program.metadata.source_id, request_id);

                // 记录执行前的状态，用于判断是否匹配
                let before_score = execution_result.score;
                let before_triggers_len = execution_result.triggered_rules.len();

                // Execute the pipeline
                let result = self.executor.execute_with_result(
                    pipeline_program,
                    request.event_data.clone(),
                    execution_result.clone()
                ).await?;

                // 判断该 pipeline 是否匹配了当前事件（when 条件命中）
                let matched = result.score != before_score
                    || result.triggered_rules.len() != before_triggers_len
                    || result.action.is_some()
                    || !result.context.is_empty();

                if matched {
                    pipeline_matched = true;
                }

                // Update execution_result with pipeline context (important for subsequent rules)
                execution_result.variables = result.context.clone();

                // Check if pipeline set a __next_ruleset__ variable
                tracing::debug!("Pipeline result context: {:?}", result.context.keys().collect::<Vec<_>>());
                if let Some(Value::String(ruleset_id)) = result.context.get("__next_ruleset__") {
                    tracing::debug!("Pipeline routing to ruleset: {}", ruleset_id);
                    // 有 __next_ruleset__ 视为命中
                    pipeline_matched = true;
                    // Execute the specified ruleset
                    if let Some(ruleset_program) = self.ruleset_map.get(ruleset_id) {
                        // IMPORTANT: Execute rules FIRST before decision logic
                        // Get the list of rules from ruleset metadata
                        if let Some(rules_str) = ruleset_program.metadata.custom.get("rules") {
                            // Dedup rule IDs to avoid accidental double execution
                            let mut seen = std::collections::HashSet::new();
                            let rule_ids: Vec<&str> = rules_str
                                .split(',')
                                .filter(|rid| seen.insert(*rid))
                                .collect();
                            tracing::debug!("Executing {} rules for ruleset {}: {:?}", rule_ids.len(), ruleset_id, rule_ids);

                            // Execute each rule and accumulate results
                            for rule_id in rule_ids {
                                if let Some(rule_program) = self.rule_map.get(rule_id) {
                                    tracing::info!("Executing rule (via ruleset {}): {}", ruleset_id, rule_id);

                                    let rule_start = std::time::Instant::now();
                                    let prev_score = execution_result.score;

                                    let rule_result = self.executor.execute_with_result(
                                        rule_program,
                                        request.event_data.clone(),
                                        execution_result.clone()
                                    ).await?;

                                    let rule_time_ms = rule_start.elapsed().as_millis() as u64;
                                    let rule_score = rule_result.score - prev_score;
                                    let triggered = rule_result.triggered_rules.contains(&rule_id.to_string());

                                    // Record rule execution
                                    rule_executions.push(Self::create_rule_execution_record(
                                        &request_id,
                                        rule_id,
                                        triggered,
                                        rule_score,
                                        rule_time_ms,
                                    ));

                                    // Update execution_result with the returned state
                                    // (execute_with_result already includes previous state + new additions)
                                    execution_result.score = rule_result.score;
                                    execution_result.triggered_rules = rule_result.triggered_rules;
                                }
                            }

                            // Mark that pipeline routing handled rule execution
                            pipeline_handled_rules = true;
                        }

                        // NOW execute the ruleset's decision logic with accumulated results
                        let ruleset_result = self.executor.execute_with_result(
                            ruleset_program,
                            request.event_data.clone(),
                            execution_result.clone()
                        ).await?;

                        // Update combined result with ruleset decision
                        if ruleset_result.action.is_some() {
                            combined_result.action = ruleset_result.action;
                        }
                        combined_result.explanation = ruleset_result.explanation;
                        combined_result.score = execution_result.score;
                        combined_result.triggered_rules = execution_result.triggered_rules.clone();
                    }
                }

                // Update state from pipeline execution
                if result.action.is_some() {
                    combined_result.action = result.action;
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

                    let result = self.executor.execute_with_result(
                        program,
                        request.event_data.clone(),
                        execution_result.clone()
                    ).await?;

                    let rule_time_ms = rule_start.elapsed().as_millis() as u64;
                    let rule_score = result.score - prev_score;
                    let rule_id = &program.metadata.source_id;
                    let triggered = result.triggered_rules.contains(rule_id);

                    // Record rule execution
                    rule_executions.push(Self::create_rule_execution_record(
                        &request_id,
                        rule_id,
                        triggered,
                        rule_score,
                        rule_time_ms,
                    ));

                    // Accumulate state
                    execution_result.score = result.score;
                    execution_result.triggered_rules = result.triggered_rules;
                    execution_result.action = result.action;
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
                    let result = self.executor.execute_with_result(
                        program,
                        request.event_data.clone(),
                        execution_result.clone()
                    ).await?;

                    // Update combined result with decision from ruleset
                    if result.action.is_some() {
                        combined_result.action = result.action;
                    }
                }
            }
        }

        let processing_time_ms = start.elapsed().as_millis() as u64;

        // Persist decision result asynchronously if result writer is configured
        tracing::debug!("Checking result_writer in DecisionEngine.decide()...");
        tracing::debug!("  Engine has result_writer: {}", self.result_writer.is_some());
        
        if let Some(ref result_writer) = self.result_writer {
            tracing::debug!("Result writer is configured, preparing to persist decision result");
            
            // Extract request_id and event_id from metadata
            // request_id was already generated at the beginning of decide(), so just retrieve it
            let request_id = request.metadata.get("request_id")
                .cloned()
                .expect("request_id should have been generated at the start of decide()");
            let event_id = request.metadata.get("event_id").cloned();
            
            tracing::debug!("Request ID: {}, Event ID: {:?}", request_id, event_id);
            
            // Determine pipeline_id (use first matched pipeline or default)
            let pipeline_id = if let Some(ref registry) = self.registry {
                // Find the matched pipeline from registry
                registry.registry.iter()
                    .find(|entry| Self::evaluate_when_block(&entry.when, &request.event_data))
                    .map(|entry| entry.pipeline.clone())
                    .unwrap_or_else(|| "unknown".to_string())
            } else if !self.pipeline_map.is_empty() {
                // Use first pipeline ID
                self.pipeline_map.keys().next().cloned().unwrap_or_else(|| "unknown".to_string())
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
                rule_executions, // Rule execution tracking implemented
            );
            
            tracing::info!("Queuing decision record for persistence: request_id={}, score={}, action={:?}", 
                request_id, combined_result.score, combined_result.action);
            
            // Write asynchronously (non-blocking)
            match result_writer.write_decision(decision_record) {
                Ok(()) => {
                    tracing::info!("Decision record queued successfully for request_id: {}", request_id);
                }
                Err(e) => {
                    tracing::error!("Failed to queue decision record for request_id {}: {}", request_id, e);
                }
            }
        } else {
            tracing::debug!("Result writer not configured, skipping persistence");
        }

        Ok(DecisionResponse {
            request_id,
            pipeline_id: matched_pipeline_id,
            result: combined_result,
            processing_time_ms,
            metadata: request.metadata,
        })
    }

    /// Get metrics collector
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        self.metrics.clone()
    }

    /// Create a rule execution record
    fn create_rule_execution_record(
        request_id: &str,
        rule_id: &str,
        triggered: bool,
        score: i32,
        execution_time_ms: u64,
    ) -> corint_runtime::RuleExecutionRecord {
        corint_runtime::RuleExecutionRecord {
            request_id: request_id.to_string(),
            rule_id: rule_id.to_string(),
            rule_name: None, // Could be enhanced to look up rule name from metadata
            triggered,
            score: if triggered { Some(score) } else { None },
            execution_time_ms: Some(execution_time_ms),
            feature_values: None, // Could be enhanced to extract from context
            rule_conditions: None, // Could be enhanced to include rule conditions
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
        use corint_core::ast::Action;

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
  decision_logic:
    - condition: amount > 100
      action: review
    - default: true
      action: approve
"#;
        let temp_file = "/tmp/test_ruleset_exec.yaml";
        std::fs::write(temp_file, yaml_content).unwrap();

        let engine = DecisionEngineBuilder::new()
            .add_rule_file(temp_file)
            .build()
            .await
            .unwrap();

        let mut event_data = HashMap::new();
        event_data.insert("event_type".to_string(), Value::String("test".to_string()));
        event_data.insert("amount".to_string(), Value::Number(150.0));

        let request = DecisionRequest::new(event_data);
        let response = engine.decide(request).await.unwrap();

        println!("Test ruleset execution:");
        println!("  Action: {:?}", response.result.action);
        println!("  Score: {}", response.result.score);

        // Should be Review because 150 > 100
        assert!(response.result.action.is_some());
        assert!(matches!(response.result.action, Some(Action::Review)));
    }

    #[tokio::test]
    async fn test_fraud_detection_ruleset() {
        use crate::builder::DecisionEngineBuilder;
        use corint_core::ast::Action;

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
  decision_logic:
    - condition: transaction_amount > 10000
      action: deny
      reason: Extremely high value transaction
      terminate: true
    - condition: transaction_amount > 1000
      action: review
      reason: High value transaction
    - condition: transaction_amount > 100
      action: review
      reason: Elevated transaction amount
    - default: true
      action: approve
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
        event_data.insert("event_type".to_string(), Value::String("transaction".to_string()));
        event_data.insert("transaction_amount".to_string(), Value::Number(50.0));
        let request = DecisionRequest::new(event_data.clone());
        let response = engine.decide(request).await.unwrap();
        println!("Test Case 1 (50.0):");
        println!("  Action: {:?}", response.result.action);
        println!("  Score: {}", response.result.score);
        println!("  Triggered Rules: {:?}", response.result.triggered_rules);
        assert!(matches!(response.result.action, Some(Action::Approve)),
            "Expected Some(Approve) but got {:?}", response.result.action);

        // Test Case 2: High value (5000.0) - should review
        event_data.insert("transaction_amount".to_string(), Value::Number(5000.0));
        let request = DecisionRequest::new(event_data.clone());
        let response = engine.decide(request).await.unwrap();
        println!("Test Case 2 (5000.0): Action: {:?}", response.result.action);
        assert!(matches!(response.result.action, Some(Action::Review)));

        // Test Case 3: Very high value (15000.0) - should deny
        event_data.insert("transaction_amount".to_string(), Value::Number(15000.0));
        let request = DecisionRequest::new(event_data);
        let response = engine.decide(request).await.unwrap();
        println!("Test Case 3 (15000.0): Action: {:?}", response.result.action);
        assert!(matches!(response.result.action, Some(Action::Deny)));
    }
}