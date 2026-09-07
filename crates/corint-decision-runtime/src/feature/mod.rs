//! Feature Engineering Module
//!
//! This module provides a comprehensive feature engineering system for risk control,
//! including:
//! - Feature methods (count, sum, avg, count_distinct, etc.)
//! - Feature definitions and registry
//! - Feature execution engine with caching
//! - Pipeline integration

mod cache;
mod dependency;
mod expression;

pub mod definition;
pub mod executor;
pub mod extractor;
pub mod operator;
pub mod registry;

pub use definition::{FeatureDefinition, FeatureType};
pub use executor::FeatureExecutor;
pub use extractor::FeatureExtractor;
pub use operator::{
    CacheBackend, CacheConfig, FilterConfig, FilterOp, Operator, OperatorParams, WindowConfig,
    WindowUnit,
};
pub use registry::FeatureRegistry;

/// Check the shared Feature expression grammar and return inferred dependencies.
/// Does not compute a feature or access any data source.
pub fn expression_dependencies(expression: &str) -> anyhow::Result<Vec<String>> {
    expression::ExpressionEvaluator::extract_dependencies(expression)
}
