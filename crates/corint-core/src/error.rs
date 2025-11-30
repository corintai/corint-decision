//! Error types for CORINT Core

use thiserror::Error;

/// Core error type
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Type error: {0}")]
    TypeError(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Field not found: {0}")]
    FieldNotFound(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
