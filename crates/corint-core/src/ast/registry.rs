//! Pipeline Registry AST definitions
//!
//! A Pipeline Registry defines the entry point routing for event processing.
//! It provides a declarative, ordered list of pipeline matching rules that determine
//! which pipeline should execute for a given event.

use crate::ast::rule::WhenBlock;
use serde::{Deserialize, Serialize};

/// A pipeline registry defines event-to-pipeline routing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineRegistry {
    /// Version of the registry format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Ordered list of registry entries
    pub registry: Vec<RegistryEntry>,
}

/// A single entry in the pipeline registry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Pipeline ID to execute if this entry matches
    pub pipeline: String,

    /// When condition that determines if this entry matches the event
    pub when: WhenBlock,
}

impl PipelineRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            version: Some("0.1".to_string()),
            registry: Vec::new(),
        }
    }

    /// Create a registry with version
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }

    /// Add a registry entry
    pub fn add_entry(mut self, entry: RegistryEntry) -> Self {
        self.registry.push(entry);
        self
    }

    /// Add multiple entries
    pub fn with_entries(mut self, entries: Vec<RegistryEntry>) -> Self {
        self.registry = entries;
        self
    }
}

impl Default for PipelineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryEntry {
    /// Create a new registry entry
    pub fn new(pipeline: String, when: WhenBlock) -> Self {
        Self { pipeline, when }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expression, Operator};
    use crate::Value;

    #[test]
    fn test_registry_creation() {
        let registry = PipelineRegistry::new()
            .add_entry(RegistryEntry::new(
                "login_pipeline".to_string(),
                WhenBlock {
                    event_type: Some("login".to_string()),
                    conditions: vec![],
                },
            ))
            .add_entry(RegistryEntry::new(
                "payment_pipeline".to_string(),
                WhenBlock {
                    event_type: Some("payment".to_string()),
                    conditions: vec![],
                },
            ));

        assert_eq!(registry.registry.len(), 2);
        assert_eq!(registry.version, Some("0.1".to_string()));
    }

    #[test]
    fn test_registry_entry_with_conditions() {
        let entry = RegistryEntry::new(
            "payment_br_pipeline".to_string(),
            WhenBlock {
                event_type: Some("payment".to_string()),
                conditions: vec![Expression::binary(
                    Expression::field_access(vec!["geo".to_string(), "country".to_string()]),
                    Operator::Eq,
                    Expression::literal(Value::String("BR".to_string())),
                )],
            },
        );

        assert_eq!(entry.pipeline, "payment_br_pipeline");
        assert_eq!(entry.when.event_type, Some("payment".to_string()));
        assert_eq!(entry.when.conditions.len(), 1);
    }

    #[test]
    fn test_registry_serde() {
        let registry = PipelineRegistry::new().add_entry(RegistryEntry::new(
            "test_pipeline".to_string(),
            WhenBlock {
                event_type: Some("test".to_string()),
                conditions: vec![],
            },
        ));

        // Serialize to JSON
        let json = serde_json::to_string(&registry).unwrap();
        assert!(json.contains("\"pipeline\":\"test_pipeline\""));

        // Deserialize back
        let deserialized: PipelineRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, registry);
    }
}
