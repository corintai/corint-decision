//! Integration tests for import functionality
//!
//! These tests demonstrate how to use the import feature with both
//! legacy and new formats.

use corint_parser::{ImportParser, PipelineParser, RuleParser, RulesetParser};

#[test]
fn test_parse_rule_without_imports_legacy() {
    // Legacy format - single document, no imports
    let yaml = r#"
version: "0.1"

rule:
  id: legacy_rule
  name: Legacy Rule
  description: This is a legacy format rule
  when:
    event.type: transaction
    conditions:
      - amount > 1000
      - country == "US"
  score: 75
"#;

    // Parse using the old method - should still work
    let rule = RuleParser::parse(yaml).unwrap();
    assert_eq!(rule.id, "legacy_rule");
    assert_eq!(rule.score, 75);

    // Parse using the new method - should also work
    let doc = RuleParser::parse_with_imports(yaml).unwrap();
    assert_eq!(doc.definition.id, "legacy_rule");
    assert!(!doc.has_imports());
}

#[test]
fn test_parse_rule_with_imports_new_format() {
    // New format - multi-document with imports
    let yaml = r#"
version: "0.1"

imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/payment/card_testing.yaml

---

rule:
  id: combined_fraud_rule
  name: Combined Fraud Rule
  description: Combines multiple fraud patterns
  when:
    event.type: transaction
    conditions:
      - total_score > 50
  score: 100
"#;

    let doc = RuleParser::parse_with_imports(yaml).unwrap();

    // Check version
    assert_eq!(doc.version, "0.1");

    // Check imports
    assert!(doc.has_imports());
    let imports = doc.imports();
    assert_eq!(imports.rules.len(), 2);
    assert_eq!(imports.rules[0], "library/rules/fraud/fraud_farm.yaml");
    assert_eq!(imports.rules[1], "library/rules/payment/card_testing.yaml");

    // Check rule definition
    assert_eq!(doc.definition.id, "combined_fraud_rule");
    assert_eq!(doc.definition.score, 100);
}

#[test]
fn test_parse_ruleset_without_imports_legacy() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: simple_ruleset
  name: Simple Ruleset
  rules:
    - rule1
    - rule2
  decision_logic:
    - condition: total_score > 100
      action: deny
      reason: "High risk"
    - default: true
      action: approve
"#;

    // Old method
    let ruleset = RulesetParser::parse(yaml).unwrap();
    assert_eq!(ruleset.id, "simple_ruleset");
    assert_eq!(ruleset.rules.len(), 2);

    // New method
    let doc = RulesetParser::parse_with_imports(yaml).unwrap();
    assert_eq!(doc.definition.id, "simple_ruleset");
    assert!(!doc.has_imports());
}

#[test]
fn test_parse_ruleset_with_imports_new_format() {
    let yaml = r#"
version: "0.1"

imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/account_takeover.yaml
    - library/rules/fraud/velocity_abuse.yaml

---

ruleset:
  id: fraud_detection_core
  name: Core Fraud Detection Ruleset
  description: Comprehensive fraud detection

  rules:
    - fraud_farm_pattern
    - account_takeover_pattern
    - velocity_abuse_pattern

  decision_logic:
    - condition: triggered_rules contains "fraud_farm_pattern"
      action: deny
      reason: "Critical: Fraud farm detected"
      terminate: true

    - condition: total_score >= 150
      action: deny
      reason: "High risk score"

    - condition: total_score >= 100
      action: review
      reason: "Multiple fraud indicators"

    - default: true
      action: approve
      reason: "No significant fraud indicators"
"#;

    let doc = RulesetParser::parse_with_imports(yaml).unwrap();

    // Check version
    assert_eq!(doc.version, "0.1");

    // Check imports
    assert!(doc.has_imports());
    let imports = doc.imports();
    assert_eq!(imports.rules.len(), 3);
    assert_eq!(imports.rules[0], "library/rules/fraud/fraud_farm.yaml");

    // Check ruleset
    assert_eq!(doc.definition.id, "fraud_detection_core");
    assert_eq!(doc.definition.rules.len(), 3);
    assert_eq!(doc.definition.decision_logic.len(), 4);
}

#[test]
fn test_parse_imports_only() {
    let yaml = r#"
version: "0.1"

imports:
  rules:
    - rule1.yaml
    - rule2.yaml
  rulesets:
    - ruleset1.yaml
  pipelines:
    - pipeline1.yaml
"#;

    let yaml_value = serde_yaml::from_str(yaml).unwrap();
    let imports = ImportParser::parse_from_yaml(&yaml_value).unwrap().unwrap();

    assert_eq!(imports.rules.len(), 2);
    assert_eq!(imports.rulesets.len(), 1);
    assert_eq!(imports.pipelines.len(), 1);
}

#[test]
fn test_backward_compatibility() {
    // Ensure that old code without imports still works

    let old_rule = r#"
rule:
  id: old_format
  name: Old Format
  when:
    conditions: []
  score: 10
"#;

    // Old method
    let rule1 = RuleParser::parse(old_rule).unwrap();
    assert_eq!(rule1.id, "old_format");

    // New method on old format
    let doc = RuleParser::parse_with_imports(old_rule).unwrap();
    assert_eq!(doc.definition.id, "old_format");
    assert!(!doc.has_imports());
}

#[test]
fn test_multi_document_parsing() {
    // Test that multi-document YAML is correctly parsed

    let yaml = r#"
version: "0.1"
imports:
  rules:
    - test.yaml

---

rule:
  id: test
  name: Test
  when:
    conditions: []
  score: 10
"#;

    let (imports, definition) = ImportParser::parse_with_imports(yaml).unwrap();

    assert!(imports.is_some());
    assert_eq!(imports.unwrap().rules.len(), 1);

    // Definition should have the rule
    assert!(definition.get("rule").is_some());
}

#[test]
fn test_parse_pipeline_without_imports_legacy() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: simple_pipeline
  name: Simple Pipeline
  when:
    event.type: transaction
  steps:
    - type: extract
      id: extract_features
      features: []
"#;

    // Old method
    let pipeline = PipelineParser::parse(yaml).unwrap();
    assert_eq!(pipeline.id, Some("simple_pipeline".to_string()));

    // New method
    let doc = PipelineParser::parse_with_imports(yaml).unwrap();
    assert_eq!(doc.definition.id, Some("simple_pipeline".to_string()));
    assert!(!doc.has_imports());
}

#[test]
fn test_parse_pipeline_with_imports_new_format() {
    let yaml = r#"
version: "0.1"

imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline
  description: Production-grade fraud detection

  when:
    event.type: transaction

  steps:
    - include:
        ruleset: fraud_detection_core
"#;

    let doc = PipelineParser::parse_with_imports(yaml).unwrap();

    // Check version
    assert_eq!(doc.version, "0.1");

    // Check imports
    assert!(doc.has_imports());
    let imports = doc.imports();
    assert_eq!(imports.rulesets.len(), 1);
    assert_eq!(imports.rulesets[0], "library/rulesets/fraud_detection_core.yaml");

    // Check pipeline
    assert_eq!(doc.definition.id, Some("fraud_detection_pipeline".to_string()));
    assert_eq!(doc.definition.steps.len(), 1);
}

#[test]
fn test_parse_pipeline_with_multiple_imports() {
    let yaml = r#"
version: "0.1"

imports:
  rulesets:
    - library/rulesets/payment_standard.yaml
    - library/rulesets/payment_high_value.yaml

---

pipeline:
  id: payment_pipeline
  name: Payment Risk Pipeline

  when:
    event.type: payment

  steps:
    - type: extract
      id: extract_features
      features: []

    - include:
        ruleset: payment_standard
"#;

    let doc = PipelineParser::parse_with_imports(yaml).unwrap();

    assert_eq!(doc.version, "0.1");
    assert!(doc.has_imports());

    let imports = doc.imports();
    assert_eq!(imports.rulesets.len(), 2);

    assert_eq!(doc.definition.id, Some("payment_pipeline".to_string()));
}

#[test]
fn test_all_parsers_backward_compatible() {
    // Test that all three parsers work with legacy format

    let rule_yaml = r#"
rule:
  id: test_rule
  name: Test
  when:
    conditions: []
  score: 10
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules: []
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: test_pipeline
  steps: []
"#;

    // All should work with old methods
    assert!(RuleParser::parse(rule_yaml).is_ok());
    assert!(RulesetParser::parse(ruleset_yaml).is_ok());
    assert!(PipelineParser::parse(pipeline_yaml).is_ok());

    // All should work with new methods
    assert!(RuleParser::parse_with_imports(rule_yaml).is_ok());
    assert!(RulesetParser::parse_with_imports(ruleset_yaml).is_ok());
    assert!(PipelineParser::parse_with_imports(pipeline_yaml).is_ok());
}
