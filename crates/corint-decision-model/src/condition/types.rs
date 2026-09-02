//! Condition types for when clause parsing

use crate::ast::operator::Operator;
use crate::types::Value;
use serde::{Deserialize, Serialize};

/// A when clause that can be simple (single condition) or complex (all/any/not)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WhenClause {
    /// Single condition expression (e.g., "type == \"transaction\"")
    Simple(String),
    /// Complex condition with logical operators
    Complex(WhenClauseComplex),
}

/// Complex when clause with all/any/not logic
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WhenClauseComplex {
    /// All conditions must be true (AND logic)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all: Option<Vec<WhenClauseItem>>,
    /// At least one condition must be true (OR logic)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub any: Option<Vec<WhenClauseItem>>,
    /// Negation of conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not: Option<Vec<WhenClauseItem>>,
}

/// An item in a when clause can be a simple string or a nested complex clause
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WhenClauseItem {
    /// Simple condition string
    Simple(String),
    /// Nested complex condition
    Complex(WhenClauseComplex),
}

/// A parsed condition ready for evaluation or SQL generation
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCondition {
    /// Field path (e.g., "user_id" or "attributes.device.fingerprint")
    pub field: String,
    /// Comparison operator
    pub operator: Operator,
    /// Value to compare against
    pub value: ParsedValue,
}

/// A parsed value that may contain a template reference
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedValue {
    /// Static literal value
    Literal(Value),
    /// Template variable reference (e.g., "{event.user_id}")
    Template {
        /// Full template path (e.g., "event.user_id")
        path: String,
        /// Resolved value after substitution (set during evaluation)
        resolved: Option<Value>,
    },
}

impl ParsedValue {
    /// Create a literal value
    pub fn literal(value: Value) -> Self {
        ParsedValue::Literal(value)
    }

    /// Create a template reference
    pub fn template(path: String) -> Self {
        ParsedValue::Template {
            path,
            resolved: None,
        }
    }

    /// Get the resolved value (either literal or resolved template)
    pub fn get_value(&self) -> Option<&Value> {
        match self {
            ParsedValue::Literal(v) => Some(v),
            ParsedValue::Template { resolved, .. } => resolved.as_ref(),
        }
    }

    /// Check if this is a template that needs resolution
    pub fn is_template(&self) -> bool {
        matches!(self, ParsedValue::Template { .. })
    }

    /// Resolve a template value with context
    pub fn resolve(&mut self, value: Value) {
        if let ParsedValue::Template { resolved, .. } = self {
            *resolved = Some(value);
        }
    }

    /// Convert to a Value, panicking if template is unresolved
    pub fn to_value(&self) -> Value {
        match self {
            ParsedValue::Literal(v) => v.clone(),
            ParsedValue::Template { resolved, path } => {
                resolved.clone().unwrap_or_else(|| {
                    panic!("Template '{}' was not resolved", path)
                })
            }
        }
    }

    /// Try to convert to a Value, returning None if template is unresolved
    pub fn try_to_value(&self) -> Option<Value> {
        match self {
            ParsedValue::Literal(v) => Some(v.clone()),
            ParsedValue::Template { resolved, .. } => resolved.clone(),
        }
    }
}

impl ParsedCondition {
    /// Create a new parsed condition
    pub fn new(field: String, operator: Operator, value: ParsedValue) -> Self {
        Self { field, operator, value }
    }

    /// Check if this condition has a template value that needs resolution
    pub fn needs_resolution(&self) -> bool {
        self.value.is_template() && self.value.get_value().is_none()
    }
}

/// Logical grouping of parsed conditions
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedConditionGroup {
    /// All conditions must be true (AND)
    All(Vec<ParsedConditionItem>),
    /// At least one condition must be true (OR)
    Any(Vec<ParsedConditionItem>),
    /// Negation of all conditions (NOT ALL)
    Not(Vec<ParsedConditionItem>),
}

/// An item in a parsed condition group
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedConditionItem {
    /// Single condition
    Condition(ParsedCondition),
    /// Nested group
    Group(ParsedConditionGroup),
}

impl ParsedConditionGroup {
    /// Create an All group
    pub fn all(items: Vec<ParsedConditionItem>) -> Self {
        ParsedConditionGroup::All(items)
    }

    /// Create an Any group
    pub fn any(items: Vec<ParsedConditionItem>) -> Self {
        ParsedConditionGroup::Any(items)
    }

    /// Create a Not group
    pub fn not(items: Vec<ParsedConditionItem>) -> Self {
        ParsedConditionGroup::Not(items)
    }

    /// Get all conditions in this group (flattened)
    pub fn all_conditions(&self) -> Vec<&ParsedCondition> {
        let mut result = Vec::new();
        self.collect_conditions(&mut result);
        result
    }

    fn collect_conditions<'a>(&'a self, result: &mut Vec<&'a ParsedCondition>) {
        let items = match self {
            ParsedConditionGroup::All(items)
            | ParsedConditionGroup::Any(items)
            | ParsedConditionGroup::Not(items) => items,
        };

        for item in items {
            match item {
                ParsedConditionItem::Condition(c) => result.push(c),
                ParsedConditionItem::Group(g) => g.collect_conditions(result),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_value_literal() {
        let val = ParsedValue::literal(Value::String("test".to_string()));
        assert!(!val.is_template());
        assert_eq!(val.get_value(), Some(&Value::String("test".to_string())));
    }

    #[test]
    fn test_parsed_value_template() {
        let mut val = ParsedValue::template("event.user_id".to_string());
        assert!(val.is_template());
        assert_eq!(val.get_value(), None);

        val.resolve(Value::String("user123".to_string()));
        assert_eq!(val.get_value(), Some(&Value::String("user123".to_string())));
    }

    #[test]
    fn test_when_clause_serde() {
        // Test simple when clause
        let simple: WhenClause = serde_json::from_str(r#""type == \"transaction\"""#).unwrap();
        assert!(matches!(simple, WhenClause::Simple(_)));

        // Test complex when clause
        let complex: WhenClause = serde_json::from_str(r#"{
            "all": ["type == \"transaction\"", "amount > 100"]
        }"#).unwrap();
        assert!(matches!(complex, WhenClause::Complex(_)));
    }
}
