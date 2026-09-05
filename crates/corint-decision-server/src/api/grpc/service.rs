// gRPC service implementation for the Decision API
//
// This module provides the gRPC service implementation that wraps the
// DecisionEngine to handle gRPC requests.

use crate::api::grpc::pb::{
    decision_service_server::DecisionService, Action, Cognition, DecideRequest, DecideResponse,
    Decision, Evidence, HealthCheckRequest, HealthCheckResponse, ReloadRepositoryRequest,
    ReloadRepositoryResponse, Scores, Value as ProtoValue,
};
use crate::snapshot::{
    EngineManager, EngineSnapshot, ReloadError, EXPECTED_REVISION_HEADER, POLICY_HEADER,
    REVISION_HEADER,
};
use corint_decision_engine::{DecisionRequest as EngineDecisionRequest, ScoreNormalizer, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{error, info};

// Include the generated protobuf code
pub mod pb {
    tonic::include_proto!("corint.decision.v1");
}

/// gRPC service implementation
pub struct DecisionGrpcService {
    engine: Arc<EngineManager>,
    access: crate::access::AccessPolicy,
}

impl DecisionGrpcService {
    /// Create a new gRPC service
    pub fn new(engine: Arc<EngineManager>, access: crate::access::AccessPolicy) -> Self {
        Self { engine, access }
    }
    fn authorized<T>(&self, request: &Request<T>, publisher: bool) -> bool {
        let values = request.metadata().get_all("authorization");
        let mut values = values.iter();
        let token = values.next().and_then(|v| v.to_str().ok());
        if values.next().is_some() || !self.access.permits(token, publisher) {
            return false;
        }
        true
    }
}

#[tonic::async_trait]
impl DecisionService for DecisionGrpcService {
    async fn decide(
        &self,
        request: Request<DecideRequest>,
    ) -> Result<Response<DecideResponse>, Status> {
        if !self.authorized(&request, false) {
            return Err(Status::unauthenticated("UNAUTHORIZED"));
        }
        let req = request.into_inner();
        if !req.user.is_empty()
            || !req.features.is_empty()
            || !req.metadata.is_empty()
            || req.event.contains_key("tenant_id")
        {
            return Err(Status::invalid_argument("Only event data is caller-owned; tenant and computed namespaces are operator-owned"));
        }
        if req
            .options
            .as_ref()
            .is_some_and(|o| o.pipeline_id.is_some() || o.score_normalization.is_some())
        {
            return Err(Status::invalid_argument(
                "pipeline_id/score_normalization overrides are not implemented",
            ));
        }

        info!(
            "Received gRPC decision request with {} event fields",
            req.event.len()
        );

        // Convert event data from protobuf to the engine format
        let event_data = convert_proto_map_to_value_map(req.event)
            .map_err(|e| Status::invalid_argument(format!("Invalid event data: {}", e)))?;

        // Create engine decision request
        let mut engine_request =
            EngineDecisionRequest::new(event_data).with_vars(HashMap::from([(
                "tenant_id".into(),
                Value::String(self.access.tenant_id.clone()),
            )]));

        // Add user namespace if provided
        if !req.user.is_empty() {
            let user_data = convert_proto_map_to_value_map(req.user)
                .map_err(|e| Status::invalid_argument(format!("Invalid user data: {}", e)))?;
            engine_request = engine_request.with_vars(user_data);
        }

        // Add features namespace if provided
        if !req.features.is_empty() {
            let features_data = convert_proto_map_to_value_map(req.features)
                .map_err(|e| Status::invalid_argument(format!("Invalid features data: {}", e)))?;
            engine_request = engine_request.with_features(features_data);
        }

        let include_features = req.options.as_ref().is_some_and(|o| o.include_features);
        // Apply request options
        if let Some(opts) = req.options {
            if opts.include_trace {
                engine_request = engine_request.with_trace();
            }
        }

        // Execute decision
        let snapshot = self.engine.snapshot().await;
        let response = snapshot.engine.decide(engine_request).await.map_err(|e| {
            error!("Decision execution failed: {}", e);
            Status::internal("Decision execution failed")
        })?;

        // Convert response
        let result_str = response
            .result
            .signal
            .map(|s| format!("{:?}", s).to_lowercase())
            .unwrap_or_else(|| "pass".to_string());

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
            pipeline_id: response
                .pipeline_id
                .unwrap_or_else(|| "default".to_string()),
            decision: Some(decision),
            error: None,
            trace: response
                .trace
                .as_ref()
                .map(|trace| crate::api::grpc::pb::ExecutionTrace {
                    canonical_json: serde_json::to_string(trace).expect("serializable trace"),
                    pipeline: trace.pipeline.as_ref().map(|p| {
                        crate::api::grpc::pb::PipelineTrace {
                            id: p.pipeline_id.clone(),
                            name: String::new(),
                            status: "executed".into(),
                        }
                    }),
                    steps: trace
                        .pipeline
                        .as_ref()
                        .map(|p| {
                            p.steps
                                .iter()
                                .map(|s| crate::api::grpc::pb::StepTrace {
                                    id: s.step_id.clone(),
                                    step_type: s.step_type.clone(),
                                    status: if s.executed { "executed" } else { "skipped" }.into(),
                                    result: HashMap::from([(
                                        "detail".into(),
                                        json_to_proto(serde_json::to_value(s).expect("step trace")),
                                    )]),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                }),
            features: if include_features {
                response
                    .result
                    .context
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k,
                            json_to_proto(serde_json::to_value(v).expect("engine value")),
                        )
                    })
                    .collect()
            } else {
                HashMap::new()
            },
        };

        Ok(with_snapshot(grpc_response, &snapshot))
    }

    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let snapshot = self.engine.snapshot().await;
        Ok(with_snapshot(
            HealthCheckResponse {
                status: "healthy".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            &snapshot,
        ))
    }

    async fn reload_repository(
        &self,
        request: Request<ReloadRepositoryRequest>,
    ) -> Result<Response<ReloadRepositoryResponse>, Status> {
        if !self.authorized(&request, true) {
            return Err(Status::unauthenticated("UNAUTHORIZED"));
        }
        let expected = request
            .metadata()
            .get(EXPECTED_REVISION_HEADER)
            .map(|value| value.to_str())
            .transpose()
            .map_err(|_| Status::invalid_argument("Invalid expected revision"))?;
        let snapshot = self
            .engine
            .reload(expected)
            .await
            .map_err(|error| match error {
                ReloadError::Busy => Status::resource_exhausted(error.to_string()),
                ReloadError::Stale => Status::aborted(error.to_string()),
                _ => Status::internal("Repository reload failed"),
            })?;
        Ok(with_snapshot(
            ReloadRepositoryResponse {
                success: true,
                message: "Repository reloaded successfully".to_string(),
                pipelines_loaded: 0,
                rules_loaded: 0,
            },
            &snapshot,
        ))
    }
}

fn with_snapshot<T>(body: T, snapshot: &EngineSnapshot) -> Response<T> {
    let mut response = Response::new(body);
    response.metadata_mut().insert(
        REVISION_HEADER,
        snapshot.revision.parse().expect("UUID metadata"),
    );
    response.metadata_mut().insert(
        POLICY_HEADER,
        snapshot.compiled_sha256.parse().expect("SHA256 metadata"),
    );
    response
}

/// Convert protobuf Value to engine Value
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

/// Convert protobuf map to engine value map
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
    if explanation.to_lowercase().contains("high") && explanation.to_lowercase().contains("amount")
    {
        codes.push("HIGH_TRANSACTION_AMOUNT".to_string());
    }
    if explanation.to_lowercase().contains("low risk") {
        codes.push("LOW_RISK".to_string());
    }

    codes
}

fn json_to_proto(value: serde_json::Value) -> ProtoValue {
    use crate::api::grpc::pb::{value::Kind, ListValue, MapValue, NullValue};
    let kind = match value {
        serde_json::Value::Null => Kind::NullValue(NullValue::NullValue as i32),
        serde_json::Value::Bool(v) => Kind::BoolValue(v),
        serde_json::Value::Number(v) => Kind::DoubleValue(v.as_f64().expect("JSON number")),
        serde_json::Value::String(v) => Kind::StringValue(v),
        serde_json::Value::Array(v) => Kind::ListValue(ListValue {
            values: v.into_iter().map(json_to_proto).collect(),
        }),
        serde_json::Value::Object(v) => Kind::MapValue(MapValue {
            fields: v.into_iter().map(|(k, v)| (k, json_to_proto(v))).collect(),
        }),
    };
    ProtoValue { kind: Some(kind) }
}
