//! CORINT Core - Core types and definitions for CORINT Decision Engine
//!
//! This crate provides the fundamental types used across the CORINT ecosystem:
//! - Value types for runtime data
//! - AST (Abstract Syntax Tree) definitions
//! - IR (Intermediate Representation) definitions
//! - Error types

pub mod value;
pub mod ast;
pub mod error;

// Re-export commonly used types
pub use value::Value;
pub use error::CoreError;
