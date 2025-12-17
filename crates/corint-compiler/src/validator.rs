//! DSL Validator - Standalone validation for rules, rulesets, and pipelines
//!
//! This module provides validation functionality that can be used independently
//! of the main compilation pipeline, suitable for web APIs and CI/CD validation.

use crate::{Compiler, CompilerOptions, SemanticAnalyzer};
use corint_parser::{PipelineParser, RuleParser, RulesetParser};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// DSL document type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DslType {
    Rule,
    Ruleset,
    Pipeline,
    Auto, // Auto-detect from content
}

impl Default for DslType {
    fn default() -> Self {
        DslType::Auto
    }
}

/// Severity level for validation diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

/// A single diagnostic message from validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity level
    pub severity: DiagnosticSeverity,

    /// Error/warning code (e.g., "E001", "W001")
    pub code: String,

    /// Human-readable message
    pub message: String,

    /// Line number (1-based, if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,

    /// Column number (1-based, if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,

    /// Source context (the problematic code snippet)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl Diagnostic {
    /// Create a new error diagnostic
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            line: None,
            column: None,
            context: None,
        }
    }

    /// Create a new warning diagnostic
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            line: None,
            column: None,
            context: None,
        }
    }

    /// Add line/column location
    pub fn with_location(mut self, line: usize, column: usize) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    /// Add context snippet
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

/// Metadata about the validated document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Detected or specified document type
    pub doc_type: DslType,

    /// Document ID (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Document name (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Referenced rules (for rulesets/pipelines)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rule_refs: Vec<String>,

    /// Referenced rulesets (for pipelines)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ruleset_refs: Vec<String>,

    /// Import paths
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        Self {
            doc_type: DslType::Auto,
            id: None,
            name: None,
            rule_refs: Vec::new(),
            ruleset_refs: Vec::new(),
            imports: Vec::new(),
        }
    }
}

/// Result of DSL validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the document is valid (no errors)
    pub valid: bool,

    /// List of errors (severity = Error)
    pub errors: Vec<Diagnostic>,

    /// List of warnings (severity = Warning)
    pub warnings: Vec<Diagnostic>,

    /// Document metadata (only populated if parsing succeeded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<DocumentMetadata>,

    /// Validation time in milliseconds
    pub validation_time_ms: u64,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn success(metadata: DocumentMetadata, validation_time_ms: u64) -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            metadata: Some(metadata),
            validation_time_ms,
        }
    }

    /// Create a failed validation result with a single error
    pub fn failure(error: Diagnostic, validation_time_ms: u64) -> Self {
        Self {
            valid: false,
            errors: vec![error],
            warnings: Vec::new(),
            metadata: None,
            validation_time_ms,
        }
    }

    /// Add a warning
    pub fn with_warning(mut self, warning: Diagnostic) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Add multiple errors
    pub fn with_errors(mut self, errors: Vec<Diagnostic>) -> Self {
        self.valid = false;
        self.errors.extend(errors);
        self
    }
}

/// DSL Validator for standalone validation
pub struct DslValidator {
    /// Compiler options
    options: CompilerOptions,
}

impl Default for DslValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl DslValidator {
    /// Create a new validator with default options
    pub fn new() -> Self {
        Self {
            options: CompilerOptions::default(),
        }
    }

    /// Create a validator with custom options
    pub fn with_options(options: CompilerOptions) -> Self {
        Self { options }
    }

    /// Validate YAML content
    ///
    /// # Arguments
    /// * `content` - YAML content to validate
    /// * `doc_type` - Document type (Auto for auto-detection)
    ///
    /// # Returns
    /// ValidationResult with errors, warnings, and metadata
    pub fn validate(&self, content: &str, doc_type: DslType) -> ValidationResult {
        let start = Instant::now();

        // Auto-detect document type if needed
        let detected_type = if doc_type == DslType::Auto {
            self.detect_type(content)
        } else {
            doc_type
        };

        match detected_type {
            DslType::Rule => self.validate_rule(content, start),
            DslType::Ruleset => self.validate_ruleset(content, start),
            DslType::Pipeline => self.validate_pipeline(content, start),
            DslType::Auto => {
                // If auto-detection failed, try all formats
                self.validate_any(content, start)
            }
        }
    }

    /// Detect document type from content
    fn detect_type(&self, content: &str) -> DslType {
        let trimmed = content.trim();

        // Check for top-level keys
        if trimmed.starts_with("rule:") || trimmed.contains("\nrule:") {
            return DslType::Rule;
        }
        if trimmed.starts_with("ruleset:") || trimmed.contains("\nruleset:") {
            return DslType::Ruleset;
        }
        if trimmed.starts_with("pipeline:") || trimmed.contains("\npipeline:") {
            return DslType::Pipeline;
        }

        // Try parsing YAML to detect
        if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(content) {
            if let serde_yaml::Value::Mapping(map) = yaml {
                if map.contains_key(&serde_yaml::Value::String("rule".to_string())) {
                    return DslType::Rule;
                }
                if map.contains_key(&serde_yaml::Value::String("ruleset".to_string())) {
                    return DslType::Ruleset;
                }
                if map.contains_key(&serde_yaml::Value::String("pipeline".to_string())) {
                    return DslType::Pipeline;
                }
            }
        }

        DslType::Auto // Could not detect
    }

    /// Validate a rule document
    fn validate_rule(&self, content: &str, start: Instant) -> ValidationResult {
        // Parse YAML
        let rule = match RuleParser::parse(content) {
            Ok(r) => r,
            Err(e) => {
                let diagnostic = self.parse_error_to_diagnostic(content, &e.to_string());
                return ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64);
            }
        };

        // Run semantic analysis
        let warnings = Vec::new();
        if self.options.enable_semantic_analysis {
            let mut analyzer = SemanticAnalyzer::new();
            if let Err(e) = analyzer.analyze_rule(&rule) {
                let diagnostic = Diagnostic::error("E002", format!("Semantic error: {}", e));
                return ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64);
            }
        }

        // Try compilation to catch any codegen errors
        let mut compiler = Compiler::with_options(self.options.clone());
        if let Err(e) = compiler.compile_rule(&rule) {
            let diagnostic = Diagnostic::error("E003", format!("Compilation error: {}", e));
            return ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64);
        }

        // Build metadata
        let metadata = DocumentMetadata {
            doc_type: DslType::Rule,
            id: Some(rule.id.clone()),
            name: Some(rule.name.clone()),
            rule_refs: Vec::new(),
            ruleset_refs: Vec::new(),
            imports: Vec::new(),
        };

        let mut result = ValidationResult::success(metadata, start.elapsed().as_millis() as u64);
        for warning in warnings {
            result = result.with_warning(warning);
        }
        result
    }

    /// Validate a ruleset document
    fn validate_ruleset(&self, content: &str, start: Instant) -> ValidationResult {
        // Parse YAML
        let ruleset = match RulesetParser::parse(content) {
            Ok(r) => r,
            Err(e) => {
                let diagnostic = self.parse_error_to_diagnostic(content, &e.to_string());
                return ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64);
            }
        };

        // Run semantic analysis
        if self.options.enable_semantic_analysis {
            let mut analyzer = SemanticAnalyzer::new();
            if let Err(e) = analyzer.analyze_ruleset(&ruleset) {
                let diagnostic = Diagnostic::error("E002", format!("Semantic error: {}", e));
                return ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64);
            }
        }

        // Try compilation
        let mut compiler = Compiler::with_options(self.options.clone());
        if let Err(e) = compiler.compile_ruleset(&ruleset) {
            let diagnostic = Diagnostic::error("E003", format!("Compilation error: {}", e));
            return ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64);
        }

        // Extract rule references (rules is Vec<String>)
        let rule_refs: Vec<String> = ruleset.rules.clone();

        // Build metadata
        let metadata = DocumentMetadata {
            doc_type: DslType::Ruleset,
            id: Some(ruleset.id.clone()),
            name: ruleset.name.clone(),
            rule_refs,
            ruleset_refs: ruleset.extends.clone().map(|e| vec![e]).unwrap_or_default(),
            imports: Vec::new(),
        };

        ValidationResult::success(metadata, start.elapsed().as_millis() as u64)
    }

    /// Validate a pipeline document
    fn validate_pipeline(&self, content: &str, start: Instant) -> ValidationResult {
        // Parse YAML
        let pipeline = match PipelineParser::parse(content) {
            Ok(p) => p,
            Err(e) => {
                let diagnostic = self.parse_error_to_diagnostic(content, &e.to_string());
                return ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64);
            }
        };

        // Run semantic analysis
        if self.options.enable_semantic_analysis {
            let mut analyzer = SemanticAnalyzer::new();
            if let Err(e) = analyzer.analyze_pipeline(&pipeline) {
                let diagnostic = Diagnostic::error("E002", format!("Semantic error: {}", e));
                return ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64);
            }
        }

        // Try compilation
        let mut compiler = Compiler::with_options(self.options.clone());
        if let Err(e) = compiler.compile_pipeline(&pipeline) {
            let diagnostic = Diagnostic::error("E003", format!("Compilation error: {}", e));
            return ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64);
        }

        // Extract references from steps
        let mut ruleset_refs = Vec::new();
        for step in &pipeline.steps {
            // Handle both new PipelineStep and legacy Step enum
            if let corint_core::ast::pipeline::StepDetails::Ruleset { ruleset } = &step.details {
                ruleset_refs.push(ruleset.clone());
            }
        }

        // Build metadata
        let metadata = DocumentMetadata {
            doc_type: DslType::Pipeline,
            id: Some(pipeline.id.clone()),
            name: Some(pipeline.name.clone()),
            rule_refs: Vec::new(),
            ruleset_refs,
            imports: Vec::new(),
        };

        ValidationResult::success(metadata, start.elapsed().as_millis() as u64)
    }

    /// Try to validate as any document type
    fn validate_any(&self, content: &str, start: Instant) -> ValidationResult {
        // Try each format in order
        if let Ok(_) = RuleParser::parse(content) {
            return self.validate_rule(content, start);
        }
        if let Ok(_) = RulesetParser::parse(content) {
            return self.validate_ruleset(content, start);
        }
        if let Ok(_) = PipelineParser::parse(content) {
            return self.validate_pipeline(content, start);
        }

        // None worked - return a generic error
        let diagnostic = Diagnostic::error(
            "E000",
            "Unable to parse document. Must be a valid rule, ruleset, or pipeline YAML.",
        );
        ValidationResult::failure(diagnostic, start.elapsed().as_millis() as u64)
    }

    /// Convert a parse error string to a diagnostic with location info
    fn parse_error_to_diagnostic(&self, content: &str, error: &str) -> Diagnostic {
        let mut diagnostic = Diagnostic::error("E001", format!("Parse error: {}", error));

        // Try to extract line/column from error message
        // serde_yaml errors often contain "at line X column Y"
        if let Some(line_start) = error.find("line ") {
            let line_str = &error[line_start + 5..];
            if let Some(end) = line_str.find(|c: char| !c.is_ascii_digit()) {
                if let Ok(line) = line_str[..end].parse::<usize>() {
                    diagnostic.line = Some(line);

                    // Try to find column
                    if let Some(col_start) = error.find("column ") {
                        let col_str = &error[col_start + 7..];
                        if let Some(end) = col_str.find(|c: char| !c.is_ascii_digit()) {
                            if let Ok(col) = col_str[..end].parse::<usize>() {
                                diagnostic.column = Some(col);
                            }
                        }
                    }

                    // Add context (the problematic line)
                    if let Some(context_line) = content.lines().nth(line.saturating_sub(1)) {
                        diagnostic.context = Some(context_line.to_string());
                    }
                }
            }
        }

        diagnostic
    }

    /// Validate multiple documents (e.g., multi-document YAML)
    pub fn validate_multi(&self, content: &str) -> Vec<ValidationResult> {
        let mut results = Vec::new();

        // Split by YAML document separator
        for doc in content.split("\n---\n") {
            let trimmed = doc.trim();
            if !trimmed.is_empty() && trimmed != "---" {
                results.push(self.validate(trimmed, DslType::Auto));
            }
        }

        if results.is_empty() {
            // No documents found, try as single document
            results.push(self.validate(content, DslType::Auto));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_rule() {
        let validator = DslValidator::new();
        let content = r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - event.amount > 100
  score: 50
"#;

        let result = validator.validate(content, DslType::Rule);
        assert!(result.valid);
        assert!(result.errors.is_empty());
        assert!(result.metadata.is_some());
        let meta = result.metadata.unwrap();
        assert_eq!(meta.id, Some("test_rule".to_string()));
    }

    #[test]
    fn test_validate_invalid_yaml() {
        let validator = DslValidator::new();
        let content = "invalid: yaml: content: [";

        let result = validator.validate(content, DslType::Auto);
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
        assert_eq!(result.errors[0].code, "E001");
    }

    #[test]
    fn test_validate_missing_field() {
        let validator = DslValidator::new();
        let content = r#"
rule:
  id: test_rule
  # missing name
  when:
    conditions:
      - event.amount > 100
  score: 50
"#;

        let result = validator.validate(content, DslType::Rule);
        assert!(!result.valid);
    }

    #[test]
    fn test_detect_type() {
        let validator = DslValidator::new();

        let rule_content = "rule:\n  id: test";
        assert_eq!(validator.detect_type(rule_content), DslType::Rule);

        let ruleset_content = "ruleset:\n  id: test";
        assert_eq!(validator.detect_type(ruleset_content), DslType::Ruleset);

        let pipeline_content = "pipeline:\n  id: test";
        assert_eq!(validator.detect_type(pipeline_content), DslType::Pipeline);
    }

    #[test]
    fn test_auto_detect_and_validate() {
        let validator = DslValidator::new();
        let content = r#"
rule:
  id: auto_detected
  name: Auto Detected Rule
  when:
    conditions:
      - event.x > 0
  score: 10
"#;

        let result = validator.validate(content, DslType::Auto);
        assert!(result.valid);
        let meta = result.metadata.unwrap();
        assert_eq!(meta.doc_type, DslType::Rule);
    }
}
