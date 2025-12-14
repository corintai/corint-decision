//! Event data validation
//!
//! Validates that user-submitted event data doesn't contain reserved fields
//! that could conflict with system-generated data.
//!
//! # Purpose
//!
//! This module ensures data integrity by preventing users from submitting event data
//! that contains field names reserved for internal use. This prevents conflicts between:
//! - User-submitted event data (event namespace)
//! - System-generated metadata (sys namespace)
//! - Computed features (features namespace)
//! - External API results (api namespace)
//! - Internal service results (service namespace)
//! - LLM analysis results (llm namespace)
//!
//! # Validation Rules
//!
//! ## Reserved Fields
//!
//! The following field names are completely reserved and cannot appear in event data:
//! - `total_score` - Reserved for aggregated risk score
//! - `triggered_rules` - Reserved for list of triggered rules
//! - `triggered_count` - Reserved for count of triggered rules
//! - `action` - Reserved for final decision action
//! - `explanation` - Reserved for decision explanation
//! - `context` - Reserved for execution context metadata
//!
//! ## Reserved Prefixes
//!
//! Field names starting with these prefixes are also forbidden:
//! - `sys_` - Reserved for system namespace fields
//! - `features_` - Reserved for computed features
//! - `api_` - Reserved for external API results
//! - `service_` - Reserved for service call results
//! - `llm_` - Reserved for LLM analysis results
//!
//! ## Nested Validation
//!
//! Validation is recursive - nested objects are also checked for reserved fields.
//! This ensures that reserved field names cannot be hidden in nested structures.
//!
//! # Integration
//!
//! Validation is automatically enforced when creating an ExecutionContext:
//! - `ExecutionContext::new()` and `ExecutionContext::from_event()` validate event data
//! - `ExecutionContext::with_result()` also validates event data
//! - If validation fails, a `RuntimeError::ReservedField` error is returned
//!
//! # Examples
//!
//! ```rust
//! use std::collections::HashMap;
//! use corint_core::Value;
//! use corint_runtime::ExecutionContext;
//!
//! // Valid event data
//! let mut valid_event = HashMap::new();
//! valid_event.insert("user_id".to_string(), Value::String("123".to_string()));
//! valid_event.insert("transaction_amount".to_string(), Value::Number(1000.0));
//! assert!(ExecutionContext::from_event(valid_event).is_ok());
//!
//! // Invalid - contains reserved field
//! let mut invalid_event = HashMap::new();
//! invalid_event.insert("total_score".to_string(), Value::Number(100.0));
//! assert!(ExecutionContext::from_event(invalid_event).is_err());
//!
//! // Invalid - uses reserved prefix
//! let mut invalid_prefix = HashMap::new();
//! invalid_prefix.insert("sys_custom".to_string(), Value::String("test".to_string()));
//! assert!(ExecutionContext::from_event(invalid_prefix).is_err());
//! ```

use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashMap;

/// Reserved field names that cannot be submitted by users in event data
const RESERVED_FIELDS: &[&str] = &[
    // System result fields
    "total_score",
    "triggered_rules",
    "triggered_count",
    "action",
    "explanation",
    "context",
];

/// Reserved namespace prefixes
const RESERVED_PREFIXES: &[&str] = &[
    "sys_",      // System namespace
    "features_", // Features namespace
    "api_",      // API namespace
    "service_",  // Service namespace
    "llm_",      // LLM namespace
];

/// Validates event data doesn't contain reserved fields
///
/// # Arguments
/// * `event_data` - The event data HashMap to validate
///
/// # Returns
/// * `Ok(())` if validation passes
/// * `Err(RuntimeError::ReservedField)` if a reserved field is found
///
/// # Example
/// ```
/// use std::collections::HashMap;
/// use corint_core::Value;
/// use corint_runtime::validation::validate_event_data;
///
/// let mut event = HashMap::new();
/// event.insert("user_id".to_string(), Value::String("123".to_string()));
/// event.insert("amount".to_string(), Value::Number(100.0));
///
/// // This should pass
/// assert!(validate_event_data(&event).is_ok());
///
/// // This should fail - reserved field
/// event.insert("total_score".to_string(), Value::Number(50.0));
/// assert!(validate_event_data(&event).is_err());
/// ```
pub fn validate_event_data(event_data: &HashMap<String, Value>) -> Result<()> {
    for key in event_data.keys() {
        // Check exact matches with reserved fields
        if RESERVED_FIELDS.contains(&key.as_str()) {
            return Err(RuntimeError::ReservedField {
                field: key.clone(),
                reason: format!("'{}' is a system-reserved field", key),
            });
        }

        // Check reserved prefixes
        for prefix in RESERVED_PREFIXES {
            if key.starts_with(prefix) {
                return Err(RuntimeError::ReservedField {
                    field: key.clone(),
                    reason: format!(
                        "'{}' starts with reserved prefix '{}' which is reserved for system use",
                        key, prefix
                    ),
                });
            }
        }

        // Recursively validate nested objects
        if let Value::Object(nested) = event_data.get(key).unwrap() {
            validate_nested_object(nested, key)?;
        }
    }

    Ok(())
}

/// Validates nested objects don't contain reserved field names
fn validate_nested_object(obj: &HashMap<String, Value>, parent_path: &str) -> Result<()> {
    for (key, value) in obj.iter() {
        let full_path = format!("{}.{}", parent_path, key);

        // Check reserved fields at any level
        if RESERVED_FIELDS.contains(&key.as_str()) {
            return Err(RuntimeError::ReservedField {
                field: full_path.clone(),
                reason: format!("'{}' contains reserved field '{}'", full_path, key),
            });
        }

        // Check reserved prefixes
        for prefix in RESERVED_PREFIXES {
            if key.starts_with(prefix) {
                return Err(RuntimeError::ReservedField {
                    field: full_path.clone(),
                    reason: format!(
                        "'{}' contains field starting with reserved prefix '{}'",
                        full_path, prefix
                    ),
                });
            }
        }

        // Recursively validate deeper nesting
        if let Value::Object(nested) = value {
            validate_nested_object(nested, &full_path)?;
        }
    }

    Ok(())
}

/// Checks if a field name is reserved
///
/// This is a convenience function for checking individual field names.
pub fn is_reserved_field(field_name: &str) -> bool {
    // Check exact matches
    if RESERVED_FIELDS.contains(&field_name) {
        return true;
    }

    // Check prefixes
    for prefix in RESERVED_PREFIXES {
        if field_name.starts_with(prefix) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_valid_event() -> HashMap<String, Value> {
        let mut event = HashMap::new();
        event.insert("user_id".to_string(), Value::String("123".to_string()));
        event.insert("amount".to_string(), Value::Number(1000.0));

        let mut user = HashMap::new();
        user.insert("age".to_string(), Value::Number(25.0));
        user.insert("country".to_string(), Value::String("US".to_string()));
        event.insert("user".to_string(), Value::Object(user));

        event
    }

    #[test]
    fn test_valid_event_data() {
        let event = create_valid_event();
        assert!(validate_event_data(&event).is_ok());
    }

    #[test]
    fn test_reserved_field_total_score() {
        let mut event = create_valid_event();
        event.insert("total_score".to_string(), Value::Number(50.0));

        let result = validate_event_data(&event);
        assert!(result.is_err());
        match result {
            Err(RuntimeError::ReservedField { field, .. }) => {
                assert_eq!(field, "total_score");
            }
            _ => panic!("Expected ReservedField error"),
        }
    }

    #[test]
    fn test_reserved_field_triggered_rules() {
        let mut event = create_valid_event();
        event.insert(
            "triggered_rules".to_string(),
            Value::Array(vec![Value::String("rule1".to_string())]),
        );

        assert!(validate_event_data(&event).is_err());
    }

    #[test]
    fn test_reserved_prefix_sys() {
        let mut event = create_valid_event();
        event.insert("sys_custom_field".to_string(), Value::String("test".to_string()));

        let result = validate_event_data(&event);
        assert!(result.is_err());
        match result {
            Err(RuntimeError::ReservedField { field, .. }) => {
                assert_eq!(field, "sys_custom_field");
            }
            _ => panic!("Expected ReservedField error"),
        }
    }

    #[test]
    fn test_reserved_prefix_features() {
        let mut event = create_valid_event();
        event.insert(
            "features_count".to_string(),
            Value::Number(10.0),
        );

        assert!(validate_event_data(&event).is_err());
    }

    #[test]
    fn test_reserved_prefix_api() {
        let mut event = create_valid_event();
        event.insert("api_result".to_string(), Value::String("data".to_string()));

        assert!(validate_event_data(&event).is_err());
    }

    #[test]
    fn test_nested_reserved_field() {
        let mut event = create_valid_event();

        let mut nested = HashMap::new();
        nested.insert("total_score".to_string(), Value::Number(50.0));
        event.insert("metadata".to_string(), Value::Object(nested));

        let result = validate_event_data(&event);
        assert!(result.is_err());
        match result {
            Err(RuntimeError::ReservedField { field, .. }) => {
                assert_eq!(field, "metadata.total_score");
            }
            _ => panic!("Expected ReservedField error"),
        }
    }

    #[test]
    fn test_nested_reserved_prefix() {
        let mut event = create_valid_event();

        let mut nested = HashMap::new();
        nested.insert("sys_id".to_string(), Value::String("123".to_string()));
        event.insert("data".to_string(), Value::Object(nested));

        let result = validate_event_data(&event);
        assert!(result.is_err());
        match result {
            Err(RuntimeError::ReservedField { field, .. }) => {
                assert_eq!(field, "data.sys_id");
            }
            _ => panic!("Expected ReservedField error"),
        }
    }

    #[test]
    fn test_is_reserved_field() {
        assert!(is_reserved_field("total_score"));
        assert!(is_reserved_field("triggered_rules"));
        assert!(is_reserved_field("sys_request_id"));
        assert!(is_reserved_field("features_count"));
        assert!(is_reserved_field("api_result"));
        assert!(is_reserved_field("service_data"));
        assert!(is_reserved_field("llm_analysis"));

        assert!(!is_reserved_field("user_id"));
        assert!(!is_reserved_field("amount"));
        assert!(!is_reserved_field("system"));  // "system" is ok, "sys_" is not
        assert!(!is_reserved_field("feature"));  // "feature" is ok, "features_" is not
    }

    #[test]
    fn test_deeply_nested_validation() {
        let mut event = HashMap::new();

        let mut level1 = HashMap::new();
        let mut level2 = HashMap::new();
        let mut level3 = HashMap::new();

        level3.insert("sys_deep_field".to_string(), Value::String("test".to_string()));
        level2.insert("level3".to_string(), Value::Object(level3));
        level1.insert("level2".to_string(), Value::Object(level2));
        event.insert("level1".to_string(), Value::Object(level1));

        let result = validate_event_data(&event);
        assert!(result.is_err());
        match result {
            Err(RuntimeError::ReservedField { field, .. }) => {
                assert_eq!(field, "level1.level2.level3.sys_deep_field");
            }
            _ => panic!("Expected ReservedField error"),
        }
    }

    #[test]
    fn test_all_reserved_fields() {
        for reserved in RESERVED_FIELDS {
            let mut event = HashMap::new();
            event.insert(reserved.to_string(), Value::Null);

            assert!(
                validate_event_data(&event).is_err(),
                "Field '{}' should be reserved",
                reserved
            );
        }
    }

    #[test]
    fn test_all_reserved_prefixes() {
        for prefix in RESERVED_PREFIXES {
            let mut event = HashMap::new();
            let field_name = format!("{}test", prefix);
            event.insert(field_name.clone(), Value::Null);

            assert!(
                validate_event_data(&event).is_err(),
                "Field starting with '{}' should be reserved",
                prefix
            );
        }
    }
}
