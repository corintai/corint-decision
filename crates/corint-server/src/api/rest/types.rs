//! REST API type definitions
//!
//! Request and response types for the REST API endpoints.

use corint_runtime::ExecutionTrace;
use corint_sdk::DecisionEngine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<RwLock<DecisionEngine>>,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Decision request payload (matches API_REQUEST.md spec)
#[derive(Debug, Deserialize)]
pub struct DecideRequestPayload {
    /// Event data (required)
    pub event: HashMap<String, serde_json::Value>,

    /// User profile/context (optional)
    #[serde(default)]
    pub user: Option<HashMap<String, serde_json::Value>>,

    /// Optional configuration
    #[serde(default)]
    pub options: Option<RequestOptions>,

    // Legacy/internal namespaces (for backward compatibility)
    /// Feature computation results (optional)
    #[serde(default)]
    pub features: Option<HashMap<String, serde_json::Value>>,

    /// External API results (optional)
    #[serde(default)]
    pub api: Option<HashMap<String, serde_json::Value>>,

    /// Service call results (optional)
    #[serde(default)]
    pub service: Option<HashMap<String, serde_json::Value>>,

    /// LLM analysis results (optional)
    #[serde(default)]
    pub llm: Option<HashMap<String, serde_json::Value>>,

    /// Variables (optional)
    #[serde(default)]
    pub vars: Option<HashMap<String, serde_json::Value>>,
}

/// Request options
#[derive(Debug, Default, Deserialize)]
pub struct RequestOptions {
    /// Whether to return computed feature values
    #[serde(default)]
    pub return_features: bool,

    /// Whether to return detailed execution trace
    #[serde(default)]
    pub enable_trace: bool,

    /// Whether to process asynchronously
    #[serde(default, rename = "async")]
    pub async_mode: bool,
}

/// Decision response payload (matches API_REQUEST.md spec)
#[derive(Debug, Serialize)]
pub struct DecideResponsePayload {
    /// Request ID (for tracking and correlation)
    pub request_id: String,

    /// HTTP status code
    pub status: u16,

    /// Processing time in milliseconds
    pub process_time_ms: u64,

    /// Pipeline ID that processed this request
    pub pipeline_id: String,

    /// Decision result
    pub decision: DecisionPayload,

    /// Computed features (only present if options.return_features = true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, serde_json::Value>>,

    /// Detailed execution trace (only present if options.enable_trace = true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<ExecutionTrace>,
}

/// Decision payload (nested in response)
#[derive(Debug, Serialize)]
pub struct DecisionPayload {
    /// Decision result: "ALLOW", "DENY", "REVIEW", "HOLD", "PASS"
    pub result: String,

    /// Actions to take
    pub actions: Vec<String>,

    /// Risk scores
    pub scores: ScoresPayload,

    /// Evidence
    pub evidence: EvidencePayload,

    /// Cognition (explanation)
    pub cognition: CognitionPayload,
}

/// Scores payload
#[derive(Debug, Serialize)]
pub struct ScoresPayload {
    /// Normalized risk score (0-1000)
    pub canonical: i32,

    /// Raw aggregated score from rules
    pub raw: i32,

    /// Confidence level (0-1) - optional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// Evidence payload
#[derive(Debug, Serialize)]
pub struct EvidencePayload {
    /// Array of triggered rule IDs
    pub triggered_rules: Vec<String>,
}

/// Cognition payload
#[derive(Debug, Serialize)]
pub struct CognitionPayload {
    /// Human-readable explanation
    pub summary: String,

    /// Machine-readable reason codes
    pub reason_codes: Vec<String>,
}

/// Reload repository endpoint response
#[derive(Debug, Serialize)]
pub struct ReloadResponse {
    pub success: bool,
    pub message: String,
}
