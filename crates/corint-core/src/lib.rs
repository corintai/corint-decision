//! CORINT Core - Core types and definitions for CORINT Decision Engine
//!
//! This crate provides the fundamental types used across the CORINT ecosystem:
//! - Value types for runtime data
//! - AST (Abstract Syntax Tree) definitions
//! - IR (Intermediate Representation) definitions
//! - Error types

pub mod ast;
pub mod condition;
pub mod error;
pub mod ir;
pub mod types;

// Keep old path for backward compatibility
#[doc(hidden)]
pub use types::value;

// Re-export commonly used types
pub use error::CoreError;
pub use types::Value;
