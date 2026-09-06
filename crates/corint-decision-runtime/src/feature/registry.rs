//! Feature Registry Module
//!
//! This module provides functionality to load and manage feature definitions
//! from YAML configuration files.

use crate::feature::definition::{FeatureCollection, FeatureDefinition, FeatureType};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

/// Feature registry that manages feature definitions
#[derive(Clone)]
pub struct FeatureRegistry {
    /// All registered features indexed by name
    features: HashMap<String, FeatureDefinition>,

    /// Features grouped by source file
    feature_files: HashMap<String, Vec<String>>,

    /// Features grouped by type
    features_by_type: HashMap<FeatureType, Vec<String>>,

    /// Features grouped by tag
    features_by_tag: HashMap<String, Vec<String>>,
}

impl FeatureRegistry {
    /// Create a new empty feature registry
    pub fn new() -> Self {
        Self {
            features: HashMap::new(),
            feature_files: HashMap::new(),
            features_by_type: HashMap::new(),
            features_by_tag: HashMap::new(),
        }
    }

    /// Load features from a single YAML file
    pub fn load_from_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let mut staged = self.clone();
        staged.load_file(path.as_ref())?;
        super::dependency::order(
            &staged.features,
            &staged.features.keys().cloned().collect::<Vec<_>>(),
            true,
        )?;
        *self = staged;
        Ok(())
    }

    fn load_file(&mut self, path: &Path) -> Result<()> {
        debug!("Loading features from: {}", path.display());

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read feature file: {}", path.display()))?;

        // Parse YAML to get raw values for post-processing lookup features
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML file: {}", path.display()))?;

        let collection: FeatureCollection = serde_yaml::from_str(&content)
            .map_err(|e| {
                warn!("Detailed parse error: {:?}", e);
                e
            })
            .with_context(|| format!("Failed to parse feature file: {}", path.display()))?;

        // Validate the collection
        collection.validate().map_err(|e| {
            anyhow::anyhow!(
                "Feature validation failed for file {}: {}",
                path.display(),
                e
            )
        })?;

        let file_key = path.to_string_lossy().to_string();
        let mut feature_names = Vec::new();

        // Get features array from YAML for post-processing
        let empty_vec = vec![];
        let features_yaml = yaml_value
            .get("features")
            .and_then(|v| v.as_sequence())
            .unwrap_or(&empty_vec);

        // Register each feature
        for (idx, mut feature) in collection.features.into_iter().enumerate() {
            // Post-process lookup features to populate lookup config from YAML
            if feature.feature_type == crate::feature::definition::FeatureType::Lookup {
                if let Some(feature_yaml) = features_yaml.get(idx) {
                    feature.fixup_lookup_from_yaml(feature_yaml);
                }
            }
            let name = feature.name.clone();
            feature_names.push(name.clone());

            super::dependency::infer_dependencies(&mut feature);
            feature.validate().map_err(anyhow::Error::msg)?;

            // Index by type
            let feature_type = feature.feature_type.clone();
            self.features_by_type
                .entry(feature_type)
                .or_default()
                .push(name.clone());

            // Index by tags
            for tag in &feature.tags {
                self.features_by_tag
                    .entry(tag.clone())
                    .or_default()
                    .push(name.clone());
            }

            // Register feature
            self.features.insert(name.clone(), feature);
        }

        // Track which features came from this file
        self.feature_files.insert(file_key.clone(), feature_names);

        info!(
            "Loaded {} features from: {}",
            self.feature_files
                .get(&file_key)
                .map(|v| v.len())
                .unwrap_or(0),
            path.display()
        );

        Ok(())
    }

    /// Load a complete directory atomically; no partial-success activation.
    pub fn load_from_directory(&mut self, dir: impl AsRef<Path>) -> Result<()> {
        self.load_directory(dir.as_ref(), false)
    }
    pub fn load_from_directory_recursive(&mut self, dir: impl AsRef<Path>) -> Result<()> {
        self.load_directory(dir.as_ref(), true)
    }
    fn load_directory(&mut self, dir: &Path, recursive: bool) -> Result<()> {
        let mut staged = self.clone();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(dir) = pending.pop() {
            let mut entries = std::fs::read_dir(&dir)?
                .map(|entry| entry.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            entries.sort();
            for path in entries {
                if path.is_dir() && recursive {
                    pending.push(path);
                } else if path.is_file()
                    && matches!(
                        path.extension().and_then(|ext| ext.to_str()),
                        Some("yaml" | "yml")
                    )
                {
                    staged.load_file(&path)?;
                }
            }
        }
        staged.validate()?;
        *self = staged;
        Ok(())
    }

    /// Get a feature by name
    pub fn get(&self, name: &str) -> Option<&FeatureDefinition> {
        self.features.get(name)
    }

    /// Get all registered features
    pub fn all_features(&self) -> Vec<&FeatureDefinition> {
        self.features.values().collect()
    }

    /// Get all enabled features
    pub fn enabled_features(&self) -> Vec<&FeatureDefinition> {
        self.features.values().filter(|f| f.is_enabled()).collect()
    }

    /// Get features by type
    pub fn features_by_type(&self, feature_type: FeatureType) -> Vec<&FeatureDefinition> {
        self.features_by_type
            .get(&feature_type)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| self.features.get(name))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get features by tag
    pub fn features_by_tag(&self, tag: &str) -> Vec<&FeatureDefinition> {
        self.features_by_tag
            .get(tag)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| self.features.get(name))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all feature names
    pub fn feature_names(&self) -> Vec<String> {
        self.features.keys().cloned().collect()
    }

    /// Get count of registered features
    pub fn count(&self) -> usize {
        self.features.len()
    }

    /// Check if a feature exists
    pub fn contains(&self, name: &str) -> bool {
        self.features.contains_key(name)
    }

    /// Get features that depend on a given feature
    pub fn dependents(&self, feature_name: &str) -> Vec<&FeatureDefinition> {
        self.features
            .values()
            .filter(|f| f.dependencies.contains(&feature_name.to_string()))
            .collect()
    }

    pub fn dependency_tree(&self, feature_name: &str) -> Result<Vec<String>> {
        super::dependency::order(&self.features, &[feature_name.to_owned()], false)
    }

    pub fn validate(&self) -> Result<()> {
        super::dependency::order(
            &self.features,
            &self.features.keys().cloned().collect::<Vec<_>>(),
            false,
        )?;
        Ok(())
    }

    /// Export all features to a single YAML file
    pub fn export_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let features: Vec<FeatureDefinition> = self.features.values().cloned().collect();

        let collection = FeatureCollection {
            version: "0.2".to_string(),
            features,
            metadata: HashMap::new(),
        };

        let yaml = serde_yaml::to_string(&collection)
            .with_context(|| "Failed to serialize features to YAML")?;

        std::fs::write(path, yaml)
            .with_context(|| format!("Failed to write feature file: {}", path.display()))?;

        info!("Exported {} features to: {}", self.count(), path.display());
        Ok(())
    }

    /// Print registry summary
    pub fn print_summary(&self) {
        info!("=== Feature Registry Summary ===");
        info!("Total features: {}", self.count());
        info!("Enabled features: {}", self.enabled_features().len());

        info!("\nFeatures by type:");
        for (feature_type, names) in &self.features_by_type {
            info!("  {:?}: {}", feature_type, names.len());
        }

        if !self.features_by_tag.is_empty() {
            info!("\nFeatures by tag:");
            for (tag, names) in &self.features_by_tag {
                info!("  {}: {}", tag, names.len());
            }
        }

        info!("\nLoaded from {} file(s)", self.feature_files.len());
    }

    /// Clear all registered features
    pub fn clear(&mut self) {
        self.features.clear();
        self.feature_files.clear();
        self.features_by_type.clear();
        self.features_by_tag.clear();
    }
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::operator::{CountOperator, OperatorParams};

    #[test]
    fn test_registry_basic_operations() {
        let mut registry = FeatureRegistry::new();

        let feature = FeatureDefinition::new(
            "test_feature",
            crate::feature::operator::Operator::Count(CountOperator {
                params: OperatorParams {
                    datasource: None,
                    entity: "events".to_string(),
                    dimension: "user_id".to_string(),
                    dimension_value: "${event.user_id}".to_string(),
                    window: None,
                    filters: Vec::new(),
                    cache: None,
                },
            }),
        );

        assert_eq!(registry.count(), 0);
        assert!(!registry.contains("test_feature"));

        // Manually insert for testing
        registry
            .features
            .insert("test_feature".to_string(), feature);

        assert_eq!(registry.count(), 1);
        assert!(registry.contains("test_feature"));
        assert!(registry.get("test_feature").is_some());
    }

    #[test]
    fn test_dependency_tree() {
        let mut registry = FeatureRegistry::new();

        // Create feature A with no dependencies
        let feature_a = FeatureDefinition::new(
            "feature_a",
            crate::feature::operator::Operator::Count(CountOperator {
                params: OperatorParams {
                    datasource: None,
                    entity: "events".to_string(),
                    dimension: "user_id".to_string(),
                    dimension_value: "${event.user_id}".to_string(),
                    window: None,
                    filters: Vec::new(),
                    cache: None,
                },
            }),
        );

        // Create feature B that depends on A
        let mut feature_b = FeatureDefinition::new(
            "feature_b",
            crate::feature::operator::Operator::Count(CountOperator {
                params: OperatorParams {
                    datasource: None,
                    entity: "events".to_string(),
                    dimension: "user_id".to_string(),
                    dimension_value: "${event.user_id}".to_string(),
                    window: None,
                    filters: Vec::new(),
                    cache: None,
                },
            }),
        );
        feature_b.dependencies.push("feature_a".to_string());

        registry.features.insert("feature_a".to_string(), feature_a);
        registry.features.insert("feature_b".to_string(), feature_b);

        let tree = registry.dependency_tree("feature_b").unwrap();
        assert_eq!(tree, vec!["feature_a", "feature_b"]);
    }
}
