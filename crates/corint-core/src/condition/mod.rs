//! Condition Parsing Module
//!
//! This module provides shared condition parsing logic used by:
//! - Feature definitions (when conditions for database filtering)
//! - Rule definitions (when conditions for rule triggering)
//! - Pipeline definitions (when conditions for step/route execution)
//! - Registry (when conditions for event routing)
//!
//! # Syntax
//!
//! ## Simple Conditions
//! ```yaml
//! when: field == "value"
//! when: amount > 100
//! when: status != "failed"
//! ```
//!
//! ## Complex Conditions (all/any)
//! ```yaml
//! when:
//!   all:
//!     - field1 == "value1"
//!     - field2 > 100
//!   any:
//!     - status == "failed"
//!     - attempts > 3
//! ```
//!
//! ## Supported Operators
//! - `==` (equal)
//! - `!=` (not equal)
//! - `>` (greater than)
//! - `>=` (greater than or equal)
//! - `<` (less than)
//! - `<=` (less than or equal)
//! - `in` (membership)
//! - `not in` (not membership)
//! - `contains` (string contains)
//! - `starts_with` (string starts with)
//! - `ends_with` (string ends with)
//!
//! ## Template Variables
//! Template variables use `{context.field}` syntax:
//! ```yaml
//! when: user_id == "{event.user_id}"
//! when: amount > {event.threshold}
//! ```

mod parser;
mod types;

pub use parser::{ConditionParser, ParseError};
pub use types::{
    ParsedCondition, ParsedConditionGroup, ParsedConditionItem, ParsedValue,
    WhenClause, WhenClauseComplex, WhenClauseItem,
};
