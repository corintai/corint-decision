//! Schema definitions for runtime type validation
//!
//! Schemas define the expected structure and types of data.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A schema defines the structure and types of data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// Schema name
    pub name: String,

    /// Schema description
    pub description: Option<String>,

    /// Fields in the schema
    pub fields: HashMap<String, SchemaField>,
}

/// A field in a schema
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaField {
    /// Field name
    pub name: String,

    /// Field type
    pub field_type: FieldType,

    /// Whether this field is required
    #[serde(default)]
    pub required: bool,

    /// Optional description
    pub description: Option<String>,

    /// Default value (as JSON string)
    pub default: Option<String>,
}

/// Field type enumeration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    /// Null type
    Null,

    /// Boolean type
    Boolean,

    /// Number type (int or float)
    Number,

    /// String type
    String,

    /// Array type
    Array {
        /// Type of array elements
        item_type: Box<FieldType>,
    },

    /// Object type
    Object {
        /// Schema for the object (optional)
        schema: Option<Box<Schema>>,
    },

    /// Any type (no validation)
    Any,
}

impl Schema {
    /// Create a new schema
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            fields: HashMap::new(),
        }
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Add a field
    pub fn add_field(mut self, field: SchemaField) -> Self {
        self.fields.insert(field.name.clone(), field);
        self
    }

    /// Add multiple fields
    pub fn with_fields(mut self, fields: HashMap<String, SchemaField>) -> Self {
        self.fields = fields;
        self
    }

    /// Get a field by name
    pub fn get_field(&self, name: &str) -> Option<&SchemaField> {
        self.fields.get(name)
    }

    /// Check if a field is required
    pub fn is_required(&self, name: &str) -> bool {
        self.fields.get(name).map(|f| f.required).unwrap_or(false)
    }
}

impl SchemaField {
    /// Create a new field
    pub fn new(name: String, field_type: FieldType) -> Self {
        Self {
            name,
            field_type,
            required: false,
            description: None,
            default: None,
        }
    }

    /// Mark field as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set default value
    pub fn with_default(mut self, default: String) -> Self {
        self.default = Some(default);
        self
    }
}

impl FieldType {
    /// Create an array type
    pub fn array(item_type: FieldType) -> Self {
        FieldType::Array {
            item_type: Box::new(item_type),
        }
    }

    /// Create an object type
    pub fn object() -> Self {
        FieldType::Object { schema: None }
    }

    /// Create an object type with schema
    pub fn object_with_schema(schema: Schema) -> Self {
        FieldType::Object {
            schema: Some(Box::new(schema)),
        }
    }

    /// Check if this type is nullable
    pub fn is_nullable(&self) -> bool {
        matches!(self, FieldType::Null | FieldType::Any)
    }

    /// Get type name as string
    pub fn type_name(&self) -> &str {
        match self {
            FieldType::Null => "null",
            FieldType::Boolean => "boolean",
            FieldType::Number => "number",
            FieldType::String => "string",
            FieldType::Array { .. } => "array",
            FieldType::Object { .. } => "object",
            FieldType::Any => "any",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_creation() {
        let schema = Schema::new("User".to_string())
            .with_description("User schema".to_string())
            .add_field(SchemaField::new("id".to_string(), FieldType::String).required())
            .add_field(SchemaField::new("age".to_string(), FieldType::Number))
            .add_field(SchemaField::new("active".to_string(), FieldType::Boolean));

        assert_eq!(schema.name, "User");
        assert_eq!(schema.description, Some("User schema".to_string()));
        assert_eq!(schema.fields.len(), 3);
    }

    #[test]
    fn test_schema_field() {
        let field = SchemaField::new("email".to_string(), FieldType::String)
            .required()
            .with_description("User email address".to_string())
            .with_default("\"user@example.com\"".to_string());

        assert_eq!(field.name, "email");
        assert_eq!(field.field_type, FieldType::String);
        assert!(field.required);
        assert_eq!(field.description, Some("User email address".to_string()));
        assert_eq!(field.default, Some("\"user@example.com\"".to_string()));
    }

    #[test]
    fn test_field_types() {
        let null_type = FieldType::Null;
        let bool_type = FieldType::Boolean;
        let num_type = FieldType::Number;
        let str_type = FieldType::String;
        let any_type = FieldType::Any;

        assert_eq!(null_type.type_name(), "null");
        assert_eq!(bool_type.type_name(), "boolean");
        assert_eq!(num_type.type_name(), "number");
        assert_eq!(str_type.type_name(), "string");
        assert_eq!(any_type.type_name(), "any");

        assert!(null_type.is_nullable());
        assert!(any_type.is_nullable());
        assert!(!str_type.is_nullable());
    }

    #[test]
    fn test_array_type() {
        let array_type = FieldType::array(FieldType::Number);

        assert_eq!(array_type.type_name(), "array");

        if let FieldType::Array { item_type } = array_type {
            assert_eq!(*item_type, FieldType::Number);
        } else {
            panic!("Expected Array type");
        }
    }

    #[test]
    fn test_object_type() {
        let obj_type = FieldType::object();
        assert_eq!(obj_type.type_name(), "object");

        // Create object with schema
        let user_schema = Schema::new("User".to_string())
            .add_field(SchemaField::new("id".to_string(), FieldType::String));

        let obj_with_schema = FieldType::object_with_schema(user_schema.clone());

        if let FieldType::Object { schema } = obj_with_schema {
            assert!(schema.is_some());
            let schema = schema.unwrap();
            assert_eq!(schema.name, "User");
        } else {
            panic!("Expected Object type");
        }
    }

    #[test]
    fn test_nested_schema() {
        // Create Address schema
        let address_schema = Schema::new("Address".to_string())
            .add_field(SchemaField::new("street".to_string(), FieldType::String))
            .add_field(SchemaField::new("city".to_string(), FieldType::String))
            .add_field(SchemaField::new("zip".to_string(), FieldType::String));

        // Create User schema with nested Address
        let user_schema = Schema::new("User".to_string())
            .add_field(SchemaField::new("id".to_string(), FieldType::String).required())
            .add_field(SchemaField::new("name".to_string(), FieldType::String).required())
            .add_field(SchemaField::new(
                "address".to_string(),
                FieldType::object_with_schema(address_schema),
            ));

        assert_eq!(user_schema.fields.len(), 3);
        assert!(user_schema.is_required("id"));
        assert!(user_schema.is_required("name"));
        assert!(!user_schema.is_required("address"));
    }

    #[test]
    fn test_get_field() {
        let schema = Schema::new("Test".to_string())
            .add_field(SchemaField::new("field1".to_string(), FieldType::String));

        let field = schema.get_field("field1");
        assert!(field.is_some());
        assert_eq!(field.unwrap().name, "field1");

        let missing = schema.get_field("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_schema_serde() {
        let schema = Schema::new("User".to_string())
            .add_field(SchemaField::new("id".to_string(), FieldType::String).required())
            .add_field(SchemaField::new("age".to_string(), FieldType::Number));

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&schema).unwrap();

        // Check that the JSON contains the expected values
        assert!(json.contains("User"), "JSON should contain 'User'");
        assert!(json.contains("id"), "JSON should contain field 'id'");
        assert!(json.contains("age"), "JSON should contain field 'age'");

        // Deserialize back
        let deserialized: Schema = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, schema);
    }

    #[test]
    fn test_complex_schema() {
        // Create a complex event schema
        let event_schema = Schema::new("LoginEvent".to_string())
            .with_description("Schema for login events".to_string())
            .add_field(
                SchemaField::new("event_id".to_string(), FieldType::String)
                    .required()
                    .with_description("Unique event identifier".to_string()),
            )
            .add_field(
                SchemaField::new("user_id".to_string(), FieldType::String)
                    .required()
                    .with_description("User identifier".to_string()),
            )
            .add_field(
                SchemaField::new("timestamp".to_string(), FieldType::Number)
                    .required()
                    .with_description("Event timestamp (Unix)".to_string()),
            )
            .add_field(
                SchemaField::new("tags".to_string(), FieldType::array(FieldType::String))
                    .with_description("Event tags".to_string()),
            )
            .add_field(
                SchemaField::new("metadata".to_string(), FieldType::object())
                    .with_description("Additional metadata".to_string()),
            );

        assert_eq!(event_schema.name, "LoginEvent");
        assert_eq!(event_schema.fields.len(), 5);

        // Check required fields
        assert!(event_schema.is_required("event_id"));
        assert!(event_schema.is_required("user_id"));
        assert!(event_schema.is_required("timestamp"));
        assert!(!event_schema.is_required("tags"));
        assert!(!event_schema.is_required("metadata"));
    }
}
