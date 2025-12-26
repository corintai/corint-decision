//! Request/Response types for DecisionEngine

use corint_core::Value;
use corint_runtime::{ContextInput, DecisionResult, ExecutionTrace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Decision request options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionOptions {
    /// Enable detailed execution tracing
    #[serde(default)]
    pub enable_trace: bool,
}

/// Decision request (supports Phase 5 multi-namespace format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRequest {
    /// Event data (required)
    pub event_data: HashMap<String, Value>,

    /// Feature computation results (optional)
    #[serde(default)]
    pub features: Option<HashMap<String, Value>>,

    /// External API results (optional)
    #[serde(default)]
    pub api: Option<HashMap<String, Value>>,

    /// Service call results (optional)
    #[serde(default)]
    pub service: Option<HashMap<String, Value>>,

    /// LLM analysis results (optional)
    #[serde(default)]
    pub llm: Option<HashMap<String, Value>>,

    /// Variables (optional)
    #[serde(default)]
    pub vars: Option<HashMap<String, Value>>,

    /// Request metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Request options (including trace enablement)
    #[serde(default)]
    pub options: DecisionOptions,
}

impl DecisionRequest {
    /// Create a new decision request
    pub fn new(event_data: HashMap<String, Value>) -> Self {
        Self {
            event_data,
            features: None,
            api: None,
            service: None,
            llm: None,
            vars: None,
            metadata: HashMap::new(),
            options: DecisionOptions::default(),
        }
    }

    /// Enable execution tracing
    pub fn with_trace(mut self) -> Self {
        self.options.enable_trace = true;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Add features
    pub fn with_features(mut self, features: HashMap<String, Value>) -> Self {
        self.features = Some(features);
        self
    }

    /// Add API results
    pub fn with_api(mut self, api: HashMap<String, Value>) -> Self {
        self.api = Some(api);
        self
    }

    /// Add service results
    pub fn with_service(mut self, service: HashMap<String, Value>) -> Self {
        self.service = Some(service);
        self
    }

    /// Add LLM results
    pub fn with_llm(mut self, llm: HashMap<String, Value>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Add variables
    pub fn with_vars(mut self, vars: HashMap<String, Value>) -> Self {
        self.vars = Some(vars);
        self
    }

    /// Convert to ContextInput for runtime execution
    pub(crate) fn to_context_input(&self) -> ContextInput {
        let mut input = ContextInput::new(self.event_data.clone());

        if let Some(features) = &self.features {
            input = input.with_features(features.clone());
        }
        if let Some(api) = &self.api {
            input = input.with_api(api.clone());
        }
        if let Some(service) = &self.service {
            input = input.with_service(service.clone());
        }
        if let Some(llm) = &self.llm {
            input = input.with_llm(llm.clone());
        }
        if let Some(vars) = &self.vars {
            input = input.with_vars(vars.clone());
        }

        input
    }
}

/// Decision response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResponse {
    /// Request ID (for tracking and correlation)
    pub request_id: String,

    /// Pipeline ID that processed this request
    pub pipeline_id: Option<String>,

    /// Decision result
    pub result: DecisionResult,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Request metadata (echoed back)
    pub metadata: HashMap<String, String>,

    /// Execution trace (only present if enable_trace was set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<ExecutionTrace>,
}
