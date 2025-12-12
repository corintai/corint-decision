//! Decision logic template AST definitions
//!
//! Templates allow reusable decision logic with parameterization.

use crate::ast::DecisionRule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A decision logic template that can be instantiated with parameters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionTemplate {
    /// Unique identifier for this template
    pub id: String,

    /// Human-readable name
    pub name: Option<String>,

    /// Description of the template
    pub description: Option<String>,

    /// Default parameter values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<HashMap<String, serde_json::Value>>,

    /// Decision logic rules (may contain param references like params.threshold)
    pub decision_logic: Vec<DecisionRule>,

    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Reference to a decision template with optional parameter overrides
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateReference {
    /// Template ID to use
    pub template: String,

    /// Parameter overrides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<HashMap<String, serde_json::Value>>,
}

impl DecisionTemplate {
    /// Create a new decision template
    pub fn new(id: String, decision_logic: Vec<DecisionRule>) -> Self {
        Self {
            id,
            name: None,
            description: None,
            params: None,
            decision_logic,
            metadata: None,
        }
    }

    /// Set the name
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the default parameters
    pub fn with_params(mut self, params: HashMap<String, serde_json::Value>) -> Self {
        self.params = Some(params);
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

impl TemplateReference {
    /// Create a new template reference
    pub fn new(template: String) -> Self {
        Self {
            template,
            params: None,
        }
    }

    /// Add parameter overrides
    pub fn with_params(mut self, params: HashMap<String, serde_json::Value>) -> Self {
        self.params = Some(params);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Action, DecisionRule};

    #[test]
    fn test_decision_template_creation() {
        let decision_logic = vec![DecisionRule::default(Action::Approve)];

        let template = DecisionTemplate::new("test_template".to_string(), decision_logic)
            .with_name("Test Template".to_string())
            .with_description("A test template".to_string());

        assert_eq!(template.id, "test_template");
        assert_eq!(template.name, Some("Test Template".to_string()));
        assert_eq!(
            template.description,
            Some("A test template".to_string())
        );
        assert_eq!(template.decision_logic.len(), 1);
    }

    #[test]
    fn test_template_with_params() {
        let mut params = HashMap::new();
        params.insert(
            "threshold".to_string(),
            serde_json::Value::Number(100.into()),
        );

        let template = DecisionTemplate::new(
            "score_based".to_string(),
            vec![DecisionRule::default(Action::Deny)],
        )
        .with_params(params.clone());

        assert_eq!(template.params, Some(params));
    }

    #[test]
    fn test_template_reference() {
        let mut params = HashMap::new();
        params.insert(
            "critical_threshold".to_string(),
            serde_json::Value::Number(150.into()),
        );

        let reference =
            TemplateReference::new("score_based_decision".to_string()).with_params(params);

        assert_eq!(reference.template, "score_based_decision");
        assert!(reference.params.is_some());
    }
}
