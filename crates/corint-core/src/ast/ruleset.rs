//! Ruleset AST definitions
//!
//! A Ruleset contains multiple rules and defines decision logic
//! based on the combined rule scores.

use crate::ast::Expression;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A ruleset groups multiple rules and defines decision logic
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ruleset {
    /// Unique identifier for this ruleset
    pub id: String,

    /// Optional human-readable name
    pub name: Option<String>,

    /// Optional parent ruleset to extend from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,

    /// List of rule IDs included in this ruleset
    pub rules: Vec<String>,

    /// Decision logic rules (direct specification)
    pub decision_logic: Vec<DecisionRule>,

    /// Optional template-based decision logic (resolved at compile time)
    /// If specified, this takes precedence and gets expanded into decision_logic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_template: Option<DecisionTemplateRef>,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A decision rule determines the action based on conditions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionRule {
    /// Condition to evaluate (e.g., total_score > 100)
    pub condition: Option<Expression>,

    /// Whether this is the default rule (catch-all)
    #[serde(default)]
    pub default: bool,

    /// Action to take if condition matches
    pub action: Action,

    /// Optional reason for this decision
    pub reason: Option<String>,

    /// Whether to terminate decision flow after this rule
    #[serde(default)]
    pub terminate: bool,
}

/// Action to take based on decision
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Action {
    /// Approve the request
    Approve,

    /// Deny the request
    Deny,

    /// Send for manual review
    Review,

    /// Challenge - require additional verification (e.g., 3DS, MFA)
    Challenge,

    /// Use LLM to infer decision
    Infer {
        /// Configuration for LLM inference
        config: InferConfig,
    },
}

/// Configuration for LLM-based inference
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferConfig {
    /// Fields to include in the data snapshot for LLM
    pub data_snapshot: Vec<String>,
}

/// Reference to a decision template with parameter overrides
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionTemplateRef {
    /// Template ID to use
    pub template: String,

    /// Parameter overrides (merged with template defaults)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<HashMap<String, serde_json::Value>>,
}

impl Ruleset {
    /// Create a new ruleset
    pub fn new(id: String) -> Self {
        Self {
            id,
            name: None,
            extends: None,
            rules: Vec::new(),
            decision_logic: Vec::new(),
            decision_template: None,
            description: None,
            metadata: None,
        }
    }

    /// Set the name
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Set the parent ruleset to extend
    pub fn with_extends(mut self, extends: String) -> Self {
        self.extends = Some(extends);
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Add a rule ID
    pub fn add_rule(mut self, rule_id: String) -> Self {
        self.rules.push(rule_id);
        self
    }

    /// Add multiple rule IDs
    pub fn with_rules(mut self, rules: Vec<String>) -> Self {
        self.rules = rules;
        self
    }

    /// Add a decision rule
    pub fn add_decision_rule(mut self, decision_rule: DecisionRule) -> Self {
        self.decision_logic.push(decision_rule);
        self
    }

    /// Add multiple decision rules
    pub fn with_decision_logic(mut self, decision_logic: Vec<DecisionRule>) -> Self {
        self.decision_logic = decision_logic;
        self
    }

    /// Set decision template reference
    pub fn with_decision_template(mut self, template_ref: DecisionTemplateRef) -> Self {
        self.decision_template = Some(template_ref);
        self
    }
}

impl DecisionTemplateRef {
    /// Create a new template reference
    pub fn new(template: String) -> Self {
        Self {
            template,
            params: None,
        }
    }

    /// Add parameter overrides
    pub fn with_params(mut self, params: HashMap<String, serde_json::Value>) -> Self {
        self.params = Some(params);
        self
    }
}

impl DecisionRule {
    /// Create a new decision rule with a condition
    pub fn new(condition: Expression, action: Action) -> Self {
        Self {
            condition: Some(condition),
            default: false,
            action,
            reason: None,
            terminate: false,
        }
    }

    /// Create a default (catch-all) decision rule
    pub fn default(action: Action) -> Self {
        Self {
            condition: None,
            default: true,
            action,
            reason: None,
            terminate: false,
        }
    }

    /// Set the reason
    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    /// Set terminate flag
    pub fn with_terminate(mut self, terminate: bool) -> Self {
        self.terminate = terminate;
        self
    }
}

impl Action {
    /// Create an Infer action
    pub fn infer(data_snapshot: Vec<String>) -> Self {
        Self::Infer {
            config: InferConfig { data_snapshot },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expression, Operator};
    use crate::Value;

    #[test]
    fn test_ruleset_creation() {
        let ruleset = Ruleset::new("fraud_detection".to_string())
            .with_name("Fraud Detection Ruleset".to_string())
            .add_rule("rule_1".to_string())
            .add_rule("rule_2".to_string());

        assert_eq!(ruleset.id, "fraud_detection");
        assert_eq!(ruleset.name, Some("Fraud Detection Ruleset".to_string()));
        assert_eq!(ruleset.rules.len(), 2);
        assert_eq!(ruleset.rules[0], "rule_1");
    }

    #[test]
    fn test_decision_rule_with_condition() {
        // total_score > 100
        let condition = Expression::binary(
            Expression::field_access(vec!["total_score".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(100.0)),
        );

        let decision = DecisionRule::new(condition.clone(), Action::Deny)
            .with_reason("Score too high".to_string())
            .with_terminate(true);

        assert!(decision.condition.is_some());
        assert_eq!(decision.action, Action::Deny);
        assert_eq!(decision.reason, Some("Score too high".to_string()));
        assert!(decision.terminate);
        assert!(!decision.default);
    }

    #[test]
    fn test_default_decision_rule() {
        let decision = DecisionRule::default(Action::Approve);

        assert!(decision.condition.is_none());
        assert!(decision.default);
        assert_eq!(decision.action, Action::Approve);
        assert!(!decision.terminate);
    }

    #[test]
    fn test_action_types() {
        let approve = Action::Approve;
        let deny = Action::Deny;
        let review = Action::Review;
        let infer = Action::infer(vec!["user.id".to_string(), "event.type".to_string()]);

        assert!(matches!(approve, Action::Approve));
        assert!(matches!(deny, Action::Deny));
        assert!(matches!(review, Action::Review));

        if let Action::Infer { config } = infer {
            assert_eq!(config.data_snapshot.len(), 2);
            assert_eq!(config.data_snapshot[0], "user.id");
        } else {
            panic!("Expected Infer action");
        }
    }

    #[test]
    fn test_complete_ruleset_with_decision_logic() {
        // Create a complete ruleset with decision logic
        let ruleset = Ruleset::new("account_takeover".to_string())
            .with_name("Account Takeover Detection".to_string())
            .with_rules(vec![
                "new_device_login".to_string(),
                "unusual_location".to_string(),
                "failed_attempts".to_string(),
            ])
            .add_decision_rule(
                DecisionRule::new(
                    Expression::binary(
                        Expression::field_access(vec!["total_score".to_string()]),
                        Operator::Gt,
                        Expression::literal(Value::Number(200.0)),
                    ),
                    Action::Deny,
                )
                .with_reason("High risk score".to_string())
                .with_terminate(true),
            )
            .add_decision_rule(
                DecisionRule::new(
                    Expression::binary(
                        Expression::field_access(vec!["total_score".to_string()]),
                        Operator::Gt,
                        Expression::literal(Value::Number(100.0)),
                    ),
                    Action::Review,
                )
                .with_reason("Medium risk score".to_string()),
            )
            .add_decision_rule(DecisionRule::default(Action::Approve));

        assert_eq!(ruleset.rules.len(), 3);
        assert_eq!(ruleset.decision_logic.len(), 3);

        // First rule: Deny if score > 200
        assert_eq!(ruleset.decision_logic[0].action, Action::Deny);
        assert!(ruleset.decision_logic[0].terminate);

        // Second rule: Review if score > 100
        assert_eq!(ruleset.decision_logic[1].action, Action::Review);
        assert!(!ruleset.decision_logic[1].terminate);

        // Third rule: Default to Approve
        assert_eq!(ruleset.decision_logic[2].action, Action::Approve);
        assert!(ruleset.decision_logic[2].default);
    }

    #[test]
    fn test_ruleset_serde() {
        let ruleset = Ruleset::new("test".to_string())
            .with_name("Test Ruleset".to_string())
            .add_rule("rule_1".to_string())
            .add_decision_rule(DecisionRule::default(Action::Approve));

        // Serialize to JSON
        let json = serde_json::to_string(&ruleset).unwrap();
        assert!(json.contains("\"id\":\"test\""));

        // Deserialize back
        let deserialized: Ruleset = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ruleset);
    }
}
