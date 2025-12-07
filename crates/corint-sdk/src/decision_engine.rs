//! DecisionEngine - Main API for executing decisions

use crate::config::EngineConfig;
use crate::error::{Result, SdkError};
use corint_compiler::{Compiler, CompilerOptions as CompilerOpts};
use corint_core::ir::Program;
use corint_core::Value;
use corint_parser::{RuleParser, RulesetParser, PipelineParser};
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

    /// Pipeline executor
    executor: Arc<PipelineExecutor>,

    /// Metrics collector
    metrics: Arc<MetricsCollector>,

    /// Configuration
    config: EngineConfig,
}

impl DecisionEngine {
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
            enable_dead_code_elimination: false, // TEMP: Disabled due to bug with default actions
        };

        let mut compiler = Compiler::with_options(compiler_opts);

        for rule_file in &config.rule_files {
            programs.extend(Self::load_and_compile_rules(rule_file, &mut compiler).await?);
        }

        // Build ruleset_map and rule_map for pipeline routing
        let mut ruleset_map = HashMap::new();
        let mut rule_map = HashMap::new();
        for program in &programs {
            match program.metadata.source_type.as_str() {
                "ruleset" => {
                    ruleset_map.insert(program.metadata.source_id.clone(), program.clone());
                }
                "rule" => {
                    rule_map.insert(program.metadata.source_id.clone(), program.clone());
                }
                _ => {}
            }
        }

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
            executor,
            metrics,
            config,
        })
    }

    /// Load and compile rules from a file
    async fn load_and_compile_rules(
        path: &Path,
        compiler: &mut Compiler,
    ) -> Result<Vec<Program>> {
        use corint_parser::YamlParser;

        // Read file
        let content = tokio::fs::read_to_string(path).await?;

        let mut programs = Vec::new();

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
                let prog = compiler.compile_pipeline(&pipeline)?;
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

        Ok(programs)
    }

    /// Execute a decision request
    pub async fn decide(&self, request: DecisionRequest) -> Result<DecisionResponse> {
        use corint_runtime::result::ExecutionResult;

        let start = std::time::Instant::now();

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

        // IMPORTANT: Execute pipelines FIRST to set up context (e.g., external API calls)
        // Rules may depend on context variables set by pipelines
        if !pipeline_programs.is_empty() {
            for pipeline_program in &pipeline_programs {
                // Execute the pipeline
                let result = self.executor.execute_with_result(
                    pipeline_program,
                    request.event_data.clone(),
                    execution_result.clone()
                ).await?;

                // Update execution_result with pipeline context (important for subsequent rules)
                execution_result.variables = result.context.clone();

                // Check if pipeline set a __next_ruleset__ variable
                tracing::debug!("Pipeline result context: {:?}", result.context.keys().collect::<Vec<_>>());
                if let Some(Value::String(ruleset_id)) = result.context.get("__next_ruleset__") {
                    tracing::debug!("Pipeline routing to ruleset: {}", ruleset_id);
                    // Execute the specified ruleset
                    if let Some(ruleset_program) = self.ruleset_map.get(ruleset_id) {
                        // IMPORTANT: Execute rules FIRST before decision logic
                        // Get the list of rules from ruleset metadata
                        if let Some(rules_str) = ruleset_program.metadata.custom.get("rules") {
                            let rule_ids: Vec<&str> = rules_str.split(',').collect();
                            tracing::debug!("Executing {} rules for ruleset {}: {:?}", rule_ids.len(), ruleset_id, rule_ids);

                            // Execute each rule and accumulate results
                            for rule_id in rule_ids {
                                if let Some(rule_program) = self.rule_map.get(rule_id) {
                                    let rule_result = self.executor.execute_with_result(
                                        rule_program,
                                        request.event_data.clone(),
                                        execution_result.clone()
                                    ).await?;

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
        if !pipeline_handled_rules {
            for program in &rule_programs {
                let result = self.executor.execute_with_result(
                    program,
                    request.event_data.clone(),
                    execution_result.clone()
                ).await?;

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

        let processing_time_ms = start.elapsed().as_millis() as u64;

        Ok(DecisionResponse {
            result: combined_result,
            processing_time_ms,
            metadata: request.metadata,
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

        // Create a temporary YAML file
        let yaml_content = r#"
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
