//! Ruleset parser
//!
//! Parses YAML ruleset definitions into Ruleset AST nodes.

use crate::error::{ParseError, Result};
use crate::expression_parser::ExpressionParser;
use crate::import_parser::ImportParser;
use crate::yaml_parser::YamlParser;
use corint_core::ast::{DecisionRule, RdlDocument, Ruleset, Signal};
use serde_yaml::Value as YamlValue;

/// Ruleset parser
pub struct RulesetParser;

impl RulesetParser {
    /// Parse a ruleset from YAML string (legacy format, no imports)
    ///
    /// This maintains backward compatibility with existing code.
    pub fn parse(yaml_str: &str) -> Result<Ruleset> {
        let yaml = YamlParser::parse(yaml_str)?;
        Self::parse_from_yaml(&yaml)
    }

    /// Parse a ruleset with optional imports from YAML string (new format)
    ///
    /// Supports both formats:
    /// 1. Legacy single-document format (backward compatible)
    /// 2. New multi-document format with imports
    ///
    /// Returns an RdlDocument<Ruleset> containing both the ruleset and its imports (if any)
    pub fn parse_with_imports(yaml_str: &str) -> Result<RdlDocument<Ruleset>> {
        let (imports, definition_yaml) = ImportParser::parse_with_imports(yaml_str)?;

        // Parse the ruleset from the definition document
        let ruleset = Self::parse_from_yaml(&definition_yaml)?;

        // Get version (default to "0.1" if not specified)
        let version = YamlParser::get_optional_string(&definition_yaml, "version")
            .unwrap_or_else(|| "0.1".to_string());

        // Create RdlDocument
        if let Some(imports) = imports {
            Ok(RdlDocument::with_imports(version, imports, ruleset))
        } else {
            Ok(RdlDocument::new(version, ruleset))
        }
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
        let rules =
            if let Some(rules_array) = ruleset_obj.get("rules").and_then(|v| v.as_sequence()) {
                rules_array
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                Vec::new()
            };

        // Parse optional extends
        let extends = YamlParser::get_optional_string(ruleset_obj, "extends");

        // Parse optional description
        let description = YamlParser::get_optional_string(ruleset_obj, "description");

        // Parse optional metadata
        let metadata = ruleset_obj
            .get("metadata")
            .and_then(|v| serde_yaml::from_value(v.clone()).ok());

        // Parse conclusion (ruleset's decision rules)
        let conclusion = if let Some(logic_array) = ruleset_obj
            .get("conclusion")
            .and_then(|v| v.as_sequence())
        {
            logic_array
                .iter()
                .map(Self::parse_decision_rule)
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        Ok(Ruleset {
            id,
            name,
            extends,
            rules,
            conclusion,
            description,
            metadata,
        })
    }

    /// Parse a decision rule (public for template parser use)
    pub fn parse_decision_rule(yaml: &YamlValue) -> Result<DecisionRule> {
        // Parse when condition (optional)
        let condition = YamlParser::get_optional_string(yaml, "when")
            .map(|s| ExpressionParser::parse(&s))
            .transpose()?;

        // Parse default flag
        let default = YamlParser::get_optional_bool(yaml, "default").unwrap_or(false);

        // Parse signal (the decision result: approve/decline/review/hold/pass)
        let signal = Self::parse_signal(yaml)?;

        // Parse actions (user-defined actions like KYC_AUTH, OTP, NOTIFY_USER)
        let actions = Self::parse_actions(yaml);

        // Parse reason (optional)
        let reason = YamlParser::get_optional_string(yaml, "reason");

        Ok(DecisionRule {
            condition,
            default,
            signal,
            actions,
            reason,
        })
    }

    /// Parse a signal (decision result: approve/decline/review/hold/pass)
    fn parse_signal(yaml: &YamlValue) -> Result<Signal> {
        // Support both "signal" and "action" fields for backward compatibility
        let signal_str = YamlParser::get_optional_string(yaml, "signal")
            .or_else(|| YamlParser::get_optional_string(yaml, "action"))
            .ok_or_else(|| ParseError::MissingField {
                field: "signal (or action)".to_string(),
            })?;

        match signal_str.as_str() {
            "approve" => Ok(Signal::Approve),
            "decline" => Ok(Signal::Decline),
            // Keep "deny" as alias for backward compatibility
            "deny" => Ok(Signal::Decline),
            "review" => Ok(Signal::Review),
            "hold" => Ok(Signal::Hold),
            // Keep "challenge" as alias for backward compatibility
            "challenge" => Ok(Signal::Hold),
            "pass" => Ok(Signal::Pass),
            _ => Err(ParseError::InvalidValue {
                field: "signal".to_string(),
                message: format!("Unknown signal type: {}. Valid signals: approve, decline, review, hold, pass", signal_str),
            }),
        }
    }

    /// Parse user-defined actions (e.g., ["KYC_AUTH", "OTP", "NOTIFY_USER"])
    fn parse_actions(yaml: &YamlValue) -> Vec<String> {
        if let Some(actions_array) = yaml.get("actions").and_then(|v| v.as_sequence()) {
            actions_array
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        } else {
            Vec::new()
        }
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
  conclusion:
    - when: total_score > 200
      action: decline
      reason: High risk score
    - when: total_score > 100
      action: review
      reason: Medium risk score
    - default: true
      action: approve
"#;

        let ruleset = RulesetParser::parse(yaml).unwrap();

        assert_eq!(ruleset.id, "fraud_detection");
        assert_eq!(ruleset.name, Some("Fraud Detection Ruleset".to_string()));
        assert_eq!(ruleset.rules.len(), 2);
        assert_eq!(ruleset.conclusion.len(), 3);
    }

    #[test]
    fn test_parse_ruleset_with_all_signals() {
        let yaml = r#"
ruleset:
  id: test_signals
  rules: []
  conclusion:
    - when: score > 300
      signal: decline
      actions:
        - BLOCK_CARD
        - NOTIFY_USER
    - when: score > 200
      signal: review
      actions:
        - KYC_AUTH
    - when: score > 150
      signal: hold
    - when: score > 50
      signal: pass
    - default: true
      signal: approve
"#;

        let ruleset = RulesetParser::parse(yaml).unwrap();

        assert_eq!(ruleset.conclusion.len(), 5);

        // Check decline signal with actions
        assert!(matches!(ruleset.conclusion[0].signal, Signal::Decline));
        assert_eq!(ruleset.conclusion[0].actions, vec!["BLOCK_CARD", "NOTIFY_USER"]);

        // Check review signal with actions
        assert!(matches!(ruleset.conclusion[1].signal, Signal::Review));
        assert_eq!(ruleset.conclusion[1].actions, vec!["KYC_AUTH"]);

        // Check hold signal
        assert!(matches!(ruleset.conclusion[2].signal, Signal::Hold));
        assert!(ruleset.conclusion[2].actions.is_empty());

        // Check pass signal
        assert!(matches!(ruleset.conclusion[3].signal, Signal::Pass));

        // Check approve signal
        assert!(matches!(ruleset.conclusion[4].signal, Signal::Approve));
    }

    #[test]
    fn test_parse_backward_compatible_action_field() {
        // Test that "action" field still works for backward compatibility
        let yaml = r#"
ruleset:
  id: test_backward_compat
  rules: []
  conclusion:
    - when: score > 100
      action: decline
    - default: true
      action: approve
"#;

        let ruleset = RulesetParser::parse(yaml).unwrap();

        assert_eq!(ruleset.conclusion.len(), 2);
        assert!(matches!(ruleset.conclusion[0].signal, Signal::Decline));
        assert!(matches!(ruleset.conclusion[1].signal, Signal::Approve));
    }


    #[test]
    fn test_parse_ruleset_default_rule() {
        let yaml = r#"
ruleset:
  id: test_default
  rules: []
  conclusion:
    - default: true
      action: approve
"#;

        let ruleset = RulesetParser::parse(yaml).unwrap();

        assert_eq!(ruleset.conclusion.len(), 1);
        assert!(ruleset.conclusion[0].default);
        assert!(ruleset.conclusion[0].condition.is_none());
    }

    #[test]
    fn test_missing_ruleset_id() {
        let yaml = r#"
ruleset:
  name: Test
  rules: []
  conclusion: []
"#;

        let result = RulesetParser::parse(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_signal() {
        let yaml = r#"
ruleset:
  id: test
  rules: []
  conclusion:
    - signal: unknown_signal
"#;

        let result = RulesetParser::parse(yaml);
        assert!(result.is_err());
    }
}
