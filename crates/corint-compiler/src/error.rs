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
}

/// Result type for compiler operations
pub type Result<T> = std::result::Result<T, CompileError>;
