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
  rulesets: [library/rulesets/risk.yaml]
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
  rules: [library/rules/amount.yaml]
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
        root.join("library/rules/amount.yaml"),
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
    for path in ["pipelines", "library/rules", "library/rulesets"] {
        std::fs::create_dir_all(dir.path().join(path)).unwrap();
    }
    std::fs::write(dir.path().join("pipelines/payment.yaml"), PIPELINE).unwrap();
    std::fs::write(dir.path().join("library/rulesets/risk.yaml"), RULESET).unwrap();
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
    grpc.decide(tonic::Request::new(pb::DecideRequest {
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
    let app = create_router(manager.clone());
    let grpc = DecisionGrpcService::new(manager.clone());
    let (initial, initial_hash) = assert_transports(&app, &grpc, "approve").await;
    write_rule(repo.path(), 70);
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/repo/reload")
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
    let mut request = tonic::Request::new(pb::ReloadRepositoryRequest {});
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
    let app = create_router(manager.clone());
    let grpc = DecisionGrpcService::new(manager.clone());
    let before = assert_transports(&app, &grpc, "approve").await;
    for path in [
        "pipelines/payment.yaml",
        "registry.yaml",
        "library/rules/amount.yaml",
    ] {
        let path = repo.path().join(path);
        let original = std::fs::read(&path).ok();
        std::fs::write(&path, "invalid: [").unwrap();
        let http = app
            .clone()
            .oneshot(
                Request::post("/v1/repo/reload")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(http.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            grpc.reload_repository(tonic::Request::new(pb::ReloadRepositoryRequest {}))
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
    let app = create_router(manager.clone());
    let grpc = DecisionGrpcService::new(manager.clone());
    let old = manager.snapshot().await;
    write_rule(repo.path(), 70);
    let current = manager.reload(Some(&old.revision)).await.unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/repo/reload")
                .header(EXPECTED_REVISION_HEADER, &old.revision)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let mut request = tonic::Request::new(pb::ReloadRepositoryRequest {});
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
    std::fs::create_dir_all(repo.path().join("configs/apis")).unwrap();
    std::fs::write(
        repo.path().join("configs/apis/slow.yaml"),
        format!(
            r#"name: slow
base_url: http://{address}
timeout_ms: 30000
endpoints:
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
        type: api
        api: slow
        endpoint: check
        next: end
  decision:
    - default: true
      result: review
"#;
    std::fs::write(repo.path().join("pipelines/payment.yaml"), slow_pipeline).unwrap();
    let manager = manager(repo.path()).await;
    let old_snapshot = manager.snapshot().await;
    let old_revision = old_snapshot.revision.clone();
    let app = create_router(manager.clone());
    let slow_http = tokio::spawn({
        let app = app.clone();
        async move { http_decide(&app).await }
    });
    let slow_grpc = tokio::spawn({
        let grpc = DecisionGrpcService::new(manager.clone());
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
    assert_transports(&app, &DecisionGrpcService::new(manager.clone()), "decline").await;
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
        "REVIEW"
    );
    assert_ne!(current.revision, old_revision);
    server.abort();
}
