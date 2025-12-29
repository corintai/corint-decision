//! Core pipeline parser implementation
//!
//! Parses YAML pipeline definitions into Pipeline AST nodes.

use crate::error::{ParseError, Result};
use crate::import_parser::ImportParser;
use crate::yaml_parser::YamlParser;
use super::step_parser::{parse_new_step, parse_step, parse_when_block};
use corint_core::ast::pipeline::{PipelineStep, StepNext, StepDetails};
use corint_core::ast::{Pipeline, RdlDocument, Step};
use serde_yaml::Value as YamlValue;

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

        // Parse optional metadata (arbitrary key-value pairs)
        let metadata = pipeline_obj
            .get("metadata")
            .and_then(|v| v.as_mapping())
            .map(|mapping| {
                let mut result = std::collections::HashMap::new();
                for (key, value) in mapping {
                    if let Some(key_str) = key.as_str() {
                        // Convert YAML value to serde_json::Value
                        if let Ok(json_value) = serde_yaml::from_value::<serde_json::Value>(value.clone()) {
                            result.insert(key_str.to_string(), json_value);
                        }
                    }
                }
                result
            });

        // Parse optional when block
        let when = if let Some(when_obj) = pipeline_obj.get("when") {
            Some(parse_when_block(when_obj)?)
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
            .map(parse_new_step)
            .collect::<Result<Vec<_>>>()?;

        // Parse optional decision rules
        let decision = if let Some(decision_array) = pipeline_obj.get("decision").and_then(|v| v.as_sequence()) {
            Some(
                decision_array
                    .iter()
                    .map(Self::parse_decision_rule)
                    .collect::<Result<Vec<_>>>()?
            )
        } else {
            None
        };

        Ok(Pipeline {
            id,
            name,
            description,
            entry,
            when,
            steps,
            decision,
            metadata,
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
            Some(parse_when_block(when_obj)?)
        } else {
            None
        };

        // Parse steps - support both array directly or object with steps
        let legacy_steps = if let Some(steps_array) = pipeline_obj.as_sequence() {
            // Direct array: pipeline: [...]
            steps_array
                .iter()
                .map(parse_step)
                .collect::<Result<Vec<_>>>()?
        } else if let Some(steps_array) = pipeline_obj.get("steps").and_then(|v| v.as_sequence()) {
            // Nested: pipeline: steps: [...]
            steps_array
                .iter()
                .map(parse_step)
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
                        api_target: corint_core::ast::pipeline::ApiTarget::Single { api },
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
            entry,
            when,
            steps,
            decision: None,
            metadata: None,
        })
    }

    /// Parse a pipeline decision rule
    fn parse_decision_rule(yaml: &YamlValue) -> Result<corint_core::ast::PipelineDecisionRule> {
        // Check if this is a default rule
        let is_default = yaml
            .get("default")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Parse when condition (optional if default is true)
        let when = if !is_default {
            if let Some(when_obj) = yaml.get("when") {
                Some(parse_when_block(when_obj)?)
            } else {
                return Err(ParseError::MissingField {
                    field: "when or default".to_string(),
                });
            }
        } else {
            None
        };

        // Parse result (required)
        let result = YamlParser::get_string(yaml, "result")?;

        // Parse optional actions
        let actions = yaml
            .get("actions")
            .and_then(|v| v.as_sequence())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Parse optional reason
        let reason = YamlParser::get_optional_string(yaml, "reason");

        Ok(corint_core::ast::PipelineDecisionRule {
            when,
            default: is_default,
            result,
            actions,
            reason,
        })
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
  metadata:
    version: "1.0"
    author: "Test Team"
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
        assert!(pipeline.metadata.is_some());
        let metadata = pipeline.metadata.unwrap();
        assert_eq!(metadata.get("version").unwrap(), &serde_json::json!("1.0"));
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
            StepDetails::Api { api_target: corint_core::ast::pipeline::ApiTarget::Single { api }, .. } if api == "geolocation_service"
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
            StepDetails::Api { api_target: corint_core::ast::pipeline::ApiTarget::Any { any }, .. } if any.len() == 3
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
                api_target: corint_core::ast::pipeline::ApiTarget::All { all },
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
