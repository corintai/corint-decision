//! Runtime value types for CORINT expressions
//!
//! The `Value` enum represents all possible runtime values in CORINT,
//! similar to JSON values but with additional type safety.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Runtime value type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Number value (f64 for simplicity, handles both int and float)
    Number(f64),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<Value>),
    /// Object (key-value map)
    Object(HashMap<String, Value>),
}

// TODO: Implement methods - will be driven by tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_null() {
        let val = Value::Null;
        assert_eq!(val, Value::Null);
    }

    #[test]
    fn test_value_bool() {
        let val_true = Value::Bool(true);
        let val_false = Value::Bool(false);

        assert_eq!(val_true, Value::Bool(true));
        assert_eq!(val_false, Value::Bool(false));
        assert_ne!(val_true, val_false);
    }

    #[test]
    fn test_value_number() {
        let val = Value::Number(42.0);
        assert_eq!(val, Value::Number(42.0));

        let val2 = Value::Number(3.5);
        assert_eq!(val2, Value::Number(3.5));
    }

    #[test]
    fn test_value_string() {
        let val = Value::String("hello".to_string());
        assert_eq!(val, Value::String("hello".to_string()));
    }

    #[test]
    fn test_value_array() {
        let val = Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]);

        assert_eq!(
            val,
            Value::Array(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
            ])
        );
    }

    #[test]
    fn test_value_object() {
        let mut map = HashMap::new();
        map.insert("name".to_string(), Value::String("Alice".to_string()));
        map.insert("age".to_string(), Value::Number(25.0));

        let val = Value::Object(map.clone());
        assert_eq!(val, Value::Object(map));
    }

    #[test]
    fn test_value_nested() {
        let user = Value::Object({
            let mut map = HashMap::new();
            map.insert("name".to_string(), Value::String("Bob".to_string()));
            map.insert("age".to_string(), Value::Number(30.0));
            map.insert("is_active".to_string(), Value::Bool(true));
            map
        });

        match &user {
            Value::Object(map) => {
                assert_eq!(map.get("name"), Some(&Value::String("Bob".to_string())));
                assert_eq!(map.get("age"), Some(&Value::Number(30.0)));
                assert_eq!(map.get("is_active"), Some(&Value::Bool(true)));
            }
            _ => panic!("Expected Object"),
        }
    }

    #[test]
    fn test_value_serde_json() {
        // Test serialization
        let val = Value::Object({
            let mut map = HashMap::new();
            map.insert("count".to_string(), Value::Number(42.0));
            map.insert("active".to_string(), Value::Bool(true));
            map
        });

        let json = serde_json::to_string(&val).unwrap();
        assert!(json.contains("count"));
        assert!(json.contains("42"));

        // Test deserialization
        let deserialized: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val, deserialized);
    }
}
