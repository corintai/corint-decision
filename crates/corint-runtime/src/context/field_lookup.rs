//! Field Lookup Utilities
//!
//! Helper functions for navigating and retrieving values from nested data structures
//! using dot-notation paths.

use crate::error::Result;
use corint_core::Value;
use std::collections::HashMap;

/// Get nested value from a HashMap following a path
///
/// Navigates through nested HashMap/Value::Object structures following the given path.
/// Returns Value::Null if any part of the path is not found (graceful handling).
///
/// # Arguments
/// * `data` - The HashMap to search in
/// * `path` - Sequence of keys to follow (e.g., ["user", "profile", "email"])
///
/// # Returns
/// * `Ok(Value)` - The value found at the path, or Value::Null if not found
/// * `Err` - Only returns error for invalid operations (not for missing keys)
pub(super) fn get_nested_value(data: &HashMap<String, Value>, path: &[String]) -> Result<Value> {
    if path.is_empty() {
        return Ok(Value::Null);
    }

    let key = &path[0];
    let value = match data.get(key) {
        Some(v) => v,
        None => {
            tracing::debug!("Field not found: {}, returning Null", key);
            return Ok(Value::Null);
        }
    };

    if path.len() == 1 {
        return Ok(value.clone());
    }

    // Continue searching down
    match value {
        Value::Object(map) => {
            let remaining = &path[1..];
            let mut hash_map = HashMap::new();
            for (k, v) in map.iter() {
                hash_map.insert(k.clone(), v.clone());
            }
            get_nested_value(&hash_map, remaining)
        }
        _ => {
            tracing::debug!("Cannot access nested field on non-object, returning Null");
            Ok(Value::Null)
        }
    }
}

/// Navigate through a path in a Value, returning Null if any part is not found
///
/// This is a convenience wrapper around get_nested_value for Value::Object variants.
///
/// # Arguments
/// * `value` - The value to navigate (should be Value::Object)
/// * `path` - Sequence of keys to follow
///
/// # Returns
/// * The value at the path, or Value::Null if not found or value is not an object
#[allow(dead_code)]
pub(super) fn navigate_path(value: &Value, path: &[String]) -> Result<Value> {
    match value {
        Value::Object(map) => {
            let mut hash_map = HashMap::new();
            for (k, v) in map.iter() {
                hash_map.insert(k.clone(), v.clone());
            }
            get_nested_value(&hash_map, path)
        }
        _ => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> HashMap<String, Value> {
        let mut data = HashMap::new();

        // Simple value
        data.insert("name".to_string(), Value::String("Alice".to_string()));

        // Nested object
        let mut user = HashMap::new();
        user.insert("id".to_string(), Value::Number(123.0));
        user.insert("email".to_string(), Value::String("alice@example.com".to_string()));

        let mut profile = HashMap::new();
        profile.insert("age".to_string(), Value::Number(30.0));
        user.insert("profile".to_string(), Value::Object(profile));

        data.insert("user".to_string(), Value::Object(user));

        data
    }

    #[test]
    fn test_get_nested_value_simple() {
        let data = create_test_data();
        let path = vec!["name".to_string()];

        let result = get_nested_value(&data, &path).unwrap();
        assert_eq!(result, Value::String("Alice".to_string()));
    }

    #[test]
    fn test_get_nested_value_nested() {
        let data = create_test_data();
        let path = vec!["user".to_string(), "email".to_string()];

        let result = get_nested_value(&data, &path).unwrap();
        assert_eq!(result, Value::String("alice@example.com".to_string()));
    }

    #[test]
    fn test_get_nested_value_deep_nested() {
        let data = create_test_data();
        let path = vec!["user".to_string(), "profile".to_string(), "age".to_string()];

        let result = get_nested_value(&data, &path).unwrap();
        assert_eq!(result, Value::Number(30.0));
    }

    #[test]
    fn test_get_nested_value_not_found() {
        let data = create_test_data();
        let path = vec!["nonexistent".to_string()];

        let result = get_nested_value(&data, &path).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_get_nested_value_partial_path_not_found() {
        let data = create_test_data();
        let path = vec!["user".to_string(), "nonexistent".to_string()];

        let result = get_nested_value(&data, &path).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_get_nested_value_empty_path() {
        let data = create_test_data();
        let path = vec![];

        let result = get_nested_value(&data, &path).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_navigate_path_object() {
        let mut user = HashMap::new();
        user.insert("id".to_string(), Value::Number(123.0));
        let value = Value::Object(user);

        let path = vec!["id".to_string()];
        let result = navigate_path(&value, &path).unwrap();
        assert_eq!(result, Value::Number(123.0));
    }

    #[test]
    fn test_navigate_path_non_object() {
        let value = Value::String("not an object".to_string());
        let path = vec!["field".to_string()];

        let result = navigate_path(&value, &path).unwrap();
        assert_eq!(result, Value::Null);
    }
}
