//! Error types for CORINT LLM module

use thiserror::Error;

/// Result type alias for LLM operations
pub type Result<T> = std::result::Result<T, LLMError>;

/// LLM module errors
#[derive(Debug, Error)]
pub enum LLMError {
    /// External API call failed
    #[error("External API call failed: {0}")]
    ApiCallFailed(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// YAML parsing error
    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    /// HTTP request error
    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Invalid response format
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),

    /// Generation failed
    #[error("Code generation failed: {0}")]
    GenerationFailed(String),

    /// Validation failed
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<String> for LLMError {
    fn from(s: String) -> Self {
        LLMError::Other(s)
    }
}

impl From<&str> for LLMError {
    fn from(s: &str) -> Self {
        LLMError::Other(s.to_string())
    }
}
