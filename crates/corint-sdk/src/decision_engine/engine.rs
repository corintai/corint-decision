//! Core DecisionEngine implementation

use super::when_evaluator::WhenEvaluator;
use super::trace_builder::TraceBuilder;
use super::compiler_helper::CompilerHelper;

use super::types::{DecisionRequest, DecisionResponse};
use crate::config::EngineConfig;
use crate::error::{Result, SdkError};
use corint_compiler::{Compiler, CompilerOptions as CompilerOpts};
use corint_core::ast::{PipelineRegistry, Signal};
use corint_core::ir::Program;
use corint_core::Value;
use corint_parser::RegistryParser;
use corint_runtime::{
    ApiConfig, ConditionTrace, DecisionResult, ExecutionTrace,
    ExternalApiClient, MetricsCollector, PipelineExecutor, PipelineTrace, RuleTrace, RulesetTrace,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

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
            programs.extend(CompilerHelper::load_and_compile_rules(rule_file, &mut compiler).await?);
        }

        // Compile rule contents (from repository)
        for (id, content) in &config.rule_contents {
            programs.extend(CompilerHelper::compile_rules_from_content(id, content, &mut compiler).await?);
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
            match CompilerHelper::load_registry(registry_file).await {
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
        tracing::debug!("Checking for API configs in: {:?}", api_config_dir);
        if api_config_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(api_config_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    tracing::debug!("Found file: {:?}", path);
                    if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                        match std::fs::read_to_string(&path) {
                            Ok(content) => {
                                match serde_yaml::from_str::<ApiConfig>(&content) {
                                    Ok(api_config) => {
                                        tracing::info!("✓ Loaded API config: {}", api_config.name);
                                        api_client.register_api(api_config);
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to parse API config from {:?}: {}", path, e);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to read API config file {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        } else {
            tracing::warn!("API config directory does not exist: {:?}", api_config_dir);
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
                if WhenEvaluator::evaluate_when_block(&entry.when, &request.event_data) {
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
                                                    TraceBuilder::create_rule_execution_record(
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

                        // Execute pipeline decision logic AFTER rulesets (if present)
                        tracing::info!(
                            "📊 Pipeline '{}': decision_instructions = {}",
                            entry.pipeline,
                            if let Some(ref di) = pipeline_program.decision_instructions {
                                format!("Some({} instructions)", di.len())
                            } else {
                                "None".to_string()
                            }
                        );
                        if let Some(ref decision_instructions) = pipeline_program.decision_instructions {
                            tracing::debug!(
                                "Executing pipeline decision logic ({} instructions)",
                                decision_instructions.len()
                            );

                            // Create a mini-program with just the decision instructions
                            let decision_program = corint_core::ir::Program {
                                instructions: decision_instructions.clone(),
                                metadata: pipeline_program.metadata.clone(),
                                decision_instructions: None,
                            };

                            // Execute decision logic with current execution_result (which now has ruleset results)
                            let decision_result = self
                                .executor
                                .execute_with_result(
                                    &decision_program,
                                    request.to_context_input(),
                                    execution_result.clone(),
                                )
                                .await?;

                            tracing::debug!(
                                "Decision logic completed: signal={:?}, explanation={:?}, actions={:?}",
                                decision_result.signal,
                                decision_result.explanation,
                                decision_result.actions
                            );

                            // Update combined_result from decision execution
                            if decision_result.signal.is_some() {
                                combined_result.signal = decision_result.signal;
                            }
                            if !decision_result.explanation.is_empty() {
                                combined_result.explanation = decision_result.explanation;
                            }
                            // Always update actions from decision result (even if empty, to override previous values)
                            combined_result.actions = decision_result.actions;
                        } else {
                            // No decision logic - update state from pipeline execution
                            if result.signal.is_some() {
                                combined_result.signal = result.signal;
                            }
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
                                                TraceBuilder::create_rule_execution_record(
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

                    // Execute pipeline decision logic (legacy routing) so actions/reasons are populated
                    if pipeline_matched {
                        if let Some(ref decision_instructions) =
                            pipeline_program.decision_instructions
                        {
                            tracing::debug!(
                                "Executing pipeline decision logic (legacy) ({} instructions)",
                                decision_instructions.len()
                            );

                            // Build a mini program for the decision section
                            let decision_program = corint_core::ir::Program {
                                instructions: decision_instructions.clone(),
                                metadata: pipeline_program.metadata.clone(),
                                decision_instructions: None,
                            };

                            // Run decision logic with accumulated execution_result (scores, triggers, context)
                            let decision_result = self
                                .executor
                                .execute_with_result(
                                    &decision_program,
                                    request.to_context_input(),
                                    execution_result.clone(),
                                )
                                .await?;

                            if decision_result.signal.is_some() {
                                combined_result.signal = decision_result.signal;
                            }
                            if !decision_result.explanation.is_empty() {
                                combined_result.explanation = decision_result.explanation;
                            }
                            // Always override actions with decision output
                            combined_result.actions = decision_result.actions;
                        }
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
                    rule_executions.push(TraceBuilder::create_rule_execution_record(
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
                    .find(|entry| WhenEvaluator::evaluate_when_block(&entry.when, &request.event_data))
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
                        let step_traces = TraceBuilder::build_step_traces_from_json(
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
                        let condition_traces = TraceBuilder::condition_group_json_to_traces(
                            condition_group_json_str,
                            rule_exec.triggered,
                            &trace_data,
                        );
                        rule_trace.conditions.extend(condition_traces);
                    } else if let Some(ref conditions_json_str) = rule_exec.conditions_json {
                        // Use structured JSON to create nested condition traces with actual values
                        let condition_traces = TraceBuilder::json_to_condition_traces(
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
                        let conclusion_traces = TraceBuilder::build_decision_logic_traces(
                            conclusion_json,
                            signal_str,
                            ruleset_score,
                            &request.event_data,
                        );
                        ruleset_trace.conclusion = conclusion_traces;
                    }
                } else if let Some(ref action) = combined_result.signal {
                    // Fallback to combined result if ruleset-specific result not found
                    let _action_str = match action {
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

    /// Get configuration
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

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
            programs.extend(CompilerHelper::compile_rules_from_content(id, content_str, &mut compiler).await?);
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
}

