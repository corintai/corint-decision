//! Feature Definition Module
//!
//! This module defines the structure and types for feature definitions,
//! which can be loaded from YAML configuration files.

use crate::feature::operator::Operator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Feature type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FeatureType {
    /// Basic aggregation features (count, sum, avg, etc.)
    Aggregation,
    /// Distinct count features
    Distinct,
    /// Temporal features (first_seen, last_seen, time_since)
    Temporal,
    /// Velocity features (threshold-based)
    Velocity,
    /// Lookup features (feature store, profile database)
    Lookup,
    /// Custom expression features
    Expression,
}

/// Feature definition loaded from YAML configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDefinition {
    /// Feature name (unique identifier)
    pub name: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Feature operator configuration
    #[serde(flatten)]
    pub operator: Operator,

    /// Feature type (automatically inferred from operator if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_type: Option<FeatureType>,

    /// Feature dependencies (other features that must be computed first)
    #[serde(default)]
    pub dependencies: Vec<String>,

    /// Tags for feature organization and filtering
    #[serde(default)]
    pub tags: Vec<String>,

    /// Whether this feature is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Feature version (for A/B testing or gradual rollout)
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
    /// Create a new feature definition
    pub fn new(name: impl Into<String>, operator: Operator) -> Self {
        let name = name.into();
        let feature_type = Self::infer_type(&operator);

        Self {
            name,
            description: String::new(),
            operator,
            feature_type: Some(feature_type),
            dependencies: Vec::new(),
            tags: Vec::new(),
            enabled: true,
            version: "1.0".to_string(),
        }
    }

    /// Infer feature type from operator
    pub fn infer_type(operator: &Operator) -> FeatureType {
        match operator {
            Operator::Count(_) | Operator::Sum(_) | Operator::Avg(_)
            | Operator::Max(_) | Operator::Min(_) => FeatureType::Aggregation,

            Operator::CountDistinct(_) | Operator::CrossDimensionCount(_) => {
                FeatureType::Distinct
            }

            Operator::FirstSeen(_) | Operator::LastSeen(_) | Operator::TimeSince(_) => {
                FeatureType::Temporal
            }

            Operator::Velocity(_) => FeatureType::Velocity,

            Operator::FeatureStoreLookup(_) | Operator::ProfileLookup(_) => {
                FeatureType::Lookup
            }

            Operator::Expression(_) => FeatureType::Expression,
        }
    }

    /// Get the feature type (infer if not set)
    pub fn get_type(&self) -> FeatureType {
        self.feature_type
            .clone()
            .unwrap_or_else(|| Self::infer_type(&self.operator))
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

        // Validate operator-specific requirements
        self.validate_operator()?;

        Ok(())
    }

    /// Validate operator-specific requirements
    fn validate_operator(&self) -> Result<(), String> {
        match &self.operator {
            Operator::Count(op) => {
                if op.params.entity.is_empty() {
                    return Err(format!(
                        "Feature '{}': entity cannot be empty for count operator",
                        self.name
                    ));
                }
                if op.params.dimension.is_empty() {
                    return Err(format!(
                        "Feature '{}': dimension cannot be empty for count operator",
                        self.name
                    ));
                }
            }

            Operator::Sum(op) => {
                if op.params.entity.is_empty() {
                    return Err(format!(
                        "Feature '{}': entity cannot be empty for sum operator",
                        self.name
                    ));
                }
                if op.field.is_empty() {
                    return Err(format!(
                        "Feature '{}': field cannot be empty for sum operator",
                        self.name
                    ));
                }
            }

            Operator::Avg(op) => {
                if op.params.entity.is_empty() {
                    return Err(format!(
                        "Feature '{}': entity cannot be empty for avg operator",
                        self.name
                    ));
                }
                if op.field.is_empty() {
                    return Err(format!(
                        "Feature '{}': field cannot be empty for avg operator",
                        self.name
                    ));
                }
            }

            Operator::Max(op) => {
                if op.params.entity.is_empty() {
                    return Err(format!(
                        "Feature '{}': entity cannot be empty for max operator",
                        self.name
                    ));
                }
                if op.field.is_empty() {
                    return Err(format!(
                        "Feature '{}': field cannot be empty for max operator",
                        self.name
                    ));
                }
            }

            Operator::Min(op) => {
                if op.params.entity.is_empty() {
                    return Err(format!(
                        "Feature '{}': entity cannot be empty for min operator",
                        self.name
                    ));
                }
                if op.field.is_empty() {
                    return Err(format!(
                        "Feature '{}': field cannot be empty for min operator",
                        self.name
                    ));
                }
            }

            Operator::CountDistinct(op) => {
                if op.distinct_field.is_empty() {
                    return Err(format!(
                        "Feature '{}': distinct_field cannot be empty for count_distinct operator",
                        self.name
                    ));
                }
            }

            Operator::CrossDimensionCount(op) => {
                if op.primary_dimension.is_empty() || op.secondary_dimension.is_empty() {
                    return Err(format!(
                        "Feature '{}': both primary and secondary dimensions required for cross_dimension_count",
                        self.name
                    ));
                }
            }

            Operator::Velocity(op) => {
                if op.threshold <= 0 {
                    return Err(format!(
                        "Feature '{}': velocity threshold must be positive",
                        self.name
                    ));
                }
            }

            Operator::FeatureStoreLookup(op) => {
                if op.key.is_empty() {
                    return Err(format!(
                        "Feature '{}': key cannot be empty for feature_store_lookup",
                        self.name
                    ));
                }
            }

            Operator::ProfileLookup(op) => {
                if op.table.is_empty() || op.field.is_empty() {
                    return Err(format!(
                        "Feature '{}': table and field required for profile_lookup",
                        self.name
                    ));
                }
            }

            _ => {}
        }

        Ok(())
    }

    /// Check if this feature has any dependencies
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
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
    /// List of feature definitions
    pub features: Vec<FeatureDefinition>,

    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl FeatureCollection {
    /// Create a new empty feature collection
    pub fn new() -> Self {
        Self {
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
            .filter(|f| f.get_type() == feature_type)
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
    use crate::feature::operator::{CountOperator, OperatorParams, WindowConfig, WindowUnit};

    #[test]
    fn test_feature_definition_creation() {
        let operator = Operator::Count(CountOperator {
            params: OperatorParams {
                entity: "user_events".to_string(),
                dimension: "user_id".to_string(),
                dimension_value: "{event.user_id}".to_string(),
                window: Some(WindowConfig {
                    value: 24,
                    unit: WindowUnit::Hours,
                }),
                filters: Vec::new(),
                cache: None,
            },
        });

        let feature = FeatureDefinition::new("login_count_24h", operator);

        assert_eq!(feature.name, "login_count_24h");
        assert_eq!(feature.get_type(), FeatureType::Aggregation);
        assert!(feature.is_enabled());
    }

    #[test]
    fn test_feature_validation() {
        let operator = Operator::Count(CountOperator {
            params: OperatorParams {
                entity: "user_events".to_string(),
                dimension: "user_id".to_string(),
                dimension_value: "{event.user_id}".to_string(),
                window: Some(WindowConfig {
                    value: 24,
                    unit: WindowUnit::Hours,
                }),
                filters: Vec::new(),
                cache: None,
            },
        });

        let feature = FeatureDefinition::new("login_count_24h", operator);
        assert!(feature.validate().is_ok());
    }

    #[test]
    fn test_feature_collection_duplicate_detection() {
        let mut collection = FeatureCollection::new();

        let operator = Operator::Count(CountOperator {
            params: OperatorParams {
                entity: "user_events".to_string(),
                dimension: "user_id".to_string(),
                dimension_value: "{event.user_id}".to_string(),
                window: None,
                filters: Vec::new(),
                cache: None,
            },
        });

        collection.add_feature(FeatureDefinition::new("feature1", operator.clone()));
        collection.add_feature(FeatureDefinition::new("feature1", operator));

        let result = collection.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate feature name"));
    }
}
