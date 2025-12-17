//! Pipeline AST definitions
//!
//! A Pipeline orchestrates the decision flow through multiple steps,
//! following the mypipeline.yml design specification.
//!
//! Key features:
//! - Explicit entry point for DAG compilation
//! - Unified Step structure with type-specific fields
//! - Router step for pure conditional routing
//! - Routes with when conditions (consistent with registry format)
//! - Convention over Configuration for outputs

use crate::ast::Expression;
use crate::ast::rule::WhenBlock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A pipeline defines a sequence of processing steps
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pipeline {
    /// Unique identifier for the pipeline (required)
    pub id: String,

    /// Human-readable name (required)
    pub name: String,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Optional version (semver format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Explicit entry point - must match a step.id (required for DAG compilation)
    pub entry: String,

    /// Optional when condition - pipeline only executes if this matches
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenBlock>,

    /// The processing steps (required, non-empty)
    pub steps: Vec<PipelineStep>,
}

/// A single step in the pipeline (unified structure)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStep {
    /// Step identifier (required, unique within pipeline, snake_case)
    pub id: String,

    /// Human-readable name (required)
    pub name: String,

    /// Step type (required)
    #[serde(rename = "type")]
    pub step_type: String,

    /// Conditional routes (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routes: Option<Vec<Route>>,

    /// Default route if no condition matches (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,

    /// Unconditional next step (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<StepNext>,

    /// Step-level when condition (optional, for cost control)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenBlock>,

    /// Type-specific fields (flattened)
    #[serde(flatten)]
    pub details: StepDetails,
}

/// Next step reference
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StepNext {
    /// Reference to another step by ID
    StepId(String),
}

/// Route with condition (consistent with registry format: target outside, condition inside)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    /// Target step id or "end"
    pub next: String,

    /// Condition for this route
    pub when: WhenBlock,
}

/// Type-specific step details
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StepDetails {
    /// Router step - pure routing, no computation
    Router {},

    /// Function step - pure computation
    Function {
        /// Function name
        function: String,
        /// Parameters
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<HashMap<String, Expression>>,
    },

    /// Rule step - execute single rule
    Rule {
        /// Rule ID
        rule: String,
    },

    /// Ruleset step - execute rule set
    Ruleset {
        /// Ruleset ID
        ruleset: String,
    },

    /// Pipeline step - call sub-pipeline
    SubPipeline {
        /// Pipeline ID
        #[serde(rename = "pipeline")]
        pipeline_id: String,
    },

    /// Service step - internal service call
    Service {
        /// Service name
        service: String,
        /// Optional query/operation
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<String>,
        /// Parameters
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<HashMap<String, Expression>>,
        /// Output variable (optional, Convention: service.<name>)
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },

    /// API step - external API call
    Api {
        /// API target (single, any, or all)
        #[serde(flatten)]
        api_target: ApiTarget,
        /// Optional endpoint
        #[serde(skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
        /// Parameters
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<HashMap<String, Expression>>,
        /// Output variable (optional, Convention: api.<name>)
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Timeout in seconds (optional, for 'all' mode aggregation)
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout: Option<u64>,
        /// Error handling for 'all' mode (optional)
        #[serde(skip_serializing_if = "Option::is_none")]
        on_error: Option<String>,
        /// Minimum successful calls for 'all' mode (optional)
        #[serde(skip_serializing_if = "Option::is_none")]
        min_success: Option<usize>,
    },

    /// Trigger step - external action (MQ, Webhook, notification)
    Trigger {
        /// Target service/endpoint
        target: String,
        /// Parameters
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<HashMap<String, Expression>>,
    },

    /// Legacy/compatibility steps
    Extract {
        /// Feature definitions
        #[serde(skip_serializing_if = "Option::is_none")]
        features: Option<Vec<FeatureDefinition>>,
    },

    Reason {
        /// LLM provider
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
        /// Model name
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        /// Prompt template
        #[serde(skip_serializing_if = "Option::is_none")]
        prompt: Option<PromptTemplate>,
        /// Output schema
        #[serde(skip_serializing_if = "Option::is_none")]
        output_schema: Option<Schema>,
    },

    /// Catch-all for unknown step types
    Unknown {},
}

/// API target specification (single, any, or all)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiTarget {
    /// Single API
    Single {
        /// API identifier
        api: String,
    },
    /// Any mode - try in sequence, use first success (fallback/degradation)
    Any {
        /// List of APIs to try
        any: Vec<String>,
    },
    /// All mode - parallel execution, wait for all (aggregation)
    All {
        /// List of APIs to call
        all: Vec<String>,
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

// Legacy types for backward compatibility
/// A single step in the pipeline (legacy enum - kept for backward compatibility)
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

    /// Call external service (internal)
    Service {
        /// Step identifier
        id: String,
        /// Service name
        service: String,
        /// Operation to perform
        operation: String,
        /// Parameters for the service call
        params: HashMap<String, Expression>,
        /// Output variable path (e.g., "context.result")
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },

    /// Call external API (third-party)
    #[serde(rename = "api")]
    Api {
        /// Step identifier
        id: String,
        /// API identifier (e.g., "ipinfo", "chainalysis")
        api: String,
        /// Endpoint name
        endpoint: String,
        /// Parameters for the API call
        params: HashMap<String, Expression>,
        /// Output variable path (e.g., "context.ip_info")
        output: String,
        /// Timeout in milliseconds
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout: Option<u64>,
        /// Error handling strategy
        #[serde(skip_serializing_if = "Option::is_none")]
        on_error: Option<ErrorHandling>,
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

/// Error handling configuration for external API calls
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorHandling {
    /// Action to take on error
    pub action: ErrorAction,
    /// Fallback value if action is "fallback"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<serde_json::Value>,
}

/// Action to take when API call fails
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorAction {
    /// Use fallback value
    Fallback,
    /// Skip this step
    Skip,
    /// Fail the entire pipeline
    Fail,
    /// Retry the call
    Retry,
}

impl Pipeline {
    /// Create a new pipeline with required fields
    pub fn new(id: String, name: String, entry: String) -> Self {
        Self {
            id,
            name,
            description: None,
            version: None,
            entry,
            when: None,
            steps: Vec::new(),
        }
    }

    /// Set the pipeline description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the pipeline version
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }

    /// Set the when condition
    pub fn with_when(mut self, when: WhenBlock) -> Self {
        self.when = Some(when);
        self
    }

    /// Add a step to the pipeline
    pub fn add_step(mut self, step: PipelineStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Add multiple steps
    pub fn with_steps(mut self, steps: Vec<PipelineStep>) -> Self {
        self.steps = steps;
        self
    }
}

impl PipelineStep {
    /// Create a router step
    pub fn router(id: String, name: String) -> Self {
        Self {
            id,
            name,
            step_type: "router".to_string(),
            routes: None,
            default: None,
            next: None,
            when: None,
            details: StepDetails::Router {},
        }
    }

    /// Create an API step
    pub fn api(id: String, name: String, api: String) -> Self {
        Self {
            id,
            name,
            step_type: "api".to_string(),
            routes: None,
            default: None,
            next: None,
            when: None,
            details: StepDetails::Api {
                api_target: ApiTarget::Single { api },
                endpoint: None,
                params: None,
                output: None,
                timeout: None,
                on_error: None,
                min_success: None,
            },
        }
    }

    /// Create a ruleset step
    pub fn ruleset(id: String, name: String, ruleset: String) -> Self {
        Self {
            id,
            name,
            step_type: "ruleset".to_string(),
            routes: None,
            default: None,
            next: None,
            when: None,
            details: StepDetails::Ruleset { ruleset },
        }
    }

    /// Add routes to the step
    pub fn with_routes(mut self, routes: Vec<Route>) -> Self {
        self.routes = Some(routes);
        self
    }

    /// Set default route
    pub fn with_default(mut self, default: String) -> Self {
        self.default = Some(default);
        self
    }

    /// Set next step
    pub fn with_next(mut self, next: String) -> Self {
        self.next = Some(StepNext::StepId(next));
        self
    }
}

impl Route {
    /// Create a new route
    pub fn new(next: String, when: WhenBlock) -> Self {
        Self { next, when }
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
    use crate::ast::rule::{ConditionGroup, Condition};
    use crate::Value;

    #[test]
    fn test_pipeline_creation() {
        let pipeline = Pipeline::new(
            "test_pipeline".to_string(),
            "Test Pipeline".to_string(),
            "step1".to_string(),
        )
        .with_version("1.0.0".to_string())
        .add_step(PipelineStep::router("step1".to_string(), "Router Step".to_string()));

        assert_eq!(pipeline.id, "test_pipeline");
        assert_eq!(pipeline.name, "Test Pipeline");
        assert_eq!(pipeline.entry, "step1");
        assert_eq!(pipeline.version, Some("1.0.0".to_string()));
        assert_eq!(pipeline.steps.len(), 1);
    }

    #[test]
    fn test_router_step() {
        let when_block = WhenBlock {
            event_type: None,
            condition_group: Some(ConditionGroup::All(vec![
                Condition::Expression(Expression::binary(
                    Expression::field_access(vec!["amount".to_string()]),
                    Operator::Gt,
                    Expression::literal(Value::Number(1000.0)),
                )),
            ])),
            conditions: None,
        };

        let route = Route::new("high_value".to_string(), when_block);

        let step = PipelineStep::router("router1".to_string(), "Amount Router".to_string())
            .with_routes(vec![route])
            .with_default("standard".to_string());

        assert_eq!(step.id, "router1");
        assert_eq!(step.step_type, "router");
        assert!(step.routes.is_some());
        assert_eq!(step.default, Some("standard".to_string()));
    }

    #[test]
    fn test_api_step_single() {
        let step = PipelineStep::api(
            "get_ip_info".to_string(),
            "Get IP Geolocation".to_string(),
            "ip_geolocation".to_string(),
        )
        .with_next("next_step".to_string());

        assert_eq!(step.id, "get_ip_info");
        assert_eq!(step.step_type, "api");
        assert!(matches!(step.next, Some(StepNext::StepId(_))));
    }

    #[test]
    fn test_api_step_any_mode() {
        let details = StepDetails::Api {
            api_target: ApiTarget::Any {
                any: vec!["maxmind".to_string(), "ipinfo".to_string()],
            },
            endpoint: None,
            params: None,
            output: None,
            timeout: None,
            on_error: None,
            min_success: None,
        };

        let step = PipelineStep {
            id: "ip_lookup".to_string(),
            name: "IP Lookup with Fallback".to_string(),
            step_type: "api".to_string(),
            routes: None,
            default: None,
            next: None,
            when: None,
            details,
        };

        if let StepDetails::Api { api_target, .. } = &step.details {
            assert!(matches!(api_target, ApiTarget::Any { .. }));
        } else {
            panic!("Expected Api step");
        }
    }

    #[test]
    fn test_api_step_all_mode() {
        let details = StepDetails::Api {
            api_target: ApiTarget::All {
                all: vec![
                    "credit_bureau".to_string(),
                    "fraud_detection".to_string(),
                ],
            },
            endpoint: None,
            params: None,
            output: None,
            timeout: Some(5),
            on_error: Some("partial".to_string()),
            min_success: Some(1),
        };

        let step = PipelineStep {
            id: "external_checks".to_string(),
            name: "Parallel External Checks".to_string(),
            step_type: "api".to_string(),
            routes: None,
            default: None,
            next: None,
            when: None,
            details,
        };

        if let StepDetails::Api {
            api_target,
            timeout,
            on_error,
            min_success,
            ..
        } = &step.details
        {
            assert!(matches!(api_target, ApiTarget::All { .. }));
            assert_eq!(*timeout, Some(5));
            assert_eq!(on_error.as_deref(), Some("partial"));
            assert_eq!(*min_success, Some(1));
        } else {
            panic!("Expected Api step");
        }
    }

    #[test]
    fn test_ruleset_step() {
        let step = PipelineStep::ruleset(
            "fraud_rules".to_string(),
            "Fraud Detection Rules".to_string(),
            "payment_fraud".to_string(),
        )
        .with_next("decision".to_string());

        assert_eq!(step.id, "fraud_rules");
        assert_eq!(step.step_type, "ruleset");

        if let StepDetails::Ruleset { ruleset } = &step.details {
            assert_eq!(ruleset, "payment_fraud");
        } else {
            panic!("Expected Ruleset step");
        }
    }

    #[test]
    fn test_pipeline_serde() {
        let pipeline = Pipeline::new(
            "test".to_string(),
            "Test".to_string(),
            "step1".to_string(),
        )
        .add_step(PipelineStep::router("step1".to_string(), "Router".to_string()));

        // Serialize to JSON
        let json = serde_json::to_string(&pipeline).unwrap();
        assert!(json.contains("\"id\":\"test\""));
        assert!(json.contains("\"entry\":\"step1\""));

        // Deserialize back
        let deserialized: Pipeline = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, pipeline.id);
        assert_eq!(deserialized.entry, pipeline.entry);
    }
}
