//! Parser for decision logic templates

use crate::error::ParseError;
use crate::import_parser::ImportParser;
use crate::ruleset_parser::RulesetParser;
use crate::yaml_parser::YamlParser;
use corint_core::ast::{DecisionRule, DecisionTemplate, RdlDocument};
use serde::Deserialize as _;
use serde_yaml::Value;
use std::collections::HashMap;

pub struct TemplateParser;

impl TemplateParser {
    /// Parse a template YAML file with imports support
    pub fn parse_with_imports(content: &str) -> Result<RdlDocument<DecisionTemplate>, ParseError> {
        // Parse multi-document YAML (imports --- template)
        let docs: Vec<Value> = serde_yaml::Deserializer::from_str(content)
            .map(|doc| serde_yaml::Value::deserialize(doc).unwrap())
            .collect();

        if docs.is_empty() {
            return Err(ParseError::ParseError("Empty YAML document".to_string()));
        }

        // Single document: just template
        if docs.len() == 1 {
            let template = Self::parse_template(&docs[0])?;
            return Ok(RdlDocument::new("0.1".to_string(), template));
        }

        // Multi-document: imports --- template
        if docs.len() != 2 {
            return Err(ParseError::ParseError(
                "Template file should have either 1 document (template only) or 2 documents (imports --- template)"
                    .to_string(),
            ));
        }

        // Parse first document (version + imports)
        let first_doc = &docs[0];
        let version = YamlParser::get_string(first_doc, "version")?;
        let imports = ImportParser::parse_from_yaml(first_doc)?;

        // Parse second document (template)
        let template = Self::parse_template(&docs[1])?;

        Ok(RdlDocument {
            version,
            imports,
            definition: template,
        })
    }

    /// Parse a template definition from YAML value
    pub fn parse_template(value: &Value) -> Result<DecisionTemplate, ParseError> {
        let template_obj = value
            .get("template")
            .ok_or_else(|| ParseError::MissingField {
                field: "template".to_string(),
            })?;

        let id = YamlParser::get_string(template_obj, "id")?;
        let name = YamlParser::get_optional_string(template_obj, "name");
        let description = YamlParser::get_optional_string(template_obj, "description");

        // Parse params (default values)
        let params = if let Some(params_obj) = template_obj.get("params") {
            let params_map = params_obj
                .as_mapping()
                .ok_or_else(|| ParseError::ParseError("params must be an object".to_string()))?;

            let mut result = HashMap::new();
            for (key, value) in params_map {
                let key_str = key
                    .as_str()
                    .ok_or_else(|| {
                        ParseError::ParseError("param key must be a string".to_string())
                    })?
                    .to_string();
                let json_value: serde_json::Value = serde_yaml::from_value(value.clone())
                    .map_err(|e| ParseError::ParseError(format!("Invalid param value: {}", e)))?;
                result.insert(key_str, json_value);
            }
            Some(result)
        } else {
            None
        };

        // Parse decision_logic
        let decision_logic_array = template_obj
            .get("decision_logic")
            .ok_or_else(|| ParseError::MissingField {
                field: "decision_logic".to_string(),
            })?
            .as_sequence()
            .ok_or_else(|| ParseError::ParseError("decision_logic must be an array".to_string()))?;

        let decision_logic: Vec<DecisionRule> = decision_logic_array
            .iter()
            .map(RulesetParser::parse_decision_rule)
            .collect::<Result<Vec<_>, _>>()?;

        // Parse metadata
        let metadata = template_obj
            .get("metadata")
            .and_then(|v| serde_yaml::from_value(v.clone()).ok());

        Ok(DecisionTemplate {
            id,
            name,
            description,
            params,
            decision_logic,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_template() {
        let yaml = r#"
template:
  id: test_template
  name: Test Template
  description: A test template

  decision_logic:
    - default: true
      action: approve
      reason: Default action
"#;

        let doc = TemplateParser::parse_with_imports(yaml).unwrap();
        assert_eq!(doc.definition.id, "test_template");
        assert_eq!(doc.definition.name, Some("Test Template".to_string()));
        assert_eq!(doc.definition.decision_logic.len(), 1);
    }

    #[test]
    fn test_parse_template_with_params() {
        let yaml = r#"
template:
  id: score_based
  name: Score Based Template

  params:
    critical_threshold: 200
    high_threshold: 100

  decision_logic:
    - condition: total_score >= params.critical_threshold
      action: deny
      reason: Critical risk
      terminate: true

    - default: true
      action: approve
"#;

        let doc = TemplateParser::parse_with_imports(yaml).unwrap();
        assert_eq!(doc.definition.id, "score_based");
        assert!(doc.definition.params.is_some());

        let params = doc.definition.params.unwrap();
        assert_eq!(
            params.get("critical_threshold"),
            Some(&serde_json::json!(200))
        );
        assert_eq!(params.get("high_threshold"), Some(&serde_json::json!(100)));
        assert_eq!(doc.definition.decision_logic.len(), 2);
    }

    #[test]
    fn test_parse_template_with_imports() {
        let yaml = r#"
version: "0.1"

imports:
  templates:
    - library/templates/base_template.yaml

---

template:
  id: extended_template
  name: Extended Template

  decision_logic:
    - default: true
      action: review
"#;

        let doc = TemplateParser::parse_with_imports(yaml).unwrap();
        assert_eq!(doc.version, "0.1");
        assert!(doc.imports.is_some());

        let imports = doc.imports.unwrap();
        assert_eq!(imports.templates.len(), 1);
        assert_eq!(doc.definition.id, "extended_template");
    }

    #[test]
    fn test_missing_template_field() {
        let yaml = r#"
id: test_template
"#;

        let result = TemplateParser::parse_with_imports(yaml);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::MissingField { .. }
        ));
    }
}
