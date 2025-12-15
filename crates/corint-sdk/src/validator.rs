//! DSL Validation Module
//!
//! Provides a high-level interface for validating CORINT DSL documents
//! including rules, rulesets, and pipelines.
//!
//! # Example
//!
//! ```rust,ignore
//! use corint_sdk::{DslValidator, DslType, ValidationResult};
//!
//! // Validate a rule
//! let rule_yaml = r#"
//! rule:
//!   id: test_rule
//!   name: Test Rule
//!   when:
//!     conditions:
//!       - event.amount > 100
//!   score: 50
//! "#;
//!
//! let result = DslValidator::validate(rule_yaml, DslType::Rule);
//! if result.valid {
//!     println!("Rule is valid!");
//! } else {
//!     for error in &result.errors {
//!         println!("Error: {} at line {}", error.message, error.line.unwrap_or(0));
//!     }
//! }
//! ```

// Re-export all types from corint_compiler::validator
pub use corint_compiler::{
    Diagnostic, DiagnosticSeverity, DocumentMetadata, DslType, DslValidator, ValidationResult,
};

/// Convenience function to validate DSL content with auto-detection
pub fn validate(content: &str) -> ValidationResult {
    let validator = DslValidator::new();
    validator.validate(content, DslType::Auto)
}

/// Convenience function to validate a rule
pub fn validate_rule(content: &str) -> ValidationResult {
    let validator = DslValidator::new();
    validator.validate(content, DslType::Rule)
}

/// Convenience function to validate a ruleset
pub fn validate_ruleset(content: &str) -> ValidationResult {
    let validator = DslValidator::new();
    validator.validate(content, DslType::Ruleset)
}

/// Convenience function to validate a pipeline
pub fn validate_pipeline(content: &str) -> ValidationResult {
    let validator = DslValidator::new();
    validator.validate(content, DslType::Pipeline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_rule() {
        let rule_yaml = r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - event.amount > 100
  score: 50
"#;

        let result = validate_rule(rule_yaml);
        assert!(result.valid, "Expected valid rule, got errors: {:?}", result.errors);
        assert!(result.errors.is_empty());
        assert_eq!(result.metadata.as_ref().unwrap().doc_type, DslType::Rule);
        assert_eq!(result.metadata.as_ref().unwrap().id.as_deref(), Some("test_rule"));
    }

    #[test]
    fn test_validate_invalid_yaml() {
        let invalid_yaml = r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - [invalid yaml syntax
"#;

        let result = validate_rule(invalid_yaml);
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
        assert_eq!(result.errors[0].code, "E001");
    }

    #[test]
    fn test_validate_missing_required_field() {
        let missing_name = r#"
rule:
  id: test_rule
  when:
    conditions:
      - event.amount > 100
  score: 50
"#;

        let result = validate_rule(missing_name);
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_validate_auto_detect() {
        let rule_yaml = r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - event.amount > 100
  score: 50
"#;

        let result = validate(rule_yaml);
        assert!(result.valid, "Expected valid rule with auto-detection, got errors: {:?}", result.errors);
        assert_eq!(result.metadata.as_ref().unwrap().doc_type, DslType::Rule);
    }

    #[test]
    fn test_validate_valid_ruleset() {
        let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  name: Test Ruleset
  rules:
    - rule:
        id: inner_rule
        name: Inner Rule
        when:
          conditions:
            - event.amount > 100
        score: 50
"#;

        let result = validate_ruleset(ruleset_yaml);
        assert!(result.valid, "Expected valid ruleset, got errors: {:?}", result.errors);
        assert_eq!(result.metadata.as_ref().unwrap().doc_type, DslType::Ruleset);
    }

    #[test]
    fn test_validate_valid_pipeline() {
        let pipeline_yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  stages:
    - stage:
        id: stage1
        name: Stage 1
        rules:
          - rule:
              id: rule1
              name: Rule 1
              when:
                conditions:
                  - event.amount > 100
              score: 50
"#;

        let result = validate_pipeline(pipeline_yaml);
        assert!(result.valid, "Expected valid pipeline, got errors: {:?}", result.errors);
        assert_eq!(result.metadata.as_ref().unwrap().doc_type, DslType::Pipeline);
    }
}
