//! Import parser
//!
//! Parses YAML import declarations into Import AST nodes.

use crate::error::{ParseError, Result};
use crate::yaml_parser::YamlParser;
use corint_core::ast::Imports;
use serde_yaml::Value as YamlValue;

/// Import parser
pub struct ImportParser;

impl ImportParser {
    /// Parse imports from YAML value
    ///
    /// Expected format:
    /// ```yaml
    /// import:
    ///   rules:
    ///     - library/rules/fraud/fraud_farm.yaml
    ///     - library/rules/payment/card_testing.yaml
    ///   rulesets:
    ///     - library/rulesets/fraud_detection_core.yaml
    ///   pipelines:
    ///     - library/pipelines/common_feature_extraction.yaml
    /// ```
    pub fn parse_from_yaml(yaml: &YamlValue) -> Result<Option<Imports>> {
        // Check if import section exists
        let imports_obj = match yaml.get("import") {
            Some(obj) => obj,
            None => return Ok(None), // No imports section
        };

        let mut imports = Imports::new();

        // Parse rule imports
        if let Some(rules_array) = YamlParser::get_optional_array(imports_obj, "rules") {
            for rule_value in rules_array {
                if let Some(path) = rule_value.as_str() {
                    imports = imports.add_rule(path.to_string());
                } else {
                    return Err(ParseError::InvalidValue {
                        field: "import.rules".to_string(),
                        message: "Rule import path must be a string".to_string(),
                    });
                }
            }
        }

        // Parse ruleset imports
        if let Some(rulesets_array) = YamlParser::get_optional_array(imports_obj, "rulesets") {
            for ruleset_value in rulesets_array {
                if let Some(path) = ruleset_value.as_str() {
                    imports = imports.add_ruleset(path.to_string());
                } else {
                    return Err(ParseError::InvalidValue {
                        field: "import.rulesets".to_string(),
                        message: "Ruleset import path must be a string".to_string(),
                    });
                }
            }
        }

        // Parse pipeline imports
        if let Some(pipelines_array) = YamlParser::get_optional_array(imports_obj, "pipelines") {
            for pipeline_value in pipelines_array {
                if let Some(path) = pipeline_value.as_str() {
                    imports = imports.add_pipeline(path.to_string());
                } else {
                    return Err(ParseError::InvalidValue {
                        field: "import.pipelines".to_string(),
                        message: "Pipeline import path must be a string".to_string(),
                    });
                }
            }
        }

        // Parse template imports (for future use)
        if let Some(templates_array) = YamlParser::get_optional_array(imports_obj, "templates") {
            for template_value in templates_array {
                if let Some(path) = template_value.as_str() {
                    imports.templates.push(path.to_string());
                } else {
                    return Err(ParseError::InvalidValue {
                        field: "import.templates".to_string(),
                        message: "Template import path must be a string".to_string(),
                    });
                }
            }
        }

        Ok(Some(imports))
    }

    /// Parse multi-document YAML with optional imports
    ///
    /// Returns (imports, definition_yaml)
    ///
    /// Supports multiple formats:
    ///
    /// 1. Single document without imports (legacy format):
    /// ```yaml
    /// version: "0.1"
    /// rule:
    ///   id: test_rule
    /// ```
    ///
    /// 2. Multi-document with import (new format):
    /// ```yaml
    /// version: "0.1"
    /// import:
    ///   rules: [...]
    /// ---
    /// rule:
    ///   id: test_rule
    /// ```
    ///
    /// 3. Multi-document with inline definitions (for pipelines with rules/rulesets):
    /// ```yaml
    /// version: "0.1"
    /// pipeline:
    ///   id: my_pipeline
    /// ---
    /// rule:
    ///   id: rule1
    /// ---
    /// ruleset:
    ///   id: ruleset1
    /// ```
    pub fn parse_with_imports(yaml_str: &str) -> Result<(Option<Imports>, YamlValue)> {
        let documents = YamlParser::parse_multi_document(yaml_str)?;

        match documents.len() {
            1 => {
                // Single document - check if it has imports section
                let doc = &documents[0];
                let imports = Self::parse_from_yaml(doc)?;
                Ok((imports, doc.clone()))
            }
            2 => {
                // Two documents - first contains version + imports, second contains definition
                let first_doc = &documents[0];
                let second_doc = &documents[1];

                // Parse imports from first document
                let imports = Self::parse_from_yaml(first_doc)?;

                // Merge version from first document if present
                let mut definition = second_doc.clone();
                if let Some(version) = YamlParser::get_optional_string(first_doc, "version") {
                    if let Some(def_map) = definition.as_mapping_mut() {
                        def_map.insert(
                            YamlValue::String("version".to_string()),
                            YamlValue::String(version),
                        );
                    }
                }

                Ok((imports, definition))
            }
            _ => {
                // Multiple documents (3+) - first contains version/imports, rest are inline definitions
                // This supports the format where pipeline, rules, and rulesets are in the same file
                let first_doc = &documents[0];

                // Parse imports from first document
                let imports = Self::parse_from_yaml(first_doc)?;

                // Find the pipeline definition (should be in one of the documents)
                let mut pipeline_doc = None;
                for doc in &documents {
                    if doc.get("pipeline").is_some() {
                        pipeline_doc = Some(doc.clone());
                        break;
                    }
                }

                // If pipeline found, use it as the definition
                // Otherwise, use the first document that has a definition (rule, ruleset, template)
                let definition = if let Some(mut pipeline) = pipeline_doc {
                    // Merge version from first document if present
                    if let Some(version) = YamlParser::get_optional_string(first_doc, "version") {
                        if let Some(def_map) = pipeline.as_mapping_mut() {
                            def_map.insert(
                                YamlValue::String("version".to_string()),
                                YamlValue::String(version),
                            );
                        }
                    }
                    pipeline
                } else {
                    // No pipeline found, use first document
                    first_doc.clone()
                };

                Ok((imports, definition))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_imports_with_rules() {
        let yaml_str = r#"
version: "0.1"
import:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/payment/card_testing.yaml
"#;
        let yaml = YamlParser::parse(yaml_str).unwrap();
        let imports = ImportParser::parse_from_yaml(&yaml).unwrap().unwrap();

        assert_eq!(imports.rules.len(), 2);
        assert_eq!(imports.rules[0], "library/rules/fraud/fraud_farm.yaml");
        assert_eq!(imports.rules[1], "library/rules/payment/card_testing.yaml");
        assert!(imports.rulesets.is_empty());
        assert!(imports.pipelines.is_empty());
    }

    #[test]
    fn test_parse_imports_with_rulesets() {
        let yaml_str = r#"
import:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml
"#;
        let yaml = YamlParser::parse(yaml_str).unwrap();
        let imports = ImportParser::parse_from_yaml(&yaml).unwrap().unwrap();

        assert_eq!(imports.rulesets.len(), 1);
        assert_eq!(
            imports.rulesets[0],
            "library/rulesets/fraud_detection_core.yaml"
        );
        assert!(imports.rules.is_empty());
    }

    #[test]
    fn test_parse_imports_all_types() {
        let yaml_str = r#"
import:
  rules:
    - rule1.yaml
  rulesets:
    - ruleset1.yaml
  pipelines:
    - pipeline1.yaml
"#;
        let yaml = YamlParser::parse(yaml_str).unwrap();
        let imports = ImportParser::parse_from_yaml(&yaml).unwrap().unwrap();

        assert_eq!(imports.rules.len(), 1);
        assert_eq!(imports.rulesets.len(), 1);
        assert_eq!(imports.pipelines.len(), 1);
    }

    #[test]
    fn test_parse_no_imports() {
        let yaml_str = r#"
version: "0.1"
rule:
  id: test
"#;
        let yaml = YamlParser::parse(yaml_str).unwrap();
        let imports = ImportParser::parse_from_yaml(&yaml).unwrap();

        assert!(imports.is_none());
    }

    #[test]
    fn test_parse_with_imports_multi_document() {
        let yaml_str = r#"
version: "0.1"
import:
  rules:
    - library/rules/fraud_farm.yaml

---

rule:
  id: test_rule
  name: Test Rule
  score: 50
  when:
    event.type: transaction
    conditions:
      - amount > 1000
"#;
        let (imports, definition) = ImportParser::parse_with_imports(yaml_str).unwrap();

        assert!(imports.is_some());
        let imports = imports.unwrap();
        assert_eq!(imports.rules.len(), 1);
        assert_eq!(imports.rules[0], "library/rules/fraud_farm.yaml");

        // Check that definition has the rule
        assert!(definition.get("rule").is_some());
    }

    #[test]
    fn test_parse_with_imports_single_document() {
        let yaml_str = r#"
version: "0.1"
rule:
  id: test_rule
  name: Test Rule
  score: 50
  when:
    event.type: transaction
"#;
        let (imports, definition) = ImportParser::parse_with_imports(yaml_str).unwrap();

        assert!(imports.is_none());
        assert!(definition.get("rule").is_some());
        assert!(definition.get("version").is_some());
    }

    #[test]
    fn test_parse_imports_invalid_rule_path() {
        let yaml_str = r#"
import:
  rules:
    - 123  # Invalid: not a string
"#;
        let yaml = YamlParser::parse(yaml_str).unwrap();
        let result = ImportParser::parse_from_yaml(&yaml);

        assert!(result.is_err());
    }
}
