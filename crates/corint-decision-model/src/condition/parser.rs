//! Condition string parser
//!
//! Parses condition strings like:
//! - `type == "transaction"`
//! - `amount > 100`
//! - `user_id == "{event.user_id}"`
//! - `attributes.risk_level in ["high", "critical"]`

use super::types::{
    ParsedCondition, ParsedConditionGroup, ParsedConditionItem, ParsedValue,
    WhenClause, WhenClauseComplex, WhenClauseItem,
};
use crate::ast::operator::Operator;
use crate::types::Value;
use std::collections::HashMap;

/// Condition parser that handles string parsing and template resolution
#[derive(Debug, Default)]
pub struct ConditionParser {
    /// Context for template variable resolution
    context: HashMap<String, Value>,
}

/// Parse error
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub condition: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse '{}': {}", self.condition, self.message)
    }
}

impl std::error::Error for ParseError {}

impl ConditionParser {
    /// Create a new parser
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a parser with context for template resolution
    pub fn with_context(context: HashMap<String, Value>) -> Self {
        Self { context }
    }

    /// Set the context for template resolution
    pub fn set_context(&mut self, context: HashMap<String, Value>) {
        self.context = context;
    }

    /// Parse a WhenClause into a ParsedConditionGroup
    pub fn parse_when_clause(&self, when: &WhenClause) -> Result<ParsedConditionGroup, ParseError> {
        match when {
            WhenClause::Simple(expr) => {
                let condition = self.parse_condition(expr)?;
                Ok(ParsedConditionGroup::All(vec![ParsedConditionItem::Condition(condition)]))
            }
            WhenClause::Complex(complex) => self.parse_complex(complex),
        }
    }

    /// Parse a complex when clause
    fn parse_complex(&self, complex: &WhenClauseComplex) -> Result<ParsedConditionGroup, ParseError> {
        // Prioritize 'all', then 'any', then 'not'
        if let Some(all) = &complex.all {
            let items = self.parse_items(all)?;
            Ok(ParsedConditionGroup::All(items))
        } else if let Some(any) = &complex.any {
            let items = self.parse_items(any)?;
            Ok(ParsedConditionGroup::Any(items))
        } else if let Some(not) = &complex.not {
            let items = self.parse_items(not)?;
            Ok(ParsedConditionGroup::Not(items))
        } else {
            // Empty complex clause
            Ok(ParsedConditionGroup::All(vec![]))
        }
    }

    /// Parse a list of when clause items
    fn parse_items(&self, items: &[WhenClauseItem]) -> Result<Vec<ParsedConditionItem>, ParseError> {
        let mut result = Vec::new();
        for item in items {
            match item {
                WhenClauseItem::Simple(expr) => {
                    let condition = self.parse_condition(expr)?;
                    result.push(ParsedConditionItem::Condition(condition));
                }
                WhenClauseItem::Complex(nested) => {
                    let group = self.parse_complex(nested)?;
                    result.push(ParsedConditionItem::Group(group));
                }
            }
        }
        Ok(result)
    }

    /// Parse a single condition string into a ParsedCondition
    ///
    /// Supported formats:
    /// - `field == "value"`
    /// - `field > 100`
    /// - `field != null`
    /// - `field in ["a", "b"]`
    /// - `field contains "substr"`
    pub fn parse_condition(&self, condition: &str) -> Result<ParsedCondition, ParseError> {
        let condition = condition.trim();
        if condition.is_empty() {
            return Err(ParseError {
                message: "Empty condition".to_string(),
                condition: condition.to_string(),
            });
        }

        // Operators to check, ordered by length (longer first to avoid partial matches)
        let operators = [
            // Two-character operators first
            ("!=", Operator::Ne),
            (">=", Operator::Ge),
            ("<=", Operator::Le),
            ("==", Operator::Eq),
            // Single-character operators
            (">", Operator::Gt),
            ("<", Operator::Lt),
            // Word operators (with spaces)
            (" not in ", Operator::NotIn),
            (" in ", Operator::In),
            (" contains ", Operator::Contains),
            (" starts_with ", Operator::StartsWith),
            (" ends_with ", Operator::EndsWith),
            (" matches ", Operator::Regex),
        ];

        for (op_str, op) in operators.iter() {
            if let Some(pos) = condition.find(op_str) {
                let field = condition[..pos].trim().to_string();
                let value_str = condition[pos + op_str.len()..].trim();

                if field.is_empty() {
                    return Err(ParseError {
                        message: "Empty field name".to_string(),
                        condition: condition.to_string(),
                    });
                }

                let value = self.parse_value(value_str)?;

                return Ok(ParsedCondition::new(field, *op, value));
            }
        }

        Err(ParseError {
            message: "No operator found".to_string(),
            condition: condition.to_string(),
        })
    }

    /// Parse a value string into a ParsedValue
    ///
    /// Supported formats:
    /// - Quoted strings: `"value"` or `'value'`
    /// - Numbers: `100`, `3.14`, `-42`
    /// - Booleans: `true`, `false`
    /// - Null: `null`, `nil`
    /// - Arrays: `["a", "b", 1, 2]`
    /// - Templates: `{event.user_id}` or `"{event.user_id}"`
    pub fn parse_value(&self, value_str: &str) -> Result<ParsedValue, ParseError> {
        let value_str = value_str.trim();

        // Check for template variable: {context.field}
        if value_str.starts_with('{') && value_str.ends_with('}') {
            let path = value_str[1..value_str.len() - 1].to_string();
            return Ok(self.resolve_template(&path));
        }

        // Check for quoted string
        if (value_str.starts_with('"') && value_str.ends_with('"'))
            || (value_str.starts_with('\'') && value_str.ends_with('\''))
        {
            let unquoted = &value_str[1..value_str.len() - 1];

            // Check for template inside quotes: "{event.field}"
            if unquoted.starts_with('{') && unquoted.ends_with('}') {
                let path = unquoted[1..unquoted.len() - 1].to_string();
                return Ok(self.resolve_template(&path));
            }

            return Ok(ParsedValue::literal(Value::String(unquoted.to_string())));
        }

        // Check for boolean
        if value_str == "true" {
            return Ok(ParsedValue::literal(Value::Bool(true)));
        }
        if value_str == "false" {
            return Ok(ParsedValue::literal(Value::Bool(false)));
        }

        // Check for null
        if value_str == "null" || value_str == "nil" {
            return Ok(ParsedValue::literal(Value::Null));
        }

        // Check for array: ["a", "b"]
        if value_str.starts_with('[') && value_str.ends_with(']') {
            let array_content = &value_str[1..value_str.len() - 1];
            let elements = self.parse_array_elements(array_content)?;
            return Ok(ParsedValue::literal(Value::Array(elements)));
        }

        // Try to parse as number
        if let Ok(num) = value_str.parse::<f64>() {
            return Ok(ParsedValue::literal(Value::Number(num)));
        }

        // If all else fails, treat as unquoted string
        Ok(ParsedValue::literal(Value::String(value_str.to_string())))
    }

    /// Resolve a template variable from context
    fn resolve_template(&self, path: &str) -> ParsedValue {
        // Try to resolve from context
        // Path format: "event.field" or "context.field.subfield"
        let parts: Vec<&str> = path.split('.').collect();

        // Try the last part of the path as the key (simple resolution)
        if let Some(key) = parts.last() {
            if let Some(value) = self.context.get(*key) {
                let mut parsed = ParsedValue::template(path.to_string());
                parsed.resolve(value.clone());
                return parsed;
            }
        }

        // Try the full path as key
        if let Some(value) = self.context.get(path) {
            let mut parsed = ParsedValue::template(path.to_string());
            parsed.resolve(value.clone());
            return parsed;
        }

        // Try nested resolution for paths like "event.user_id"
        if parts.len() >= 2 {
            // Skip the first part (usually "event", "context", etc.)
            let field_key = parts[1..].join(".");
            if let Some(value) = self.context.get(&field_key) {
                let mut parsed = ParsedValue::template(path.to_string());
                parsed.resolve(value.clone());
                return parsed;
            }
        }

        // Return unresolved template
        ParsedValue::template(path.to_string())
    }

    /// Parse array elements from a string like `"a", "b", 1, 2`
    fn parse_array_elements(&self, content: &str) -> Result<Vec<Value>, ParseError> {
        let mut elements = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        let mut string_char = '"';

        for c in content.chars() {
            match c {
                '"' | '\'' if !in_string => {
                    in_string = true;
                    string_char = c;
                    current.push(c);
                }
                c if c == string_char && in_string => {
                    in_string = false;
                    current.push(c);
                }
                ',' if !in_string => {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        let value = self.parse_value(trimmed)?;
                        if let Some(v) = value.try_to_value() {
                            elements.push(v);
                        }
                    }
                    current.clear();
                }
                _ => {
                    current.push(c);
                }
            }
        }

        // Handle last element
        let trimmed = current.trim();
        if !trimmed.is_empty() {
            let value = self.parse_value(trimmed)?;
            if let Some(v) = value.try_to_value() {
                elements.push(v);
            }
        }

        Ok(elements)
    }

    /// Parse multiple condition strings (all/and logic)
    pub fn parse_all(&self, conditions: &[String]) -> Result<ParsedConditionGroup, ParseError> {
        let items: Result<Vec<_>, _> = conditions
            .iter()
            .map(|c| self.parse_condition(c).map(ParsedConditionItem::Condition))
            .collect();
        Ok(ParsedConditionGroup::All(items?))
    }

    /// Parse multiple condition strings (any/or logic)
    pub fn parse_any(&self, conditions: &[String]) -> Result<ParsedConditionGroup, ParseError> {
        let items: Result<Vec<_>, _> = conditions
            .iter()
            .map(|c| self.parse_condition(c).map(ParsedConditionItem::Condition))
            .collect();
        Ok(ParsedConditionGroup::Any(items?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_eq() {
        let parser = ConditionParser::new();
        let result = parser.parse_condition(r#"type == "transaction""#).unwrap();

        assert_eq!(result.field, "type");
        assert_eq!(result.operator, Operator::Eq);
        assert_eq!(result.value.to_value(), Value::String("transaction".to_string()));
    }

    #[test]
    fn test_parse_numeric_gt() {
        let parser = ConditionParser::new();
        let result = parser.parse_condition("amount > 100").unwrap();

        assert_eq!(result.field, "amount");
        assert_eq!(result.operator, Operator::Gt);
        assert_eq!(result.value.to_value(), Value::Number(100.0));
    }

    #[test]
    fn test_parse_not_eq() {
        let parser = ConditionParser::new();
        let result = parser.parse_condition("status != null").unwrap();

        assert_eq!(result.field, "status");
        assert_eq!(result.operator, Operator::Ne);
        assert_eq!(result.value.to_value(), Value::Null);
    }

    #[test]
    fn test_parse_in_operator() {
        let parser = ConditionParser::new();
        let result = parser.parse_condition(r#"country in ["US", "CA"]"#).unwrap();

        assert_eq!(result.field, "country");
        assert_eq!(result.operator, Operator::In);
        assert_eq!(
            result.value.to_value(),
            Value::Array(vec![
                Value::String("US".to_string()),
                Value::String("CA".to_string())
            ])
        );
    }

    #[test]
    fn test_parse_template_variable() {
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("user123".to_string()));

        let parser = ConditionParser::with_context(context);
        let result = parser.parse_condition(r#"user_id == "{event.user_id}""#).unwrap();

        assert_eq!(result.field, "user_id");
        assert_eq!(result.operator, Operator::Eq);
        assert_eq!(result.value.to_value(), Value::String("user123".to_string()));
    }

    #[test]
    fn test_parse_unresolved_template() {
        let parser = ConditionParser::new();
        let result = parser.parse_condition(r#"user_id == {event.user_id}"#).unwrap();

        assert!(result.value.is_template());
        assert!(result.needs_resolution());
    }

    #[test]
    fn test_parse_contains() {
        let parser = ConditionParser::new();
        let result = parser.parse_condition(r#"email contains "@example.com""#).unwrap();

        assert_eq!(result.field, "email");
        assert_eq!(result.operator, Operator::Contains);
        assert_eq!(result.value.to_value(), Value::String("@example.com".to_string()));
    }

    #[test]
    fn test_parse_nested_field() {
        let parser = ConditionParser::new();
        let result = parser.parse_condition(r#"attributes.risk_level == "high""#).unwrap();

        assert_eq!(result.field, "attributes.risk_level");
        assert_eq!(result.operator, Operator::Eq);
    }

    #[test]
    fn test_parse_when_clause_simple() {
        let parser = ConditionParser::new();
        let when = WhenClause::Simple(r#"type == "transaction""#.to_string());
        let result = parser.parse_when_clause(&when).unwrap();

        match result {
            ParsedConditionGroup::All(items) => {
                assert_eq!(items.len(), 1);
            }
            _ => panic!("Expected All group"),
        }
    }

    #[test]
    fn test_parse_when_clause_complex() {
        let parser = ConditionParser::new();
        let when = WhenClause::Complex(WhenClauseComplex {
            all: Some(vec![
                WhenClauseItem::Simple(r#"type == "transaction""#.to_string()),
                WhenClauseItem::Simple("amount > 100".to_string()),
            ]),
            any: None,
            not: None,
        });
        let result = parser.parse_when_clause(&when).unwrap();

        match result {
            ParsedConditionGroup::All(items) => {
                assert_eq!(items.len(), 2);
            }
            _ => panic!("Expected All group"),
        }
    }

    #[test]
    fn test_parse_error_no_operator() {
        let parser = ConditionParser::new();
        let result = parser.parse_condition("invalid condition");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_boolean_value() {
        let parser = ConditionParser::new();

        let result = parser.parse_condition("is_verified == true").unwrap();
        assert_eq!(result.value.to_value(), Value::Bool(true));

        let result = parser.parse_condition("is_active == false").unwrap();
        assert_eq!(result.value.to_value(), Value::Bool(false));
    }
}
