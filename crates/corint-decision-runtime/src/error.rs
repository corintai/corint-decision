//! Runtime error types

use thiserror::Error;

/// Runtime error
#[derive(Error, Debug)]
pub enum RuntimeError {
    /// Structured error from a strict Core program; no raw operands are included.
    #[error("{code} in {source_file} ({resource_id}) at {field_path}: {message}")]
    CoreExecution {
        code: String,
        source_file: String,
        resource_id: String,
        field_path: String,
        message: String,
    },
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

    /// External API call failed
    #[error("External API call failed: {0}")]
    ServiceCallFailed(String),

    /// Reserved field in event data
    #[error("Reserved field '{field}': {reason}")]
    ReservedField {
        /// The reserved field name
        field: String,
        /// Reason why it's reserved
        reason: String,
    },

    /// Core error (from corint-decision-model)
    #[error("Core error: {0}")]
    CoreError(
        #[from]
        #[source]
        corint_decision_model::error::CoreError,
    ),

    /// Invalid value error
    #[error("Invalid value: {0}")]
    InvalidValue(String),

    /// Generic runtime error
    #[error("Runtime error: {0}")]
    RuntimeError(String),
}

/// Result type for runtime operations
pub type Result<T> = std::result::Result<T, RuntimeError>;
