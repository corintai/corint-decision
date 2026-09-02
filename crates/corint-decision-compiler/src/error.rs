//! Compiler error types

use thiserror::Error;

/// Compiler error
#[derive(Error, Debug)]
pub enum CompileError {
    /// Undefined symbol
    #[error("Undefined symbol: {0}")]
    UndefinedSymbol(String),

    /// Type error
    #[error("Type error: {0}")]
    TypeError(String),

    /// Invalid expression
    #[error("Invalid expression: {0}")]
    InvalidExpression(String),

    /// Unsupported feature
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    /// Generic compilation error
    #[error("Compilation error: {0}")]
    CompileError(String),

    // Import-related errors
    /// Import file not found
    #[error("Import not found: {path}")]
    ImportNotFound {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Invalid YAML in imported file
    #[error("Invalid YAML in {path}")]
    InvalidYaml {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },

    /// Parse error in imported file
    #[error("Parse error in {path}: {message}")]
    ParseError { path: String, message: String },

    /// No rule found in file
    #[error("No rule found in file: {path}")]
    NoRuleInFile { path: String },

    /// No ruleset found in file
    #[error("No ruleset found in file: {path}")]
    NoRulesetInFile { path: String },

    /// No pipeline found in file
    #[error("No pipeline found in file: {path}")]
    NoPipelineInFile { path: String },

    /// Duplicate rule ID
    #[error("Duplicate rule ID: '{id}'\n  First defined in: {first_defined}\n  Also defined in: {also_defined}")]
    DuplicateRuleId {
        id: String,
        first_defined: String,
        also_defined: String,
    },

    /// Duplicate ruleset ID
    #[error("Duplicate ruleset ID: '{id}'\n  First defined in: {first_defined}\n  Also defined in: {also_defined}")]
    DuplicateRulesetId {
        id: String,
        first_defined: String,
        also_defined: String,
    },

    /// ID conflict between rule and ruleset
    #[error("ID conflict: '{id}'\n  {conflict}")]
    IdConflict { id: String, conflict: String },

    /// Rule not found
    #[error("Rule not found: {id}")]
    RuleNotFound { id: String },

    /// Ruleset not found
    #[error("Ruleset not found: {id}")]
    RulesetNotFound { id: String },

    /// Circular dependency detected
    #[error("Circular dependency detected: {path}\n  Loading stack: {}", stack.join(" -> "))]
    CircularDependency { path: String, stack: Vec<String> },

    /// Extends parent ruleset not found
    #[error("Ruleset '{child_id}' extends '{extends_id}', but parent ruleset not found\n  Child path: {child_path}\n  Hint: Make sure to import the parent ruleset before the child")]
    ExtendsNotFound {
        child_id: String,
        extends_id: String,
        child_path: String,
    },

    /// Circular extends chain detected
    #[error("Circular extends chain detected: '{child_id}' extends '{extends_id}', which eventually extends back to '{child_id}'")]
    CircularExtends {
        child_id: String,
        extends_id: String,
    },
}

/// Result type for compiler operations
pub type Result<T> = std::result::Result<T, CompileError>;
