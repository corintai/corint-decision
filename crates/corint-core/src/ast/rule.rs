//! Rule AST definitions

use super::expression::Expression;
use serde::{Deserialize, Serialize};

/// Rule definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    /// Unique rule ID
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Optional parameters for parameterized rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<RuleParams>,

    /// When conditions (when this rule should be evaluated)
    pub when: WhenBlock,

    /// Score to add if rule is triggered
    pub score: i32,

    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Parameters for parameterized rules
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleParams {
    /// Parameter definitions with default values
    #[serde(flatten)]
    pub values: std::collections::HashMap<String, serde_json::Value>,
}

/// When block for rule evaluation conditions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhenBlock {
    /// Event type filter (e.g., "login", "transaction")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,

    /// Condition group (all/any/not) - new format
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub condition_group: Option<ConditionGroup>,

    /// List of conditions (legacy format, for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Expression>>,
}

/// Condition group with logical operators
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConditionGroup {
    /// All conditions must be true (AND)
    All(Vec<Condition>),
    /// At least one condition must be true (OR)
    Any(Vec<Condition>),
    /// Negation of a condition group
    Not(Vec<Condition>),
}

/// A condition can be either an expression or a nested condition group
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Condition {
    /// A simple expression
    Expression(Expression),
    /// A nested condition group
    Group(Box<ConditionGroup>),
}

impl Rule {
    /// Create a new rule
    pub fn new(id: String, name: String, when: WhenBlock, score: i32) -> Self {
        Rule {
            id,
            name,
            description: None,
            params: None,
            when,
            score,
            metadata: None,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the parameters
    pub fn with_params(mut self, params: RuleParams) -> Self {
        self.params = Some(params);
        self
    }

    /// Set the metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

impl RuleParams {
    /// Create new empty parameters
    pub fn new() -> Self {
        Self {
            values: std::collections::HashMap::new(),
        }
    }

    /// Create parameters from a HashMap
    pub fn from_map(values: std::collections::HashMap<String, serde_json::Value>) -> Self {
        Self { values }
    }

    /// Add a parameter
    pub fn with_param(mut self, key: String, value: serde_json::Value) -> Self {
        self.values.insert(key, value);
        self
    }
}

impl Default for RuleParams {
    fn default() -> Self {
        Self::new()
    }
}

impl WhenBlock {
    /// Create a new when block
    pub fn new() -> Self {
        WhenBlock {
            event_type: None,
            condition_group: None,
            conditions: None,
        }
    }

    /// Set the event type filter
    pub fn with_event_type(mut self, event_type: String) -> Self {
        self.event_type = Some(event_type);
        self
    }

    /// Add a condition (legacy format)
    pub fn add_condition(mut self, condition: Expression) -> Self {
        if let Some(ref mut conditions) = self.conditions {
            conditions.push(condition);
        } else {
            self.conditions = Some(vec![condition]);
        }
        self
    }

    /// Add multiple conditions (legacy format)
    pub fn with_conditions(mut self, conditions: Vec<Expression>) -> Self {
        self.conditions = Some(conditions);
        self
    }

    /// Set condition group (new format)
    pub fn with_condition_group(mut self, group: ConditionGroup) -> Self {
        self.condition_group = Some(group);
        self
    }

    /// Get all conditions as a flat list (for backward compatibility)
    pub fn get_conditions(&self) -> Vec<&Expression> {
        let mut result = Vec::new();

        // First try legacy format
        if let Some(ref conditions) = self.conditions {
            for expr in conditions {
                result.push(expr);
            }
        }

        // Then try new format
        if let Some(ref group) = self.condition_group {
            self.collect_expressions(group, &mut result);
        }

        result
    }

    /// Recursively collect all expressions from condition groups
    fn collect_expressions<'a>(&self, group: &'a ConditionGroup, result: &mut Vec<&'a Expression>) {
        let conditions = match group {
            ConditionGroup::All(conds) | ConditionGroup::Any(conds) | ConditionGroup::Not(conds) => conds,
        };

        for condition in conditions {
            match condition {
                Condition::Expression(expr) => result.push(expr),
                Condition::Group(nested_group) => self.collect_expressions(nested_group, result),
            }
        }
    }
}

impl ConditionGroup {
    /// Create a new "all" condition group
    pub fn all(conditions: Vec<Condition>) -> Self {
        ConditionGroup::All(conditions)
    }

    /// Create a new "any" condition group
    pub fn any(conditions: Vec<Condition>) -> Self {
        ConditionGroup::Any(conditions)
    }

    /// Create a new "not" condition group
    pub fn not(conditions: Vec<Condition>) -> Self {
        ConditionGroup::Not(conditions)
    }
}

impl Condition {
    /// Create a condition from an expression
    pub fn from_expression(expr: Expression) -> Self {
        Condition::Expression(expr)
    }

    /// Create a condition from a condition group
    pub fn from_group(group: ConditionGroup) -> Self {
        Condition::Group(Box::new(group))
    }
}

impl Default for WhenBlock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::operator::Operator;
    use crate::types::Value;

    #[test]
    fn test_rule_creation() {
        let when = WhenBlock::new()
            .with_event_type("login".to_string())
            .add_condition(Expression::binary(
                Expression::field_access(vec!["user".to_string(), "age".to_string()]),
                Operator::Gt,
                Expression::literal(Value::Number(18.0)),
            ));

        let rule = Rule::new("test_rule".to_string(), "Test Rule".to_string(), when, 50);

        assert_eq!(rule.id, "test_rule");
        assert_eq!(rule.name, "Test Rule");
        assert_eq!(rule.score, 50);
        assert!(rule.description.is_none());
    }

    #[test]
    fn test_rule_with_description() {
        let when = WhenBlock::new();
        let rule = Rule::new("test_rule".to_string(), "Test Rule".to_string(), when, 50)
            .with_description("This is a test rule".to_string());

        assert_eq!(rule.description, Some("This is a test rule".to_string()));
    }

    #[test]
    fn test_when_block_with_multiple_conditions() {
        let when = WhenBlock::new()
            .with_event_type("transaction".to_string())
            .add_condition(Expression::binary(
                Expression::field_access(vec!["amount".to_string()]),
                Operator::Gt,
                Expression::literal(Value::Number(1000.0)),
            ))
            .add_condition(Expression::binary(
                Expression::field_access(vec!["country".to_string()]),
                Operator::In,
                Expression::literal(Value::Array(vec![
                    Value::String("RU".to_string()),
                    Value::String("NG".to_string()),
                ])),
            ));

        assert_eq!(when.event_type, Some("transaction".to_string()));
        assert_eq!(when.conditions.as_ref().map_or(0, |c| c.len()), 2);
    }

    #[test]
    fn test_complex_rule() {
        // Rule: Detect high-risk login
        // - Event type: login
        // - Conditions:
        //   1. user.age > 60
        //   2. device.is_new == true
        //   3. geo.country in ["RU", "NG"]

        let when = WhenBlock::new()
            .with_event_type("login".to_string())
            .with_conditions(vec![
                Expression::binary(
                    Expression::field_access(vec!["user".to_string(), "age".to_string()]),
                    Operator::Gt,
                    Expression::literal(Value::Number(60.0)),
                ),
                Expression::binary(
                    Expression::field_access(vec!["device".to_string(), "is_new".to_string()]),
                    Operator::Eq,
                    Expression::literal(Value::Bool(true)),
                ),
                Expression::binary(
                    Expression::field_access(vec!["geo".to_string(), "country".to_string()]),
                    Operator::In,
                    Expression::literal(Value::Array(vec![
                        Value::String("RU".to_string()),
                        Value::String("NG".to_string()),
                    ])),
                ),
            ]);

        let rule = Rule::new(
            "high_risk_login".to_string(),
            "High Risk Login Detection".to_string(),
            when,
            80,
        )
        .with_description("Detects high-risk login attempts".to_string());

        assert_eq!(rule.id, "high_risk_login");
        assert_eq!(rule.score, 80);
        assert_eq!(rule.when.conditions.as_ref().map_or(0, |c| c.len()), 3);
        assert_eq!(rule.when.event_type, Some("login".to_string()));
    }

    #[test]
    fn test_rule_clone() {
        let when = WhenBlock::new().with_event_type("test".to_string());
        let rule = Rule::new("id".to_string(), "name".to_string(), when, 10);

        let cloned = rule.clone();
        assert_eq!(rule, cloned);
    }
}
