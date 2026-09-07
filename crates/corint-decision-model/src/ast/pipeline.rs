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

use crate::ast::rule::WhenBlock;
use crate::ast::Expression;
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

    /// Explicit entry point - must match a step.id (required for DAG compilation)
    pub entry: String,

    /// Optional when condition - pipeline only executes if this matches
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenBlock>,

    /// The processing steps (required, non-empty)
    pub steps: Vec<PipelineStep>,

    /// Optional decision logic for final decision based on step results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<Vec<PipelineDecisionRule>>,

    /// Metadata for documentation and versioning (stored as arbitrary key-value pairs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
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

/// Pipeline decision rule for final decision logic
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineDecisionRule {
    /// Condition to evaluate (e.g., results.fraud_detection.signal == "decline")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenBlock>,

    /// Whether this is the default rule (catch-all)
    #[serde(default)]
    pub default: bool,

    /// Decision result: approve/decline/review/hold/pass
    pub result: String,

    /// User-defined actions to execute (e.g., ["KYC", "OTP", "BLOCK_DEVICE"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,

    /// Optional reason for this decision
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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

    /// Service invocation, independent of transport and deployment boundary.
    Service {
        service: String,
        operation: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<HashMap<String, Expression>>,
        /// Defaults to service.<step_id>.
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Per-invocation deadline in milliseconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
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

    /// Catch-all for unknown step types
    Unknown {},
}

/// Feature definition for extraction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureDefinition {
    /// Feature name
    pub name: String,
    /// Expression to calculate the feature value
    pub value: Expression,
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

    /// Invoke a service capability
    Service {
        /// Step identifier
        id: String,
        /// Service name
        service: String,
        /// Operation to perform
        operation: String,
        /// Parameters for the service call
        params: HashMap<String, Expression>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        /// Output path under service or vars
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
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

impl Pipeline {
    /// Create a new pipeline with required fields
    pub fn new(id: String, name: String, entry: String) -> Self {
        Self {
            id,
            name,
            description: None,
            entry,
            when: None,
            steps: Vec::new(),
            decision: None,
            metadata: None,
        }
    }

    /// Set the pipeline description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the pipeline metadata
    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
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

    /// Set decision rules
    pub fn with_decision(mut self, decision: Vec<PipelineDecisionRule>) -> Self {
        self.decision = Some(decision);
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

    /// Create a transport-independent service step.
    pub fn service(id: String, name: String, service: String, operation: String) -> Self {
        Self {
            id,
            name,
            step_type: "service".into(),
            routes: None,
            default: None,
            next: None,
            when: None,
            details: StepDetails::Service {
                service,
                operation,
                params: None,
                output: None,
                timeout_ms: None,
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
    use crate::ast::rule::{Condition, ConditionGroup};
    use crate::ast::{Expression, Operator};
    use crate::Value;

    #[test]
    fn test_pipeline_creation() {
        let mut metadata = HashMap::new();
        metadata.insert("version".to_string(), serde_json::json!("1.0.0"));
        metadata.insert("author".to_string(), serde_json::json!("Test Author"));

        let pipeline = Pipeline::new(
            "test_pipeline".to_string(),
            "Test Pipeline".to_string(),
            "step1".to_string(),
        )
        .with_metadata(metadata.clone())
        .add_step(PipelineStep::router(
            "step1".to_string(),
            "Router Step".to_string(),
        ));

        assert_eq!(pipeline.id, "test_pipeline");
        assert_eq!(pipeline.name, "Test Pipeline");
        assert_eq!(pipeline.entry, "step1");
        assert_eq!(pipeline.metadata, Some(metadata));
        assert_eq!(pipeline.steps.len(), 1);
    }

    #[test]
    fn test_router_step() {
        let when_block = WhenBlock {
            event_type: None,
            condition_group: Some(ConditionGroup::All(vec![Condition::Expression(
                Expression::binary(
                    Expression::field_access(vec!["amount".to_string()]),
                    Operator::Gt,
                    Expression::literal(Value::Number(1000.0)),
                ),
            )])),
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
        let pipeline =
            Pipeline::new("test".to_string(), "Test".to_string(), "step1".to_string()).add_step(
                PipelineStep::router("step1".to_string(), "Router".to_string()),
            );

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
