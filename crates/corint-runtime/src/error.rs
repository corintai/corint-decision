//! Runtime error types

use thiserror::Error;

/// Runtime error
#[derive(Error, Debug)]
pub enum RuntimeError {
    /// Stack underflow
    #[error("Stack underflow")]
    StackUnderflow,

    /// Type error
    #[error("Type error: {0}")]
    TypeError(String),

    /// Field not found
    #[error("Field not found: {0}")]
    FieldNotFound(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Division by zero
    #[error("Division by zero")]
    DivisionByZero,

    /// Program counter out of bounds
    #[error("Program counter out of bounds: {0}")]
    PCOutOfBounds(usize),

    /// Generic runtime error
    #[error("Runtime error: {0}")]
    RuntimeError(String),
}

/// Result type for runtime operations
pub type Result<T> = std::result::Result<T, RuntimeError>;
