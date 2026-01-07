//! Feature Operators (Deprecated)
//!
//! This module contains the old operator-based architecture and is kept for backward compatibility with existing tests.
//!
//! **Note:** In DSL v0.2, the concept of "operators" has been renamed to "methods" and integrated into a type-based
//! feature system (see `definition.rs`). This module will be removed once all code is migrated.

use crate::datasource::{
    Aggregation, AggregationType, DataSourceClient, Filter, FilterOperator, Query, QueryType,
    TimeUnit, TimeWindow, TimeWindowType,
};
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Operator type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operator", rename_all = "snake_case")]
pub enum Operator {
    /// Count events matching filters
    Count(CountOperator),

    /// Sum numeric field values
    Sum(SumOperator),

    /// Average of numeric field values
    Avg(AvgOperator),

    /// Maximum value
    Max(MaxOperator),

    /// Minimum value
    Min(MinOperator),

    /// Count distinct values
    CountDistinct(CountDistinctOperator),

    /// Cross-dimension count (e.g., devices per IP)
    CrossDimensionCount(CrossDimensionCountOperator),

    /// Get first occurrence timestamp
    FirstSeen(FirstSeenOperator),

    /// Get last occurrence timestamp
    LastSeen(LastSeenOperator),

    /// Calculate time since event
    TimeSince(TimeSinceOperator),

    /// Lookup from feature store
    FeatureStoreLookup(FeatureStoreLookupOperator),

    /// Lookup from profile table
    ProfileLookup(ProfileLookupOperator),

    /// Velocity check
    Velocity(VelocityOperator),

    /// Custom expression
    Expression(ExpressionOperator),
}

/// Common parameters for operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorParams {
    /// Data source name (optional, defaults to "default")
    /// Examples: "clickhouse_events", "postgresql_profiles", "redis_features"
    #[serde(default)]
    pub datasource: Option<String>,

    /// Data entity (table/collection)
    pub entity: String,

    /// Primary dimension (e.g., user_id, device_id)
    pub dimension: String,

    /// Dimension value (supports template like "${event.user_id}")
    pub dimension_value: String,

    /// Time window
    #[serde(default)]
    pub window: Option<WindowConfig>,

    /// Additional filters
    #[serde(default)]
    pub filters: Vec<FilterConfig>,

    /// Cache configuration
    #[serde(default)]
    pub cache: Option<CacheConfig>,
}

/// Time window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub value: u64,
    pub unit: WindowUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowUnit {
    Minutes,
    Hours,
    Days,
}

impl WindowUnit {
    pub fn to_time_unit(&self) -> TimeUnit {
        match self {
            WindowUnit::Minutes => TimeUnit::Minutes,
            WindowUnit::Hours => TimeUnit::Hours,
            WindowUnit::Days => TimeUnit::Days,
        }
    }
}

/// Filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub field: String,
    pub operator: FilterOp,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterOp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    NotIn,
}

impl FilterOp {
    pub fn to_filter_operator(&self) -> FilterOperator {
        match self {
            FilterOp::Eq => FilterOperator::Eq,
            FilterOp::Ne => FilterOperator::Ne,
            FilterOp::Gt => FilterOperator::Gt,
            FilterOp::Gte => FilterOperator::Ge,
            FilterOp::Lt => FilterOperator::Lt,
            FilterOp::Lte => FilterOperator::Le,
            FilterOp::In => FilterOperator::In,
            FilterOp::NotIn => FilterOperator::NotIn,
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl: u64,
    #[serde(default = "default_cache_backend")]
    pub backend: CacheBackend,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CacheBackend {
    Local,
    Redis,
}

fn default_cache_backend() -> CacheBackend {
    CacheBackend::Redis
}

// ============================================================================
// Count Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountOperator {
    #[serde(flatten)]
    pub params: OperatorParams,
}

impl CountOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let dimension_value = resolve_template(&self.params.dimension_value, context)?;

        let query = Query {
            query_type: QueryType::Count,
            entity: self.params.entity.clone(),
            filters: build_filters(&self.params, &dimension_value, context)?,
            time_window: build_time_window(&self.params)?,
            aggregations: vec![],
            group_by: vec![],
            limit: None,
        };

        let result = datasource.query(query).await?;

        // Extract count from result
        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("count") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Number(0.0))
    }
}

// ============================================================================
// Sum Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SumOperator {
    #[serde(flatten)]
    pub params: OperatorParams,

    /// Field to sum
    pub field: String,
}

impl SumOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let dimension_value = resolve_template(&self.params.dimension_value, context)?;

        let query = Query {
            query_type: QueryType::Aggregate,
            entity: self.params.entity.clone(),
            filters: build_filters(&self.params, &dimension_value, context)?,
            time_window: build_time_window(&self.params)?,
            aggregations: vec![Aggregation {
                agg_type: AggregationType::Sum,
                field: Some(self.field.clone()),
                output_name: "sum".to_string(),
            }],
            group_by: vec![],
            limit: None,
        };

        let result = datasource.query(query).await?;

        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("sum") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Number(0.0))
    }
}

// ============================================================================
// Avg Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvgOperator {
    #[serde(flatten)]
    pub params: OperatorParams,

    /// Field to average
    pub field: String,
}

impl AvgOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let dimension_value = resolve_template(&self.params.dimension_value, context)?;

        let query = Query {
            query_type: QueryType::Aggregate,
            entity: self.params.entity.clone(),
            filters: build_filters(&self.params, &dimension_value, context)?,
            time_window: build_time_window(&self.params)?,
            aggregations: vec![Aggregation {
                agg_type: AggregationType::Avg,
                field: Some(self.field.clone()),
                output_name: "avg".to_string(),
            }],
            group_by: vec![],
            limit: None,
        };

        let result = datasource.query(query).await?;

        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("avg") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Number(0.0))
    }
}

// ============================================================================
// Max Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaxOperator {
    #[serde(flatten)]
    pub params: OperatorParams,

    /// Field to get maximum
    pub field: String,
}

impl MaxOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let dimension_value = resolve_template(&self.params.dimension_value, context)?;

        let query = Query {
            query_type: QueryType::Aggregate,
            entity: self.params.entity.clone(),
            filters: build_filters(&self.params, &dimension_value, context)?,
            time_window: build_time_window(&self.params)?,
            aggregations: vec![Aggregation {
                agg_type: AggregationType::Max,
                field: Some(self.field.clone()),
                output_name: "max".to_string(),
            }],
            group_by: vec![],
            limit: None,
        };

        let result = datasource.query(query).await?;

        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("max") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Null)
    }
}

// ============================================================================
// Min Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinOperator {
    #[serde(flatten)]
    pub params: OperatorParams,

    /// Field to get minimum
    pub field: String,
}

impl MinOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let dimension_value = resolve_template(&self.params.dimension_value, context)?;

        let query = Query {
            query_type: QueryType::Aggregate,
            entity: self.params.entity.clone(),
            filters: build_filters(&self.params, &dimension_value, context)?,
            time_window: build_time_window(&self.params)?,
            aggregations: vec![Aggregation {
                agg_type: AggregationType::Min,
                field: Some(self.field.clone()),
                output_name: "min".to_string(),
            }],
            group_by: vec![],
            limit: None,
        };

        let result = datasource.query(query).await?;

        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("min") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Null)
    }
}

// ============================================================================
// CountDistinct Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountDistinctOperator {
    #[serde(flatten)]
    pub params: OperatorParams,

    /// Field to count distinct values of
    pub distinct_field: String,
}

impl CountDistinctOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let dimension_value = resolve_template(&self.params.dimension_value, context)?;

        let query = Query {
            query_type: QueryType::CountDistinct,
            entity: self.params.entity.clone(),
            filters: build_filters(&self.params, &dimension_value, context)?,
            time_window: build_time_window(&self.params)?,
            aggregations: vec![Aggregation {
                agg_type: AggregationType::CountDistinct,
                field: Some(self.distinct_field.clone()),
                output_name: "count_distinct".to_string(),
            }],
            group_by: vec![],
            limit: None,
        };

        let result = datasource.query(query).await?;

        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("count_distinct") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Number(0.0))
    }
}

// ============================================================================
// CrossDimensionCount Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDimensionCountOperator {
    /// Data entity
    pub entity: String,

    /// Primary dimension
    pub primary_dimension: String,

    /// Primary dimension value
    pub primary_value: String,

    /// Secondary dimension to count
    pub secondary_dimension: String,

    /// Time window
    #[serde(default)]
    pub window: Option<WindowConfig>,

    /// Additional filters
    #[serde(default)]
    pub filters: Vec<FilterConfig>,
}

impl CrossDimensionCountOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let primary_value = resolve_template(&self.primary_value, context)?;

        let mut filters = vec![Filter {
            field: self.primary_dimension.clone(),
            operator: FilterOperator::Eq,
            value: Value::String(primary_value),
        }];

        // Add additional filters
        for filter_config in &self.filters {
            let value = resolve_value(&filter_config.value, context)?;
            filters.push(Filter {
                field: filter_config.field.clone(),
                operator: filter_config.operator.to_filter_operator(),
                value,
            });
        }

        let time_window = self.window.as_ref().map(|window| TimeWindow {
            window_type: TimeWindowType::Relative(crate::datasource::RelativeWindow {
                value: window.value,
                unit: window.unit.to_time_unit(),
            }),
            time_field: "event_timestamp".to_string(), // PostgreSQL events table uses event_timestamp
        });

        let query = Query {
            query_type: QueryType::CountDistinct,
            entity: self.entity.clone(),
            filters,
            time_window,
            aggregations: vec![Aggregation {
                agg_type: AggregationType::CountDistinct,
                field: Some(self.secondary_dimension.clone()),
                output_name: "count".to_string(),
            }],
            group_by: vec![],
            limit: None,
        };

        let result = datasource.query(query).await?;

        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("count") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Number(0.0))
    }
}

// ============================================================================
// FirstSeen Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstSeenOperator {
    pub entity: String,
    pub dimension: String,
    pub dimension_value: String,
}

impl FirstSeenOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let dimension_value = resolve_template(&self.dimension_value, context)?;

        let query = Query {
            query_type: QueryType::Aggregate,
            entity: self.entity.clone(),
            filters: vec![Filter {
                field: self.dimension.clone(),
                operator: FilterOperator::Eq,
                value: Value::String(dimension_value),
            }],
            time_window: None,
            aggregations: vec![Aggregation {
                agg_type: AggregationType::Min,
                field: Some("timestamp".to_string()),
                output_name: "first_seen".to_string(),
            }],
            group_by: vec![],
            limit: None,
        };

        let result = datasource.query(query).await?;

        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("first_seen") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Null)
    }
}

// ============================================================================
// LastSeen Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastSeenOperator {
    pub entity: String,
    pub dimension: String,
    pub dimension_value: String,
    #[serde(default)]
    pub filters: Vec<FilterConfig>,
}

impl LastSeenOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let dimension_value = resolve_template(&self.dimension_value, context)?;

        let mut filters = vec![Filter {
            field: self.dimension.clone(),
            operator: FilterOperator::Eq,
            value: Value::String(dimension_value),
        }];

        for filter_config in &self.filters {
            let value = resolve_value(&filter_config.value, context)?;
            filters.push(Filter {
                field: filter_config.field.clone(),
                operator: filter_config.operator.to_filter_operator(),
                value,
            });
        }

        let query = Query {
            query_type: QueryType::Aggregate,
            entity: self.entity.clone(),
            filters,
            time_window: None,
            aggregations: vec![Aggregation {
                agg_type: AggregationType::Max,
                field: Some("timestamp".to_string()),
                output_name: "last_seen".to_string(),
            }],
            group_by: vec![],
            limit: None,
        };

        let result = datasource.query(query).await?;

        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("last_seen") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Null)
    }
}

// ============================================================================
// TimeSince Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSinceOperator {
    pub entity: String,
    pub dimension: String,
    pub dimension_value: String,
    #[serde(default)]
    pub filters: Vec<FilterConfig>,
    pub unit: WindowUnit,
}

impl TimeSinceOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        // Get first seen timestamp
        let first_seen_op = FirstSeenOperator {
            entity: self.entity.clone(),
            dimension: self.dimension.clone(),
            dimension_value: self.dimension_value.clone(),
        };

        let _first_seen = first_seen_op.execute(datasource, context).await?;

        // TODO: Calculate elapsed time
        // For now, return placeholder
        Ok(Value::Number(0.0))
    }
}

// ============================================================================
// FeatureStoreLookup Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStoreLookupOperator {
    pub datasource: String,
    pub key: String,
    #[serde(default)]
    pub fallback: Option<Value>,
}

impl FeatureStoreLookupOperator {
    pub async fn execute(
        &self,
        _datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let _key = resolve_template(&self.key, context)?;

        // TODO: Implement Redis lookup
        // For now, return fallback or null
        Ok(self.fallback.clone().unwrap_or(Value::Null))
    }
}

// ============================================================================
// ProfileLookup Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileLookupOperator {
    pub datasource: String,
    pub table: String,
    pub dimension: String,
    pub dimension_value: String,
    pub field: String,
}

impl ProfileLookupOperator {
    pub async fn execute(
        &self,
        _datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let _dimension_value = resolve_template(&self.dimension_value, context)?;

        // TODO: Implement SQL lookup
        // SELECT {field} FROM {table} WHERE {dimension} = {dimension_value}
        Ok(Value::Null)
    }
}

// ============================================================================
// Velocity Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocityOperator {
    #[serde(flatten)]
    pub params: OperatorParams,
    pub threshold: i64,
}

impl VelocityOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        // Count events
        let count_op = CountOperator {
            params: self.params.clone(),
        };

        let count_value = count_op.execute(datasource, context).await?;

        let count = match count_value {
            Value::Number(n) => n as i64,
            _ => 0,
        };

        let exceeds = count > self.threshold;

        Ok(Value::Bool(exceeds))
    }
}

// ============================================================================
// Expression Operator
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionOperator {
    pub expression: String,
}

impl ExpressionOperator {
    pub async fn execute(&self, _context: &HashMap<String, Value>) -> Result<Value> {
        // TODO: Implement expression evaluation
        // For now, return null
        Ok(Value::Null)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Resolve template string with context values
/// Supports:
/// - Direct reference: "event.user_id" -> lookup context["event.user_id"]
/// - String interpolation: "prefix:${event.user_id}:suffix" -> "prefix:value:suffix"
fn resolve_template(template: &str, context: &HashMap<String, Value>) -> Result<String> {
    // Check for string interpolation: contains "${...}"
    if template.contains("${") {
        let mut result = template.to_string();
        let mut search_start = 0;

        while let Some(start) = result[search_start..].find("${") {
            let start = search_start + start;
            if let Some(end_offset) = result[start..].find('}') {
                let end = start + end_offset;
                let key = &result[start + 2..end];
                if let Some(value) = context.get(key) {
                    let replacement = value_to_string(value);
                    result = format!("{}{}{}", &result[..start], replacement, &result[end + 1..]);
                    search_start = start + replacement.len();
                } else {
                    return Err(RuntimeError::RuntimeError(format!(
                        "Template variable not found: {}",
                        key
                    )));
                }
            } else {
                break;
            }
        }
        return Ok(result);
    }

    // Direct reference: "event.user_id" -> lookup context["event.user_id"]
    if let Some(value) = context.get(template) {
        return Ok(value_to_string(value));
    }

    // Return as-is if not a template
    Ok(template.to_string())
}

/// Convert Value to String representation
fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(_) => "[array]".to_string(),
        Value::Object(_) => "{object}".to_string(),
    }
}

/// Resolve value from template
fn resolve_value(value: &Value, context: &HashMap<String, Value>) -> Result<Value> {
    match value {
        Value::String(s) => {
            if s.starts_with("${") && s.ends_with('}') {
                let key = &s[2..s.len() - 1];
                context
                    .get(key)
                    .cloned()
                    .ok_or_else(|| RuntimeError::RuntimeError(format!("Key not found: {}", key)))
            } else {
                Ok(value.clone())
            }
        }
        _ => Ok(value.clone()),
    }
}

/// Build filters from operator params
fn build_filters(
    params: &OperatorParams,
    dimension_value: &str,
    context: &HashMap<String, Value>,
) -> Result<Vec<Filter>> {
    let mut filters = vec![Filter {
        field: params.dimension.clone(),
        operator: FilterOperator::Eq,
        value: Value::String(dimension_value.to_string()),
    }];

    for filter_config in &params.filters {
        let value = resolve_value(&filter_config.value, context)?;
        filters.push(Filter {
            field: filter_config.field.clone(),
            operator: filter_config.operator.to_filter_operator(),
            value,
        });
    }

    Ok(filters)
}

/// Build time window from operator params
fn build_time_window(params: &OperatorParams) -> Result<Option<TimeWindow>> {
    if let Some(window) = &params.window {
        Ok(Some(TimeWindow {
            window_type: TimeWindowType::Relative(crate::datasource::RelativeWindow {
                value: window.value,
                unit: window.unit.to_time_unit(),
            }),
            time_field: "event_timestamp".to_string(), // PostgreSQL events table uses event_timestamp
        }))
    } else {
        Ok(None)
    }
}
