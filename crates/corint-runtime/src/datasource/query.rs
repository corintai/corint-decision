//! Query Abstraction Layer
//!
//! Unified query interface for different data sources.

use corint_core::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unified query structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    /// Query type
    pub query_type: QueryType,

    /// Entity/table/collection to query
    pub entity: String,

    /// Filters/conditions
    #[serde(default)]
    pub filters: Vec<Filter>,

    /// Time window
    pub time_window: Option<TimeWindow>,

    /// Aggregations to compute
    #[serde(default)]
    pub aggregations: Vec<Aggregation>,

    /// Group by fields
    #[serde(default)]
    pub group_by: Vec<String>,

    /// Limit number of results
    pub limit: Option<usize>,
}

/// Query type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryType {
    /// Get pre-computed feature from feature store
    GetFeature,

    /// Count aggregation
    Count,

    /// Count distinct (unique values)
    CountDistinct,

    /// Statistical aggregations
    Aggregate,

    /// Raw event query
    RawEvents,
}

/// Filter condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    /// Field to filter on
    pub field: String,

    /// Operator
    pub operator: FilterOperator,

    /// Value to compare against
    pub value: Value,
}

/// Filter operators
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperator {
    Eq,      // ==
    Ne,      // !=
    Gt,      // >
    Ge,      // >=
    Lt,      // <
    Le,      // <=
    In,      // IN
    NotIn,   // NOT IN
    Like,    // LIKE (for SQL)
    Regex,   // Regex match
}

/// Time window specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    /// Window type
    #[serde(flatten)]
    pub window_type: TimeWindowType,

    /// Time field to use
    #[serde(default = "default_timestamp_field")]
    pub time_field: String,
}

/// Time window types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TimeWindowType {
    /// Relative window (e.g., last_24h, last_7d)
    Relative(RelativeWindow),

    /// Absolute time range
    Absolute {
        start: i64,  // Unix timestamp
        end: i64,    // Unix timestamp
    },
}

/// Relative time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelativeWindow {
    /// Duration value
    pub value: u64,

    /// Duration unit
    pub unit: TimeUnit,
}

/// Time units
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeUnit {
    Minutes,
    Hours,
    Days,
    Weeks,
    Months,
}

impl RelativeWindow {
    /// Convert to seconds
    pub fn to_seconds(&self) -> u64 {
        match self.unit {
            TimeUnit::Minutes => self.value * 60,
            TimeUnit::Hours => self.value * 3600,
            TimeUnit::Days => self.value * 86400,
            TimeUnit::Weeks => self.value * 604800,
            TimeUnit::Months => self.value * 2592000, // 30 days
        }
    }

    /// Parse from string like "24h", "7d", "5m"
    pub fn from_string(s: &str) -> Option<Self> {
        if s.starts_with("last_") {
            let s = &s[5..]; // Remove "last_" prefix
            Self::parse_duration(s)
        } else {
            Self::parse_duration(s)
        }
    }

    fn parse_duration(s: &str) -> Option<Self> {
        let len = s.len();
        if len < 2 {
            return None;
        }

        let (value_str, unit_str) = s.split_at(len - 1);
        let value = value_str.parse::<u64>().ok()?;

        let unit = match unit_str {
            "m" => TimeUnit::Minutes,
            "h" => TimeUnit::Hours,
            "d" => TimeUnit::Days,
            "w" => TimeUnit::Weeks,
            _ => return None,
        };

        Some(RelativeWindow { value, unit })
    }
}

/// Aggregation specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregation {
    /// Aggregation type
    pub agg_type: AggregationType,

    /// Field to aggregate (None for COUNT(*))
    pub field: Option<String>,

    /// Output field name
    pub output_name: String,
}

/// Aggregation types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationType {
    Count,
    CountDistinct,
    Sum,
    Avg,
    Min,
    Max,
    Median,
    Stddev,
    Percentile { p: u8 },
}

/// Query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Result rows
    pub rows: Vec<HashMap<String, Value>>,

    /// Execution time in milliseconds
    pub execution_time_ms: u64,

    /// Data source that provided the result
    pub source: String,

    /// Whether result was from cache
    pub from_cache: bool,
}

fn default_timestamp_field() -> String {
    "timestamp".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_relative_window() {
        let window = RelativeWindow::from_string("last_24h").unwrap();
        assert_eq!(window.value, 24);
        assert!(matches!(window.unit, TimeUnit::Hours));
        assert_eq!(window.to_seconds(), 24 * 3600);

        let window = RelativeWindow::from_string("7d").unwrap();
        assert_eq!(window.value, 7);
        assert!(matches!(window.unit, TimeUnit::Days));

        let window = RelativeWindow::from_string("5m").unwrap();
        assert_eq!(window.value, 5);
        assert!(matches!(window.unit, TimeUnit::Minutes));
    }

    #[test]
    fn test_query_serialization() {
        let query = Query {
            query_type: QueryType::CountDistinct,
            entity: "user_logins".to_string(),
            filters: vec![Filter {
                field: "user_id".to_string(),
                operator: FilterOperator::Eq,
                value: Value::String("user123".to_string()),
            }],
            time_window: Some(TimeWindow {
                window_type: TimeWindowType::Relative(RelativeWindow {
                    value: 24,
                    unit: TimeUnit::Hours,
                }),
                time_field: "timestamp".to_string(),
            }),
            aggregations: vec![Aggregation {
                agg_type: AggregationType::CountDistinct,
                field: Some("device_id".to_string()),
                output_name: "unique_devices".to_string(),
            }],
            group_by: vec![],
            limit: None,
        };

        let json = serde_json::to_string_pretty(&query).unwrap();
        println!("Query JSON:\n{}", json);

        let deserialized: Query = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.entity, "user_logins");
    }
}
