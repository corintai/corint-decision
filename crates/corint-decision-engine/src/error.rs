//! Decision engine error types.

use thiserror::Error;

/// Error returned by decision-engine operations.
#[derive(Error, Debug)]
pub enum EngineError {
    /// Structured error from the opt-in CDL Core gate.
    #[error("CDL Core: {0}")]
    Core(#[from] corint_decision_compiler::core::CoreError),
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Parser error.
    #[error("Parser error: {0}")]
    ParseError(
        #[from]
        #[source]
        corint_decision_dsl_parser::error::ParseError,
    ),

    /// Compiler error.
    #[error("Compiler error: {0}")]
    CompileError(
        #[from]
        #[source]
        corint_decision_compiler::error::CompileError,
    ),

    /// Runtime error.
    #[error("Runtime error: {0}")]
    RuntimeError(#[source] corint_decision_runtime::RuntimeError),

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(
        #[from]
        #[source]
        std::io::Error,
    ),

    /// Invalid rule file.
    #[error("Invalid rule file: {0}")]
    InvalidRuleFile(String),

    /// Engine not initialized.
    #[error("Engine not initialized")]
    NotInitialized,

    /// Generic engine error.
    #[error("Engine error: {0}")]
    GenericError(String),
}

impl From<corint_decision_runtime::RuntimeError> for EngineError {
    fn from(error: corint_decision_runtime::RuntimeError) -> Self {
        match error {
            corint_decision_runtime::RuntimeError::CoreExecution {
                code,
                source_file,
                resource_id,
                field_path,
                message,
            } => Self::Core(corint_decision_compiler::core::diagnostic(
                &source_file,
                &field_path,
                "execute",
                &code,
                format!("{resource_id}: {message}"),
            )),
            other => Self::RuntimeError(other),
        }
    }
}

/// Result type for decision-engine operations.
pub type Result<T> = std::result::Result<T, EngineError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error() {
        let error = EngineError::Config("Invalid configuration".to_string());
        assert!(error.to_string().contains("Configuration error"));
        assert!(error.to_string().contains("Invalid configuration"));
    }

    #[test]
    fn test_invalid_rule_file() {
        let error = EngineError::InvalidRuleFile("rules.yaml".to_string());
        assert!(error.to_string().contains("Invalid rule file"));
        assert!(error.to_string().contains("rules.yaml"));
    }

    #[test]
    fn test_not_initialized_error() {
        let error = EngineError::NotInitialized;
        assert_eq!(error.to_string(), "Engine not initialized");
    }

    #[test]
    fn test_generic_error() {
        let error = EngineError::GenericError("Something went wrong".to_string());
        assert!(error.to_string().contains("Engine error"));
        assert!(error.to_string().contains("Something went wrong"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let engine_error: EngineError = io_error.into();
        assert!(engine_error.to_string().contains("I/O error"));
        assert!(engine_error.to_string().contains("File not found"));
    }

    #[test]
    fn test_result_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        if let Ok(value) = result {
            assert_eq!(value, 42);
        }
    }

    #[test]
    fn test_result_err() {
        let result: Result<i32> = Err(EngineError::NotInitialized);
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.to_string(), "Engine not initialized");
        }
    }

    #[test]
    fn test_error_debug_format() {
        let error = EngineError::Config("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Config"));
    }

    #[test]
    fn test_multiple_error_types() {
        let errors = [
            EngineError::Config("config".to_string()),
            EngineError::InvalidRuleFile("rule.yaml".to_string()),
            EngineError::NotInitialized,
            EngineError::GenericError("generic".to_string()),
        ];

        assert_eq!(errors.len(), 4);
        assert!(errors[0].to_string().contains("Configuration error"));
        assert!(errors[1].to_string().contains("Invalid rule file"));
        assert!(errors[2].to_string().contains("Engine not initialized"));
        assert!(errors[3].to_string().contains("Engine error"));
    }
}
