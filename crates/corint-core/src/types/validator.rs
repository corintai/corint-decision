//! Value validation against schemas

use super::schema::{FieldType, Schema};
use super::value::Value;
use thiserror::Error;

/// Validation error
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Type mismatch
    #[error("Type mismatch for field '{field}': expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },

    /// Required field missing
    #[error("Required field missing: {field}")]
    RequiredFieldMissing { field: String },

    /// Unknown field
    #[error("Unknown field: {field}")]
    UnknownField { field: String },

    /// Array item validation failed
    #[error("Array item validation failed at index {index}: {message}")]
    ArrayItemError { index: usize, message: String },

    /// Nested object validation failed
    #[error("Nested object validation failed for field '{field}': {message}")]
    NestedObjectError { field: String, message: String },
}

/// Validator for values against schemas
pub struct Validator {
    /// Whether to allow unknown fields
    allow_unknown_fields: bool,
}

impl Validator {
    /// Create a new validator with default settings
    pub fn new() -> Self {
        Self {
            allow_unknown_fields: false,
        }
    }

    /// Allow unknown fields in validation
    pub fn allow_unknown_fields(mut self, allow: bool) -> Self {
        self.allow_unknown_fields = allow;
        self
    }

    /// Validate a value against a schema
    pub fn validate(&self, value: &Value, schema: &Schema) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Value must be an object for schema validation
        let obj = match value {
            Value::Object(obj) => obj,
            _ => {
                errors.push(ValidationError::TypeMismatch {
                    field: "root".to_string(),
                    expected: "object".to_string(),
                    actual: self.get_value_type_name(value).to_string(),
                });
                return Err(errors);
            }
        };

        // Check required fields
        for (field_name, field) in &schema.fields {
            if field.required && !obj.contains_key(field_name) {
                errors.push(ValidationError::RequiredFieldMissing {
                    field: field_name.clone(),
                });
            }
        }

        // Validate each field in the value
        for (field_name, field_value) in obj {
            match schema.get_field(field_name) {
                Some(schema_field) => {
                    // Validate field type
                    if let Err(err) =
                        self.validate_field(field_name, field_value, &schema_field.field_type)
                    {
                        errors.push(err);
                    }
                }
                None => {
                    // Unknown field
                    if !self.allow_unknown_fields {
                        errors.push(ValidationError::UnknownField {
                            field: field_name.clone(),
                        });
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate a single field
    fn validate_field(
        &self,
        field_name: &str,
        value: &Value,
        field_type: &FieldType,
    ) -> Result<(), ValidationError> {
        match field_type {
            FieldType::Null => {
                if !matches!(value, Value::Null) {
                    return Err(ValidationError::TypeMismatch {
                        field: field_name.to_string(),
                        expected: "null".to_string(),
                        actual: self.get_value_type_name(value).to_string(),
                    });
                }
            }

            FieldType::Boolean => {
                if !matches!(value, Value::Bool(_)) {
                    return Err(ValidationError::TypeMismatch {
                        field: field_name.to_string(),
                        expected: "boolean".to_string(),
                        actual: self.get_value_type_name(value).to_string(),
                    });
                }
            }

            FieldType::Number => {
                if !matches!(value, Value::Number(_)) {
                    return Err(ValidationError::TypeMismatch {
                        field: field_name.to_string(),
                        expected: "number".to_string(),
                        actual: self.get_value_type_name(value).to_string(),
                    });
                }
            }

            FieldType::String => {
                if !matches!(value, Value::String(_)) {
                    return Err(ValidationError::TypeMismatch {
                        field: field_name.to_string(),
                        expected: "string".to_string(),
                        actual: self.get_value_type_name(value).to_string(),
                    });
                }
            }

            FieldType::Array { item_type } => {
                if let Value::Array(items) = value {
                    // Validate each item
                    for (index, item) in items.iter().enumerate() {
                        if let Err(err) = self.validate_field("item", item, item_type) {
                            return Err(ValidationError::ArrayItemError {
                                index,
                                message: err.to_string(),
                            });
                        }
                    }
                } else {
                    return Err(ValidationError::TypeMismatch {
                        field: field_name.to_string(),
                        expected: "array".to_string(),
                        actual: self.get_value_type_name(value).to_string(),
                    });
                }
            }

            FieldType::Object { schema } => {
                if !matches!(value, Value::Object(_)) {
                    return Err(ValidationError::TypeMismatch {
                        field: field_name.to_string(),
                        expected: "object".to_string(),
                        actual: self.get_value_type_name(value).to_string(),
                    });
                }

                // If schema is provided, validate nested object
                if let Some(nested_schema) = schema {
                    if let Err(errors) = self.validate(value, nested_schema) {
                        return Err(ValidationError::NestedObjectError {
                            field: field_name.to_string(),
                            message: format!("{} validation errors", errors.len()),
                        });
                    }
                }
            }

            FieldType::Any => {
                // Any type - no validation needed
            }
        }

        Ok(())
    }

    /// Get the type name of a value
    fn get_value_type_name(&self, value: &Value) -> &str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::schema::SchemaField;
    use std::collections::HashMap;

    #[test]
    fn test_valid_object() {
        let schema = Schema::new("User".to_string())
            .add_field(SchemaField::new("id".to_string(), FieldType::String).required())
            .add_field(SchemaField::new("age".to_string(), FieldType::Number));

        let mut obj = HashMap::new();
        obj.insert("id".to_string(), Value::String("123".to_string()));
        obj.insert("age".to_string(), Value::Number(25.0));

        let value = Value::Object(obj);
        let validator = Validator::new();

        assert!(validator.validate(&value, &schema).is_ok());
    }

    #[test]
    fn test_missing_required_field() {
        let schema = Schema::new("User".to_string())
            .add_field(SchemaField::new("id".to_string(), FieldType::String).required());

        let obj = HashMap::new();
        let value = Value::Object(obj);
        let validator = Validator::new();

        let result = validator.validate(&value, &schema);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0],
            ValidationError::RequiredFieldMissing { .. }
        ));
    }

    #[test]
    fn test_type_mismatch() {
        let schema = Schema::new("User".to_string())
            .add_field(SchemaField::new("age".to_string(), FieldType::Number));

        let mut obj = HashMap::new();
        obj.insert("age".to_string(), Value::String("not a number".to_string()));

        let value = Value::Object(obj);
        let validator = Validator::new();

        let result = validator.validate(&value, &schema);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);

        if let ValidationError::TypeMismatch {
            field,
            expected,
            actual,
        } = &errors[0]
        {
            assert_eq!(field, "age");
            assert_eq!(expected, "number");
            assert_eq!(actual, "string");
        } else {
            panic!("Expected TypeMismatch error");
        }
    }

    #[test]
    fn test_unknown_field() {
        let schema = Schema::new("User".to_string())
            .add_field(SchemaField::new("id".to_string(), FieldType::String));

        let mut obj = HashMap::new();
        obj.insert("id".to_string(), Value::String("123".to_string()));
        obj.insert("unknown".to_string(), Value::Number(42.0));

        let value = Value::Object(obj);

        // By default, unknown fields are not allowed
        let validator = Validator::new();
        let result = validator.validate(&value, &schema);
        assert!(result.is_err());

        // Allow unknown fields
        let validator = Validator::new().allow_unknown_fields(true);
        let result = validator.validate(&value, &schema);
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_validation() {
        let schema = Schema::new("Data".to_string()).add_field(SchemaField::new(
            "numbers".to_string(),
            FieldType::array(FieldType::Number),
        ));

        // Valid array
        let mut obj = HashMap::new();
        obj.insert(
            "numbers".to_string(),
            Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]),
        );

        let value = Value::Object(obj);
        let validator = Validator::new();
        assert!(validator.validate(&value, &schema).is_ok());

        // Invalid array item
        let mut obj = HashMap::new();
        obj.insert(
            "numbers".to_string(),
            Value::Array(vec![
                Value::Number(1.0),
                Value::String("not a number".to_string()),
            ]),
        );

        let value = Value::Object(obj);
        let result = validator.validate(&value, &schema);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(matches!(errors[0], ValidationError::ArrayItemError { .. }));
    }

    #[test]
    fn test_nested_object_validation() {
        // Create nested schema
        let address_schema = Schema::new("Address".to_string())
            .add_field(SchemaField::new("street".to_string(), FieldType::String).required())
            .add_field(SchemaField::new("city".to_string(), FieldType::String).required());

        let user_schema = Schema::new("User".to_string()).add_field(SchemaField::new(
            "address".to_string(),
            FieldType::object_with_schema(address_schema),
        ));

        // Valid nested object
        let mut address = HashMap::new();
        address.insert(
            "street".to_string(),
            Value::String("123 Main St".to_string()),
        );
        address.insert("city".to_string(), Value::String("Boston".to_string()));

        let mut user = HashMap::new();
        user.insert("address".to_string(), Value::Object(address));

        let value = Value::Object(user);
        let validator = Validator::new();
        assert!(validator.validate(&value, &user_schema).is_ok());

        // Invalid nested object (missing required field)
        let mut address = HashMap::new();
        address.insert(
            "street".to_string(),
            Value::String("123 Main St".to_string()),
        );
        // Missing 'city'

        let mut user = HashMap::new();
        user.insert("address".to_string(), Value::Object(address));

        let value = Value::Object(user);
        let result = validator.validate(&value, &user_schema);
        assert!(result.is_err());
    }

    #[test]
    fn test_not_an_object() {
        let schema = Schema::new("Test".to_string());
        let value = Value::String("not an object".to_string());
        let validator = Validator::new();

        let result = validator.validate(&value, &schema);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::TypeMismatch { .. }));
    }

    #[test]
    fn test_all_field_types() {
        let schema = Schema::new("AllTypes".to_string())
            .add_field(SchemaField::new("null_field".to_string(), FieldType::Null))
            .add_field(SchemaField::new(
                "bool_field".to_string(),
                FieldType::Boolean,
            ))
            .add_field(SchemaField::new("num_field".to_string(), FieldType::Number))
            .add_field(SchemaField::new("str_field".to_string(), FieldType::String))
            .add_field(SchemaField::new(
                "arr_field".to_string(),
                FieldType::array(FieldType::String),
            ))
            .add_field(SchemaField::new(
                "obj_field".to_string(),
                FieldType::object(),
            ))
            .add_field(SchemaField::new("any_field".to_string(), FieldType::Any));

        let mut obj = HashMap::new();
        obj.insert("null_field".to_string(), Value::Null);
        obj.insert("bool_field".to_string(), Value::Bool(true));
        obj.insert("num_field".to_string(), Value::Number(42.0));
        obj.insert("str_field".to_string(), Value::String("hello".to_string()));
        obj.insert(
            "arr_field".to_string(),
            Value::Array(vec![Value::String("a".to_string())]),
        );
        obj.insert("obj_field".to_string(), Value::Object(HashMap::new()));
        obj.insert("any_field".to_string(), Value::Number(123.0)); // Can be anything

        let value = Value::Object(obj);
        let validator = Validator::new();
        assert!(validator.validate(&value, &schema).is_ok());
    }
}
