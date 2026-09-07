//! Pipeline step parsing
//!
//! Parses different types of pipeline steps from YAML format.

use super::validation::get_valid_fields_for_step_type;
use crate::error::{ParseError, Result};
use crate::expression_parser::ExpressionParser;
use crate::rule_parser::RuleParser;
use crate::yaml_parser::YamlParser;
use corint_decision_model::ast::pipeline::{PipelineStep, Route, StepDetails, StepNext};
use corint_decision_model::ast::{Branch, FeatureDefinition, MergeStrategy, Step, WhenBlock};
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;

/// Parse a step in new unified format
pub(super) fn parse_new_step(yaml: &YamlValue) -> Result<PipelineStep> {
    // Get the "step" wrapper
    let step_obj = yaml.get("step").ok_or_else(|| ParseError::MissingField {
        field: "step".to_string(),
    })?;

    // Parse required fields
    let id = YamlParser::get_string(step_obj, "id")?;
    let name = YamlParser::get_string(step_obj, "name")?;
    let step_type = YamlParser::get_string(step_obj, "type")?;

    for field in ["default", "next", "endpoint", "output", "on_error"] {
        if step_obj.get(field).is_some_and(|v| v.as_str().is_none()) {
            return Err(ParseError::InvalidValue {
                field: field.into(),
                message: "Expected a string".into(),
            });
        }
    }
    for field in ["timeout", "min_success"] {
        if step_obj.get(field).is_some_and(|v| v.as_u64().is_none()) {
            return Err(ParseError::InvalidValue {
                field: field.into(),
                message: "Expected a non-negative integer".into(),
            });
        }
    }
    if step_obj
        .get("routes")
        .is_some_and(|v| v.as_sequence().is_none())
    {
        return Err(ParseError::InvalidValue {
            field: "routes".into(),
            message: "Expected a route array".into(),
        });
    }
    // Parse optional routes
    let routes = if let Some(routes_array) = step_obj.get("routes").and_then(|v| v.as_sequence()) {
        Some(
            routes_array
                .iter()
                .map(parse_route)
                .collect::<Result<Vec<_>>>()?,
        )
    } else {
        None
    };

    // Parse optional default
    let default = YamlParser::get_optional_string(step_obj, "default");

    // Parse optional next
    let next = YamlParser::get_optional_string(step_obj, "next").map(StepNext::StepId);

    // Parse optional when
    let when = if let Some(when_obj) = step_obj.get("when") {
        Some(parse_when_block(when_obj)?)
    } else {
        None
    };

    // Parse type-specific details based on step_type
    let details = parse_step_details(step_obj, &step_type)?;

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
pub(super) fn parse_step_details(step_obj: &YamlValue, step_type: &str) -> Result<StepDetails> {
    // Validate fields strictly for this step type
    let valid_fields = get_valid_fields_for_step_type(step_type);
    YamlParser::validate_fields_strict(step_obj, &valid_fields, &format!("{} step", step_type))?;

    match step_type {
        "router" => Ok(StepDetails::Router {}),

        "function" => {
            let function = YamlParser::get_string(step_obj, "function")?;
            let params = parse_params(step_obj)?;
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
            let operation = YamlParser::get_string(step_obj, "operation")?;
            let timeout_ms = step_obj
                .get("timeout_ms")
                .map(|v| {
                    v.as_u64()
                        .filter(|v| *v > 0)
                        .ok_or_else(|| ParseError::InvalidValue {
                            field: "timeout_ms".into(),
                            message: "Expected a positive integer in milliseconds".into(),
                        })
                })
                .transpose()?;
            Ok(StepDetails::Service {
                service,
                operation,
                params: parse_params(step_obj)?,
                output: YamlParser::get_optional_string(step_obj, "output"),
                timeout_ms,
            })
        }

        "trigger" => {
            // According to Pipeline DSL v2.0, trigger steps use "target" field, not "trigger"
            let target = YamlParser::get_string(step_obj, "target")?;
            let params = parse_params(step_obj)?;
            Ok(StepDetails::Trigger { target, params })
        }

        "extract" => {
            let features = if let Some(features_array) =
                step_obj.get("features").and_then(|v| v.as_sequence())
            {
                Some(
                    features_array
                        .iter()
                        .map(parse_feature_definition)
                        .collect::<Result<Vec<_>>>()?,
                )
            } else {
                None
            };
            Ok(StepDetails::Extract { features })
        }

        _ => Err(ParseError::InvalidValue {
            field: "type".into(),
            message: format!(
                "Unknown step type: {step_type}; service invocations use type: service"
            ),
        }),
    }
}

/// Parse parameters as HashMap<String, Expression>
pub(super) fn parse_params(
    step_obj: &YamlValue,
) -> Result<Option<HashMap<String, corint_decision_model::ast::Expression>>> {
    if step_obj
        .get("params")
        .is_some_and(|v| v.as_mapping().is_none())
    {
        return Err(ParseError::InvalidValue {
            field: "params".into(),
            message: "Expected a parameter map".into(),
        });
    }
    if let Some(params_obj) = step_obj.get("params").and_then(|v| v.as_mapping()) {
        let mut map = HashMap::new();
        for (key, value) in params_obj {
            if let Some(key_str) = key.as_str().filter(|name| !name.trim().is_empty()) {
                use corint_decision_model::ast::Expression;
                use corint_decision_model::Value;

                let expr = if let Some(text) = value.as_str() {
                    if let Some(expression) =
                        text.strip_prefix("${").and_then(|v| v.strip_suffix('}'))
                    {
                        ExpressionParser::parse(expression)?
                    } else if ["event.", "service.", "vars.", "features.", "sys.", "env."]
                        .iter()
                        .any(|prefix| text.starts_with(prefix))
                    {
                        ExpressionParser::parse(text)?
                    } else {
                        Expression::literal(Value::String(text.to_owned()))
                    }
                } else {
                    let literal: Value =
                        serde_yaml::from_value(value.clone()).map_err(|error| {
                            ParseError::InvalidValue {
                                field: format!("params.{key_str}"),
                                message: error.to_string(),
                            }
                        })?;
                    Expression::literal(literal)
                };
                map.insert(key_str.to_string(), expr);
            } else {
                return Err(ParseError::InvalidValue {
                    field: "params".into(),
                    message: "Parameter names must be strings".into(),
                });
            }
        }
        Ok(Some(map))
    } else {
        Ok(None)
    }
}

/// Parse a route (next + when)
pub(super) fn parse_route(yaml: &YamlValue) -> Result<Route> {
    let next = YamlParser::get_string(yaml, "next")?;
    let when_obj = yaml.get("when").ok_or_else(|| ParseError::MissingField {
        field: "when".to_string(),
    })?;
    let when = parse_when_block(when_obj)?;

    Ok(Route { next, when })
}

/// Parse when block for pipelines
pub(super) fn parse_when_block(when_obj: &YamlValue) -> Result<WhenBlock> {
    // Check if when_obj is a simple string expression (shorthand format)
    if let Some(expr_str) = when_obj.as_str() {
        // Parse as a single expression and wrap in "all" condition group
        let expr = ExpressionParser::parse(expr_str)?;
        use corint_decision_model::ast::rule::{Condition, ConditionGroup};
        return Ok(WhenBlock {
            event_type: None,
            condition_group: Some(ConditionGroup::All(vec![Condition::Expression(expr)])),
            conditions: None,
        });
    }

    RuleParser::validate_when_shape(when_obj)?;
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
pub(super) fn parse_step(yaml: &YamlValue) -> Result<Step> {
    // Check if this is a shorthand format (branch:, include:, parallel:)
    if let Some(branch_val) = yaml.get("branch") {
        return parse_branch_shorthand(branch_val);
    }
    if let Some(include_val) = yaml.get("include") {
        return parse_include_shorthand(include_val);
    }
    if let Some(parallel_val) = yaml.get("parallel") {
        return parse_parallel_shorthand(parallel_val, yaml);
    }

    // Otherwise expect type field
    let step_type = YamlParser::get_string(yaml, "type")?;

    match step_type.as_str() {
        "extract" => parse_extract_step(yaml),
        "service" => parse_service_step(yaml),
        "include" => parse_include_step(yaml),
        "branch" => parse_branch_step(yaml),
        "parallel" => parse_parallel_step(yaml),
        _ => Err(ParseError::InvalidValue {
            field: "type".to_string(),
            message: format!("Unknown step type: {}", step_type),
        }),
    }
}

/// Parse extract step
pub(super) fn parse_extract_step(yaml: &YamlValue) -> Result<Step> {
    let id = YamlParser::get_string(yaml, "id")?;

    let features = if let Some(features_array) = yaml.get("features").and_then(|v| v.as_sequence())
    {
        features_array
            .iter()
            .map(parse_feature_definition)
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };

    Ok(Step::Extract { id, features })
}

/// Parse feature definition
pub(super) fn parse_feature_definition(yaml: &YamlValue) -> Result<FeatureDefinition> {
    let name = YamlParser::get_string(yaml, "name")?;
    let value_str = YamlParser::get_string(yaml, "value")?;
    let value = ExpressionParser::parse(&value_str)?;

    Ok(FeatureDefinition { name, value })
}

/// Parse service step
pub(super) fn parse_service_step(yaml: &YamlValue) -> Result<Step> {
    let id = YamlParser::get_string(yaml, "id")?;
    let StepDetails::Service {
        service,
        operation,
        params,
        output,
        timeout_ms,
    } = parse_step_details(yaml, "service")?
    else {
        unreachable!()
    };
    Ok(Step::Service {
        id,
        service,
        operation,
        params: params.unwrap_or_default(),
        output,
        timeout_ms,
    })
}

/// Parse include step
pub(super) fn parse_include_step(yaml: &YamlValue) -> Result<Step> {
    let ruleset = YamlParser::get_string(yaml, "ruleset")?;
    Ok(Step::Include { ruleset })
}

/// Parse branch step
pub(super) fn parse_branch_step(yaml: &YamlValue) -> Result<Step> {
    let branches = if let Some(branches_array) = yaml.get("branches").and_then(|v| v.as_sequence())
    {
        branches_array
            .iter()
            .map(parse_branch)
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };

    Ok(Step::Branch { branches })
}

/// Parse a single branch
pub(super) fn parse_branch(yaml: &YamlValue) -> Result<Branch> {
    let condition_str = YamlParser::get_string(yaml, "condition")?;
    let condition = ExpressionParser::parse(&condition_str)?;

    let pipeline = if let Some(steps_array) = yaml.get("pipeline").and_then(|v| v.as_sequence()) {
        steps_array
            .iter()
            .map(parse_step)
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
pub(super) fn parse_parallel_step(yaml: &YamlValue) -> Result<Step> {
    let steps = if let Some(steps_array) = yaml.get("steps").and_then(|v| v.as_sequence()) {
        steps_array
            .iter()
            .map(parse_step)
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };

    let merge_str = YamlParser::get_string(yaml, "merge")?;
    let merge = parse_merge_strategy(&merge_str)?;

    Ok(Step::Parallel { steps, merge })
}

/// Parse merge strategy
pub(super) fn parse_merge_strategy(s: &str) -> Result<MergeStrategy> {
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
pub(super) fn parse_branch_shorthand(yaml: &YamlValue) -> Result<Step> {
    // Get the "when" array
    let when_array = yaml
        .get("when")
        .and_then(|v| v.as_sequence())
        .ok_or_else(|| ParseError::MissingField {
            field: "when".to_string(),
        })?;

    let branches = when_array
        .iter()
        .map(parse_branch)
        .collect::<Result<Vec<_>>>()?;

    Ok(Step::Branch { branches })
}

/// Parse include shorthand format: - include: ruleset: xxx
pub(super) fn parse_include_shorthand(yaml: &YamlValue) -> Result<Step> {
    let ruleset = YamlParser::get_string(yaml, "ruleset")?;
    Ok(Step::Include { ruleset })
}

/// Parse parallel shorthand format: - parallel: [...] with merge
pub(super) fn parse_parallel_shorthand(
    parallel_val: &YamlValue,
    parent: &YamlValue,
) -> Result<Step> {
    let steps = if let Some(steps_array) = parallel_val.as_sequence() {
        steps_array
            .iter()
            .map(parse_step)
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

    let merge = parse_merge_strategy(merge_str)?;

    Ok(Step::Parallel { steps, merge })
}
