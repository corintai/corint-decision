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
        let path = path.as_ref();
        debug!("Loading features from: {}", path.display());

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read feature file: {}", path.display()))?;

        let collection: FeatureCollection = serde_yaml::from_str(&content)
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

        // Register each feature
        for feature in collection.features {
            let name = feature.name.clone();
            feature_names.push(name.clone());

            // Index by type
            let feature_type = feature.get_type();
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
            self.feature_files.get(&file_key).map(|v| v.len()).unwrap_or(0),
            path.display()
        );

        Ok(())
    }

    /// Load features from a directory (all .yaml and .yml files)
    pub fn load_from_directory(&mut self, dir: impl AsRef<Path>) -> Result<()> {
        let dir = dir.as_ref();
        info!("Loading features from directory: {}", dir.display());

        if !dir.is_dir() {
            return Err(anyhow::anyhow!("Not a directory: {}", dir.display()));
        }

        let mut loaded_count = 0;
        let mut error_count = 0;

        // Read all .yaml and .yml files
        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "yaml" || ext == "yml" {
                        match self.load_from_file(&path) {
                            Ok(_) => loaded_count += 1,
                            Err(e) => {
                                warn!("Failed to load {}: {}", path.display(), e);
                                error_count += 1;
                            }
                        }
                    }
                }
            }
        }

        if error_count > 0 {
            warn!(
                "Loaded {} feature files with {} errors from: {}",
                loaded_count,
                error_count,
                dir.display()
            );
        } else {
            info!(
                "Successfully loaded {} feature files from: {}",
                loaded_count,
                dir.display()
            );
        }

        Ok(())
    }

    /// Load features recursively from a directory
    pub fn load_from_directory_recursive(&mut self, dir: impl AsRef<Path>) -> Result<()> {
        let dir = dir.as_ref();
        self.load_from_directory_recursive_impl(dir)
    }

    fn load_from_directory_recursive_impl(&mut self, dir: &Path) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        // Load features from current directory
        let _ = self.load_from_directory(dir);

        // Recursively load from subdirectories
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.load_from_directory_recursive_impl(&path)?;
            }
        }

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
        self.features
            .values()
            .filter(|f| f.is_enabled())
            .collect()
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

    /// Get dependency graph for a feature (recursive)
    pub fn dependency_tree(&self, feature_name: &str) -> Result<Vec<String>> {
        let mut tree = Vec::new();
        let mut visited = std::collections::HashSet::new();
        self.build_dependency_tree(feature_name, &mut tree, &mut visited)?;
        Ok(tree)
    }

    fn build_dependency_tree(
        &self,
        feature_name: &str,
        tree: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        if visited.contains(feature_name) {
            return Ok(());
        }

        visited.insert(feature_name.to_string());

        let feature = self
            .features
            .get(feature_name)
            .with_context(|| format!("Feature '{}' not found", feature_name))?;

        // Add dependencies first
        for dep in &feature.dependencies {
            self.build_dependency_tree(dep, tree, visited)?;
        }

        // Then add this feature
        tree.push(feature_name.to_string());

        Ok(())
    }

    /// Validate all registered features and their dependencies
    pub fn validate(&self) -> Result<()> {
        // Check all dependencies exist
        for (name, feature) in &self.features {
            for dep in &feature.dependencies {
                if !self.features.contains_key(dep) {
                    return Err(anyhow::anyhow!(
                        "Feature '{}' depends on non-existent feature '{}'",
                        name,
                        dep
                    ));
                }
            }
        }

        // Check for circular dependencies
        for name in self.features.keys() {
            if let Err(e) = self.dependency_tree(name) {
                return Err(anyhow::anyhow!(
                    "Circular dependency detected for feature '{}': {}",
                    name,
                    e
                ));
            }
        }

        Ok(())
    }

    /// Export all features to a single YAML file
    pub fn export_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let features: Vec<FeatureDefinition> =
            self.features.values().cloned().collect();

        let collection = FeatureCollection {
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
                    dimension_value: "{event.user_id}".to_string(),
                    window: None,
                    filters: Vec::new(),
                    cache: None,
                },
            }),
        );

        assert_eq!(registry.count(), 0);
        assert!(!registry.contains("test_feature"));

        // Manually insert for testing
        registry.features.insert("test_feature".to_string(), feature);

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
                    dimension_value: "{event.user_id}".to_string(),
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
                    dimension_value: "{event.user_id}".to_string(),
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
