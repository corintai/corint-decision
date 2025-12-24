//! Ruleset AST definitions
//!
//! A Ruleset contains multiple rules and defines decision logic
//! based on the combined rule scores.

use crate::ast::Expression;
use serde::{Deserialize, Serialize};

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
    pub conclusion: Vec<DecisionRule>,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A decision rule determines the signal and actions based on conditions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionRule {
    /// Condition to evaluate (e.g., total_score > 100)
    pub condition: Option<Expression>,

    /// Whether this is the default rule (catch-all)
    #[serde(default)]
    pub default: bool,

    /// Signal to emit if condition matches (approve/decline/review/hold/pass)
    pub signal: Signal,

    /// User-defined actions to execute (e.g., ["KYC_AUTH", "OTP", "NOTIFY_USER"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,

    /// Optional reason for this decision
    pub reason: Option<String>,

    /// Whether to terminate decision flow after this rule
    #[serde(default)]
    pub terminate: bool,
}

/// Decision signal (the decision result)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Signal {
    /// Approve the request
    Approve,

    /// Decline the request
    Decline,

    /// Send for manual review
    Review,

    /// Hold - temporarily suspend, waiting for additional verification or async process
    /// (e.g., 2FA challenge, KYC verification, cooling period)
    Hold,

    /// Pass - skip/no decision, let downstream handle
    Pass,
}

impl Ruleset {
    /// Create a new ruleset
    pub fn new(id: String) -> Self {
        Self {
            id,
            name: None,
            extends: None,
            rules: Vec::new(),
            conclusion: Vec::new(),
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
        self.conclusion.push(decision_rule);
        self
    }

    /// Add multiple decision rules
    pub fn with_conclusion(mut self, conclusion: Vec<DecisionRule>) -> Self {
        self.conclusion = conclusion;
        self
    }
}

impl DecisionRule {
    /// Create a new decision rule with a condition
    pub fn new(condition: Expression, signal: Signal) -> Self {
        Self {
            condition: Some(condition),
            default: false,
            signal,
            actions: Vec::new(),
            reason: None,
            terminate: false,
        }
    }

    /// Create a default (catch-all) decision rule
    pub fn default(signal: Signal) -> Self {
        Self {
            condition: None,
            default: true,
            signal,
            actions: Vec::new(),
            reason: None,
            terminate: false,
        }
    }

    /// Set user-defined actions
    pub fn with_actions(mut self, actions: Vec<String>) -> Self {
        self.actions = actions;
        self
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

        let decision = DecisionRule::new(condition.clone(), Signal::Decline)
            .with_actions(vec!["BLOCK_CARD".to_string()])
            .with_reason("Score too high".to_string())
            .with_terminate(true);

        assert!(decision.condition.is_some());
        assert_eq!(decision.signal, Signal::Decline);
        assert_eq!(decision.actions, vec!["BLOCK_CARD".to_string()]);
        assert_eq!(decision.reason, Some("Score too high".to_string()));
        assert!(decision.terminate);
        assert!(!decision.default);
    }

    #[test]
    fn test_default_decision_rule() {
        let decision = DecisionRule::default(Signal::Approve);

        assert!(decision.condition.is_none());
        assert!(decision.default);
        assert_eq!(decision.signal, Signal::Approve);
        assert!(decision.actions.is_empty());
        assert!(!decision.terminate);
    }

    #[test]
    fn test_signal_types() {
        let approve = Signal::Approve;
        let decline = Signal::Decline;
        let review = Signal::Review;
        let hold = Signal::Hold;
        let pass = Signal::Pass;

        assert!(matches!(approve, Signal::Approve));
        assert!(matches!(decline, Signal::Decline));
        assert!(matches!(review, Signal::Review));
        assert!(matches!(hold, Signal::Hold));
        assert!(matches!(pass, Signal::Pass));
    }

    #[test]
    fn test_complete_ruleset_with_conclusion() {
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
                    Signal::Decline,
                )
                .with_actions(vec!["BLOCK_CARD".to_string(), "NOTIFY_USER".to_string()])
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
                    Signal::Review,
                )
                .with_actions(vec!["KYC_AUTH".to_string()])
                .with_reason("Medium risk score".to_string()),
            )
            .add_decision_rule(DecisionRule::default(Signal::Approve));

        assert_eq!(ruleset.rules.len(), 3);
        assert_eq!(ruleset.conclusion.len(), 3);

        // First rule: Decline if score > 200
        assert_eq!(ruleset.conclusion[0].signal, Signal::Decline);
        assert_eq!(ruleset.conclusion[0].actions, vec!["BLOCK_CARD", "NOTIFY_USER"]);
        assert!(ruleset.conclusion[0].terminate);

        // Second rule: Review if score > 100
        assert_eq!(ruleset.conclusion[1].signal, Signal::Review);
        assert_eq!(ruleset.conclusion[1].actions, vec!["KYC_AUTH"]);
        assert!(!ruleset.conclusion[1].terminate);

        // Third rule: Default to Approve
        assert_eq!(ruleset.conclusion[2].signal, Signal::Approve);
        assert!(ruleset.conclusion[2].actions.is_empty());
        assert!(ruleset.conclusion[2].default);
    }

    #[test]
    fn test_ruleset_serde() {
        let ruleset = Ruleset::new("test".to_string())
            .with_name("Test Ruleset".to_string())
            .add_rule("rule_1".to_string())
            .add_decision_rule(DecisionRule::default(Signal::Approve));

        // Serialize to JSON
        let json = serde_json::to_string(&ruleset).unwrap();
        assert!(json.contains("\"id\":\"test\""));

        // Deserialize back
        let deserialized: Ruleset = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ruleset);
    }
}
