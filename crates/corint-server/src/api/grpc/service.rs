// gRPC service implementation for the Decision API
//
// This module provides the gRPC service implementation that wraps the
// DecisionEngine to handle gRPC requests.

use crate::api::grpc::pb::{
    decision_service_server::DecisionService, Action, Cognition, Decision, DecideRequest,
    DecideResponse, Evidence, HealthCheckRequest, HealthCheckResponse, ReloadRepositoryRequest,
    ReloadRepositoryResponse, Scores, Value as ProtoValue,
};
use corint_core::Value;
use corint_sdk::{DecisionEngine, DecisionRequest as SdkDecisionRequest, ScoreNormalizer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::{error, info};

// Include the generated protobuf code
pub mod pb {
    tonic::include_proto!("corint.decision.v1");
}

/// gRPC service implementation
pub struct DecisionGrpcService {
    engine: Arc<RwLock<DecisionEngine>>,
}

impl DecisionGrpcService {
    /// Create a new gRPC service
    pub fn new(engine: Arc<RwLock<DecisionEngine>>) -> Self {
        Self { engine }
    }
}

#[tonic::async_trait]
impl DecisionService for DecisionGrpcService {
    async fn decide(
        &self,
        request: Request<DecideRequest>,
    ) -> Result<Response<DecideResponse>, Status> {
        let req = request.into_inner();

        info!(
            "Received gRPC decision request with {} event fields",
            req.event.len()
        );

        // Convert event data from protobuf to SDK format
        let event_data = convert_proto_map_to_value_map(req.event)
            .map_err(|e| Status::invalid_argument(format!("Invalid event data: {}", e)))?;

        // Create SDK decision request
        let mut sdk_request = SdkDecisionRequest::new(event_data);

        // Add user namespace if provided
        if !req.user.is_empty() {
            let user_data = convert_proto_map_to_value_map(req.user)
                .map_err(|e| Status::invalid_argument(format!("Invalid user data: {}", e)))?;
            sdk_request = sdk_request.with_vars(user_data);
        }

        // Add features namespace if provided
        if !req.features.is_empty() {
            let features_data = convert_proto_map_to_value_map(req.features)
                .map_err(|e| Status::invalid_argument(format!("Invalid features data: {}", e)))?;
            sdk_request = sdk_request.with_features(features_data);
        }

        // Apply request options
        if let Some(opts) = req.options {
            if opts.include_trace {
                sdk_request = sdk_request.with_trace();
            }
        }

        // Execute decision
        let engine = self.engine.read().await;
        let response = engine.decide(sdk_request).await.map_err(|e| {
            error!("Decision execution failed: {}", e);
            Status::internal(format!("Decision execution failed: {}", e))
        })?;
        drop(engine);

        // Convert response
        let result_str = response
            .result
            .signal
            .map(|s| format!("{:?}", s).to_uppercase())
            .unwrap_or_else(|| "PASS".to_string());

        let decision = Decision {
            result: result_str,
            actions: response
                .result
                .actions
                .iter()
                .map(|a| Action {
                    action_type: a.clone(),
                    params: HashMap::new(),
                })
                .collect(),
            scores: Some(Scores {
                canonical: ScoreNormalizer::default().normalize(response.result.score) as f64,
                raw: response.result.score as f64,
            }),
            evidence: Some(Evidence {
                triggered_rules: response.result.triggered_rules.clone(),
                data: HashMap::new(),
            }),
            cognition: Some(Cognition {
                summary: response.result.explanation.clone(),
                reason_codes: extract_reason_codes(&response.result.explanation),
                data: HashMap::new(),
            }),
        };

        let grpc_response = DecideResponse {
            request_id: response.request_id,
            status: 200,
            process_time_ms: response.processing_time_ms as i64,
            pipeline_id: response.pipeline_id.unwrap_or_else(|| "default".to_string()),
            decision: Some(decision),
            error: None,
            trace: None, // TODO: Convert trace if requested
            features: HashMap::new(),
        };

        Ok(Response::new(grpc_response))
    }

    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Ok(Response::new(HealthCheckResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }))
    }

    async fn reload_repository(
        &self,
        _request: Request<ReloadRepositoryRequest>,
    ) -> Result<Response<ReloadRepositoryResponse>, Status> {
        info!("Reloading repository via gRPC");

        let mut engine = self.engine.write().await;
        match engine.reload().await {
            Ok(_) => {
                info!("Repository reloaded successfully via gRPC");
                Ok(Response::new(ReloadRepositoryResponse {
                    success: true,
                    message: "Repository reloaded successfully".to_string(),
                    pipelines_loaded: 0, // TODO: Get actual count from reload result
                    rules_loaded: 0,     // TODO: Get actual count from reload result
                }))
            }
            Err(e) => {
                error!("Failed to reload repository: {}", e);
                Err(Status::internal(format!(
                    "Failed to reload repository: {}",
                    e
                )))
            }
        }
    }
}

/// Convert protobuf Value to SDK Value
fn convert_proto_value_to_value(proto_val: ProtoValue) -> Result<Value, String> {
    use crate::api::grpc::pb::value::Kind;

    match proto_val.kind {
        Some(Kind::BoolValue(b)) => Ok(Value::Bool(b)),
        Some(Kind::IntValue(i)) => Ok(Value::Number(i as f64)),
        Some(Kind::DoubleValue(d)) => Ok(Value::Number(d)),
        Some(Kind::StringValue(s)) => Ok(Value::String(s)),
        Some(Kind::ListValue(list)) => {
            let values: Result<Vec<Value>, String> = list
                .values
                .into_iter()
                .map(convert_proto_value_to_value)
                .collect();
            Ok(Value::Array(values?))
        }
        Some(Kind::MapValue(map)) => {
            let fields: Result<HashMap<String, Value>, String> = map
                .fields
                .into_iter()
                .map(|(k, v)| convert_proto_value_to_value(v).map(|val| (k, val)))
                .collect();
            Ok(Value::Object(fields?))
        }
        Some(Kind::NullValue(_)) | None => Ok(Value::Null),
    }
}

/// Convert protobuf map to SDK value map
fn convert_proto_map_to_value_map(
    proto_map: HashMap<String, ProtoValue>,
) -> Result<HashMap<String, Value>, String> {
    proto_map
        .into_iter()
        .map(|(k, v)| convert_proto_value_to_value(v).map(|val| (k, val)))
        .collect()
}

/// Extract reason codes from explanation string
fn extract_reason_codes(explanation: &str) -> Vec<String> {
    let mut codes = Vec::new();

    if explanation.to_lowercase().contains("email")
        && explanation.to_lowercase().contains("not verified")
    {
        codes.push("EMAIL_NOT_VERIFIED".to_string());
    }
    if explanation.to_lowercase().contains("phone")
        && explanation.to_lowercase().contains("not verified")
    {
        codes.push("PHONE_NOT_VERIFIED".to_string());
    }
    if explanation.to_lowercase().contains("new account")
        || explanation.to_lowercase().contains("account_age")
    {
        codes.push("NEW_ACCOUNT".to_string());
    }
    if explanation.to_lowercase().contains("high")
        && explanation.to_lowercase().contains("amount")
    {
        codes.push("HIGH_TRANSACTION_AMOUNT".to_string());
    }
    if explanation.to_lowercase().contains("low risk") {
        codes.push("LOW_RISK".to_string());
    }

    codes
}
