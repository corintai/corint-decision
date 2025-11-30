//! Type system for CORINT
//!
//! This module contains the runtime type system including:
//! - Value types
//! - Schema definitions
//! - Value validators

pub mod schema;
pub mod validator;
pub mod value;

pub use schema::{FieldType, Schema, SchemaField};
pub use validator::{ValidationError, Validator};
pub use value::Value;
