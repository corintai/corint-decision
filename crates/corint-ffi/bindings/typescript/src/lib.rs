#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use corint_sdk::{DecisionEngineBuilder, DecisionRequest as SdkDecisionRequest, RepositoryConfig};
use corint_core::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Returns the version of the CORINT Decision Engine
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Decision Engine wrapper for Node.js
#[napi]
pub struct Engine {
    inner: Arc<RwLock<corint_sdk::DecisionEngine>>,
}

#[napi]
impl Engine {
    /// Create a new Decision Engine instance from repository path
    ///
    /// # Arguments
    /// * `repository_path` - Path to the rule repository directory
    #[napi(factory)]
    pub async fn from_repository(repository_path: String) -> Result<Self> {
        let repo_config = RepositoryConfig::file_system(repository_path);

        let engine = DecisionEngineBuilder::new()
            .with_repository(repo_config)
            .enable_metrics(true)
            .build()
            .await
            .map_err(|e| Error::from_reason(format!("Failed to create engine: {}", e)))?;

        Ok(Self {
            inner: Arc::new(RwLock::new(engine)),
        })
    }

    /// Create a new Decision Engine instance with rule content
    ///
    /// # Arguments
    /// * `pipeline_id` - Identifier for the pipeline
    /// * `yaml_content` - YAML content of the pipeline
    #[napi(factory)]
    pub async fn from_yaml(pipeline_id: String, yaml_content: String) -> Result<Self> {
        let engine = DecisionEngineBuilder::new()
            .add_rule_content(pipeline_id, yaml_content)
            .enable_metrics(true)
            .build()
            .await
            .map_err(|e| Error::from_reason(format!("Failed to create engine: {}", e)))?;

        Ok(Self {
            inner: Arc::new(RwLock::new(engine)),
        })
    }

    /// Execute a decision request
    ///
    /// # Arguments
    /// * `request_json` - Decision request as JSON string
    ///
    /// # Returns
    /// Decision response as JSON string
    #[napi]
    pub async fn decide(&self, request_json: String) -> Result<String> {
        // Parse JSON string to DecisionRequest
        let decision_request: SdkDecisionRequest = serde_json::from_str(&request_json)
            .map_err(|e| Error::from_reason(format!("Failed to parse request: {}", e)))?;

        // Execute decision
        let engine = self.inner.read().await;
        let response = engine.decide(decision_request).await
            .map_err(|e| Error::from_reason(format!("Decision execution failed: {}", e)))?;

        // Convert response to JSON string
        let response_json = serde_json::to_string(&response)
            .map_err(|e| Error::from_reason(format!("Failed to serialize response: {}", e)))?;

        Ok(response_json)
    }

    /// Execute a simple decision with event data only
    ///
    /// # Arguments
    /// * `event_data_json` - Event data as JSON string
    ///
    /// # Returns
    /// Decision response as JSON string
    #[napi]
    pub async fn decide_simple(&self, event_data_json: String) -> Result<String> {
        // Parse event data
        let event_data: HashMap<String, Value> = serde_json::from_str(&event_data_json)
            .map_err(|e| Error::from_reason(format!("Failed to parse event data: {}", e)))?;

        // Create decision request
        let decision_request = SdkDecisionRequest::new(event_data);

        // Execute decision
        let engine = self.inner.read().await;
        let response = engine.decide(decision_request).await
            .map_err(|e| Error::from_reason(format!("Decision execution failed: {}", e)))?;

        // Convert response to JSON string
        let response_json = serde_json::to_string(&response)
            .map_err(|e| Error::from_reason(format!("Failed to serialize response: {}", e)))?;

        Ok(response_json)
    }
}
