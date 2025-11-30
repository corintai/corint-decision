//! Pipeline AST definitions
//!
//! A Pipeline orchestrates the decision flow through multiple steps,
//! including feature extraction, LLM reasoning, service calls, and rulesets.

use crate::ast::Expression;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A pipeline defines a sequence of processing steps
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pipeline {
    /// The processing steps in execution order
    pub steps: Vec<Step>,
}

/// A single step in the pipeline
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Step {
    /// Extract features from data
    Extract {
        /// Step identifier
        id: String,
        /// Feature definitions to extract
        features: Vec<FeatureDefinition>,
    },

    /// Call LLM for reasoning
    Reason {
        /// Step identifier
        id: String,
        /// LLM provider (e.g., "openai", "anthropic")
        provider: String,
        /// Model name (e.g., "gpt-4", "claude-3")
        model: String,
        /// Prompt template
        prompt: PromptTemplate,
        /// Optional output schema for structured responses
        output_schema: Option<Schema>,
    },

    /// Call external service
    Service {
        /// Step identifier
        id: String,
        /// Service name
        service: String,
        /// Operation to perform
        operation: String,
        /// Parameters for the service call
        params: HashMap<String, Expression>,
    },

    /// Include a ruleset
    Include {
        /// Ruleset ID to include
        ruleset: String,
    },

    /// Conditional branching
    Branch {
        /// List of conditional branches
        branches: Vec<Branch>,
    },

    /// Parallel execution
    Parallel {
        /// Steps to execute in parallel
        steps: Vec<Step>,
        /// Strategy for merging results
        merge: MergeStrategy,
    },
}

/// Feature definition for extraction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureDefinition {
    /// Feature name
    pub name: String,
    /// Expression to calculate the feature value
    pub value: Expression,
}

/// Prompt template for LLM reasoning
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Template string with placeholders
    pub template: String,
}

/// Schema definition for structured data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// Schema type (e.g., "object")
    #[serde(rename = "type")]
    pub schema_type: String,
    /// Properties for object schemas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, SchemaProperty>>,
}

/// Property in a schema
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaProperty {
    /// Property type (e.g., "string", "number", "boolean")
    #[serde(rename = "type")]
    pub property_type: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A conditional branch in the pipeline
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    /// Condition to evaluate
    pub condition: Expression,
    /// Steps to execute if condition is true
    pub pipeline: Vec<Step>,
}

/// Strategy for merging results from parallel execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    /// Wait for all steps to complete
    All,
    /// Return when any step completes
    Any,
    /// Return the fastest result
    Fastest,
    /// Use majority voting
    Majority,
}

impl Pipeline {
    /// Create a new empty pipeline
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Add a step to the pipeline
    pub fn add_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Add multiple steps
    pub fn with_steps(mut self, steps: Vec<Step>) -> Self {
        self.steps = steps;
        self
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureDefinition {
    /// Create a new feature definition
    pub fn new(name: String, value: Expression) -> Self {
        Self { name, value }
    }
}

impl PromptTemplate {
    /// Create a new prompt template
    pub fn new(template: String) -> Self {
        Self { template }
    }
}

impl Schema {
    /// Create a new object schema
    pub fn object() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: Some(HashMap::new()),
        }
    }

    /// Add a property to the schema
    pub fn add_property(mut self, name: String, property: SchemaProperty) -> Self {
        self.properties
            .get_or_insert_with(HashMap::new)
            .insert(name, property);
        self
    }
}

impl SchemaProperty {
    /// Create a string property
    pub fn string() -> Self {
        Self {
            property_type: "string".to_string(),
            description: None,
        }
    }

    /// Create a number property
    pub fn number() -> Self {
        Self {
            property_type: "number".to_string(),
            description: None,
        }
    }

    /// Create a boolean property
    pub fn boolean() -> Self {
        Self {
            property_type: "boolean".to_string(),
            description: None,
        }
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}

impl Branch {
    /// Create a new branch
    pub fn new(condition: Expression, pipeline: Vec<Step>) -> Self {
        Self {
            condition,
            pipeline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expression, Operator};
    use crate::Value;

    #[test]
    fn test_pipeline_creation() {
        let pipeline = Pipeline::new()
            .add_step(Step::Extract {
                id: "extract_1".to_string(),
                features: vec![],
            })
            .add_step(Step::Include {
                ruleset: "fraud_rules".to_string(),
            });

        assert_eq!(pipeline.steps.len(), 2);
    }

    #[test]
    fn test_extract_step() {
        let feature = FeatureDefinition::new(
            "login_count".to_string(),
            Expression::field_access(vec!["user".to_string(), "login_count".to_string()]),
        );

        let step = Step::Extract {
            id: "extract_features".to_string(),
            features: vec![feature],
        };

        if let Step::Extract { id, features } = step {
            assert_eq!(id, "extract_features");
            assert_eq!(features.len(), 1);
            assert_eq!(features[0].name, "login_count");
        } else {
            panic!("Expected Extract step");
        }
    }

    #[test]
    fn test_reason_step() {
        let schema = Schema::object()
            .add_property("is_fraud".to_string(), SchemaProperty::boolean())
            .add_property(
                "confidence".to_string(),
                SchemaProperty::number().with_description("Confidence score".to_string()),
            );

        let step = Step::Reason {
            id: "llm_analysis".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            prompt: PromptTemplate::new("Analyze this transaction: {{event.data}}".to_string()),
            output_schema: Some(schema),
        };

        if let Step::Reason {
            id,
            provider,
            model,
            prompt,
            output_schema,
        } = step
        {
            assert_eq!(id, "llm_analysis");
            assert_eq!(provider, "openai");
            assert_eq!(model, "gpt-4");
            assert!(prompt.template.contains("{{event.data}}"));
            assert!(output_schema.is_some());

            let schema = output_schema.unwrap();
            assert_eq!(schema.schema_type, "object");
            assert_eq!(schema.properties.unwrap().len(), 2);
        } else {
            panic!("Expected Reason step");
        }
    }

    #[test]
    fn test_service_step() {
        let mut params = HashMap::new();
        params.insert(
            "user_id".to_string(),
            Expression::field_access(vec!["user".to_string(), "id".to_string()]),
        );

        let step = Step::Service {
            id: "check_blacklist".to_string(),
            service: "blacklist_service".to_string(),
            operation: "check".to_string(),
            params,
        };

        if let Step::Service {
            id,
            service,
            operation,
            params,
        } = step
        {
            assert_eq!(id, "check_blacklist");
            assert_eq!(service, "blacklist_service");
            assert_eq!(operation, "check");
            assert_eq!(params.len(), 1);
        } else {
            panic!("Expected Service step");
        }
    }

    #[test]
    fn test_include_step() {
        let step = Step::Include {
            ruleset: "account_takeover".to_string(),
        };

        if let Step::Include { ruleset } = step {
            assert_eq!(ruleset, "account_takeover");
        } else {
            panic!("Expected Include step");
        }
    }

    #[test]
    fn test_branch_step() {
        let condition = Expression::binary(
            Expression::field_access(vec!["user".to_string(), "age".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(18.0)),
        );

        let branch = Branch::new(
            condition.clone(),
            vec![Step::Include {
                ruleset: "adult_rules".to_string(),
            }],
        );

        let step = Step::Branch {
            branches: vec![branch],
        };

        if let Step::Branch { branches } = step {
            assert_eq!(branches.len(), 1);
            assert!(branches[0].condition == condition);
            assert_eq!(branches[0].pipeline.len(), 1);
        } else {
            panic!("Expected Branch step");
        }
    }

    #[test]
    fn test_parallel_step() {
        let step = Step::Parallel {
            steps: vec![
                Step::Include {
                    ruleset: "rules_1".to_string(),
                },
                Step::Include {
                    ruleset: "rules_2".to_string(),
                },
            ],
            merge: MergeStrategy::All,
        };

        if let Step::Parallel { steps, merge } = step {
            assert_eq!(steps.len(), 2);
            assert_eq!(merge, MergeStrategy::All);
        } else {
            panic!("Expected Parallel step");
        }
    }

    #[test]
    fn test_merge_strategies() {
        assert_eq!(MergeStrategy::All, MergeStrategy::All);
        assert_ne!(MergeStrategy::All, MergeStrategy::Any);
        assert_ne!(MergeStrategy::Fastest, MergeStrategy::Majority);
    }

    #[test]
    fn test_complete_pipeline() {
        // Create a realistic pipeline
        let pipeline = Pipeline::new()
            // Step 1: Extract features
            .add_step(Step::Extract {
                id: "extract".to_string(),
                features: vec![
                    FeatureDefinition::new(
                        "login_count".to_string(),
                        Expression::field_access(vec!["user".to_string(), "logins".to_string()]),
                    ),
                    FeatureDefinition::new(
                        "device_count".to_string(),
                        Expression::field_access(vec!["user".to_string(), "devices".to_string()]),
                    ),
                ],
            })
            // Step 2: Run fraud detection rules
            .add_step(Step::Include {
                ruleset: "fraud_detection".to_string(),
            })
            // Step 3: If score is high, use LLM for analysis
            .add_step(Step::Branch {
                branches: vec![Branch::new(
                    Expression::binary(
                        Expression::field_access(vec!["total_score".to_string()]),
                        Operator::Gt,
                        Expression::literal(Value::Number(100.0)),
                    ),
                    vec![Step::Reason {
                        id: "llm_check".to_string(),
                        provider: "openai".to_string(),
                        model: "gpt-4".to_string(),
                        prompt: PromptTemplate::new("Analyze fraud risk".to_string()),
                        output_schema: None,
                    }],
                )],
            });

        assert_eq!(pipeline.steps.len(), 3);
    }

    #[test]
    fn test_pipeline_serde() {
        let pipeline = Pipeline::new().add_step(Step::Include {
            ruleset: "test_rules".to_string(),
        });

        // Serialize to JSON
        let json = serde_json::to_string(&pipeline).unwrap();
        assert!(json.contains("\"ruleset\":\"test_rules\""));

        // Deserialize back
        let deserialized: Pipeline = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, pipeline);
    }
}
