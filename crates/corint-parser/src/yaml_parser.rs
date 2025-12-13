//! YAML Parser
//!
//! Provides utilities for parsing YAML content into structured data.

use crate::error::{ParseError, Result};
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;

/// YAML parser utilities
pub struct YamlParser;

impl YamlParser {
    /// Parse YAML string into a YAML value
    pub fn parse(yaml_str: &str) -> Result<YamlValue> {
        serde_yaml::from_str(yaml_str).map_err(|e| ParseError::ParseError(e.to_string()))
    }

    /// Parse YAML string containing multiple documents (separated by ---)
    /// Returns a vector of YAML values, one for each document
    pub fn parse_multi_document(yaml_str: &str) -> Result<Vec<YamlValue>> {
        use serde::Deserialize;

        let deserializer = serde_yaml::Deserializer::from_str(yaml_str);
        let mut documents = Vec::new();

        for document in deserializer {
            let value = YamlValue::deserialize(document)
                .map_err(|e| ParseError::ParseError(e.to_string()))?;
            documents.push(value);
        }

        // If no documents were parsed, try parsing as single document
        if documents.is_empty() {
            let value = Self::parse(yaml_str)?;
            documents.push(value);
        }

        Ok(documents)
    }

    /// Get a required string field from YAML object
    pub fn get_string(obj: &YamlValue, field: &str) -> Result<String> {
        obj.get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ParseError::MissingField {
                field: field.to_string(),
            })
    }

    /// Get an optional string field from YAML object
    pub fn get_optional_string(obj: &YamlValue, field: &str) -> Option<String> {
        obj.get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Get a required integer field from YAML object
    pub fn get_i32(obj: &YamlValue, field: &str) -> Result<i32> {
        obj.get(field)
            .and_then(|v| v.as_i64())
            .map(|n| n as i32)
            .ok_or_else(|| ParseError::MissingField {
                field: field.to_string(),
            })
    }

    /// Get an optional integer field from YAML object
    pub fn get_optional_i32(obj: &YamlValue, field: &str) -> Option<i32> {
        obj.get(field).and_then(|v| v.as_i64()).map(|n| n as i32)
    }

    /// Get a required boolean field from YAML object
    pub fn get_bool(obj: &YamlValue, field: &str) -> Result<bool> {
        obj.get(field)
            .and_then(|v| v.as_bool())
            .ok_or_else(|| ParseError::MissingField {
                field: field.to_string(),
            })
    }

    /// Get an optional boolean field from YAML object
    pub fn get_optional_bool(obj: &YamlValue, field: &str) -> Option<bool> {
        obj.get(field).and_then(|v| v.as_bool())
    }

    /// Get a required float field from YAML object
    pub fn get_f64(obj: &YamlValue, field: &str) -> Result<f64> {
        obj.get(field)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ParseError::MissingField {
                field: field.to_string(),
            })
    }

    /// Get an optional float field from YAML object
    pub fn get_optional_f64(obj: &YamlValue, field: &str) -> Option<f64> {
        obj.get(field).and_then(|v| v.as_f64())
    }

    /// Get a required array field from YAML object
    pub fn get_array<'a>(obj: &'a YamlValue, field: &str) -> Result<&'a Vec<YamlValue>> {
        obj.get(field)
            .and_then(|v| v.as_sequence())
            .ok_or_else(|| ParseError::MissingField {
                field: field.to_string(),
            })
    }

    /// Get an optional array field from YAML object
    pub fn get_optional_array<'a>(obj: &'a YamlValue, field: &str) -> Option<&'a Vec<YamlValue>> {
        obj.get(field).and_then(|v| v.as_sequence())
    }

    /// Get a required object field from YAML object
    pub fn get_object<'a>(obj: &'a YamlValue, field: &str) -> Result<&'a serde_yaml::Mapping> {
        obj.get(field)
            .and_then(|v| v.as_mapping())
            .ok_or_else(|| ParseError::MissingField {
                field: field.to_string(),
            })
    }

    /// Get an optional object field from YAML object
    pub fn get_optional_object<'a>(
        obj: &'a YamlValue,
        field: &str,
    ) -> Option<&'a serde_yaml::Mapping> {
        obj.get(field).and_then(|v| v.as_mapping())
    }

    /// Get a field by path (e.g., "event.type" or "user.profile.name")
    pub fn get_nested_string(obj: &YamlValue, path: &str) -> Option<String> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = obj;

        for part in &parts[..parts.len() - 1] {
            current = current.get(part)?;
        }

        current
            .get(parts[parts.len() - 1])
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Convert YAML mapping to HashMap<String, String>
    pub fn mapping_to_hashmap(mapping: &serde_yaml::Mapping) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for (key, value) in mapping {
            if let (Some(k), Some(v)) = (key.as_str(), value.as_str()) {
                map.insert(k.to_string(), v.to_string());
            }
        }
        map
    }

    /// Parse a YAML value to a string representation
    pub fn to_string(value: &YamlValue) -> String {
        match value {
            YamlValue::Null => "null".to_string(),
            YamlValue::Bool(b) => b.to_string(),
            YamlValue::Number(n) => n.to_string(),
            YamlValue::String(s) => s.clone(),
            YamlValue::Sequence(_) => serde_yaml::to_string(value).unwrap_or_default(),
            YamlValue::Mapping(_) => serde_yaml::to_string(value).unwrap_or_default(),
            YamlValue::Tagged(t) => serde_yaml::to_string(&t.value).unwrap_or_default(),
        }
    }

    /// Check if a field exists in YAML object
    pub fn has_field(obj: &YamlValue, field: &str) -> bool {
        obj.get(field).is_some()
    }

    /// Get all keys from a YAML mapping
    pub fn get_keys(obj: &YamlValue) -> Vec<String> {
        if let Some(mapping) = obj.as_mapping() {
            mapping
                .keys()
                .filter_map(|k| k.as_str())
                .map(|s| s.to_string())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Validate YAML structure has required fields
    pub fn validate_required_fields(obj: &YamlValue, fields: &[&str]) -> Result<()> {
        for field in fields {
            if !Self::has_field(obj, field) {
                return Err(ParseError::MissingField {
                    field: field.to_string(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml() {
        let yaml_str = r#"
name: test
value: 42
enabled: true
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        assert!(yaml.is_mapping());
    }

    #[test]
    fn test_get_string() {
        let yaml_str = r#"
name: John Doe
age: 30
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let name = YamlParser::get_string(&yaml, "name").unwrap();
        assert_eq!(name, "John Doe");
    }

    #[test]
    fn test_get_optional_string() {
        let yaml_str = r#"
name: John Doe
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let name = YamlParser::get_optional_string(&yaml, "name");
        assert_eq!(name, Some("John Doe".to_string()));

        let missing = YamlParser::get_optional_string(&yaml, "missing");
        assert_eq!(missing, None);
    }

    #[test]
    fn test_get_i32() {
        let yaml_str = r#"
count: 42
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let count = YamlParser::get_i32(&yaml, "count").unwrap();
        assert_eq!(count, 42);
    }

    #[test]
    fn test_get_bool() {
        let yaml_str = r#"
enabled: true
disabled: false
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let enabled = YamlParser::get_bool(&yaml, "enabled").unwrap();
        assert!(enabled);

        let disabled = YamlParser::get_bool(&yaml, "disabled").unwrap();
        assert!(!disabled);
    }

    #[test]
    fn test_get_f64() {
        let yaml_str = r#"
price: 19.99
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let price = YamlParser::get_f64(&yaml, "price").unwrap();
        assert!((price - 19.99).abs() < 0.001);
    }

    #[test]
    fn test_get_array() {
        let yaml_str = r#"
items:
  - apple
  - banana
  - cherry
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let items = YamlParser::get_array(&yaml, "items").unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_get_object() {
        let yaml_str = r#"
user:
  name: John
  age: 30
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let user = YamlParser::get_object(&yaml, "user").unwrap();
        assert_eq!(user.len(), 2);
    }

    #[test]
    fn test_get_nested_string() {
        let yaml_str = r#"
user:
  profile:
    name: John Doe
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let name = YamlParser::get_nested_string(&yaml, "user.profile.name");
        assert_eq!(name, Some("John Doe".to_string()));
    }

    #[test]
    fn test_has_field() {
        let yaml_str = r#"
name: test
value: 42
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        assert!(YamlParser::has_field(&yaml, "name"));
        assert!(YamlParser::has_field(&yaml, "value"));
        assert!(!YamlParser::has_field(&yaml, "missing"));
    }

    #[test]
    fn test_get_keys() {
        let yaml_str = r#"
name: test
value: 42
enabled: true
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let keys = YamlParser::get_keys(&yaml);
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"name".to_string()));
        assert!(keys.contains(&"value".to_string()));
        assert!(keys.contains(&"enabled".to_string()));
    }

    #[test]
    fn test_validate_required_fields() {
        let yaml_str = r#"
id: test
name: Test Rule
score: 50
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let result = YamlParser::validate_required_fields(&yaml, &["id", "name", "score"]);
        assert!(result.is_ok());

        let result = YamlParser::validate_required_fields(&yaml, &["id", "name", "missing"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_mapping_to_hashmap() {
        let yaml_str = r#"
key1: value1
key2: value2
key3: value3
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let mapping = yaml.as_mapping().unwrap();
        let hashmap = YamlParser::mapping_to_hashmap(mapping);

        assert_eq!(hashmap.len(), 3);
        assert_eq!(hashmap.get("key1"), Some(&"value1".to_string()));
        assert_eq!(hashmap.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_missing_required_field() {
        let yaml_str = r#"
name: test
"#;

        let yaml = YamlParser::parse(yaml_str).unwrap();
        let result = YamlParser::get_string(&yaml, "missing");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let yaml_str = "invalid: yaml: content: [";
        let result = YamlParser::parse(yaml_str);
        assert!(result.is_err());
    }
}
