//! Ruleset parser
//!
//! Parses YAML ruleset definitions into Ruleset AST nodes.

use corint_core::ast::{Action, DecisionRule, InferConfig, Ruleset};
use crate::error::{ParseError, Result};
use crate::expression_parser::ExpressionParser;
use crate::yaml_parser::YamlParser;
use serde_yaml::Value as YamlValue;

/// Ruleset parser
pub struct RulesetParser;

impl RulesetParser {
    /// Parse a ruleset from YAML string
    pub fn parse(yaml_str: &str) -> Result<Ruleset> {
        let yaml = YamlParser::parse(yaml_str)?;
        Self::parse_from_yaml(&yaml)
    }

    /// Parse a ruleset from YAML value
    pub fn parse_from_yaml(yaml: &YamlValue) -> Result<Ruleset> {
        // Get the "ruleset" object
        let ruleset_obj = yaml
            .get("ruleset")
            .ok_or_else(|| ParseError::MissingField {
                field: "ruleset".to_string(),
            })?;

        // Parse required fields using YamlParser
        let id = YamlParser::get_string(ruleset_obj, "id")?;

        // Parse optional fields
        let name = YamlParser::get_optional_string(ruleset_obj, "name");

        // Parse rules array
        let rules = if let Some(rules_array) = ruleset_obj.get("rules").and_then(|v| v.as_sequence()) {
            rules_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        } else {
            Vec::new()
        };

        // Parse decision logic
        let decision_logic = if let Some(logic_array) = ruleset_obj.get("decision_logic").and_then(|v| v.as_sequence()) {
            logic_array
                .iter()
                .map(|v| Self::parse_decision_rule(v))
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        Ok(Ruleset {
            id,
            name,
            rules,
            decision_logic,
        })
    }

    /// Parse a decision rule
    fn parse_decision_rule(yaml: &YamlValue) -> Result<DecisionRule> {
        // Parse condition (optional)
        let condition = if YamlParser::has_field(yaml, "condition") {
            let s = YamlParser::get_string(yaml, "condition")?;
            Some(ExpressionParser::parse(&s)?)
        } else {
            None
        };

        // Parse default flag
        let default = YamlParser::get_optional_bool(yaml, "default")
            .unwrap_or(false);

        // Parse action
        let action = Self::parse_action(yaml)?;

        // Parse reason (optional)
        let reason = YamlParser::get_optional_string(yaml, "reason");

        // Parse terminate flag
        let terminate = YamlParser::get_optional_bool(yaml, "terminate")
            .unwrap_or(false);

        Ok(DecisionRule {
            condition,
            default,
            action,
            reason,
            terminate,
        })
    }

    /// Parse an action
    fn parse_action(yaml: &YamlValue) -> Result<Action> {
        let action_str = YamlParser::get_string(yaml, "action")?;

        match action_str.as_str() {
            "approve" => Ok(Action::Approve),
            "deny" => Ok(Action::Deny),
            "review" => Ok(Action::Review),
            "infer" => {
                // Parse infer config
                let config = if let Some(config_obj) = yaml.get("config") {
                    Self::parse_infer_config(config_obj)?
                } else {
                    InferConfig {
                        data_snapshot: Vec::new(),
                    }
                };
                Ok(Action::Infer { config })
            }
            _ => Err(ParseError::InvalidValue {
                field: "action".to_string(),
                message: format!("Unknown action type: {}", action_str),
            }),
        }
    }

    /// Parse infer config
    fn parse_infer_config(yaml: &YamlValue) -> Result<InferConfig> {
        let data_snapshot = if let Some(snapshot_array) = yaml.get("data_snapshot").and_then(|v| v.as_sequence()) {
            snapshot_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        } else {
            Vec::new()
        };

        Ok(InferConfig { data_snapshot })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_ruleset() {
        let yaml = r#"
ruleset:
  id: fraud_detection
  name: Fraud Detection Ruleset
  rules:
    - high_amount_transaction
    - new_device_login
  decision_logic:
    - condition: total_score > 200
      action: deny
      reason: High risk score
      terminate: true
    - condition: total_score > 100
      action: review
      reason: Medium risk score
    - default: true
      action: approve
"#;

        let ruleset = RulesetParser::parse(yaml).unwrap();

        assert_eq!(ruleset.id, "fraud_detection");
        assert_eq!(ruleset.name, Some("Fraud Detection Ruleset".to_string()));
        assert_eq!(ruleset.rules.len(), 2);
        assert_eq!(ruleset.decision_logic.len(), 3);
    }

    #[test]
    fn test_parse_ruleset_with_all_actions() {
        let yaml = r#"
ruleset:
  id: test_actions
  rules: []
  decision_logic:
    - condition: score > 300
      action: deny
    - condition: score > 200
      action: review
    - condition: score > 100
      action: infer
      config:
        data_snapshot:
          - user.id
          - event.type
    - default: true
      action: approve
"#;

        let ruleset = RulesetParser::parse(yaml).unwrap();

        assert_eq!(ruleset.decision_logic.len(), 4);

        // Check deny action
        assert!(matches!(ruleset.decision_logic[0].action, Action::Deny));

        // Check review action
        assert!(matches!(ruleset.decision_logic[1].action, Action::Review));

        // Check infer action
        if let Action::Infer { config } = &ruleset.decision_logic[2].action {
            assert_eq!(config.data_snapshot.len(), 2);
        } else {
            panic!("Expected Infer action");
        }

        // Check approve action
        assert!(matches!(ruleset.decision_logic[3].action, Action::Approve));
    }

    #[test]
    fn test_parse_ruleset_with_terminate() {
        let yaml = r#"
ruleset:
  id: test_terminate
  rules: []
  decision_logic:
    - condition: critical == true
      action: deny
      terminate: true
      reason: Critical condition met
"#;

        let ruleset = RulesetParser::parse(yaml).unwrap();

        assert_eq!(ruleset.decision_logic.len(), 1);
        assert!(ruleset.decision_logic[0].terminate);
        assert_eq!(
            ruleset.decision_logic[0].reason,
            Some("Critical condition met".to_string())
        );
    }

    #[test]
    fn test_parse_ruleset_default_rule() {
        let yaml = r#"
ruleset:
  id: test_default
  rules: []
  decision_logic:
    - default: true
      action: approve
"#;

        let ruleset = RulesetParser::parse(yaml).unwrap();

        assert_eq!(ruleset.decision_logic.len(), 1);
        assert!(ruleset.decision_logic[0].default);
        assert!(ruleset.decision_logic[0].condition.is_none());
    }

    #[test]
    fn test_missing_ruleset_id() {
        let yaml = r#"
ruleset:
  name: Test
  rules: []
  decision_logic: []
"#;

        let result = RulesetParser::parse(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_action() {
        let yaml = r#"
ruleset:
  id: test
  rules: []
  decision_logic:
    - action: unknown_action
"#;

        let result = RulesetParser::parse(yaml);
        assert!(result.is_err());
    }
}
