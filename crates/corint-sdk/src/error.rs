//! SDK error types

use thiserror::Error;

/// SDK error type
#[derive(Error, Debug)]
pub enum SdkError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Parser error
    #[error("Parser error: {0}")]
    ParseError(#[from] corint_parser::error::ParseError),

    /// Compiler error
    #[error("Compiler error: {0}")]
    CompileError(#[from] corint_compiler::error::CompileError),

    /// Runtime error
    #[error("Runtime error: {0}")]
    RuntimeError(#[from] corint_runtime::RuntimeError),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Invalid rule file
    #[error("Invalid rule file: {0}")]
    InvalidRuleFile(String),

    /// Engine not initialized
    #[error("Engine not initialized")]
    NotInitialized,

    /// Generic SDK error
    #[error("SDK error: {0}")]
    GenericError(String),
}

/// Result type for SDK operations
pub type Result<T> = std::result::Result<T, SdkError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error() {
        let error = SdkError::ConfigError("Invalid configuration".to_string());
        assert!(error.to_string().contains("Configuration error"));
        assert!(error.to_string().contains("Invalid configuration"));
    }

    #[test]
    fn test_invalid_rule_file() {
        let error = SdkError::InvalidRuleFile("rules.yaml".to_string());
        assert!(error.to_string().contains("Invalid rule file"));
        assert!(error.to_string().contains("rules.yaml"));
    }

    #[test]
    fn test_not_initialized_error() {
        let error = SdkError::NotInitialized;
        assert_eq!(error.to_string(), "Engine not initialized");
    }

    #[test]
    fn test_generic_error() {
        let error = SdkError::GenericError("Something went wrong".to_string());
        assert!(error.to_string().contains("SDK error"));
        assert!(error.to_string().contains("Something went wrong"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let sdk_error: SdkError = io_error.into();
        assert!(sdk_error.to_string().contains("I/O error"));
        assert!(sdk_error.to_string().contains("File not found"));
    }

    #[test]
    fn test_result_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_result_err() {
        let result: Result<i32> = Err(SdkError::NotInitialized);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Engine not initialized");
    }

    #[test]
    fn test_error_debug_format() {
        let error = SdkError::ConfigError("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("ConfigError"));
    }

    #[test]
    fn test_multiple_error_types() {
        let errors = vec![
            SdkError::ConfigError("config".to_string()),
            SdkError::InvalidRuleFile("rule.yaml".to_string()),
            SdkError::NotInitialized,
            SdkError::GenericError("generic".to_string()),
        ];

        assert_eq!(errors.len(), 4);
        assert!(errors[0].to_string().contains("Configuration error"));
        assert!(errors[1].to_string().contains("Invalid rule file"));
        assert!(errors[2].to_string().contains("Engine not initialized"));
        assert!(errors[3].to_string().contains("SDK error"));
    }
}
