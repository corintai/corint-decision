//! Pipeline parser
//!
//! Parses YAML pipeline definitions into Pipeline AST nodes.

use corint_core::ast::{
    Branch, FeatureDefinition, MergeStrategy, Pipeline, PromptTemplate,
    RdlDocument, Schema, SchemaProperty, Step, WhenBlock,
};
use corint_core::ast::pipeline::{ErrorAction, ErrorHandling};
use crate::error::{ParseError, Result};
use crate::expression_parser::ExpressionParser;
use crate::import_parser::ImportParser;
use crate::yaml_parser::YamlParser;
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;

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

        // Parse optional id, name, description
        let id = YamlParser::get_optional_string(pipeline_obj, "id");
        let name = YamlParser::get_optional_string(pipeline_obj, "name");
        let description = YamlParser::get_optional_string(pipeline_obj, "description");

        // Parse optional when block
        let when = if let Some(when_obj) = pipeline_obj.get("when") {
            Some(Self::parse_when_block(when_obj)?)
        } else {
            None
        };

        // Parse steps - support both array directly or object with steps
        let steps = if let Some(steps_array) = pipeline_obj.as_sequence() {
            // Direct array: pipeline: [...]
            steps_array
                .iter()
                .map(|v| Self::parse_step(v))
                .collect::<Result<Vec<_>>>()?
        } else if let Some(steps_array) = pipeline_obj.get("steps").and_then(|v| v.as_sequence()) {
            // Nested: pipeline: steps: [...]
            steps_array
                .iter()
                .map(|v| Self::parse_step(v))
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        Ok(Pipeline { id, name, description, when, steps })
    }

    /// Parse when block (similar to rule when block)
    fn parse_when_block(when_obj: &YamlValue) -> Result<WhenBlock> {
        // Parse event type (optional)
        // Try three formats: 1) flat "event.type" key, 2) "event_type" key, 3) nested path
        let event_type = YamlParser::get_optional_string(when_obj, "event.type")
            .or_else(|| YamlParser::get_optional_string(when_obj, "event_type"))
            .or_else(|| YamlParser::get_nested_string(when_obj, "event.type"));

        // Parse conditions (optional for pipeline when block)
        let conditions = if let Some(cond_array) = when_obj.get("conditions").and_then(|v| v.as_sequence()) {
            cond_array
                .iter()
                .map(|cond| {
                    if let Some(s) = cond.as_str() {
                        ExpressionParser::parse(s)
                    } else {
                        Err(ParseError::InvalidValue {
                            field: "condition".to_string(),
                            message: "Condition must be a string expression".to_string(),
                        })
                    }
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        Ok(WhenBlock {
            event_type,
            conditions,
        })
    }

    /// Parse a single step
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

        let features = if let Some(features_array) = yaml.get("features").and_then(|v| v.as_sequence()) {
            features_array
                .iter()
                .map(|v| Self::parse_feature_definition(v))
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
            .map(|v| Self::parse_schema(v))
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

        let properties = if let Some(props_obj) = yaml.get("properties").and_then(|v| v.as_mapping()) {
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
        let branches = if let Some(branches_array) = yaml.get("branches").and_then(|v| v.as_sequence()) {
            branches_array
                .iter()
                .map(|v| Self::parse_branch(v))
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

        let pipeline = if let Some(steps_array) = yaml.get("pipeline").and_then(|v| v.as_sequence()) {
            steps_array
                .iter()
                .map(|v| Self::parse_step(v))
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
                .map(|v| Self::parse_step(v))
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
            .map(|v| Self::parse_branch(v))
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
                .map(|v| Self::parse_step(v))
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

        assert_eq!(pipeline.steps.len(), 1);

        if let Step::Extract { id, features } = &pipeline.steps[0] {
            assert_eq!(id, "extract_features");
            assert_eq!(features.len(), 2);
            assert_eq!(features[0].name, "login_count");
        } else {
            panic!("Expected Extract step");
        }
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

        assert_eq!(pipeline.steps.len(), 1);

        if let Step::Reason {
            id,
            provider,
            model,
            output_schema,
            ..
        } = &pipeline.steps[0]
        {
            assert_eq!(id, "llm_analysis");
            assert_eq!(provider, "openai");
            assert_eq!(model, "gpt-4");
            assert!(output_schema.is_some());
        } else {
            panic!("Expected Reason step");
        }
    }

    #[test]
    fn test_parse_include_step() {
        let yaml = r#"
pipeline:
  steps:
    - type: include
      ruleset: fraud_detection
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        assert_eq!(pipeline.steps.len(), 1);

        if let Step::Include { ruleset } = &pipeline.steps[0] {
            assert_eq!(ruleset, "fraud_detection");
        } else {
            panic!("Expected Include step");
        }
    }

    #[test]
    fn test_parse_branch_step() {
        let yaml = r#"
pipeline:
  steps:
    - type: branch
      branches:
        - condition: user.age > 18
          pipeline:
            - type: include
              ruleset: adult_rules
        - condition: user.age <= 18
          pipeline:
            - type: include
              ruleset: minor_rules
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        assert_eq!(pipeline.steps.len(), 1);

        if let Step::Branch { branches } = &pipeline.steps[0] {
            assert_eq!(branches.len(), 2);
        } else {
            panic!("Expected Branch step");
        }
    }

    #[test]
    fn test_parse_parallel_step() {
        let yaml = r#"
pipeline:
  steps:
    - type: parallel
      merge: all
      steps:
        - type: include
          ruleset: rules_1
        - type: include
          ruleset: rules_2
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        assert_eq!(pipeline.steps.len(), 1);

        if let Step::Parallel { steps, merge } = &pipeline.steps[0] {
            assert_eq!(steps.len(), 2);
            assert_eq!(*merge, MergeStrategy::All);
        } else {
            panic!("Expected Parallel step");
        }
    }

    #[test]
    fn test_parse_complex_pipeline() {
        let yaml = r#"
pipeline:
  steps:
    - type: extract
      id: extract
      features:
        - name: count
          value: user.count
    - type: include
      ruleset: fraud_detection
    - type: branch
      branches:
        - condition: total_score > 100
          pipeline:
            - type: reason
              id: llm_check
              provider: openai
              model: gpt-4
              prompt: "Check fraud"
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        assert_eq!(pipeline.steps.len(), 3);
    }
}
