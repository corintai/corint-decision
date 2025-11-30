//! Parser error types

use thiserror::Error;

/// Parser error
#[derive(Error, Debug)]
pub enum ParseError {
    /// YAML parsing error
    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    /// Missing required field
    #[error("Missing required field: {field}")]
    MissingField { field: String },

    /// Invalid field value
    #[error("Invalid value for field '{field}': {message}")]
    InvalidValue { field: String, message: String },

    /// Invalid expression syntax
    #[error("Invalid expression syntax: {0}")]
    InvalidExpression(String),

    /// Invalid operator
    #[error("Invalid operator: {0}")]
    InvalidOperator(String),

    /// Type mismatch
    #[error("Type mismatch for field '{field}': expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },

    /// Unknown field
    #[error("Unknown field: {0}")]
    UnknownField(String),

    /// Generic parse error
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Result type for parser operations
pub type Result<T> = std::result::Result<T, ParseError>;
