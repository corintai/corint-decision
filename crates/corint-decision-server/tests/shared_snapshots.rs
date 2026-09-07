//! Exercises the production REST router and gRPC service against one repository.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use corint_decision_engine::{DecisionEngine, DecisionEngineBuilder, RepositoryConfig};
use corint_decision_server::{
    api::{
        create_router,
        grpc::{
            pb::{self, decision_service_server::DecisionService},
            DecisionGrpcService,
        },
    },
    snapshot::{EngineManager, EXPECTED_REVISION_HEADER, POLICY_HEADER, REVISION_HEADER},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Duration};
use tempfile::TempDir;
use tower::ServiceExt;

const PIPELINE: &str = r#"version: "0.1"
import:
  rulesets: [rulesets/risk.yaml]
---
pipeline:
  id: payment
  name: Payment
  entry: check
  when:
    all: [event.amount > 0]
  steps:
    - step:
        id: check
        name: Check
        type: ruleset
        ruleset: risk
        next: end
  decision:
    - when: results.risk.score >= 60
      result: decline
      actions: [BLOCK]
    - default: true
      result: approve
"#;
const RULESET: &str = r#"version: "0.1"
import:
  rules: [rules/amount.yaml]
---
ruleset:
  id: risk
  rules: [amount]
  conclusion:
    - when: total_score >= 60
      signal: decline
    - default: true
      signal: approve
"#;
fn write_rule(root: &Path, score: i32) {
    std::fs::write(
        root.join("rules/amount.yaml"),
        format!(
            r#"version: "0.1"
rule:
  id: amount
  name: Amount
  when: event.amount > 0
  score: {score}
"#
        ),
    )
    .unwrap();
}
fn repository() -> TempDir {
    let dir = TempDir::new().unwrap();
    for path in ["pipelines", "rules", "rulesets"] {
        std::fs::create_dir_all(dir.path().join(path)).unwrap();
    }
    std::fs::write(dir.path().join("pipelines/payment.yaml"), PIPELINE).unwrap();
    std::fs::write(dir.path().join("rulesets/risk.yaml"), RULESET).unwrap();
    write_rule(dir.path(), 10);
    dir
}
async fn engine(root: &Path) -> Result<DecisionEngine, corint_decision_engine::EngineError> {
    DecisionEngineBuilder::new()
        .with_repository(RepositoryConfig::file_system(root.to_string_lossy()))
        .build()
        .await
}
async fn manager(root: &Path) -> Arc<EngineManager> {
    Arc::new(EngineManager::new(Arc::new(engine(root).await.unwrap())).unwrap())
}
async fn http_decide(app: &Router) -> (String, String, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/decide")
                .header("authorization", format!("Bearer {DECISION_TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"event":{"amount":100}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let exposed = response.headers()["access-control-expose-headers"]
        .to_str()
        .unwrap();
    assert!(exposed.contains(REVISION_HEADER));
    assert!(exposed.contains(POLICY_HEADER));
    let revision = response.headers()[REVISION_HEADER]
        .to_str()
        .unwrap()
        .to_owned();
    let hash = response.headers()[POLICY_HEADER]
        .to_str()
        .unwrap()
        .to_owned();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (revision, hash, serde_json::from_slice(&body).unwrap())
}
async fn grpc_decide(grpc: &DecisionGrpcService) -> tonic::Response<pb::DecideResponse> {
    grpc.decide(decision_request(pb::DecideRequest {
        event: [(
            "amount".into(),
            pb::Value {
                kind: Some(pb::value::Kind::IntValue(100)),
            },
        )]
        .into(),
        ..Default::default()
    }))
    .await
    .unwrap()
}
async fn assert_transports(
    app: &Router,
    grpc: &DecisionGrpcService,
    expected_signal: &str,
) -> (String, String) {
    let (revision, hash, http) = http_decide(app).await;
    let health = app
        .clone()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(health.headers()[REVISION_HEADER], revision);
    assert_eq!(health.headers()[POLICY_HEADER], hash);
    let health = grpc
        .health_check(tonic::Request::new(pb::HealthCheckRequest {}))
        .await
        .unwrap();
    assert_eq!(
        health
            .metadata()
            .get(REVISION_HEADER)
            .unwrap()
            .to_str()
            .unwrap(),
        revision
    );
    assert_eq!(
        health
            .metadata()
            .get(POLICY_HEADER)
            .unwrap()
            .to_str()
            .unwrap(),
        hash
    );
    let grpc = grpc_decide(grpc).await;
    assert_eq!(
        grpc.metadata()
            .get(REVISION_HEADER)
            .unwrap()
            .to_str()
            .unwrap(),
        revision
    );
    assert_eq!(
        grpc.metadata()
            .get(POLICY_HEADER)
            .unwrap()
            .to_str()
            .unwrap(),
        hash
    );
    let grpc = grpc.into_inner();
    let decision = grpc.decision.unwrap();
    assert_eq!(http["pipeline_id"], grpc.pipeline_id);
    assert_eq!(http["decision"]["result"], expected_signal);
    assert_eq!(decision.result.to_lowercase(), expected_signal);
    assert_eq!(
        http["decision"]["scores"]["raw"].as_f64().unwrap(),
        decision.scores.unwrap().raw
    );
    assert_eq!(
        http["decision"]["actions"],
        json!(decision
            .actions
            .into_iter()
            .map(|action| action.action_type)
            .collect::<Vec<_>>())
    );
    assert_eq!(
        http["decision"]["evidence"]["triggered_rules"],
        json!(decision.evidence.unwrap().triggered_rules)
    );
    (revision, hash)
}

#[tokio::test]
async fn either_transport_reloads_both_and_restart_reads_repository() {
    let repo = repository();
    let manager = manager(repo.path()).await;
    let app = create_router(manager.clone(), access());
    let grpc = DecisionGrpcService::new(manager.clone(), access());
    let (initial, initial_hash) = assert_transports(&app, &grpc, "approve").await;
    write_rule(repo.path(), 70);
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/repo/reload")
                .header("authorization", format!("Bearer {PUBLISHER_TOKEN}"))
                .header(EXPECTED_REVISION_HEADER, &initial)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (updated, updated_hash) = assert_transports(&app, &grpc, "decline").await;
    assert_eq!(response.headers()[REVISION_HEADER], updated);
    assert_ne!(initial, updated);
    assert_ne!(initial_hash, updated_hash);

    let restarted = EngineManager::new(Arc::new(engine(repo.path()).await.unwrap())).unwrap();
    let restarted = restarted.snapshot().await;
    assert_eq!(restarted.compiled_sha256, updated_hash);
    assert_ne!(restarted.revision, updated);

    // Restore repo content, then reload through the other administrative entry.
    write_rule(repo.path(), 10);
    let mut request = publisher_request(pb::ReloadRepositoryRequest {});
    request
        .metadata_mut()
        .insert(EXPECTED_REVISION_HEADER, updated.parse().unwrap());
    let response = grpc.reload_repository(request).await.unwrap();
    let (rolled_back, rollback_hash) = assert_transports(&app, &grpc, "approve").await;
    assert_eq!(
        response
            .metadata()
            .get(REVISION_HEADER)
            .unwrap()
            .to_str()
            .unwrap(),
        rolled_back
    );
    assert_eq!(rollback_hash, initial_hash);
    assert_ne!(rolled_back, updated);
}

#[tokio::test]
async fn invalid_pipeline_registry_and_import_preserve_both_transports() {
    let repo = repository();
    let manager = manager(repo.path()).await;
    let app = create_router(manager.clone(), access());
    let grpc = DecisionGrpcService::new(manager.clone(), access());
    let before = assert_transports(&app, &grpc, "approve").await;
    for path in [
        "pipelines/payment.yaml",
        "registry.yaml",
        "rules/amount.yaml",
    ] {
        let path = repo.path().join(path);
        let original = std::fs::read(&path).ok();
        std::fs::write(&path, "invalid: [").unwrap();
        let http = app
            .clone()
            .oneshot(
                Request::post("/v1/repo/reload")
                    .header("authorization", format!("Bearer {PUBLISHER_TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(http.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            grpc.reload_repository(publisher_request(pb::ReloadRepositoryRequest {}))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::Internal
        );
        assert_eq!(assert_transports(&app, &grpc, "approve").await, before);
        assert!(
            engine(repo.path()).await.is_err(),
            "restart must also reject an invalid repo"
        );
        match original {
            Some(bytes) => std::fs::write(path, bytes).unwrap(),
            None => std::fs::remove_file(path).unwrap(),
        }
    }
}

#[tokio::test]
async fn stale_admin_requests_are_rejected_in_both_transports() {
    let repo = repository();
    let manager = manager(repo.path()).await;
    let app = create_router(manager.clone(), access());
    let grpc = DecisionGrpcService::new(manager.clone(), access());
    let old = manager.snapshot().await;
    write_rule(repo.path(), 70);
    let current = manager.reload(Some(&old.revision)).await.unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/repo/reload")
                .header("authorization", format!("Bearer {PUBLISHER_TOKEN}"))
                .header(EXPECTED_REVISION_HEADER, &old.revision)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let mut request = publisher_request(pb::ReloadRepositoryRequest {});
    request
        .metadata_mut()
        .insert(EXPECTED_REVISION_HEADER, old.revision.parse().unwrap());
    assert_eq!(
        grpc.reload_repository(request).await.unwrap_err().code(),
        tonic::Code::Aborted
    );
    assert_eq!(manager.snapshot().await.revision, current.revision);
}

#[tokio::test]
async fn request_waiting_for_connector_does_not_block_reload() {
    let repo = repository();
    let entered = Arc::new(tokio::sync::Semaphore::new(0));
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let connector = Router::new().route(
        "/check",
        axum::routing::get({
            let entered = entered.clone();
            let release = release.clone();
            move || {
                let entered = entered.clone();
                let release = release.clone();
                async move {
                    entered.add_permits(1);
                    release.acquire().await.unwrap().forget();
                    axum::Json(json!({"ok": true}))
                }
            }
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, connector).await.unwrap() });
    std::fs::create_dir_all(repo.path().join("services")).unwrap();
    std::fs::write(
        repo.path().join("services/slow.yaml"),
        format!(
            r#"name: slow
base_url: http://{address}
timeout_ms: 30000
operations:
  check:
    method: GET
    path: /check
"#
        ),
    )
    .unwrap();
    let slow_pipeline = r#"version: "0.1"
pipeline:
  id: payment
  name: Slow payment
  entry: check
  when:
    all: [event.amount > 0]
  steps:
    - step:
        id: check
        name: Slow check
        type: service
        service: slow
        operation: check
        next: end
  decision:
    - default: true
      result: review
"#;
    std::fs::write(repo.path().join("pipelines/payment.yaml"), slow_pipeline).unwrap();
    let manager = manager(repo.path()).await;
    let old_snapshot = manager.snapshot().await;
    let old_revision = old_snapshot.revision.clone();
    let app = create_router(manager.clone(), access());
    let slow_http = tokio::spawn({
        let app = app.clone();
        async move { http_decide(&app).await }
    });
    let slow_grpc = tokio::spawn({
        let grpc = DecisionGrpcService::new(manager.clone(), access());
        async move { grpc_decide(&grpc).await }
    });
    tokio::time::timeout(Duration::from_secs(10), entered.acquire_many(2))
        .await
        .expect("both requests reached connector")
        .unwrap()
        .forget();
    std::fs::write(repo.path().join("pipelines/payment.yaml"), PIPELINE).unwrap();
    write_rule(repo.path(), 70);
    let current = tokio::time::timeout(Duration::from_secs(5), manager.reload(Some(&old_revision)))
        .await
        .expect("reload must not wait for the old decision")
        .unwrap();
    assert_transports(
        &app,
        &DecisionGrpcService::new(manager.clone(), access()),
        "decline",
    )
    .await;
    assert!(!slow_http.is_finished());
    assert!(!slow_grpc.is_finished());
    assert!(Arc::ptr_eq(
        &old_snapshot.engine.metrics(),
        &current.engine.metrics()
    ));
    release.add_permits(2);
    let (old_response_revision, _, old_response) = slow_http.await.unwrap();
    assert_eq!(old_response_revision, old_revision);
    assert_eq!(old_response["decision"]["result"], "review");
    let grpc_response = slow_grpc.await.unwrap();
    assert_eq!(
        grpc_response
            .metadata()
            .get(REVISION_HEADER)
            .unwrap()
            .to_str()
            .unwrap(),
        old_revision
    );
    assert_eq!(
        grpc_response.into_inner().decision.unwrap().result,
        "review"
    );
    assert_ne!(current.revision, old_revision);
    server.abort();
}

const DECISION_TOKEN: &str = "test-decision-token-0000000000000000";
const PUBLISHER_TOKEN: &str = "test-publisher-token-000000000000000";
fn access() -> corint_decision_server::access::AccessPolicy {
    corint_decision_server::access::AccessPolicy::new(DECISION_TOKEN, PUBLISHER_TOKEN, "test")
        .unwrap()
}
fn decision_request<T>(value: T) -> tonic::Request<T> {
    authorized_request(value, DECISION_TOKEN)
}
fn publisher_request<T>(value: T) -> tonic::Request<T> {
    authorized_request(value, PUBLISHER_TOKEN)
}
fn authorized_request<T>(value: T, token: &str) -> tonic::Request<T> {
    let mut r = tonic::Request::new(value);
    r.metadata_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    r
}

#[tokio::test]
async fn both_transports_enforce_roles_and_reject_caller_owned_trusted_namespaces() {
    let repo = repository();
    let manager = manager(repo.path()).await;
    let app = create_router(manager.clone(), access());
    let grpc = DecisionGrpcService::new(manager.clone(), access());
    for (path, token) in [
        ("/v1/decide", None),
        ("/v1/decide", Some(PUBLISHER_TOKEN)),
        ("/v1/repo/reload", Some(DECISION_TOKEN)),
    ] {
        let mut request = Request::post(path).header("content-type", "application/json");
        if let Some(t) = token {
            request = request.header("authorization", format!("Bearer {t}"));
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::from("{}")).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        grpc.decide(tonic::Request::new(pb::DecideRequest::default()))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::Unauthenticated
    );
    assert_eq!(
        grpc.reload_repository(decision_request(pb::ReloadRepositoryRequest {}))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::Unauthenticated
    );
    for body in [
        json!({"event":{"amount":100},"features":{}}),
        json!({"event":{"amount":100,"tenant_id":"other"}}),
        json!({"event":{"amount":100},"options":{"async":true}}),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/decide")
                    .header("authorization", format!("Bearer {DECISION_TOKEN}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let mut req = pb::DecideRequest::default();
    req.metadata.insert("tenant_id".into(), "other".into());
    assert_eq!(
        grpc.decide(decision_request(req)).await.unwrap_err().code(),
        tonic::Code::InvalidArgument
    );
    assert!(manager.snapshot().await.engine.compiled_policy().is_ok());
}

#[tokio::test]
async fn grpc_preserves_complete_trace_and_requested_features() {
    let repo = repository();
    let manager = manager(repo.path()).await;
    let grpc = DecisionGrpcService::new(manager.clone(), access());
    let request =
        corint_decision_engine::DecisionRequest::new(std::collections::HashMap::from([(
            "amount".into(),
            corint_decision_engine::Value::Number(100.0),
        )]))
        .with_vars(std::collections::HashMap::from([(
            "tenant_id".into(),
            corint_decision_engine::Value::String("test".into()),
        )]))
        .with_trace();
    let engine = manager
        .snapshot()
        .await
        .engine
        .decide(request)
        .await
        .unwrap();
    let mut request = pb::DecideRequest::default();
    request.event.insert(
        "amount".into(),
        pb::Value {
            kind: Some(pb::value::Kind::IntValue(100)),
        },
    );
    request.options = Some(pb::RequestOptions {
        include_trace: true,
        include_features: true,
        ..Default::default()
    });
    let result = grpc
        .decide(decision_request(request))
        .await
        .unwrap()
        .into_inner();
    let trace: Value = serde_json::from_str(&result.trace.unwrap().canonical_json).unwrap();
    let mut expected = serde_json::to_value(engine.trace.unwrap()).unwrap();
    // Individual execution timing is intentionally non-deterministic.
    fn remove_times(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("execution_time_ms");
                for v in m.values_mut() {
                    remove_times(v);
                }
            }
            Value::Array(a) => {
                for v in a {
                    remove_times(v);
                }
            }
            _ => (),
        }
    }
    let mut actual = trace;
    remove_times(&mut actual);
    remove_times(&mut expected);
    assert_eq!(actual, expected);
    assert_eq!(result.features.len(), engine.result.context.len());
}

#[tokio::test]
async fn terminal_router_branch_cannot_fall_through_into_the_other_branch() {
    let repo = repository();
    std::fs::write(
        repo.path().join("pipelines/payment.yaml"),
        r#"version: "0.1"
import:
  rulesets: [rulesets/risk.yaml]
---
pipeline:
  id: payment
  name: Payment
  entry: route
  steps:
    - step:
        id: route
        name: Route
        type: router
        routes:
          - when: event.amount > 0
            next: chosen
        default: other
    - step:
        id: chosen
        name: Chosen
        type: ruleset
        ruleset: risk
    - step:
        id: other
        name: Other
        type: ruleset
        ruleset: risk
  decision:
    - default: true
      result: approve
"#,
    )
    .unwrap();
    let engine = engine(repo.path()).await.unwrap();
    let response = engine
        .decide(
            corint_decision_engine::DecisionRequest::new(std::collections::HashMap::from([(
                "amount".into(),
                corint_decision_engine::Value::Number(100.0),
            )]))
            .with_trace(),
        )
        .await
        .unwrap();
    assert_eq!(response.result.score, 10);
    let steps = response.trace.unwrap().pipeline.unwrap().steps;
    assert_eq!(
        steps
            .iter()
            .filter(|s| s.executed)
            .map(|s| s.step_id.as_str())
            .collect::<Vec<_>>(),
        vec!["route", "chosen"]
    );
}

#[tokio::test]
async fn documented_http_example_runs_and_reserved_fields_are_rejected() {
    let documentation = include_str!("../../../docs/API_REQUEST.md");
    let request_json = documentation
        .split("<!-- executable-example: decide-request -->")
        .nth(1)
        .unwrap()
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let example: Value = serde_json::from_str(request_json).unwrap();
    let repo = repository();
    let app = create_router(manager(repo.path()).await, access());
    for (field, expected) in [
        (None, StatusCode::OK),
        (Some("user"), StatusCode::BAD_REQUEST),
        (Some("async"), StatusCode::BAD_REQUEST),
    ] {
        let mut payload = example.clone();
        match field {
            Some("user") => payload["user"] = json!({"risk_level":"low"}),
            Some("async") => payload["options"]["async"] = true.into(),
            _ => {}
        }
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/decide")
                    .header("authorization", format!("Bearer {DECISION_TOKEN}"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        if expected == StatusCode::OK {
            let body: Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(body["decision"]["result"], "approve");
            assert!(body.get("features").is_none());
        }
    }
}
