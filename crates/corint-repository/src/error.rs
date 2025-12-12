//! Error types for the repository layer

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for repository operations
pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Errors that can occur during repository operations
#[derive(Error, Debug)]
pub enum RepositoryError {
    /// File not found at the specified path
    #[error("Artifact not found: {path}")]
    NotFound { path: String },

    /// I/O error occurred
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML parsing error
    #[error("Failed to parse YAML: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    /// Parser error from corint-parser
    #[error("Parser error: {0}")]
    Parser(String),

    /// Invalid path provided
    #[error("Invalid path: {path}")]
    InvalidPath { path: PathBuf },

    /// Artifact ID not found in any standard location
    #[error("Artifact ID not found: {id}")]
    IdNotFound { id: String },

    /// Database error (when database feature is enabled)
    #[cfg(feature = "postgres")]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Cache error
    #[error("Cache error: {0}")]
    Cache(String),

    /// API error (HTTP requests, authentication, etc.)
    #[error("API error: {0}")]
    ApiError(String),

    /// Parse error (generic YAML/JSON parsing)
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Generic error
    #[error("Repository error: {0}")]
    Other(String),
}

impl From<corint_parser::ParseError> for RepositoryError {
    fn from(err: corint_parser::ParseError) -> Self {
        RepositoryError::Parser(err.to_string())
    }
}
