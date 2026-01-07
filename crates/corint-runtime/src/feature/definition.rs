//! Feature Definition Module
//!
//! This module defines the structure and types for feature definitions based on
//! the CORINT Feature DSL specification (v0.2).
//!
//! Supports 6 feature categories:
//! - Aggregation: Count and aggregate events/values (count, sum, avg, max, min, distinct)
//! - State: Statistical comparisons (z_score, deviation, percentile)
//! - Sequence: Pattern and trend analysis (consecutive, streak, percent_change)
//! - Graph: Network and relationship analysis (centrality, community_size, shared_entity)
//! - Expression: Compute from other features (rate, ratio, ML models)
//! - Lookup: Retrieve pre-computed values (Redis cache)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use corint_core::Value;

/// Feature type classification based on DSL v0.2
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FeatureType {
    /// Aggregation features - count and aggregate events/values
    Aggregation,

    /// State features - statistical comparisons and baseline analysis
    State,

    /// Sequence features - pattern and trend analysis over time
    Sequence,

    /// Graph features - network and relationship analysis
    Graph,

    /// Expression features - compute from other features
    Expression,

    /// Lookup features - retrieve pre-computed values
    Lookup,
}

/// Aggregation methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AggregationMethod {
    /// Count events matching conditions
    Count,

    /// Sum numeric field values
    Sum,

    /// Average of field values
    Avg,

    /// Maximum value
    Max,

    /// Minimum value
    Min,

    /// Count unique values
    Distinct,

    /// Standard deviation (planned)
    Stddev,

    /// Variance (planned)
    Variance,

    /// Nth percentile value (planned)
    Percentile,

    /// Median value (planned)
    Median,

    /// Most frequent value (planned)
    Mode,

    /// Shannon entropy (planned)
    Entropy,
}

/// State methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateMethod {
    /// Statistical z-score compared to baseline
    ZScore,

    /// Compare to historical average
    DeviationFromBaseline,

    /// Rank compared to history
    PercentileRank,

    /// Statistical outlier detection
    IsOutlier,

    /// Timezone pattern consistency check
    TimezoneConsistency,
}

/// Sequence methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SequenceMethod {
    /// Count consecutive occurrences
    ConsecutiveCount,

    /// Longest streak of condition
    Streak,

    /// Match event sequences
    SequenceMatch,

    /// Frequency of specific patterns
    PatternFrequency,

    /// Calculate trend (increasing/decreasing/stable)
    Trend,

    /// Percentage change between windows
    PercentChange,

    /// Rate of change over time
    RateOfChange,

    /// Statistical anomaly detection
    AnomalyScore,

    /// Moving average over window
    MovingAverage,
}

/// Graph methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphMethod {
    /// Network centrality score
    GraphCentrality,

    /// Size of connected component
    CommunitySize,

    /// Count shared connections
    SharedEntityCount,

    /// Distance between entities in graph
    NetworkDistance,
}

/// Expression methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpressionMethod {
    /// Custom expression
    Expression,

    /// ML model prediction
    MlModelScore,

    /// Embedding similarity (planned)
    EmbeddingSimilarity,

    /// Clustering label (planned)
    ClusteringLabel,

    /// ML anomaly score (planned)
    MlAnomalyScore,
}

/// Time window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Window value (e.g., 1, 24, 7, 30)
    pub value: u64,

    /// Window unit (h, d, etc.)
    pub unit: String,
}

/// Filter condition - supports both simple string and complex all/any format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WhenCondition {
    /// Simple single condition as a string expression
    Simple(String),

    /// Complex conditions with all/any logic
    Complex {
        /// All conditions must be true
        #[serde(skip_serializing_if = "Option::is_none")]
        all: Option<Vec<String>>,

        /// Any condition must be true
        #[serde(skip_serializing_if = "Option::is_none")]
        any: Option<Vec<String>>,
    },
}

/// Aggregation feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationConfig {
    /// Data source name (logical name like events_datasource or lookup_datasource, mapped to actual datasources in config/server.yaml)
    pub datasource: String,

    /// Table/entity name
    pub entity: String,

    /// Grouping dimension (e.g., user_id, device_id)
    pub dimension: String,

    /// Template for dimension value (e.g., "${event.user_id}")
    pub dimension_value: String,

    /// Field to aggregate (optional for count)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,

    /// Time window (e.g., "24h", "7d", "30d")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,

    /// Timestamp field name (defaults to "event_timestamp")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_field: Option<String>,

    /// Filter conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenCondition>,

    /// Percentile value (for percentile method)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentile: Option<u8>,
}

/// State feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateConfig {
    pub datasource: String,
    pub entity: String,
    pub dimension: String,
    pub dimension_value: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,

    /// Timestamp field name (defaults to "event_timestamp")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_field: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenCondition>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_timezone: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// Sequence feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceConfig {
    pub datasource: String,
    pub entity: String,
    pub dimension: String,
    pub dimension_value: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenCondition>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_when: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_by: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregation: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_window: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_size: Option<usize>,
}

/// Graph feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    pub datasource: String,
    pub dimension: String,
    pub dimension_value: String,
    pub dimension2: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension_value2: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
}

/// Expression feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionConfig {
    /// Expression string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,

    /// ML model ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Model input features
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<String>>,

    /// Output field name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// Lookup feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupConfig {
    pub datasource: String,
    pub key: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Value>,
}

/// Feature definition following DSL v0.2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDefinition {
    /// Feature name (unique identifier)
    pub name: String,

    /// Feature type
    #[serde(rename = "type")]
    pub feature_type: FeatureType,

    /// Method name (e.g., count, sum, avg) - not needed for Lookup
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,

    /// Aggregation-specific configuration
    #[serde(flatten, default)]
    pub aggregation: Option<AggregationConfig>,

    /// State-specific configuration
    #[serde(flatten, default)]
    pub state: Option<StateConfig>,

    /// Sequence-specific configuration
    #[serde(flatten, default)]
    pub sequence: Option<SequenceConfig>,

    /// Graph-specific configuration
    #[serde(flatten, default)]
    pub graph: Option<GraphConfig>,

    /// Expression-specific configuration
    #[serde(flatten, default)]
    pub expression: Option<ExpressionConfig>,

    /// Lookup-specific configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lookup: Option<LookupConfig>,

    /// Human-readable description
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,

    /// Feature dependencies
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,

    /// Tags for organization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Whether enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Version
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_enabled() -> bool {
    true
}

fn default_version() -> String {
    "1.0".to_string()
}

impl FeatureDefinition {
    /// Create a new feature from old-style Operator (for backward compatibility with tests)
    #[cfg(test)]
    pub fn new(name: impl Into<String>, operator: crate::feature::operator::Operator) -> Self {
        use crate::feature::operator::Operator;

        // This is a temporary compatibility shim for tests
        // Converts old Operator enum to new DSL v0.2 structure with type+method fields
        let name = name.into();

        match operator {
            Operator::Count(op) => {
                Self {
                    name,
                    feature_type: FeatureType::Aggregation,
                    method: Some("count".to_string()),
                    aggregation: Some(AggregationConfig {
                        datasource: op.params.datasource.unwrap_or_else(|| "default".to_string()),
                        entity: op.params.entity,
                        dimension: op.params.dimension,
                        dimension_value: op.params.dimension_value,
                        field: None,
                        window: op.params.window.map(|w| format!("{}{}", w.value, match w.unit {
                            crate::feature::operator::WindowUnit::Minutes => "m",
                            crate::feature::operator::WindowUnit::Hours => "h",
                            crate::feature::operator::WindowUnit::Days => "d",
                        })),
                        timestamp_field: None,
                        when: None,
                        percentile: None,
                    }),
                    state: None,
                    sequence: None,
                    graph: None,
                    expression: None,
                    lookup: None,
                    description: String::new(),
                    dependencies: Vec::new(),
                    tags: Vec::new(),
                    enabled: true,
                    version: "1.0".to_string(),
                }
            }
            _ => {
                // For other operators, create a minimal valid feature
                Self {
                    name,
                    feature_type: FeatureType::Aggregation,
                    method: Some("count".to_string()),
                    aggregation: Some(AggregationConfig {
                        datasource: "default".to_string(),
                        entity: "events".to_string(),
                        dimension: "user_id".to_string(),
                        dimension_value: "${event.user_id}".to_string(),
                        field: None,
                        window: None,
                        timestamp_field: None,
                        when: None,
                        percentile: None,
                    }),
                    state: None,
                    sequence: None,
                    graph: None,
                    expression: None,
                    lookup: None,
                    description: String::new(),
                    dependencies: Vec::new(),
                    tags: Vec::new(),
                    enabled: true,
                    version: "1.0".to_string(),
                }
            }
        }
    }

    /// Validate the feature definition
    pub fn validate(&self) -> Result<(), String> {
        // Check name is not empty
        if self.name.is_empty() {
            return Err("Feature name cannot be empty".to_string());
        }

        // Check for circular dependencies
        if self.dependencies.contains(&self.name) {
            return Err(format!(
                "Feature '{}' has circular dependency on itself",
                self.name
            ));
        }

        // Validate type-specific configuration
        // Note: Due to serde's flatten with Option<T>, config objects (aggregation, state, etc.)
        // may not be properly populated even when fields exist in YAML. We only validate method field.
        match self.feature_type {
            FeatureType::Aggregation | FeatureType::State | FeatureType::Sequence
            | FeatureType::Graph => {
                if self.method.is_none() {
                    return Err(format!(
                        "Feature '{}': method required for {:?} type",
                        self.name, self.feature_type
                    ));
                }
            }
            FeatureType::Expression => {
                // Expression method is optional - defaults to "expression" if not specified
                // This avoids redundancy since expression type always uses expression method
            }
            FeatureType::Lookup => {
                // Lookup does not require method
                // Config fields are validated during execution
            }
        }

        Ok(())
    }

    /// Check if this feature is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Check if this feature has a specific tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Get a description suitable for logging
    pub fn log_description(&self) -> String {
        if self.description.is_empty() {
            self.name.clone()
        } else {
            format!("{} ({})", self.name, self.description)
        }
    }
}

/// Feature collection loaded from YAML file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureCollection {
    /// Feature format version
    #[serde(default = "default_collection_version")]
    pub version: String,

    /// List of feature definitions
    pub features: Vec<FeatureDefinition>,

    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_collection_version() -> String {
    "0.2".to_string()
}

impl FeatureCollection {
    /// Create a new empty feature collection
    pub fn new() -> Self {
        Self {
            version: "0.2".to_string(),
            features: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a feature to the collection
    pub fn add_feature(&mut self, feature: FeatureDefinition) {
        self.features.push(feature);
    }

    /// Get all feature names
    pub fn feature_names(&self) -> Vec<String> {
        self.features.iter().map(|f| f.name.clone()).collect()
    }

    /// Validate all features in the collection
    pub fn validate(&self) -> Result<(), String> {
        // Check for duplicate feature names
        let mut names = std::collections::HashSet::new();
        for feature in &self.features {
            if !names.insert(&feature.name) {
                return Err(format!("Duplicate feature name: {}", feature.name));
            }
        }

        // Validate each feature
        for feature in &self.features {
            feature.validate()?;
        }

        // Validate dependencies exist
        for feature in &self.features {
            for dep in &feature.dependencies {
                if !names.contains(dep) {
                    return Err(format!(
                        "Feature '{}' depends on non-existent feature '{}'",
                        feature.name, dep
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get enabled features only
    pub fn enabled_features(&self) -> Vec<&FeatureDefinition> {
        self.features.iter().filter(|f| f.enabled).collect()
    }

    /// Get features by tag
    pub fn features_by_tag(&self, tag: &str) -> Vec<&FeatureDefinition> {
        self.features.iter().filter(|f| f.has_tag(tag)).collect()
    }

    /// Get features by type
    pub fn features_by_type(&self, feature_type: FeatureType) -> Vec<&FeatureDefinition> {
        self.features
            .iter()
            .filter(|f| f.feature_type == feature_type)
            .collect()
    }
}

impl Default for FeatureCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregation_feature_definition() {
        let feature = FeatureDefinition {
            name: "cnt_userid_login_24h".to_string(),
            feature_type: FeatureType::Aggregation,
            method: Some("count".to_string()),
            aggregation: Some(AggregationConfig {
                datasource: "postgresql_events".to_string(),
                entity: "events".to_string(),
                dimension: "user_id".to_string(),
                dimension_value: "{event.user_id}".to_string(),
                field: None,
                window: Some("24h".to_string()),
                timestamp_field: None,
                when: None,
                percentile: None,
            }),
            state: None,
            sequence: None,
            graph: None,
            expression: None,
            lookup: None,
            description: String::new(),
            dependencies: Vec::new(),
            tags: Vec::new(),
            enabled: true,
            version: "1.0".to_string(),
        };

        assert!(feature.validate().is_ok());
        assert_eq!(feature.feature_type, FeatureType::Aggregation);
    }

    #[test]
    fn test_lookup_feature_definition() {
        let feature = FeatureDefinition {
            name: "user_risk_score_90d".to_string(),
            feature_type: FeatureType::Lookup,
            method: None,  // Lookup doesn't need method
            aggregation: None,
            state: None,
            sequence: None,
            graph: None,
            expression: None,
            lookup: Some(LookupConfig {
                datasource: "redis_features".to_string(),
                key: "user_risk_score:{event.user_id}".to_string(),
                fallback: Some(Value::Number(50.0)),
            }),
            description: String::new(),
            dependencies: Vec::new(),
            tags: Vec::new(),
            enabled: true,
            version: "1.0".to_string(),
        };

        assert!(feature.validate().is_ok());
        assert_eq!(feature.feature_type, FeatureType::Lookup);
    }

    #[test]
    fn test_expression_feature_definition() {
        let feature = FeatureDefinition {
            name: "rate_userid_login_failure".to_string(),
            feature_type: FeatureType::Expression,
            method: Some("expression".to_string()),
            aggregation: None,
            state: None,
            sequence: None,
            graph: None,
            expression: Some(ExpressionConfig {
                expression: Some("failed_login_count_1h / login_count_1h".to_string()),
                model: None,
                inputs: None,
                output: None,
            }),
            lookup: None,
            description: String::new(),
            dependencies: Vec::new(),
            tags: Vec::new(),
            enabled: true,
            version: "1.0".to_string(),
        };

        assert!(feature.validate().is_ok());
        assert_eq!(feature.feature_type, FeatureType::Expression);
    }

    #[test]
    fn test_expression_feature_without_method() {
        // Expression features should validate successfully even without method field
        // since method is always "expression" for this type (redundant)
        let feature = FeatureDefinition {
            name: "txn_velocity_ratio".to_string(),
            feature_type: FeatureType::Expression,
            method: None,  // Method field is optional for expression type
            aggregation: None,
            state: None,
            sequence: None,
            graph: None,
            expression: Some(ExpressionConfig {
                expression: Some("txn_count_1h / max(txn_count_24h, 1)".to_string()),
                model: None,
                inputs: None,
                output: None,
            }),
            lookup: None,
            description: "Transaction velocity ratio".to_string(),
            dependencies: Vec::new(),
            tags: Vec::new(),
            enabled: true,
            version: "1.0".to_string(),
        };

        assert!(feature.validate().is_ok());
        assert_eq!(feature.feature_type, FeatureType::Expression);
        assert_eq!(feature.method, None);
    }

    #[test]
    fn test_feature_collection_duplicate_detection() {
        let mut collection = FeatureCollection::new();

        let feature1 = FeatureDefinition {
            name: "feature1".to_string(),
            feature_type: FeatureType::Aggregation,
            method: Some("count".to_string()),
            aggregation: Some(AggregationConfig {
                datasource: "postgresql_events".to_string(),
                entity: "events".to_string(),
                dimension: "user_id".to_string(),
                dimension_value: "{event.user_id}".to_string(),
                field: None,
                window: None,
                timestamp_field: None,
                when: None,
                percentile: None,
            }),
            state: None,
            sequence: None,
            graph: None,
            expression: None,
            lookup: None,
            description: String::new(),
            dependencies: Vec::new(),
            tags: Vec::new(),
            enabled: true,
            version: "1.0".to_string(),
        };

        collection.add_feature(feature1.clone());
        collection.add_feature(feature1);

        let result = collection.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate feature name"));
    }
}
