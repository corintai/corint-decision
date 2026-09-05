//! In-process HTTP evidence, not remote deployment, business evaluation or SSO.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use corint_decision_compiler::core::CoreSource;
use corint_decision_server::core::{self, CoreConfig};
use corint_decision_toolchain::{package, transfer::SourceBundle};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tower::ServiceExt;

const DECISION: &str = "test-only-decision-credential-1234567890";
const PUBLISHER: &str = "test-only-publisher-credential-0987654321";
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}
fn source(file: &str) -> CoreSource {
    CoreSource {
        path: file.into(),
        yaml: std::fs::read_to_string(root().join("cdl_core").join(file)).unwrap(),
    }
}
fn bundle(change: &str) -> SourceBundle {
    let mut sources: Vec<_> = [
        "rule.yaml",
        "ruleset.yaml",
        "pipeline.yaml",
        "registry.yaml",
    ]
    .into_iter()
    .map(source)
    .collect();
    match change {
        "wrong" => sources[0].yaml = sources[0].yaml.replace("> 1000", ">= 1000"),
        // Different behavior outside the fixed acceptance cases makes mixed
        // identity/engine snapshots observable. This is synthetic test data.
        "second" => {
            sources[0].yaml = sources[0]
                .yaml
                .replace("> 1000", "> 1000 && event.amount < 2000")
        }
        "third" => {
            sources[0].yaml = sources[0]
                .yaml
                .replace("> 1000", "> 1000 && event.amount < 3000")
        }
        _ => (),
    }
    SourceBundle::new(source("input-schema.yaml"), sources).unwrap()
}
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
fn identity(bundle: &SourceBundle) -> String {
    package::policy_identity(&bundle.sources, &bundle.input_schema).unwrap()
}
fn save(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}
fn setup(approved: &[&str]) -> (TempDir, Value) {
    let dir = tempfile::tempdir().unwrap();
    let context = std::fs::read_to_string(root().join("contracts/business-context.yaml")).unwrap();
    let target =
        std::fs::read_to_string(root().join("contracts/target-capabilities.json")).unwrap();
    let cases = source("behavior.yaml").yaml;
    std::fs::write(dir.path().join("context.yaml"), &context).unwrap();
    std::fs::write(dir.path().join("target.json"), &target).unwrap();
    std::fs::write(dir.path().join("cases.yaml"), &cases).unwrap();
    save(&dir.path().join("initial.json"), &json!(bundle("initial")));
    let config = json!({
        "config_version":"1", "listen":"127.0.0.1:0",
        "context":"context.yaml", "target":"target.json", "cases":"cases.yaml",
        "initial_bundle":"initial.json", "decision_token_env":"TEST_DECISION_TOKEN",
        "publisher_token_env":"TEST_PUBLISHER_TOKEN",
        "approvals":approved.iter().map(|change| json!({
            "policy_sha256":identity(&bundle(change)), "context_sha256":hash(&context),
            "target_sha256":hash(&target), "cases_sha256":hash(&cases),
        })).collect::<Vec<_>>()
    });
    (dir, config)
}
async fn app(dir: &Path, config: &Value) -> Router {
    core::create_router(
        serde_json::from_value(config.clone()).unwrap(),
        dir,
        DECISION,
        PUBLISHER,
    )
    .await
    .unwrap()
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    raw(app, method, path, token, body.to_string()).await
}
async fn raw(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: String,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("Content-Type", "application/json");
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"text":String::from_utf8_lossy(&bytes)}));
    (status, value)
}
async fn current(app: &Router) -> Value {
    let (status, value) = call(app, "GET", "/v1/core/target", Some(PUBLISHER), json!(null)).await;
    assert_eq!(status, StatusCode::OK);
    value
}
fn candidate(revision: &Value, change: &str) -> Value {
    json!({"expected_revision":revision["revision"], "bundle":bundle(change)})
}

#[tokio::test]
async fn roles_authenticate_before_body_and_legacy_routes_are_absent() {
    let (dir, config) = setup(&["initial"]);
    let app = app(dir.path(), &config).await;
    for (path, method, good, wrong) in [
        ("/v1/core/target", "GET", PUBLISHER, DECISION),
        ("/v1/core/policies/activate", "POST", PUBLISHER, DECISION),
        ("/v1/core/decide", "POST", DECISION, PUBLISHER),
    ] {
        for token in [None, Some(wrong), Some("incorrect")] {
            let (status, value) = raw(&app, method, path, token, "not JSON".into()).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(value["error"], "E_CORE_UNAUTHORIZED");
            assert!(!value.to_string().contains(good));
        }
    }
    for path in ["/v1/decide", "/v1/repo/reload"] {
        assert_eq!(
            call(&app, "POST", path, Some(PUBLISHER), json!({})).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let request = Request::builder()
        .uri("/v1/core/target")
        .header("Authorization", format!("Bearer {PUBLISHER}"))
        .header("Authorization", format!("Bearer {PUBLISHER}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn active_identity_is_attached_to_real_decisions_and_input_injection_is_rejected() {
    let (dir, config) = setup(&["initial"]);
    let app = app(dir.path(), &config).await;
    let state = current(&app).await;
    assert_eq!(state["policy_sha256"], identity(&bundle("initial")));
    assert_eq!(state["scope"], "local_operator_activation");
    assert_eq!(state["local_engine_constructed"], true);
    assert_eq!(state["business_evaluation"], "not_performed");
    for (amount, score) in [(1001, 60), (1000, 0), (999, 0)] {
        let (status, result) = call(
            &app,
            "POST",
            "/v1/core/decide",
            Some(DECISION),
            json!({"event":{"amount":amount}, "enable_trace":true}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(result["snapshot"], state);
        assert_eq!(result["decision"]["result"]["score"], score);
        assert!(result["decision"].get("trace").is_some());
    }
    for key in ["features", "api", "vars", "llm", "target", "permissions"] {
        let mut event = json!({"event":{"amount":1001}});
        event[key] = json!({"trusted":true});
        assert_eq!(
            call(&app, "POST", "/v1/core/decide", Some(DECISION), event)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    for event in [json!({}), json!({"amount":"1001"})] {
        let (status, result) = call(
            &app,
            "POST",
            "/v1/core/decide",
            Some(DECISION),
            json!({"event":event}),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(result["error"], "E_CORE_DECISION");
    }
}

#[tokio::test]
async fn unapproved_or_behaviorally_wrong_candidates_leave_active_policy_unchanged() {
    let (dir, config) = setup(&["initial", "wrong"]);
    let app = app(dir.path(), &config).await;
    let original = current(&app).await;
    for (change, code, status) in [
        (
            "second",
            "E_OPERATOR_APPROVAL_REQUIRED",
            StatusCode::FORBIDDEN,
        ),
        (
            "wrong",
            "E_CORE_BEHAVIOR_REJECTED",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let (actual, response) = call(
            &app,
            "POST",
            "/v1/core/policies/activate",
            Some(PUBLISHER),
            candidate(&original, change),
        )
        .await;
        assert_eq!(actual, status);
        assert_eq!(response["error"], code);
        assert!(!response.to_string().contains("above_threshold"));
        assert_eq!(current(&app).await, original);
    }
}

#[tokio::test]
async fn activation_is_compare_and_swap_and_failed_or_stale_requests_do_not_mutate_state() {
    let (dir, config) = setup(&["initial", "second", "third"]);
    let app = app(dir.path(), &config).await;
    let original = current(&app).await;
    let (status, updated) = call(
        &app,
        "POST",
        "/v1/core/policies/activate",
        Some(PUBLISHER),
        candidate(&original, "second"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(updated["revision"], original["revision"]);
    assert_eq!(updated["policy_sha256"], identity(&bundle("second")));
    let (status, value) = call(
        &app,
        "POST",
        "/v1/core/policies/activate",
        Some(PUBLISHER),
        candidate(&original, "third"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(value["error"], "E_ACTIVE_REVISION");
    assert_eq!(current(&app).await, updated);
    let (status, result) = call(
        &app,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"event":{"amount":1001}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["snapshot"], updated);
    let (_, changed) = call(
        &app,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"event":{"amount":2500}}),
    )
    .await;
    assert_eq!(changed["snapshot"], updated);
    assert_eq!(changed["decision"]["result"]["score"], 0);
}

#[tokio::test]
async fn racing_activations_have_one_winner_and_decisions_keep_one_snapshot() {
    let (dir, config) = setup(&["initial", "second", "third"]);
    let app = app(dir.path(), &config).await;
    let original = current(&app).await;
    let (first, second, decision) = tokio::join!(
        call(
            &app,
            "POST",
            "/v1/core/policies/activate",
            Some(PUBLISHER),
            candidate(&original, "second")
        ),
        call(
            &app,
            "POST",
            "/v1/core/policies/activate",
            Some(PUBLISHER),
            candidate(&original, "third")
        ),
        call(
            &app,
            "POST",
            "/v1/core/decide",
            Some(DECISION),
            json!({"event":{"amount":2500}})
        ),
    );
    assert_eq!(
        [first.0, second.0]
            .iter()
            .filter(|&&status| status == StatusCode::OK)
            .count(),
        1
    );
    assert!([first.0, second.0].iter().any(|s| [
        StatusCode::CONFLICT,
        StatusCode::TOO_MANY_REQUESTS
    ]
    .contains(s)));
    let final_state = current(&app).await;
    assert!(decision.1["snapshot"] == original || decision.1["snapshot"] == final_state);
    let expected_score = if decision.1["snapshot"]["policy_sha256"] == identity(&bundle("second")) {
        0
    } else {
        60
    };
    assert_eq!(decision.1["decision"]["result"]["score"], expected_score);
    assert_eq!(
        final_state,
        if first.0 == StatusCode::OK {
            first.1
        } else {
            second.1
        }
    );
}

#[tokio::test]
async fn client_cannot_replace_contracts_cases_or_approvals_and_payloads_are_bounded() {
    let (dir, config) = setup(&["initial"]);
    let app = app(dir.path(), &config).await;
    let original = current(&app).await;
    for key in [
        "context",
        "target",
        "cases",
        "approval",
        "permissions",
        "report",
        "compatible",
    ] {
        let mut value = candidate(&original, "initial");
        value[key] = json!(true);
        let (status, error) = call(
            &app,
            "POST",
            "/v1/core/policies/activate",
            Some(PUBLISHER),
            value,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["error"], "E_CORE_REQUEST");
    }
    let mut invalid = candidate(&original, "initial");
    invalid["bundle"]["sources"][0]["yaml"] = json!("version: '0.1'\nrule: {id: unsupported}\n");
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/core/policies/activate",
            Some(PUBLISHER),
            invalid
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let duplicate = format!(
        "{{\"expected_revision\":\"{}\",\"expected_revision\":\"{}\",\"bundle\":{}}}",
        original["revision"].as_str().unwrap(),
        original["revision"].as_str().unwrap(),
        json!(bundle("initial"))
    );
    assert_eq!(
        raw(
            &app,
            "POST",
            "/v1/core/policies/activate",
            Some(PUBLISHER),
            duplicate
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        raw(
            &app,
            "POST",
            "/v1/core/policies/activate",
            Some(PUBLISHER),
            "x".repeat(8 * 1024 * 1024 + 1)
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(current(&app).await, original);
}

#[tokio::test]
async fn operator_bindings_and_initial_policy_fail_closed_at_startup() {
    for field in [
        "policy_sha256",
        "context_sha256",
        "target_sha256",
        "cases_sha256",
    ] {
        let (dir, mut config) = setup(&["initial"]);
        config["approvals"][0][field] = json!("0".repeat(64));
        let result = core::create_router(
            serde_json::from_value(config).unwrap(),
            dir.path(),
            DECISION,
            PUBLISHER,
        )
        .await;
        assert!(result.is_err(), "{field}");
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("E_OPERATOR_APPROVAL_REQUIRED"));
    }
    for file in ["context.yaml", "target.json", "cases.yaml"] {
        let (dir, config) = setup(&["initial"]);
        let path = dir.path().join(file);
        let mut bytes = std::fs::read_to_string(&path).unwrap();
        bytes.push('\n');
        std::fs::write(path, bytes).unwrap();
        assert!(core::create_router(
            serde_json::from_value(config).unwrap(),
            dir.path(),
            DECISION,
            PUBLISHER
        )
        .await
        .is_err());
    }
    let (dir, config) = setup(&["wrong"]);
    save(&dir.path().join("initial.json"), &json!(bundle("wrong")));
    assert!(core::create_router(
        serde_json::from_value(config).unwrap(),
        dir.path(),
        DECISION,
        PUBLISHER
    )
    .await
    .err()
    .unwrap()
    .to_string()
    .contains("E_CORE_BEHAVIOR_REJECTED"));
}

#[tokio::test]
async fn insecure_configuration_is_rejected_without_exposing_credentials() {
    let (dir, config) = setup(&["initial"]);
    for (key, value) in [
        ("config_version", json!("2")),
        ("listen", json!("0.0.0.0:8080")),
        ("approvals", json!([])),
    ] {
        let mut invalid = config.clone();
        invalid[key] = value;
        assert!(core::create_router(
            serde_json::from_value(invalid).unwrap(),
            dir.path(),
            DECISION,
            PUBLISHER
        )
        .await
        .is_err());
    }
    for (decision, publisher) in [("", PUBLISHER), (DECISION, DECISION), (DECISION, "short")] {
        let error = core::create_router(
            serde_json::from_value(config.clone()).unwrap(),
            dir.path(),
            decision,
            publisher,
        )
        .await
        .err()
        .unwrap();
        assert!(!error.to_string().contains(PUBLISHER));
        assert!(!error.to_string().contains(DECISION));
    }
    let mut invalid = config.clone();
    invalid["unknown"] = json!(true);
    assert!(serde_json::from_value::<CoreConfig>(invalid).is_err());
}

#[tokio::test]
async fn restart_creates_a_new_revision_and_rejects_old_activation_requests() {
    let (dir, config) = setup(&["initial", "second"]);
    let first = app(dir.path(), &config).await;
    let original = current(&first).await;
    let (status, updated) = call(
        &first,
        "POST",
        "/v1/core/policies/activate",
        Some(PUBLISHER),
        candidate(&original, "second"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    drop(first);
    let restarted = app(dir.path(), &config).await;
    let active = current(&restarted).await;
    assert_ne!(active["revision"], original["revision"]);
    assert_ne!(active["revision"], updated["revision"]);
    assert_eq!(active["policy_sha256"], original["policy_sha256"]);
    assert_eq!(
        call(
            &restarted,
            "POST",
            "/v1/core/policies/activate",
            Some(PUBLISHER),
            candidate(&updated, "second")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[test]
fn binary_core_startup_failure_does_not_log_secrets_or_fall_back_to_legacy_loading() {
    let (dir, mut config) = setup(&["initial"]);
    config["approvals"][0]["policy_sha256"] = json!("0".repeat(64));
    let path = dir.path().join("core.json");
    save(&path, &config);
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_corint-decision-server"))
        .current_dir(dir.path())
        .env("CORINT_CORE_CONFIG", &path)
        .env("TEST_DECISION_TOKEN", DECISION)
        .env("TEST_PUBLISHER_TOKEN", PUBLISHER)
        .output()
        .unwrap();
    assert!(!result.status.success());
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.contains("E_OPERATOR_APPROVAL_REQUIRED"), "{output}");
    assert!(!output.contains("Loaded configuration"));
    assert!(!output.contains("listening"));
    assert!(!output.contains(DECISION));
    assert!(!output.contains(PUBLISHER));
}

#[test]
fn published_server_capability_scope_matches_this_gate() {
    let inventory: Value =
        serde_json::from_str(include_str!("../../../docs/cdl/schema/capabilities.json")).unwrap();
    let capability = &inventory["tools"]["core_server"];
    assert_eq!(capability["entry_point"], "CORINT_CORE_CONFIG");
    assert_eq!(capability["scope"], "local_operator_activation");
    assert_eq!(capability["loopback_only"], true);
    assert_eq!(capability["durable_activation"], false);
    assert_eq!(capability["business_evaluation"], "not_performed");
    assert_eq!(
        capability["evidence"],
        "../../../crates/corint-decision-server/tests/core_activation.rs"
    );
}

#[tokio::test]
async fn frozen_import_closure_runs_through_server_approval_and_real_engine() {
    let resolved = corint_decision_toolchain::resolve::resolve(
        &root().join("cdl_imports"),
        "input-schema.yaml",
        &["registry.yaml".into()],
    )
    .unwrap();
    let (dir, mut config) = setup(&["initial"]);
    save(&dir.path().join("initial.json"), &json!(resolved.bundle()));
    config["approvals"][0]["policy_sha256"] = json!(resolved.receipt().policy_sha256);
    assert!(!dir.path().join("rules/amount.yaml").exists());
    let app = app(dir.path(), &config).await;
    let (status, result) = call(
        &app,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"event":{"amount":1001}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        result["snapshot"]["policy_sha256"],
        resolved.receipt().policy_sha256
    );
    assert_eq!(result["decision"]["result"]["score"], 60);
}
