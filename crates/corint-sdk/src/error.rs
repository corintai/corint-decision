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
