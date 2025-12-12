//! Rule parser
//!
//! Parses YAML rule definitions into Rule AST nodes.

use corint_core::ast::{Expression, RdlDocument, Rule, WhenBlock};
use crate::error::{ParseError, Result};
use crate::expression_parser::ExpressionParser;
use crate::import_parser::ImportParser;
use crate::yaml_parser::YamlParser;
use serde_yaml::Value as YamlValue;

/// Rule parser
pub struct RuleParser;

impl RuleParser {
    /// Parse a rule from YAML string (legacy format, no imports)
    ///
    /// This maintains backward compatibility with existing code.
    pub fn parse(yaml_str: &str) -> Result<Rule> {
        let yaml = YamlParser::parse(yaml_str)?;
        Self::parse_from_yaml(&yaml)
    }

    /// Parse a rule with optional imports from YAML string (new format)
    ///
    /// Supports both formats:
    /// 1. Legacy single-document format (backward compatible)
    /// 2. New multi-document format with imports
    ///
    /// Returns an RdlDocument<Rule> containing both the rule and its imports (if any)
    pub fn parse_with_imports(yaml_str: &str) -> Result<RdlDocument<Rule>> {
        let (imports, definition_yaml) = ImportParser::parse_with_imports(yaml_str)?;

        // Parse the rule from the definition document
        let rule = Self::parse_from_yaml(&definition_yaml)?;

        // Get version (default to "0.1" if not specified)
        let version = YamlParser::get_optional_string(&definition_yaml, "version")
            .unwrap_or_else(|| "0.1".to_string());

        // Create RdlDocument
        if let Some(imports) = imports {
            Ok(RdlDocument::with_imports(version, imports, rule))
        } else {
            Ok(RdlDocument::new(version, rule))
        }
    }

    /// Parse a rule from YAML value
    pub fn parse_from_yaml(yaml: &YamlValue) -> Result<Rule> {
        // Get the "rule" object
        let rule_obj = yaml
            .get("rule")
            .ok_or_else(|| ParseError::MissingField {
                field: "rule".to_string(),
            })?;

        // Parse required fields using YamlParser utilities
        let id = YamlParser::get_string(rule_obj, "id")?;
        let name = YamlParser::get_string(rule_obj, "name")?;
        let score = YamlParser::get_i32(rule_obj, "score")?;

        // Parse optional description
        let description = YamlParser::get_optional_string(rule_obj, "description");

        // Parse optional params
        let params = if let Some(params_obj) = rule_obj.get("params") {
            Some(Self::parse_params(params_obj)?)
        } else {
            None
        };

        // Parse optional metadata
        let metadata = rule_obj.get("metadata").and_then(|v| {
            serde_yaml::from_value(v.clone()).ok()
        });

        // Parse when block
        let when = Self::parse_when_block(rule_obj)?;

        Ok(Rule {
            id,
            name,
            description,
            params,
            when,
            score,
            metadata,
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
        // Try three formats: 1) flat "event.type" key, 2) "event_type" key, 3) nested path
        let event_type = YamlParser::get_optional_string(when_obj, "event.type")
            .or_else(|| YamlParser::get_optional_string(when_obj, "event_type"))
            .or_else(|| YamlParser::get_nested_string(when_obj, "event.type"));

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
        } else if let Some(_obj) = yaml.as_mapping() {
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

    /// Parse params object
    fn parse_params(params_obj: &YamlValue) -> Result<corint_core::ast::RuleParams> {
        use corint_core::ast::RuleParams;
        use std::collections::HashMap;

        let mut values = HashMap::new();

        if let Some(map) = params_obj.as_mapping() {
            for (key, value) in map {
                if let Some(key_str) = key.as_str() {
                    // Convert YAML value to serde_json::Value
                    let json_value = serde_yaml::from_value(value.clone())
                        .map_err(|e| ParseError::ParseError(format!("Failed to parse param '{}': {}", key_str, e)))?;
                    values.insert(key_str.to_string(), json_value);
                }
            }
        }

        Ok(RuleParams::from_map(values))
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

    // Tests for new import functionality
    #[test]
    fn test_parse_with_imports_legacy_format() {
        let yaml = r#"
version: "0.1"
rule:
  id: test_rule
  name: Test Rule
  when:
    event.type: transaction
    conditions:
      - amount > 1000
  score: 50
"#;

        let doc = RuleParser::parse_with_imports(yaml).unwrap();

        assert_eq!(doc.version, "0.1");
        assert!(!doc.has_imports());
        assert_eq!(doc.definition.id, "test_rule");
        assert_eq!(doc.definition.score, 50);
    }

    #[test]
    fn test_parse_with_imports_new_format() {
        let yaml = r#"
version: "0.1"
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml

---

rule:
  id: fraud_detection
  name: Fraud Detection Rule
  description: Detect fraud patterns
  when:
    event.type: transaction
    conditions:
      - amount > 10000
  score: 100
"#;

        let doc = RuleParser::parse_with_imports(yaml).unwrap();

        assert_eq!(doc.version, "0.1");
        assert!(doc.has_imports());

        let imports = doc.imports();
        assert_eq!(imports.rules.len(), 1);
        assert_eq!(imports.rules[0], "library/rules/fraud/fraud_farm.yaml");

        assert_eq!(doc.definition.id, "fraud_detection");
        assert_eq!(doc.definition.name, "Fraud Detection Rule");
        assert_eq!(doc.definition.score, 100);
    }

    #[test]
    fn test_parse_with_imports_multiple_imports() {
        let yaml = r#"
version: "0.1"
imports:
  rules:
    - library/rules/rule1.yaml
    - library/rules/rule2.yaml
  rulesets:
    - library/rulesets/ruleset1.yaml

---

rule:
  id: combined_rule
  name: Combined Rule
  when:
    conditions:
      - total_score > 50
  score: 75
"#;

        let doc = RuleParser::parse_with_imports(yaml).unwrap();

        assert!(doc.has_imports());

        let imports = doc.imports();
        assert_eq!(imports.rules.len(), 2);
        assert_eq!(imports.rulesets.len(), 1);

        assert_eq!(doc.definition.id, "combined_rule");
    }

    #[test]
    fn test_backward_compatibility() {
        // The old parse() method should still work for legacy format
        let yaml = r#"
rule:
  id: legacy_rule
  name: Legacy Rule
  when:
    conditions: []
  score: 10
"#;

        let rule = RuleParser::parse(yaml).unwrap();
        assert_eq!(rule.id, "legacy_rule");

        // The new parse_with_imports() should also work for the same content
        let doc = RuleParser::parse_with_imports(yaml).unwrap();
        assert_eq!(doc.definition.id, "legacy_rule");
        assert!(!doc.has_imports());
    }
}
