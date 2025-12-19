//! Registry parser
//!
//! Parses YAML registry definitions into PipelineRegistry AST nodes.

use crate::error::{ParseError, Result};
use crate::expression_parser::ExpressionParser;
use crate::yaml_parser::YamlParser;
use corint_core::ast::{PipelineRegistry, RegistryEntry, WhenBlock};
use serde_yaml::Value as YamlValue;

/// Registry parser
pub struct RegistryParser;

impl RegistryParser {
    /// Parse a registry from YAML string
    pub fn parse(yaml_str: &str) -> Result<PipelineRegistry> {
        let yaml = YamlParser::parse(yaml_str)?;
        Self::parse_from_yaml(&yaml)
    }

    /// Parse a registry from YAML value
    pub fn parse_from_yaml(yaml: &YamlValue) -> Result<PipelineRegistry> {
        // Parse optional version
        let version = YamlParser::get_optional_string(yaml, "version");

        // Get the "registry" array
        let registry_array = yaml
            .get("registry")
            .and_then(|v| v.as_sequence())
            .ok_or_else(|| ParseError::MissingField {
                field: "registry".to_string(),
            })?;

        // Parse all registry entries
        let entries = registry_array
            .iter()
            .enumerate()
            .map(|(idx, v)| Self::parse_entry(v, idx))
            .collect::<Result<Vec<_>>>()?;

        let mut registry = PipelineRegistry::new().with_entries(entries);
        if let Some(ver) = version {
            registry = registry.with_version(ver);
        }

        Ok(registry)
    }

    /// Parse a single registry entry
    fn parse_entry(yaml: &YamlValue, index: usize) -> Result<RegistryEntry> {
        // Get pipeline ID
        let pipeline =
            YamlParser::get_string(yaml, "pipeline").map_err(|_| ParseError::MissingField {
                field: format!("registry[{}].pipeline", index),
            })?;

        // Get when block
        let when_obj = yaml.get("when").ok_or_else(|| ParseError::MissingField {
            field: format!("registry[{}].when", index),
        })?;

        let when = Self::parse_when_block(when_obj)?;

        Ok(RegistryEntry::new(pipeline, when))
    }

    /// Parse when block
    ///
    /// Supports:
    /// 1. Direct field filters (e.g., event.type: login, event.channel: stripe)
    /// 2. conditions array for complex expressions
    ///
    /// All direct field filters are combined with AND logic.
    fn parse_when_block(when_obj: &YamlValue) -> Result<WhenBlock> {
        let mut event_type = None;
        let mut conditions = Vec::new();

        if let Some(mapping) = when_obj.as_mapping() {
            for (key, value) in mapping {
                if let Some(key_str) = key.as_str() {
                    match key_str {
                        // Special handling for conditions array
                        "conditions" => {
                            if let Some(cond_array) = value.as_sequence() {
                                for cond in cond_array {
                                    if let Some(s) = cond.as_str() {
                                        conditions.push(ExpressionParser::parse(s)?);
                                    } else {
                                        return Err(ParseError::InvalidValue {
                                            field: "condition".to_string(),
                                            message: "Condition must be a string expression"
                                                .to_string(),
                                        });
                                    }
                                }
                            }
                        }
                        // All other fields are treated as direct field filters
                        _ => {
                            // Parse the field path (e.g., "event.type" -> ["event", "type"])
                            let field_path: Vec<String> =
                                key_str.split('.').map(|s| s.to_string()).collect();

                            // Special case: event.type gets stored in event_type field for optimization
                            if field_path.len() == 2
                                && field_path[0] == "event"
                                && field_path[1] == "type"
                            {
                                if let Some(value_str) = value.as_str() {
                                    event_type = Some(value_str.to_string());
                                    continue;
                                }
                            }

                            // Convert direct field filter to an equality condition
                            // e.g., event.channel: stripe  ->  event.channel == "stripe"
                            let value_expr = if let Some(s) = value.as_str() {
                                use corint_core::ast::Expression;
                                use corint_core::Value;
                                Expression::literal(Value::String(s.to_string()))
                            } else if let Some(n) = value.as_f64() {
                                use corint_core::ast::Expression;
                                use corint_core::Value;
                                Expression::literal(Value::Number(n))
                            } else if let Some(b) = value.as_bool() {
                                use corint_core::ast::Expression;
                                use corint_core::Value;
                                Expression::literal(Value::Bool(b))
                            } else {
                                continue; // Skip unsupported value types
                            };

                            // Create field access expression
                            use corint_core::ast::{Expression, Operator};
                            let field_expr = Expression::field_access(field_path);

                            // Create equality condition: field == value
                            let condition =
                                Expression::binary(field_expr, Operator::Eq, value_expr);
                            conditions.push(condition);
                        }
                    }
                }
            }
        }

        Ok(WhenBlock {
            event_type,
            condition_group: None,
            conditions: if conditions.is_empty() {
                None
            } else {
                Some(conditions)
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_registry() {
        let yaml = r#"
version: "0.1"

registry:
  - pipeline: login_pipeline
    when:
      event.type: login

  - pipeline: payment_pipeline
    when:
      event.type: payment
"#;

        let registry = RegistryParser::parse(yaml).unwrap();

        assert_eq!(registry.version, Some("0.1".to_string()));
        assert_eq!(registry.registry.len(), 2);
        assert_eq!(registry.registry[0].pipeline, "login_pipeline");
        assert_eq!(
            registry.registry[0].when.event_type,
            Some("login".to_string())
        );
        assert_eq!(registry.registry[1].pipeline, "payment_pipeline");
        assert_eq!(
            registry.registry[1].when.event_type,
            Some("payment".to_string())
        );
    }

    #[test]
    fn test_parse_registry_with_conditions() {
        let yaml = r#"
version: "0.1"

registry:
  - pipeline: payment_br_pipeline
    when:
      event.type: payment
      conditions:
        - geo.country == "BR"
        - amount > 1000

  - pipeline: payment_main_pipeline
    when:
      event.type: payment
"#;

        let registry = RegistryParser::parse(yaml).unwrap();

        assert_eq!(registry.registry.len(), 2);
        assert_eq!(registry.registry[0].when.conditions.as_ref().unwrap().len(), 2);
        assert!(registry.registry[1].when.conditions.is_none() || registry.registry[1].when.conditions.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_parse_registry_with_arbitrary_fields() {
        let yaml = r#"
registry:
  - pipeline: stripe_pipeline
    when:
      event.type: payment
      event.channel: stripe

  - pipeline: city_pipeline
    when:
      event.country.city: sao_paulo

  - pipeline: multi_field_pipeline
    when:
      event.type: transaction
      event.channel: api
      event.currency: BRL
"#;

        let registry = RegistryParser::parse(yaml).unwrap();

        assert_eq!(registry.registry.len(), 3);

        // First entry: event.type and event.channel
        assert_eq!(registry.registry[0].pipeline, "stripe_pipeline");
        assert_eq!(
            registry.registry[0].when.event_type,
            Some("payment".to_string())
        );
        // event.channel should be converted to a condition
        assert_eq!(registry.registry[0].when.conditions.as_ref().unwrap().len(), 1);

        // Second entry: nested field event.country.city
        assert_eq!(registry.registry[1].pipeline, "city_pipeline");
        assert_eq!(registry.registry[1].when.conditions.as_ref().unwrap().len(), 1);

        // Third entry: multiple fields
        assert_eq!(registry.registry[2].pipeline, "multi_field_pipeline");
        assert_eq!(
            registry.registry[2].when.event_type,
            Some("transaction".to_string())
        );
        // event.channel and event.currency should be converted to conditions
        assert_eq!(registry.registry[2].when.conditions.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_parse_registry_with_mixed_filters() {
        let yaml = r#"
registry:
  - pipeline: complex_pipeline
    when:
      event.type: payment
      event.channel: stripe
      conditions:
        - amount > 1000
        - user.verified == true
"#;

        let registry = RegistryParser::parse(yaml).unwrap();

        assert_eq!(registry.registry.len(), 1);

        let entry = &registry.registry[0];
        assert_eq!(entry.pipeline, "complex_pipeline");
        assert_eq!(entry.when.event_type, Some("payment".to_string()));
        // Should have 3 conditions total:
        // 1. event.channel == "stripe" (from direct field filter)
        // 2. amount > 1000 (from conditions array)
        // 3. user.verified == true (from conditions array)
        assert_eq!(entry.when.conditions.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn test_parse_registry_missing_pipeline() {
        let yaml = r#"
registry:
  - when:
      event.type: login
"#;

        let result = RegistryParser::parse(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_registry_missing_when() {
        let yaml = r#"
registry:
  - pipeline: login_pipeline
"#;

        let result = RegistryParser::parse(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_registry_serde_roundtrip() {
        let yaml = r#"
version: "0.1"

registry:
  - pipeline: test_pipeline
    when:
      event.type: test
      event.channel: api
      conditions:
        - amount > 100
"#;

        let registry = RegistryParser::parse(yaml).unwrap();

        // Serialize to JSON
        let json = serde_json::to_string(&registry).unwrap();

        // Deserialize back
        let deserialized: PipelineRegistry = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, registry.version);
        assert_eq!(deserialized.registry.len(), registry.registry.len());
    }
}
