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

    /// Parse YAML string containing multiple documents (separated by --- or auto-detected)
    /// Returns a vector of YAML values, one for each document
    ///
    /// This function supports two formats:
    /// 1. Traditional YAML multi-document format with explicit `---` separators
    /// 2. Auto-detection of `rule:`, `ruleset:`, `pipeline:` keywords at line start
    ///    (automatically inserts `---` before these keywords)
    pub fn parse_multi_document(yaml_str: &str) -> Result<Vec<YamlValue>> {
        use serde::Deserialize;

        // Preprocess: auto-insert --- before rule:/ruleset:/pipeline: at line start
        let preprocessed = Self::preprocess_multi_document(yaml_str);

        let deserializer = serde_yaml::Deserializer::from_str(&preprocessed);
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

    /// Preprocess YAML content to auto-insert `---` separators
    /// before `rule:`, `ruleset:`, `pipeline:` keywords at line start
    fn preprocess_multi_document(yaml_str: &str) -> String {
        let mut result = String::with_capacity(yaml_str.len() + 100);
        let mut seen_definition = false;
        let mut has_content_before_first_def = false;
        let mut recent_separator = false;

        for line in yaml_str.lines() {
            let trimmed = line.trim();

            // Check if this line starts a new definition (rule/ruleset/pipeline at column 0)
            let is_definition_start = !line.starts_with(' ')
                && !line.starts_with('\t')
                && (trimmed.starts_with("rule:")
                    || trimmed.starts_with("ruleset:")
                    || trimmed.starts_with("pipeline:"));

            // Track if there's meaningful content before first definition
            // (not just comments, empty lines, or version header)
            if !seen_definition && !is_definition_start && !trimmed.is_empty() && !trimmed.starts_with('#') {
                // Check if it's a YAML key (like "version:")
                if trimmed.contains(':') {
                    has_content_before_first_def = true;
                }
            }

            // Insert --- before definitions:
            // - Before first definition if there's content before it (like version:)
            // - Before subsequent definitions (unless we recently saw ---)
            if is_definition_start && !recent_separator {
                if seen_definition || has_content_before_first_def {
                    result.push_str("\n---\n");
                }
            }

            if is_definition_start {
                seen_definition = true;
            }

            result.push_str(line);
            result.push('\n');

            // Track if we've seen a separator; only reset on meaningful content
            if trimmed == "---" {
                recent_separator = true;
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                recent_separator = false;
            }
        }

        result
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

    /// Validate fields in a YAML object against a list of known fields
    /// Returns warnings for unknown fields with suggestions
    pub fn validate_fields(
        obj: &YamlValue,
        known_fields: &[&str],
        context: &str,
    ) -> Vec<String> {
        let mut warnings = Vec::new();

        if let Some(mapping) = obj.as_mapping() {
            for (key, _) in mapping {
                if let Some(field_name) = key.as_str() {
                    if !known_fields.contains(&field_name) {
                        // Check if this is a common typo
                        let typo_correction = FIELD_CORRECTIONS
                            .iter()
                            .find(|(typo, _)| *typo == field_name)
                            .map(|(_, correct)| *correct);

                        // Try fuzzy matching if no exact typo match
                        let suggestion = if let Some(correct) = typo_correction {
                            format!(" Did you mean '{}'?", correct)
                        } else if let Some(similar) = Self::find_similar_field(field_name, known_fields) {
                            format!(" Did you mean '{}'?", similar)
                        } else {
                            String::new()
                        };

                        warnings.push(format!(
                            "Unknown field '{}' in {}.{}",
                            field_name, context, suggestion
                        ));
                    }
                }
            }
        }

        warnings
    }

    /// Validate fields strictly - returns error if unknown fields found
    pub fn validate_fields_strict(
        obj: &YamlValue,
        known_fields: &[&str],
        context: &str,
    ) -> Result<()> {
        let errors = Self::validate_fields(obj, known_fields, context);

        if !errors.is_empty() {
            // Log each error using the log crate
            for error in &errors {
                log::error!("Field validation error: {}", error);
            }

            return Err(ParseError::InvalidValue {
                field: context.to_string(),
                message: errors.join("; "),
            });
        }

        Ok(())
    }

    /// Find similar field names using Levenshtein distance
    fn find_similar_field(field: &str, known_fields: &[&str]) -> Option<String> {
        known_fields
            .iter()
            .filter(|known| levenshtein_distance(field, known) <= 2)
            .min_by_key(|known| levenshtein_distance(field, known))
            .map(|s| s.to_string())
    }
}

/// Common field name typos and their corrections
const FIELD_CORRECTIONS: &[(&str, &str)] = &[
    ("output_var", "output"),
    ("outputs", "output"),
    ("param", "params"),
    ("parameter", "params"),
    ("end_point", "endpoint"),
    ("api_name", "api"),
    ("rule_set", "ruleset"),
    ("pipe_line", "pipeline"),
    ("step_type", "type"),
    ("step_id", "id"),
    ("step_name", "name"),
];

/// Calculate Levenshtein distance between two strings
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    // Initialize first column and row
    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    // Fill in the matrix
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    for (i, &c1) in s1_chars.iter().enumerate() {
        for (j, &c2) in s2_chars.iter().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = std::cmp::min(
                std::cmp::min(
                    matrix[i][j + 1] + 1,      // deletion
                    matrix[i + 1][j] + 1,      // insertion
                ),
                matrix[i][j] + cost,           // substitution
            );
        }
    }

    matrix[len1][len2]
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

    #[test]
    fn test_parse_multi_document_without_separators() {
        let yaml_str = r#"
version: "0.1"

rule:
  id: rule_1
  name: Rule 1

rule:
  id: rule_2
  name: Rule 2

ruleset:
  id: ruleset_1
  name: Ruleset 1

pipeline:
  id: pipeline_1
  name: Pipeline 1
"#;

        let docs = YamlParser::parse_multi_document(yaml_str).unwrap();
        // Should produce 5 documents: version header, 2 rules, 1 ruleset, 1 pipeline
        assert_eq!(docs.len(), 5);

        // First document contains version
        assert!(docs[0].get("version").is_some());

        // Second document is rule_1
        assert!(docs[1].get("rule").is_some());
        let rule1 = docs[1].get("rule").unwrap();
        assert_eq!(rule1.get("id").unwrap().as_str(), Some("rule_1"));

        // Third document is rule_2
        assert!(docs[2].get("rule").is_some());
        let rule2 = docs[2].get("rule").unwrap();
        assert_eq!(rule2.get("id").unwrap().as_str(), Some("rule_2"));

        // Fourth document is ruleset_1
        assert!(docs[3].get("ruleset").is_some());

        // Fifth document is pipeline_1
        assert!(docs[4].get("pipeline").is_some());
    }

    #[test]
    fn test_parse_multi_document_with_explicit_separators() {
        let yaml_str = r#"
version: "0.1"
---
rule:
  id: rule_1
---
ruleset:
  id: ruleset_1
"#;

        let docs = YamlParser::parse_multi_document(yaml_str).unwrap();
        // Should produce 3 documents (explicit --- separators)
        assert_eq!(docs.len(), 3);
    }

    #[test]
    fn test_parse_multi_document_preserves_existing_separators() {
        // File that already has --- separators should not get duplicate separators
        let yaml_str = r#"
version: "0.1"

imports:
  rules:
    - some/path.yaml

---

ruleset:
  id: my_ruleset
  name: My Ruleset
"#;

        let docs = YamlParser::parse_multi_document(yaml_str).unwrap();
        // Should produce 2 documents: header (version + imports), ruleset
        assert_eq!(docs.len(), 2);

        // First document has version and imports
        assert!(docs[0].get("version").is_some());
        assert!(docs[0].get("imports").is_some());

        // Second document has ruleset
        assert!(docs[1].get("ruleset").is_some());
    }
}
