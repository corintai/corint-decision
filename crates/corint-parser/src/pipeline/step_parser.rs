//! Pipeline step parsing
//!
//! Parses different types of pipeline steps from YAML format.

use crate::error::{ParseError, Result};
use crate::expression_parser::ExpressionParser;
use crate::rule_parser::RuleParser;
use crate::yaml_parser::YamlParser;
use super::validation::get_valid_fields_for_step_type;
use corint_core::ast::pipeline::{
    ApiTarget, ErrorAction, ErrorHandling, PipelineStep, Route, StepDetails, StepNext,
};
use corint_core::ast::{
    Branch, FeatureDefinition, MergeStrategy, Step, WhenBlock,
};
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

    // Parse optional routes
    let routes = if let Some(routes_array) = step_obj.get("routes").and_then(|v| v.as_sequence())
    {
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
    let next = if let Some(next_str) = YamlParser::get_optional_string(step_obj, "next") {
        Some(StepNext::StepId(next_str))
    } else {
        None
    };

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
    YamlParser::validate_fields_strict(
        step_obj,
        &valid_fields,
        &format!("{} step", step_type),
    )?;

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
            let query = YamlParser::get_optional_string(step_obj, "query");
            let params = parse_params(step_obj)?;
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
            let api_target = parse_api_target(step_obj)?;
            let endpoint = YamlParser::get_optional_string(step_obj, "endpoint");
            let params = parse_params(step_obj)?;
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

        _ => Ok(StepDetails::Unknown {}),
    }
}

/// Parse API target (single, any, all)
pub(super) fn parse_api_target(step_obj: &YamlValue) -> Result<ApiTarget> {
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
pub(super) fn parse_params(step_obj: &YamlValue) -> Result<Option<HashMap<String, corint_core::ast::Expression>>> {
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
        "api" => parse_api_step(yaml),
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

    let features =
        if let Some(features_array) = yaml.get("features").and_then(|v| v.as_sequence()) {
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
pub(super) fn parse_api_step(yaml: &YamlValue) -> Result<Step> {
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
        Some(parse_error_handling(error_obj)?)
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
pub(super) fn parse_error_handling(yaml: &YamlValue) -> Result<ErrorHandling> {
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
pub(super) fn parse_include_step(yaml: &YamlValue) -> Result<Step> {
    let ruleset = YamlParser::get_string(yaml, "ruleset")?;
    Ok(Step::Include { ruleset })
}

/// Parse branch step
pub(super) fn parse_branch_step(yaml: &YamlValue) -> Result<Step> {
    let branches =
        if let Some(branches_array) = yaml.get("branches").and_then(|v| v.as_sequence()) {
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

    let pipeline = if let Some(steps_array) = yaml.get("pipeline").and_then(|v| v.as_sequence())
    {
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
pub(super) fn parse_parallel_shorthand(parallel_val: &YamlValue, parent: &YamlValue) -> Result<Step> {
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
