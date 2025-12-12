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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_not_found_error() {
        let err = RepositoryError::NotFound {
            path: "test/path.yaml".to_string(),
        };

        assert!(err.to_string().contains("Artifact not found"));
        assert!(err.to_string().contains("test/path.yaml"));
    }

    #[test]
    fn test_parser_error() {
        let err = RepositoryError::Parser("Invalid YAML syntax".to_string());

        assert!(err.to_string().contains("Parser error"));
        assert!(err.to_string().contains("Invalid YAML syntax"));
    }

    #[test]
    fn test_invalid_path_error() {
        let path = PathBuf::from("/invalid/path");
        let err = RepositoryError::InvalidPath { path: path.clone() };

        assert!(err.to_string().contains("Invalid path"));
    }

    #[test]
    fn test_id_not_found_error() {
        let err = RepositoryError::IdNotFound {
            id: "rule_123".to_string(),
        };

        assert!(err.to_string().contains("Artifact ID not found"));
        assert!(err.to_string().contains("rule_123"));
    }

    #[test]
    fn test_cache_error() {
        let err = RepositoryError::Cache("Cache full".to_string());

        assert!(err.to_string().contains("Cache error"));
        assert!(err.to_string().contains("Cache full"));
    }

    #[test]
    fn test_api_error() {
        let err = RepositoryError::ApiError("HTTP 404".to_string());

        assert!(err.to_string().contains("API error"));
        assert!(err.to_string().contains("HTTP 404"));
    }

    #[test]
    fn test_parse_error() {
        let err = RepositoryError::ParseError("JSON parse failed".to_string());

        assert!(err.to_string().contains("Parse error"));
        assert!(err.to_string().contains("JSON parse failed"));
    }

    #[test]
    fn test_other_error() {
        let err = RepositoryError::Other("Unknown error".to_string());

        assert!(err.to_string().contains("Repository error"));
        assert!(err.to_string().contains("Unknown error"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let repo_err: RepositoryError = io_err.into();

        assert!(repo_err.to_string().contains("I/O error"));
    }

    #[test]
    fn test_repository_result_ok() {
        let result: RepositoryResult<i32> = Ok(42);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_repository_result_err() {
        let result: RepositoryResult<i32> = Err(RepositoryError::Other("test".to_string()));

        assert!(result.is_err());
    }

    #[test]
    fn test_error_debug_format() {
        let err = RepositoryError::Cache("test".to_string());
        let debug_str = format!("{:?}", err);

        assert!(debug_str.contains("Cache"));
    }
}
