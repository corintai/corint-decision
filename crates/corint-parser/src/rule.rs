//! Rule parser
//!
//! Parses YAML rule definitions into Rule AST nodes.

use corint_core::ast::{Expression, Rule, WhenBlock};
use crate::error::{ParseError, Result};
use crate::expression::ExpressionParser;
use serde_yaml::Value as YamlValue;

/// Rule parser
pub struct RuleParser;

impl RuleParser {
    /// Parse a rule from YAML string
    pub fn parse(yaml_str: &str) -> Result<Rule> {
        let yaml: YamlValue = serde_yaml::from_str(yaml_str)?;
        Self::parse_from_yaml(&yaml)
    }

    /// Parse a rule from YAML value
    pub fn parse_from_yaml(yaml: &YamlValue) -> Result<Rule> {
        // Get the "rule" object
        let rule_obj = yaml
            .get("rule")
            .ok_or_else(|| ParseError::MissingField {
                field: "rule".to_string(),
            })?;

        // Parse required fields
        let id = Self::get_string(rule_obj, "id")?;
        let name = Self::get_string(rule_obj, "name")?;
        let score = Self::get_i32(rule_obj, "score")?;

        // Parse optional description
        let description = Self::get_optional_string(rule_obj, "description");

        // Parse when block
        let when = Self::parse_when_block(rule_obj)?;

        Ok(Rule {
            id,
            name,
            description,
            when,
            score,
        })
    }

    /// Parse when block
    fn parse_when_block(rule_obj: &YamlValue) -> Result<WhenBlock> {
        let when_obj = rule_obj
            .get("when")
            .ok_or_else(|| ParseError::MissingField {
                field: "when".to_string(),
            })?;

        // Parse event type (optional)
        let event_type = Self::get_optional_string(when_obj, "event.type")
            .or_else(|| Self::get_optional_string(when_obj, "event_type"));

        // Parse conditions
        let conditions = if let Some(cond_array) = when_obj.get("conditions").and_then(|v| v.as_sequence()) {
            cond_array
                .iter()
                .map(|cond| Self::parse_condition(cond))
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        Ok(WhenBlock {
            event_type,
            conditions,
        })
    }

    /// Parse a single condition
    fn parse_condition(yaml: &YamlValue) -> Result<Expression> {
        if let Some(s) = yaml.as_str() {
            // Simple string condition - parse as expression
            ExpressionParser::parse(s)
        } else if let Some(obj) = yaml.as_mapping() {
            // Object-based condition (for complex conditions)
            // For now, convert to string and parse
            // TODO: Support more complex YAML-based condition syntax
            let yaml_str = serde_yaml::to_string(yaml)
                .map_err(|e| ParseError::ParseError(e.to_string()))?;
            Err(ParseError::InvalidValue {
                field: "condition".to_string(),
                message: format!("Object-based conditions not yet supported: {}", yaml_str),
            })
        } else {
            Err(ParseError::InvalidValue {
                field: "condition".to_string(),
                message: "Condition must be a string expression".to_string(),
            })
        }
    }

    /// Get a required string field
    fn get_string(obj: &YamlValue, field: &str) -> Result<String> {
        obj.get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ParseError::MissingField {
                field: field.to_string(),
            })
    }

    /// Get an optional string field
    fn get_optional_string(obj: &YamlValue, field: &str) -> Option<String> {
        obj.get(field).and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    /// Get a required i32 field
    fn get_i32(obj: &YamlValue, field: &str) -> Result<i32> {
        obj.get(field)
            .and_then(|v| v.as_i64())
            .map(|n| n as i32)
            .ok_or_else(|| ParseError::MissingField {
                field: field.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let yaml = r#"
rule:
  id: age_check
  name: Age Check Rule
  description: Check if user is over 18
  when:
    event.type: login
    conditions:
      - user.age > 18
  score: 50
"#;

        let rule = RuleParser::parse(yaml).unwrap();

        assert_eq!(rule.id, "age_check");
        assert_eq!(rule.name, "Age Check Rule");
        assert_eq!(rule.description, Some("Check if user is over 18".to_string()));
        assert_eq!(rule.score, 50);
        assert_eq!(rule.when.event_type, Some("login".to_string()));
        assert_eq!(rule.when.conditions.len(), 1);
    }

    #[test]
    fn test_parse_rule_with_multiple_conditions() {
        let yaml = r#"
rule:
  id: fraud_check
  name: Fraud Detection
  when:
    event.type: transaction
    conditions:
      - amount > 1000
      - country == "US"
      - user.verified == false
  score: 100
"#;

        let rule = RuleParser::parse(yaml).unwrap();

        assert_eq!(rule.id, "fraud_check");
        assert_eq!(rule.when.conditions.len(), 3);
    }

    #[test]
    fn test_parse_rule_without_event_type() {
        let yaml = r#"
rule:
  id: generic_rule
  name: Generic Rule
  when:
    conditions:
      - score > 50
  score: 10
"#;

        let rule = RuleParser::parse(yaml).unwrap();

        assert_eq!(rule.id, "generic_rule");
        assert_eq!(rule.when.event_type, None);
        assert_eq!(rule.when.conditions.len(), 1);
    }

    #[test]
    fn test_parse_rule_complex_conditions() {
        let yaml = r#"
rule:
  id: complex_rule
  name: Complex Rule
  when:
    event.type: login
    conditions:
      - user.age > 18 && user.country == "US"
      - device.is_new == true || user.suspicious == true
  score: 75
"#;

        let rule = RuleParser::parse(yaml).unwrap();

        assert_eq!(rule.id, "complex_rule");
        assert_eq!(rule.when.conditions.len(), 2);
    }

    #[test]
    fn test_missing_required_field() {
        let yaml = r#"
rule:
  name: Missing ID
  when:
    conditions: []
  score: 10
"#;

        let result = RuleParser::parse(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_when_block() {
        let yaml = r#"
rule:
  id: test
  name: Test
  score: 10
"#;

        let result = RuleParser::parse(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_rule_with_function_calls() {
        let yaml = r#"
rule:
  id: velocity_check
  name: Velocity Check
  when:
    event.type: transaction
    conditions:
      - count(transactions, user.id == event.user.id) > 10
  score: 60
"#;

        let rule = RuleParser::parse(yaml).unwrap();

        assert_eq!(rule.id, "velocity_check");
        assert_eq!(rule.when.conditions.len(), 1);
    }
}
