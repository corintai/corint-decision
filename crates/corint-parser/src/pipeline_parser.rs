//! Pipeline parser
//!
//! Parses YAML pipeline definitions into Pipeline AST nodes.

use crate::error::{ParseError, Result};
use crate::expression_parser::ExpressionParser;
use crate::import_parser::ImportParser;
use crate::rule_parser::RuleParser;
use crate::yaml_parser::YamlParser;
use corint_core::ast::pipeline::{
    ApiTarget, ErrorAction, ErrorHandling, PipelineStep, Route, StepDetails, StepNext,
};
use corint_core::ast::{
    Branch, FeatureDefinition, MergeStrategy, Pipeline, PromptTemplate, RdlDocument, Schema,
    SchemaProperty, Step, WhenBlock,
};
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;

// ============================================================================
// Known Field Definitions for Validation
// ============================================================================

/// Common fields for all step types
const COMMON_STEP_FIELDS: &[&str] = &["id", "name", "type", "next", "when"];

/// Fields specific to each step type (aligned with Pipeline DSL v2.0)
const FUNCTION_STEP_FIELDS: &[&str] = &["function", "params"];
const RULE_STEP_FIELDS: &[&str] = &["rule"];
const RULESET_STEP_FIELDS: &[&str] = &["ruleset"];
const PIPELINE_STEP_FIELDS: &[&str] = &["pipeline", "inline"];
const API_STEP_FIELDS: &[&str] = &["api", "any", "all", "params", "endpoint", "output", "timeout", "on_error", "min_success"];
const SERVICE_STEP_FIELDS: &[&str] = &["service", "query", "params"];
const ROUTER_STEP_FIELDS: &[&str] = &["routes", "default"];
const TRIGGER_STEP_FIELDS: &[&str] = &["target", "params"];
// Legacy step types (for backward compatibility only)
const EXTRACT_STEP_FIELDS: &[&str] = &["features"];
const REASON_STEP_FIELDS: &[&str] = &["provider", "model", "prompt", "output_schema"];

/// Helper function to combine common fields with type-specific fields
fn get_valid_fields_for_step_type(step_type: &str) -> Vec<&'static str> {
    let mut fields: Vec<&str> = COMMON_STEP_FIELDS.to_vec();

    let type_specific = match step_type {
        // Pipeline DSL v2.0 step types
        "router" => ROUTER_STEP_FIELDS,
        "function" => FUNCTION_STEP_FIELDS,
        "rule" => RULE_STEP_FIELDS,
        "ruleset" => RULESET_STEP_FIELDS,
        "pipeline" => PIPELINE_STEP_FIELDS,
        "service" => SERVICE_STEP_FIELDS,
        "api" => API_STEP_FIELDS,
        "trigger" => TRIGGER_STEP_FIELDS,
        // Legacy step types (backward compatibility)
        "extract" => EXTRACT_STEP_FIELDS,
        "reason" => REASON_STEP_FIELDS,
        _ => &[],
    };

    fields.extend_from_slice(type_specific);
    fields
}

/// Pipeline parser
pub struct PipelineParser;

impl PipelineParser {
    /// Parse a pipeline from YAML string (legacy format, no imports)
    ///
    /// This maintains backward compatibility with existing code.
    pub fn parse(yaml_str: &str) -> Result<Pipeline> {
        let yaml = YamlParser::parse(yaml_str)?;
        Self::parse_from_yaml(&yaml)
    }

    /// Parse a pipeline with optional imports from YAML string (new format)
    ///
    /// Supports both formats:
    /// 1. Legacy single-document format (backward compatible)
    /// 2. New multi-document format with imports
    ///
    /// Returns an RdlDocument<Pipeline> containing both the pipeline and its imports (if any)
    pub fn parse_with_imports(yaml_str: &str) -> Result<RdlDocument<Pipeline>> {
        let (imports, definition_yaml) = ImportParser::parse_with_imports(yaml_str)?;

        // Parse the pipeline from the definition document
        let pipeline = Self::parse_from_yaml(&definition_yaml)?;

        // Get version (default to "0.1" if not specified)
        let version = YamlParser::get_optional_string(&definition_yaml, "version")
            .unwrap_or_else(|| "0.1".to_string());

        // Create RdlDocument
        if let Some(imports) = imports {
            Ok(RdlDocument::with_imports(version, imports, pipeline))
        } else {
            Ok(RdlDocument::new(version, pipeline))
        }
    }

    /// Parse a pipeline from YAML value
    pub fn parse_from_yaml(yaml: &YamlValue) -> Result<Pipeline> {
        // Get the "pipeline" object
        let pipeline_obj = yaml
            .get("pipeline")
            .ok_or_else(|| ParseError::MissingField {
                field: "pipeline".to_string(),
            })?;

        // Try to detect format: new format has "entry" field or "step:" wrapper
        let is_new_format = pipeline_obj.get("entry").is_some()
            || pipeline_obj
                .get("steps")
                .and_then(|s| s.as_sequence())
                .map(|arr| arr.iter().any(|item| item.get("step").is_some()))
                .unwrap_or(false);

        if is_new_format {
            Self::parse_new_format(pipeline_obj)
        } else {
            Self::parse_legacy_format(pipeline_obj)
        }
    }

    /// Parse new format pipeline (with entry point and unified step structure)
    fn parse_new_format(pipeline_obj: &YamlValue) -> Result<Pipeline> {
        // Parse required fields
        let id = YamlParser::get_string(pipeline_obj, "id")?;
        let name = YamlParser::get_string(pipeline_obj, "name")?;
        let entry = YamlParser::get_string(pipeline_obj, "entry")?;

        // Parse optional fields
        let description = YamlParser::get_optional_string(pipeline_obj, "description");
        let version = YamlParser::get_optional_string(pipeline_obj, "version");

        // Parse optional when block
        let when = if let Some(when_obj) = pipeline_obj.get("when") {
            Some(Self::parse_when_block(when_obj)?)
        } else {
            None
        };

        // Parse steps array
        let steps_array = pipeline_obj
            .get("steps")
            .and_then(|v| v.as_sequence())
            .ok_or_else(|| ParseError::MissingField {
                field: "steps".to_string(),
            })?;

        let steps = steps_array
            .iter()
            .map(Self::parse_new_step)
            .collect::<Result<Vec<_>>>()?;

        Ok(Pipeline {
            id,
            name,
            description,
            version,
            entry,
            when,
            steps,
        })
    }

    /// Parse legacy format pipeline (backward compatibility)
    fn parse_legacy_format(pipeline_obj: &YamlValue) -> Result<Pipeline> {
        // Parse optional id, name, description (for legacy format)
        let id = YamlParser::get_optional_string(pipeline_obj, "id")
            .unwrap_or_else(|| "legacy_pipeline".to_string());
        let name = YamlParser::get_optional_string(pipeline_obj, "name")
            .unwrap_or_else(|| "Legacy Pipeline".to_string());
        let description = YamlParser::get_optional_string(pipeline_obj, "description");

        // Parse optional when block
        let when = if let Some(when_obj) = pipeline_obj.get("when") {
            Some(Self::parse_when_block(when_obj)?)
        } else {
            None
        };

        // Parse steps - support both array directly or object with steps
        let legacy_steps = if let Some(steps_array) = pipeline_obj.as_sequence() {
            // Direct array: pipeline: [...]
            steps_array
                .iter()
                .map(Self::parse_step)
                .collect::<Result<Vec<_>>>()?
        } else if let Some(steps_array) = pipeline_obj.get("steps").and_then(|v| v.as_sequence()) {
            // Nested: pipeline: steps: [...]
            steps_array
                .iter()
                .map(Self::parse_step)
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        // Convert legacy Step enum to new PipelineStep format
        let mut steps: Vec<PipelineStep> = Vec::new();
        for (idx, step) in legacy_steps.into_iter().enumerate() {
            let step_id = format!("step_{}", idx);

            let pipeline_step = match step {
                Step::Include { ruleset } => PipelineStep {
                    id: step_id.clone(),
                    name: format!("Include {}", ruleset),
                    step_type: "ruleset".to_string(),
                    routes: None,
                    default: None,
                    next: Some(StepNext::StepId("end".to_string())),
                    when: None,
                    details: StepDetails::Ruleset { ruleset },
                },
                Step::Extract { id: _, features } => PipelineStep {
                    id: step_id.clone(),
                    name: "Extract Features".to_string(),
                    step_type: "extract".to_string(),
                    routes: None,
                    default: None,
                    next: Some(StepNext::StepId("end".to_string())),
                    when: None,
                    details: StepDetails::Extract {
                        features: Some(features),
                    },
                },
                Step::Reason {
                    id: _,
                    provider,
                    model,
                    prompt,
                    output_schema,
                } => PipelineStep {
                    id: step_id.clone(),
                    name: "LLM Reasoning".to_string(),
                    step_type: "reason".to_string(),
                    routes: None,
                    default: None,
                    next: Some(StepNext::StepId("end".to_string())),
                    when: None,
                    details: StepDetails::Reason {
                        provider: Some(provider),
                        model: Some(model),
                        prompt: Some(prompt),
                        output_schema,
                    },
                },
                Step::Service {
                    id: _,
                    service,
                    operation,
                    params,
                    output,
                } => PipelineStep {
                    id: step_id.clone(),
                    name: format!("Service {}", service),
                    step_type: "service".to_string(),
                    routes: None,
                    default: None,
                    next: Some(StepNext::StepId("end".to_string())),
                    when: None,
                    details: StepDetails::Service {
                        service,
                        query: Some(operation),
                        params: Some(params),
                        output,
                    },
                },
                Step::Api {
                    id: _,
                    api,
                    endpoint,
                    params,
                    output,
                    timeout,
                    on_error: _,
                } => PipelineStep {
                    id: step_id.clone(),
                    name: format!("API {}", api),
                    step_type: "api".to_string(),
                    routes: None,
                    default: None,
                    next: Some(StepNext::StepId("end".to_string())),
                    when: None,
                    details: StepDetails::Api {
                        api_target: ApiTarget::Single { api },
                        endpoint: Some(endpoint),
                        params: Some(params),
                        output: Some(output),
                        timeout,
                        on_error: None,
                        min_success: None,
                    },
                },
                Step::Branch { branches: _ } => {
                    // Branch steps are complex - for now create a simple router
                    // Note: The legacy Branch only has 'branches' field with condition and pipeline
                    PipelineStep {
                        id: step_id.clone(),
                        name: "Branch".to_string(),
                        step_type: "router".to_string(),
                        routes: None,
                        default: Some("end".to_string()),
                        next: None,
                        when: None,
                        details: StepDetails::Router {},
                    }
                }
                Step::Parallel { steps: _, merge: _ } => {
                    // Parallel steps are complex - for now just create a placeholder
                    PipelineStep {
                        id: step_id.clone(),
                        name: "Parallel Execution".to_string(),
                        step_type: "router".to_string(),
                        routes: None,
                        default: None,
                        next: Some(StepNext::StepId("end".to_string())),
                        when: None,
                        details: StepDetails::Router {},
                    }
                }
            };
            steps.push(pipeline_step);
        }

        // Update next pointers for sequential steps
        let step_count = steps.len();
        for i in 0..step_count {
            if i + 1 < step_count {
                steps[i].next = Some(StepNext::StepId(format!("step_{}", i + 1)));
            }
        }

        // For legacy format, use the first step's ID as entry, or "start" if empty
        let entry = if !steps.is_empty() {
            steps[0].id.clone()
        } else {
            "start".to_string()
        };

        Ok(Pipeline {
            id,
            name,
            description,
            version: None,
            entry,
            when,
            steps,
        })
    }

    /// Parse a step in new unified format
    fn parse_new_step(yaml: &YamlValue) -> Result<PipelineStep> {
        // Get the "step" wrapper
        let step_obj = yaml.get("step").ok_or_else(|| ParseError::MissingField {
            field: "step".to_string(),
        })?;

        // Parse required fields
        let id = YamlParser::get_string(step_obj, "id")?;
        let name = YamlParser::get_string(step_obj, "name")?;
        let step_type = YamlParser::get_string(step_obj, "type")?;

        // Parse optional routes
        let routes = if let Some(routes_array) = step_obj.get("routes").and_then(|v| v.as_sequence())
        {
            Some(
                routes_array
                    .iter()
                    .map(Self::parse_route)
                    .collect::<Result<Vec<_>>>()?,
            )
        } else {
            None
        };

        // Parse optional default
        let default = YamlParser::get_optional_string(step_obj, "default");

        // Parse optional next
        let next = if let Some(next_str) = YamlParser::get_optional_string(step_obj, "next") {
            Some(StepNext::StepId(next_str))
        } else {
            None
        };

        // Parse optional when
        let when = if let Some(when_obj) = step_obj.get("when") {
            Some(Self::parse_when_block(when_obj)?)
        } else {
            None
        };

        // Parse type-specific details based on step_type
        let details = Self::parse_step_details(step_obj, &step_type)?;

        Ok(PipelineStep {
            id,
            name,
            step_type,
            routes,
            default,
            next,
            when,
            details,
        })
    }

    /// Parse step-specific details based on type
    fn parse_step_details(step_obj: &YamlValue, step_type: &str) -> Result<StepDetails> {
        // Validate fields strictly for this step type
        let valid_fields = get_valid_fields_for_step_type(step_type);
        YamlParser::validate_fields_strict(
            step_obj,
            &valid_fields,
            &format!("{} step", step_type),
        )?;

        match step_type {
            "router" => Ok(StepDetails::Router {}),

            "function" => {
                let function = YamlParser::get_string(step_obj, "function")?;
                let params = Self::parse_params(step_obj)?;
                Ok(StepDetails::Function { function, params })
            }

            "rule" => {
                let rule = YamlParser::get_string(step_obj, "rule")?;
                Ok(StepDetails::Rule { rule })
            }

            "ruleset" => {
                let ruleset = YamlParser::get_string(step_obj, "ruleset")?;
                Ok(StepDetails::Ruleset { ruleset })
            }

            "pipeline" => {
                let pipeline_id = YamlParser::get_string(step_obj, "pipeline")?;
                Ok(StepDetails::SubPipeline { pipeline_id })
            }

            "service" => {
                let service = YamlParser::get_string(step_obj, "service")?;
                let query = YamlParser::get_optional_string(step_obj, "query");
                let params = Self::parse_params(step_obj)?;
                let output = YamlParser::get_optional_string(step_obj, "output");
                Ok(StepDetails::Service {
                    service,
                    query,
                    params,
                    output,
                })
            }

            "api" => {
                // Parse API target (single, any, all)
                let api_target = Self::parse_api_target(step_obj)?;
                let endpoint = YamlParser::get_optional_string(step_obj, "endpoint");
                let params = Self::parse_params(step_obj)?;
                let output = YamlParser::get_optional_string(step_obj, "output");
                let timeout = step_obj.get("timeout").and_then(|v| v.as_u64());
                let on_error = YamlParser::get_optional_string(step_obj, "on_error");
                let min_success = step_obj.get("min_success").and_then(|v| v.as_u64());

                Ok(StepDetails::Api {
                    api_target,
                    endpoint,
                    params,
                    output,
                    timeout,
                    on_error,
                    min_success: min_success.map(|v| v as usize),
                })
            }

            "trigger" => {
                // According to Pipeline DSL v2.0, trigger steps use "target" field, not "trigger"
                let target = YamlParser::get_string(step_obj, "target")?;
                let params = Self::parse_params(step_obj)?;
                Ok(StepDetails::Trigger { target, params })
            }

            "extract" => {
                let features = if let Some(features_array) =
                    step_obj.get("features").and_then(|v| v.as_sequence())
                {
                    Some(
                        features_array
                            .iter()
                            .map(Self::parse_feature_definition)
                            .collect::<Result<Vec<_>>>()?,
                    )
                } else {
                    None
                };
                Ok(StepDetails::Extract { features })
            }

            "reason" => {
                let provider = YamlParser::get_optional_string(step_obj, "provider");
                let model = YamlParser::get_optional_string(step_obj, "model");
                let prompt = if let Some(prompt_str) =
                    YamlParser::get_optional_string(step_obj, "prompt")
                {
                    Some(PromptTemplate {
                        template: prompt_str,
                    })
                } else {
                    None
                };
                let output_schema = step_obj
                    .get("output_schema")
                    .map(Self::parse_schema)
                    .transpose()?;

                Ok(StepDetails::Reason {
                    provider,
                    model,
                    prompt,
                    output_schema,
                })
            }

            _ => Ok(StepDetails::Unknown {}),
        }
    }

    /// Parse API target (single, any, all)
    fn parse_api_target(step_obj: &YamlValue) -> Result<ApiTarget> {
        // Try single API
        if let Some(api) = YamlParser::get_optional_string(step_obj, "api") {
            return Ok(ApiTarget::Single { api });
        }

        // Try "any" array (fallback mode)
        if let Some(any_array) = step_obj.get("any").and_then(|v| v.as_sequence()) {
            let any = any_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            return Ok(ApiTarget::Any { any });
        }

        // Try "all" array (aggregation mode)
        if let Some(all_array) = step_obj.get("all").and_then(|v| v.as_sequence()) {
            let all = all_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            return Ok(ApiTarget::All { all });
        }

        Err(ParseError::MissingField {
            field: "api, any, or all".to_string(),
        })
    }

    /// Parse parameters as HashMap<String, Expression>
    fn parse_params(step_obj: &YamlValue) -> Result<Option<HashMap<String, corint_core::ast::Expression>>> {
        if let Some(params_obj) = step_obj.get("params").and_then(|v| v.as_mapping()) {
            let mut map = HashMap::new();
            for (key, value) in params_obj {
                if let Some(key_str) = key.as_str() {
                    use corint_core::ast::Expression;
                    use corint_core::Value;

                    // Support both string expressions and direct values
                    let expr = if let Some(value_str) = value.as_str() {
                        // If string contains '.', treat as field access (e.g., "event.ip_address")
                        // Otherwise, treat as string literal (e.g., "a63066c9a63590")
                        if value_str.contains('.') {
                            ExpressionParser::parse(value_str)?
                        } else {
                            Expression::literal(Value::String(value_str.to_string()))
                        }
                    } else if let Some(num) = value.as_f64() {
                        Expression::literal(Value::Number(num))
                    } else if let Some(bool_val) = value.as_bool() {
                        Expression::literal(Value::Bool(bool_val))
                    } else {
                        continue; // Skip unsupported types
                    };
                    map.insert(key_str.to_string(), expr);
                }
            }
            Ok(Some(map))
        } else {
            Ok(None)
        }
    }

    /// Parse a route (next + when)
    fn parse_route(yaml: &YamlValue) -> Result<Route> {
        let next = YamlParser::get_string(yaml, "next")?;
        let when_obj = yaml.get("when").ok_or_else(|| ParseError::MissingField {
            field: "when".to_string(),
        })?;
        let when = Self::parse_when_block(when_obj)?;

        Ok(Route { next, when })
    }

    /// Parse when block for pipelines
    fn parse_when_block(when_obj: &YamlValue) -> Result<WhenBlock> {
        // Check if when_obj is a simple string expression (shorthand format)
        if let Some(expr_str) = when_obj.as_str() {
            // Parse as a single expression and wrap in "all" condition group
            let expr = ExpressionParser::parse(expr_str)?;
            use corint_core::ast::rule::{Condition, ConditionGroup};
            return Ok(WhenBlock {
                event_type: None,
                condition_group: Some(ConditionGroup::All(vec![Condition::Expression(expr)])),
                conditions: None,
            });
        }

        // Parse event type (optional)
        // Try three formats: 1) flat "event.type" key, 2) "event_type" key, 3) nested path
        let event_type = YamlParser::get_optional_string(when_obj, "event.type")
            .or_else(|| YamlParser::get_optional_string(when_obj, "event_type"))
            .or_else(|| YamlParser::get_nested_string(when_obj, "event.type"));

        // Detect the deprecated "conditions" field and provide a helpful error message
        if when_obj.get("conditions").is_some() {
            return Err(ParseError::InvalidValue {
                field: "conditions".to_string(),
                message: "The 'conditions' field is not supported. Use 'all', 'any', or 'not' directly instead. Example: 'when: { all: [\"condition1\", \"condition2\"] }'".to_string(),
            });
        }

        // Parse condition group (DSL v2.0 format: all/any/not)
        // Delegate to RuleParser for parsing condition groups
        let condition_group = if let Some(all_cond) = when_obj.get("all") {
            Some(RuleParser::parse_condition_group_all_public(all_cond)?)
        } else if let Some(any_cond) = when_obj.get("any") {
            Some(RuleParser::parse_condition_group_any_public(any_cond)?)
        } else if let Some(not_cond) = when_obj.get("not") {
            Some(RuleParser::parse_condition_group_not_public(not_cond)?)
        } else {
            None
        };

        Ok(WhenBlock {
            event_type,
            condition_group,
            conditions: None,
        })
    }

    /// Parse a single step (legacy format)
    fn parse_step(yaml: &YamlValue) -> Result<Step> {
        // Check if this is a shorthand format (branch:, include:, parallel:)
        if let Some(branch_val) = yaml.get("branch") {
            return Self::parse_branch_shorthand(branch_val);
        }
        if let Some(include_val) = yaml.get("include") {
            return Self::parse_include_shorthand(include_val);
        }
        if let Some(parallel_val) = yaml.get("parallel") {
            return Self::parse_parallel_shorthand(parallel_val, yaml);
        }

        // Otherwise expect type field
        let step_type = YamlParser::get_string(yaml, "type")?;

        match step_type.as_str() {
            "extract" => Self::parse_extract_step(yaml),
            "reason" => Self::parse_reason_step(yaml),
            "service" => Self::parse_service_step(yaml),
            "api" => Self::parse_api_step(yaml),
            "include" => Self::parse_include_step(yaml),
            "branch" => Self::parse_branch_step(yaml),
            "parallel" => Self::parse_parallel_step(yaml),
            _ => Err(ParseError::InvalidValue {
                field: "type".to_string(),
                message: format!("Unknown step type: {}", step_type),
            }),
        }
    }

    /// Parse extract step
    fn parse_extract_step(yaml: &YamlValue) -> Result<Step> {
        let id = YamlParser::get_string(yaml, "id")?;

        let features =
            if let Some(features_array) = yaml.get("features").and_then(|v| v.as_sequence()) {
                features_array
                    .iter()
                    .map(Self::parse_feature_definition)
                    .collect::<Result<Vec<_>>>()?
            } else {
                Vec::new()
            };

        Ok(Step::Extract { id, features })
    }

    /// Parse feature definition
    fn parse_feature_definition(yaml: &YamlValue) -> Result<FeatureDefinition> {
        let name = YamlParser::get_string(yaml, "name")?;
        let value_str = YamlParser::get_string(yaml, "value")?;
        let value = ExpressionParser::parse(&value_str)?;

        Ok(FeatureDefinition { name, value })
    }

    /// Parse reason step
    fn parse_reason_step(yaml: &YamlValue) -> Result<Step> {
        let id = YamlParser::get_string(yaml, "id")?;
        let provider = YamlParser::get_string(yaml, "provider")?;
        let model = YamlParser::get_string(yaml, "model")?;

        let prompt_str = YamlParser::get_string(yaml, "prompt")?;
        let prompt = PromptTemplate {
            template: prompt_str,
        };

        let output_schema = yaml
            .get("output_schema")
            .map(Self::parse_schema)
            .transpose()?;

        Ok(Step::Reason {
            id,
            provider,
            model,
            prompt,
            output_schema,
        })
    }

    /// Parse schema (simplified version)
    fn parse_schema(yaml: &YamlValue) -> Result<Schema> {
        let schema_type = YamlParser::get_string(yaml, "type")?;

        let properties =
            if let Some(props_obj) = yaml.get("properties").and_then(|v| v.as_mapping()) {
                let mut map = HashMap::new();
                for (key, value) in props_obj {
                    if let Some(key_str) = key.as_str() {
                        let prop = Self::parse_schema_property(value)?;
                        map.insert(key_str.to_string(), prop);
                    }
                }
                Some(map)
            } else {
                None
            };

        Ok(Schema {
            schema_type,
            properties,
        })
    }

    /// Parse schema property
    fn parse_schema_property(yaml: &YamlValue) -> Result<SchemaProperty> {
        let property_type = YamlParser::get_string(yaml, "type")?;
        let description = YamlParser::get_optional_string(yaml, "description");

        Ok(SchemaProperty {
            property_type,
            description,
        })
    }

    /// Parse service step
    fn parse_service_step(yaml: &YamlValue) -> Result<Step> {
        let id = YamlParser::get_string(yaml, "id")?;
        let service = YamlParser::get_string(yaml, "service")?;
        let operation = YamlParser::get_string(yaml, "operation")?;

        let params = if let Some(params_obj) = yaml.get("params").and_then(|v| v.as_mapping()) {
            let mut map = HashMap::new();
            for (key, value) in params_obj {
                if let Some(key_str) = key.as_str() {
                    if let Some(value_str) = value.as_str() {
                        let expr = ExpressionParser::parse(value_str)?;
                        map.insert(key_str.to_string(), expr);
                    }
                }
            }
            map
        } else {
            HashMap::new()
        };

        let output = YamlParser::get_optional_string(yaml, "output");

        Ok(Step::Service {
            id,
            service,
            operation,
            params,
            output,
        })
    }

    /// Parse API step (external API call)
    fn parse_api_step(yaml: &YamlValue) -> Result<Step> {
        let id = YamlParser::get_string(yaml, "id")?;
        let api = YamlParser::get_string(yaml, "api")?;
        let endpoint = YamlParser::get_string(yaml, "endpoint")?;
        let output = YamlParser::get_string(yaml, "output")?;

        let params = if let Some(params_obj) = yaml.get("params").and_then(|v| v.as_mapping()) {
            let mut map = HashMap::new();
            for (key, value) in params_obj {
                if let Some(key_str) = key.as_str() {
                    use corint_core::ast::Expression;
                    use corint_core::Value;

                    // Support both string expressions and direct values
                    let expr = if let Some(value_str) = value.as_str() {
                        // If string contains '.', treat as field access (e.g., "event.ip_address")
                        // Otherwise, treat as string literal (e.g., "a63066c9a63590")
                        if value_str.contains('.') {
                            ExpressionParser::parse(value_str)?
                        } else {
                            Expression::literal(Value::String(value_str.to_string()))
                        }
                    } else if let Some(num) = value.as_f64() {
                        Expression::literal(Value::Number(num))
                    } else if let Some(bool_val) = value.as_bool() {
                        Expression::literal(Value::Bool(bool_val))
                    } else {
                        continue; // Skip unsupported types
                    };
                    map.insert(key_str.to_string(), expr);
                }
            }
            map
        } else {
            HashMap::new()
        };

        let timeout = yaml.get("timeout").and_then(|v| v.as_u64());

        let on_error = if let Some(error_obj) = yaml.get("on_error") {
            Some(Self::parse_error_handling(error_obj)?)
        } else {
            None
        };

        Ok(Step::Api {
            id,
            api,
            endpoint,
            params,
            output,
            timeout,
            on_error,
        })
    }

    /// Parse error handling configuration
    fn parse_error_handling(yaml: &YamlValue) -> Result<ErrorHandling> {
        let action_str = YamlParser::get_string(yaml, "action")?;
        let action = match action_str.as_str() {
            "fallback" => ErrorAction::Fallback,
            "skip" => ErrorAction::Skip,
            "fail" => ErrorAction::Fail,
            "retry" => ErrorAction::Retry,
            _ => {
                return Err(ParseError::InvalidValue {
                    field: "action".to_string(),
                    message: format!("Unknown error action: {}", action_str),
                })
            }
        };

        let fallback = yaml.get("fallback").and_then(|v| {
            // Convert YAML value to serde_json::Value
            serde_json::to_value(v).ok()
        });

        Ok(ErrorHandling { action, fallback })
    }

    /// Parse include step
    fn parse_include_step(yaml: &YamlValue) -> Result<Step> {
        let ruleset = YamlParser::get_string(yaml, "ruleset")?;
        Ok(Step::Include { ruleset })
    }

    /// Parse branch step
    fn parse_branch_step(yaml: &YamlValue) -> Result<Step> {
        let branches =
            if let Some(branches_array) = yaml.get("branches").and_then(|v| v.as_sequence()) {
                branches_array
                    .iter()
                    .map(Self::parse_branch)
                    .collect::<Result<Vec<_>>>()?
            } else {
                Vec::new()
            };

        Ok(Step::Branch { branches })
    }

    /// Parse a single branch
    fn parse_branch(yaml: &YamlValue) -> Result<Branch> {
        let condition_str = YamlParser::get_string(yaml, "condition")?;
        let condition = ExpressionParser::parse(&condition_str)?;

        let pipeline = if let Some(steps_array) = yaml.get("pipeline").and_then(|v| v.as_sequence())
        {
            steps_array
                .iter()
                .map(Self::parse_step)
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        Ok(Branch {
            condition,
            pipeline,
        })
    }

    /// Parse parallel step
    fn parse_parallel_step(yaml: &YamlValue) -> Result<Step> {
        let steps = if let Some(steps_array) = yaml.get("steps").and_then(|v| v.as_sequence()) {
            steps_array
                .iter()
                .map(Self::parse_step)
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        let merge_str = YamlParser::get_string(yaml, "merge")?;
        let merge = Self::parse_merge_strategy(&merge_str)?;

        Ok(Step::Parallel { steps, merge })
    }

    /// Parse merge strategy
    fn parse_merge_strategy(s: &str) -> Result<MergeStrategy> {
        match s {
            "all" => Ok(MergeStrategy::All),
            "any" => Ok(MergeStrategy::Any),
            "fastest" => Ok(MergeStrategy::Fastest),
            "majority" => Ok(MergeStrategy::Majority),
            _ => Err(ParseError::InvalidValue {
                field: "merge".to_string(),
                message: format!("Unknown merge strategy: {}", s),
            }),
        }
    }

    /// Parse branch shorthand format: - branch: when: [...]
    fn parse_branch_shorthand(yaml: &YamlValue) -> Result<Step> {
        // Get the "when" array
        let when_array = yaml
            .get("when")
            .and_then(|v| v.as_sequence())
            .ok_or_else(|| ParseError::MissingField {
                field: "when".to_string(),
            })?;

        let branches = when_array
            .iter()
            .map(Self::parse_branch)
            .collect::<Result<Vec<_>>>()?;

        Ok(Step::Branch { branches })
    }

    /// Parse include shorthand format: - include: ruleset: xxx
    fn parse_include_shorthand(yaml: &YamlValue) -> Result<Step> {
        let ruleset = YamlParser::get_string(yaml, "ruleset")?;
        Ok(Step::Include { ruleset })
    }

    /// Parse parallel shorthand format: - parallel: [...] with merge
    fn parse_parallel_shorthand(parallel_val: &YamlValue, parent: &YamlValue) -> Result<Step> {
        let steps = if let Some(steps_array) = parallel_val.as_sequence() {
            steps_array
                .iter()
                .map(Self::parse_step)
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        // Get merge strategy from parent
        let merge_obj = parent
            .get("merge")
            .ok_or_else(|| ParseError::MissingField {
                field: "merge".to_string(),
            })?;

        let merge_str = merge_obj
            .get("method")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ParseError::MissingField {
                field: "merge.method".to_string(),
            })?;

        let merge = Self::parse_merge_strategy(merge_str)?;

        Ok(Step::Parallel { steps, merge })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_new_format_pipeline() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: step1
  version: "1.0"
  steps:
    - step:
        id: step1
        name: Router Step
        type: router
        routes:
          - next: step2
            when:
              all:
                - event.amount > 100
        default: step3
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        assert_eq!(pipeline.id, "test_pipeline");
        assert_eq!(pipeline.name, "Test Pipeline");
        assert_eq!(pipeline.entry, "step1");
        assert_eq!(pipeline.version, Some("1.0".to_string()));
        assert_eq!(pipeline.steps.len(), 1);

        let step = &pipeline.steps[0];
        assert_eq!(step.id, "step1");
        assert_eq!(step.name, "Router Step");
        assert_eq!(step.step_type, "router");
        assert!(step.routes.is_some());
        assert_eq!(step.default, Some("step3".to_string()));
    }

    #[test]
    fn test_parse_ruleset_step() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: ruleset_step
  steps:
    - step:
        id: ruleset_step
        name: Execute Ruleset
        type: ruleset
        ruleset: fraud_detection
        next: end
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        assert_eq!(pipeline.steps.len(), 1);
        let step = &pipeline.steps[0];
        assert_eq!(step.id, "ruleset_step");
        assert_eq!(step.step_type, "ruleset");
        assert!(matches!(
            &step.details,
            StepDetails::Ruleset { ruleset } if ruleset == "fraud_detection"
        ));
        // StepNext::End was removed - "end" is now represented as StepNext::StepId("end".to_string())
        assert_eq!(step.next, Some(StepNext::StepId("end".to_string())));
    }

    #[test]
    fn test_parse_api_single() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: api_step
  steps:
    - step:
        id: api_step
        name: Call Single API
        type: api
        api: geolocation_service
        endpoint: /check
        output: api.geo
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        let step = &pipeline.steps[0];
        assert!(matches!(
            &step.details,
            StepDetails::Api { api_target: ApiTarget::Single { api }, .. } if api == "geolocation_service"
        ));
    }

    #[test]
    fn test_parse_api_any_mode() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: api_step
  steps:
    - step:
        id: api_step
        name: Call Any API
        type: api
        any: [primary_api, backup_api, fallback_api]
        output: api.result
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        let step = &pipeline.steps[0];
        assert!(matches!(
            &step.details,
            StepDetails::Api { api_target: ApiTarget::Any { any }, .. } if any.len() == 3
        ));
    }

    #[test]
    fn test_parse_api_all_mode() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: api_step
  steps:
    - step:
        id: api_step
        name: Call All APIs
        type: api
        all: [api1, api2, api3]
        timeout: 5000
        min_success: 2
        on_error: continue
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        let step = &pipeline.steps[0];
        assert!(matches!(
            &step.details,
            StepDetails::Api {
                api_target: ApiTarget::All { all },
                timeout: Some(5000),
                min_success: Some(2),
                ..
            } if all.len() == 3
        ));
    }

    #[test]
    fn test_parse_extract_step() {
        let yaml = r#"
pipeline:
  steps:
    - type: extract
      id: extract_features
      features:
        - name: login_count
          value: user.login_count
        - name: device_count
          value: user.device_count
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        // Legacy format now converts to new PipelineStep format
        assert_eq!(pipeline.steps.len(), 1);
        assert_eq!(pipeline.steps[0].step_type, "extract");
    }

    #[test]
    fn test_parse_reason_step() {
        let yaml = r#"
pipeline:
  steps:
    - type: reason
      id: llm_analysis
      provider: openai
      model: gpt-4
      prompt: "Analyze this transaction"
      output_schema:
        type: object
        properties:
          is_fraud:
            type: boolean
          confidence:
            type: number
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();
        // Legacy format now converts to new PipelineStep format
        assert_eq!(pipeline.steps.len(), 1);
        assert_eq!(pipeline.steps[0].step_type, "reason");
    }
}
