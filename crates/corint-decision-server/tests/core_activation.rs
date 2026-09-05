//! In-process HTTP evidence, not remote deployment, business evaluation or SSO.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use corint_decision_compiler::core::CoreSource;
use corint_decision_server::core::{self, CoreConfig};
use corint_decision_toolchain::transfer::SourceBundle;
#[path = "../../../tests/support/core_repository.rs"]
mod repository_fixture;
use http_body_util::BodyExt;
use repository_fixture::{identity, publish};
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
    publish(dir.path(), &bundle("initial"), "initial");
    let config = json!({
        "config_version":"2", "listen":"127.0.0.1:0",
        "context":"context.yaml", "target":"target.json", "cases":"cases.yaml",
        "repository":"repository", "decision_token_env":"TEST_DECISION_TOKEN",
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
fn candidate(dir: &Path, revision: &Value, change: &str) -> Value {
    publish(dir, &bundle(change), change);
    json!({"expected_revision":revision["revision"]})
}

#[tokio::test]
async fn roles_authenticate_before_body_and_legacy_routes_are_absent() {
    let (dir, config) = setup(&["initial"]);
    let app = app(dir.path(), &config).await;
    for (path, method, good, wrong) in [
        ("/v1/core/target", "GET", PUBLISHER, DECISION),
        ("/v1/core/repo/reload", "POST", PUBLISHER, DECISION),
        ("/v1/core/decide", "POST", DECISION, PUBLISHER),
    ] {
        for token in [None, Some(wrong), Some("incorrect")] {
            let (status, value) = raw(&app, method, path, token, "not JSON".into()).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(value["error"], "E_CORE_UNAUTHORIZED");
            assert!(!value.to_string().contains(good));
        }
    }
    for path in [
        "/v1/decide",
        "/v1/repo/reload",
        "/v1/core/policies/activate",
    ] {
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
    assert_eq!(state["scope"], "local_repository_reload");
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
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            candidate(dir.path(), &original, change),
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
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        candidate(dir.path(), &original, "second"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(updated["revision"], original["revision"]);
    assert_eq!(updated["policy_sha256"], identity(&bundle("second")));
    let (status, value) = call(
        &app,
        "POST",
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        candidate(dir.path(), &original, "third"),
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
async fn racing_repository_reloads_have_one_winner_and_decisions_keep_one_snapshot() {
    let (dir, config) = setup(&["initial", "second", "third"]);
    let app = app(dir.path(), &config).await;
    let original = current(&app).await;
    let request = candidate(dir.path(), &original, "second");
    let (first, second, decision) = tokio::join!(
        call(
            &app,
            "POST",
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            request.clone()
        ),
        call(
            &app,
            "POST",
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            request
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
        "bundle",
        "repository",
        "path",
    ] {
        let mut value = candidate(dir.path(), &original, "initial");
        value[key] = json!(true);
        let (status, error) =
            call(&app, "POST", "/v1/core/repo/reload", Some(PUBLISHER), value).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["error"], "E_CORE_REQUEST");
    }
    let invalid = candidate(dir.path(), &original, "initial");
    std::fs::write(
        dir.path().join("repository/rule.yaml"),
        "version: '0.1'\nrule: {id: unsupported}\n",
    )
    .unwrap();
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            invalid
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let duplicate = format!(
        "{{\"expected_revision\":\"{}\",\"expected_revision\":\"{}\"}}",
        original["revision"].as_str().unwrap(),
        original["revision"].as_str().unwrap()
    );
    assert_eq!(
        raw(
            &app,
            "POST",
            "/v1/core/repo/reload",
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
            "/v1/core/repo/reload",
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
    publish(dir.path(), &bundle("wrong"), "wrong");
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
        ("config_version", json!("1")),
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
    let mut legacy = config;
    legacy["config_version"] = json!("1");
    legacy["initial_bundle"] = json!("initial.json");
    legacy.as_object_mut().unwrap().remove("repository");
    assert!(serde_json::from_value::<CoreConfig>(legacy).is_err());
}

#[tokio::test]
async fn restart_creates_a_new_revision_and_rejects_old_activation_requests() {
    let (dir, config) = setup(&["initial", "second"]);
    let first = app(dir.path(), &config).await;
    let original = current(&first).await;
    let (status, updated) = call(
        &first,
        "POST",
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        candidate(dir.path(), &original, "second"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    drop(first);
    let restarted = app(dir.path(), &config).await;
    let active = current(&restarted).await;
    assert_ne!(active["revision"], original["revision"]);
    assert_ne!(active["revision"], updated["revision"]);
    assert_eq!(active["policy_sha256"], updated["policy_sha256"]);
    assert_eq!(active["repository"], updated["repository"]);
    assert_eq!(
        call(
            &restarted,
            "POST",
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            candidate(dir.path(), &updated, "second")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn rollback_and_restart_use_the_repository_without_server_writeback() {
    let (dir, config) = setup(&["initial", "second"]);
    let service = app(dir.path(), &config).await;
    let original = current(&service).await;
    let request = candidate(dir.path(), &original, "second");
    let manifest = dir.path().join("repository/published.json");
    let published = std::fs::read(&manifest).unwrap();
    let (status, updated) = call(
        &service,
        "POST",
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        request,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(std::fs::read(&manifest).unwrap(), published);
    assert_eq!(updated["repository"]["revision"], "second");
    let (status, rolled_back) = call(
        &service,
        "POST",
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        candidate(dir.path(), &updated, "initial"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rolled_back["policy_sha256"], original["policy_sha256"]);
    assert_eq!(rolled_back["repository"], original["repository"]);
    assert_ne!(rolled_back["revision"], original["revision"]);
    let restarted = app(dir.path(), &config).await;
    let state = current(&restarted).await;
    assert_eq!(state["repository"], rolled_back["repository"]);
    assert_eq!(state["policy_sha256"], rolled_back["policy_sha256"]);
    let (status, decision) = call(
        &restarted,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"event":{"amount":2500}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(decision["decision"]["result"]["score"], 60);
    assert_eq!(decision["snapshot"], state);
}

#[tokio::test]
async fn unpublished_repository_edits_preserve_live_snapshot_and_fail_restart() {
    let (dir, config) = setup(&["initial", "second"]);
    let service = app(dir.path(), &config).await;
    let original = current(&service).await;
    let path = dir.path().join("repository/rule.yaml");
    let original_source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, original_source.replace("> 1000", "> 1200")).unwrap();
    let (status, error) = call(
        &service,
        "POST",
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        json!({"expected_revision":original["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error"], "E_REPOSITORY_DIGEST");
    assert_eq!(current(&service).await, original);
    let (status, decision) = call(
        &service,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"event":{"amount":1001}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(decision["decision"]["result"]["score"], 60);
    assert_eq!(decision["snapshot"], original);
    let error = core::create_router(
        serde_json::from_value(config).unwrap(),
        dir.path(),
        DECISION,
        PUBLISHER,
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("E_REPOSITORY_DIGEST"));
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        original_source.replace("> 1000", "> 1200")
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
    assert_eq!(capability["scope"], "local_repository_reload");
    assert_eq!(capability["loopback_only"], true);
    assert_eq!(capability["source_of_truth"], "repository");
    assert_eq!(capability["reload_reads_repository"], true);
    assert_eq!(capability["uploads_policy_content"], false);
    assert_eq!(capability["business_evaluation"], "not_performed");
    assert_eq!(
        capability["evidence"],
        "../../../crates/corint-decision-server/tests/core_activation.rs"
    );
}

#[tokio::test]
async fn repository_import_closure_runs_through_server_approval_and_real_engine() {
    let resolved = corint_decision_toolchain::resolve::resolve(
        &root().join("cdl_imports"),
        "input-schema.yaml",
        &["registry.yaml".into()],
    )
    .unwrap();
    let (dir, mut config) = setup(&["initial"]);
    for source in resolved.originals() {
        let path = dir.path().join("repository").join(&source.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &source.yaml).unwrap();
    }
    save(
        &dir.path().join("repository/published.json"),
        &json!({
            "format":"corint-core-repository", "format_version":"1", "revision":"imported",
            "input_schema":"input-schema.yaml", "entries":["registry.yaml"],
            "policy_sha256":resolved.receipt().policy_sha256,
        }),
    );
    config["approvals"][0]["policy_sha256"] = json!(resolved.receipt().policy_sha256);
    assert!(dir.path().join("repository/rules/amount.yaml").exists());
    let app = app(dir.path(), &config).await;
    let (status, result) = call(
        &app,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"event":{"amount":1001}, "enable_trace": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        result["snapshot"]["policy_sha256"],
        resolved.receipt().policy_sha256
    );
    assert_eq!(result["decision"]["result"]["score"], 60);
    let records = result["decision"]["trace"]["core_conditions_v1"]
        .as_array()
        .unwrap();
    assert!(records.iter().any(|r| r["source"] == "rules/amount.yaml"
        && r["field_path"] == "/rule/when"
        && r["node_path"] == ""
        && r["outcome"] == json!({"status":"evaluated","result":true})));
    assert!(records.iter().all(|r| resolved
        .bundle()
        .sources
        .iter()
        .any(|s| r["source"] == s.path)));
}

#[tokio::test]
async fn journal_records_real_decisions_errors_and_recovers_outbox_on_restart() {
    let (dir, mut config) = setup(&["initial"]);
    let consumer = "test-consumer-credential-0000000000000000";
    std::env::set_var("CORE_JOURNAL_TEST_CONSUMER", consumer);
    config["config_version"] = json!("3");
    config["journal"] = json!({"path":"events.sqlite","tenant_id":"test","max_records":10,"max_bytes":1_000_000,"consumer_token_env":"CORE_JOURNAL_TEST_CONSUMER"});
    let router = app(dir.path(), &config).await;
    let (status, value) = call(
        &router,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"business_event_id":"payment-1","event":{"amount":1500}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let record = &value["record"];
    assert_eq!(record["business_event_id"], "payment-1");
    assert_eq!(record["runtime"]["revision"], value["snapshot"]["revision"]);
    assert_eq!(
        record["runtime"]["repository_manifest_sha256"],
        value["snapshot"]["repository"]["manifest_sha256"]
    );
    assert_eq!(
        record["subject"]["policy_sha256"],
        value["snapshot"]["policy_sha256"]
    );
    let (status, error) = call(
        &router,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"business_event_id":"bad-1","event":{"amount":"wrong"}}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
    assert_eq!(error["diagnostic"]["record"]["result"], "error");
    assert_eq!(
        call(
            &router,
            "POST",
            "/v1/core/outbox/claim",
            Some(DECISION),
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(record["result"], "decline");
    let mut outcome: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("contracts/phase0/outcome-event.json")).unwrap(),
    )
    .unwrap();
    outcome["tenant_id"] = record["tenant_id"].clone();
    outcome["decision_id"] = record["decision_id"].clone();
    outcome["business_event_id"] = record["business_event_id"].clone();
    let duplicate = format!("{{\"tenant_id\":\"wrong\",{}", &outcome.to_string()[1..]);
    assert_eq!(
        raw(
            &router,
            "POST",
            "/v1/core/feedback/outcome",
            Some(consumer),
            duplicate
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/v1/core/feedback/outcome",
            Some(consumer),
            outcome.clone()
        )
        .await
        .1["status"],
        "inserted"
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/v1/core/feedback/outcome",
            Some(consumer),
            outcome
        )
        .await
        .1["status"],
        "duplicate"
    );
    let mut action: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("contracts/phase0/action-receipt.json")).unwrap(),
    )
    .unwrap();
    for key in ["tenant_id", "decision_id", "business_event_id"] {
        action[key] = record[key].clone();
    }
    action["action_id"] = record["actions"][0]["action_id"].clone();
    action["idempotency_key"] = record["actions"][0]["idempotency_key"].clone();
    action["executed_at_ms"] = record["decided_at_ms"].clone();
    assert_eq!(
        call(
            &router,
            "POST",
            "/v1/core/feedback/receipt",
            Some(consumer),
            action
        )
        .await
        .1["status"],
        "inserted"
    );
    drop(router);
    let router = app(dir.path(), &config).await;
    let (status, claimed) = call(
        &router,
        "POST",
        "/v1/core/outbox/claim",
        Some(consumer),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(claimed["events"].as_array().unwrap().len(), 4);
    assert_eq!(claimed["events"][0]["event"], *record);
}

#[tokio::test]
async fn configured_business_evidence_is_required_and_revocation_stops_new_requests() {
    use corint_decision_server::journal::contract;
    let (dir, mut config) = setup(&["initial"]);
    let router = app(dir.path(), &config).await;
    let (_, target) = call(
        &router,
        "GET",
        "/v1/core/target",
        Some(PUBLISHER),
        json!({}),
    )
    .await;
    drop(router);
    let load_fixture = |name: &str| -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(root().join(format!("contracts/phase0/{name}.json"))).unwrap(),
        )
        .unwrap()
    };
    // Fixtures exercise operator attestation handling, not actual business data.
    let mut evaluation = load_fixture("evaluation-evidence");
    evaluation["subject"] = target["subject"].clone();
    evaluation["features"] = json!([]);
    evaluation["samples"] = json!([]);
    let mut approval = load_fixture("approval-evidence");
    approval["subject"] = target["subject"].clone();
    approval["evaluation_sha256"] = json!(contract("evaluation-evidence", &evaluation)
        .unwrap()
        .sha256());
    approval["expires_at_ms"] = json!(chrono::Utc::now().timestamp_millis() + 60_000);
    let approval_hash = contract("approval-evidence", &approval).unwrap().sha256();
    let trust = json!({"evaluations":{evaluation["subject"]["policy_sha256"].as_str().unwrap():"fixture-author"},"approvals":{approval_hash.clone():"fixture-reviewer"},"approvers":["fixture-reviewer"]});
    let mut trust = trust;
    trust["evaluations"] =
        json!({approval["evaluation_sha256"].as_str().unwrap():"fixture-author"});
    save(&dir.path().join("evaluation.json"), &evaluation);
    save(&dir.path().join("approval.json"), &approval);
    save(&dir.path().join("trust.json"), &trust);
    config["business_evidence"] =
        json!({"evaluation":"evaluation.json","approval":"approval.json","trust":"trust.json"});
    let router = app(dir.path(), &config).await;
    assert_eq!(
        call(
            &router,
            "POST",
            "/v1/core/decide",
            Some(DECISION),
            json!({"event":{"amount":10}})
        )
        .await
        .0,
        StatusCode::OK
    );
    trust["approvals"] = json!({});
    save(&dir.path().join("trust.json"), &trust);
    assert_eq!(
        call(
            &router,
            "POST",
            "/v1/core/decide",
            Some(DECISION),
            json!({"event":{"amount":10}})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let result = core::create_router(
        serde_json::from_value(config).unwrap(),
        dir.path(),
        DECISION,
        PUBLISHER,
    )
    .await;
    assert!(result.is_err());
}

fn publication_document(dir: &Path) -> String {
    let snapshot = corint_decision_toolchain::repository::load(&dir.join("repository")).unwrap();
    serde_json::to_string(&corint_decision_toolchain::repository::PublishedSources {
        manifest: std::fs::read_to_string(dir.join("repository/published.json")).unwrap(),
        sources: snapshot.closure.originals().to_vec(),
    })
    .unwrap()
}
#[tokio::test]
async fn sqlite_repository_uses_one_atomic_document_and_same_strict_reload_gate() {
    use sqlx::Connection;
    let (dir, mut config) = setup(&["initial", "second"]);
    let db = dir.path().join("policies.sqlite");
    let mut conn = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE corint_core_publication(slot TEXT PRIMARY KEY,document TEXT NOT NULL)",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    sqlx::query("INSERT INTO corint_core_publication VALUES('published',?)")
        .bind(publication_document(dir.path()))
        .execute(&mut conn)
        .await
        .unwrap();
    config.as_object_mut().unwrap().remove("repository");
    config["repository_backend"] = json!({"type":"sqlite","path":"policies.sqlite"});
    let router = app(dir.path(), &config).await;
    let (_, before) = call(
        &router,
        "GET",
        "/v1/core/target",
        Some(PUBLISHER),
        json!({}),
    )
    .await;
    publish(dir.path(), &bundle("second"), "second");
    sqlx::query("UPDATE corint_core_publication SET document=? WHERE slot='published'")
        .bind(publication_document(dir.path()))
        .execute(&mut conn)
        .await
        .unwrap();
    let (status, after) = call(
        &router,
        "POST",
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        json!({"expected_revision":before["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(after["repository"]["revision"], "second");
    let mut broken: Value = serde_json::from_str(&publication_document(dir.path())).unwrap();
    broken["sources"][0]["yaml"] = json!("invalid");
    sqlx::query("UPDATE corint_core_publication SET document=? WHERE slot='published'")
        .bind(broken.to_string())
        .execute(&mut conn)
        .await
        .unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            json!({"expected_revision":after["revision"]})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/v1/core/target",
            Some(PUBLISHER),
            json!({})
        )
        .await
        .1["revision"],
        after["revision"]
    );
}
#[tokio::test]
async fn http_repository_loads_exact_document_and_detects_changed_publication() {
    use corint_decision_server::repo_source::{BackendConfig, Source};
    let (dir, _) = setup(&["initial"]);
    let document = std::sync::Arc::new(tokio::sync::RwLock::new(publication_document(dir.path())));
    let data = document.clone();
    let api = axum::Router::new().route(
        "/published",
        axum::routing::get(move |headers: axum::http::HeaderMap| {
            let data = data.clone();
            async move {
                assert_eq!(
                    headers["authorization"],
                    "Bearer test-repository-token-0000000000000000"
                );
                data.read().await.clone()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    std::env::set_var(
        "CORE_TEST_REPOSITORY_TOKEN",
        "test-repository-token-0000000000000000",
    );
    let source = Source::configure(
        dir.path(),
        Path::new(""),
        Some(BackendConfig::Http {
            url: format!("http://{address}/published"),
            token_env: "CORE_TEST_REPOSITORY_TOKEN".into(),
        }),
    )
    .unwrap();
    let reader = source.clone();
    let snapshot = tokio::task::spawn_blocking(move || reader.load())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.identity.revision, "initial");
    publish(dir.path(), &bundle("second"), "second");
    *document.write().await = publication_document(dir.path());
    assert!(
        tokio::task::spawn_blocking(move || source.verify(&snapshot.identity))
            .await
            .unwrap()
            .is_err()
    );
    server.abort();
}

/// Run against the isolated database created by run_p1_postgres_tests.py.
#[tokio::test]
#[ignore = "requires isolated CORINT_TEST_POSTGRES_URL; run tests/scripts/run_p1_postgres_tests.py"]
async fn postgres_repository_uses_atomic_publication_and_rejects_stale_or_invalid_data() {
    let url = std::env::var("CORINT_TEST_POSTGRES_URL").expect("isolated PostgreSQL URL required");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let (dir, mut config) = setup(&["initial", "second"]);
    sqlx::query(
        "CREATE TABLE corint_core_publication(slot TEXT PRIMARY KEY,document TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO corint_core_publication VALUES('published',$1)")
        .bind(publication_document(dir.path()))
        .execute(&pool)
        .await
        .unwrap();
    config.as_object_mut().unwrap().remove("repository");
    config["repository_backend"] = json!({"type":"postgres","url_env":"CORINT_TEST_POSTGRES_URL"});
    let router = app(dir.path(), &config).await;
    let (_, before) = call(
        &router,
        "GET",
        "/v1/core/target",
        Some(PUBLISHER),
        json!({}),
    )
    .await;
    publish(dir.path(), &bundle("second"), "second");
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE corint_core_publication SET document=$1 WHERE slot='published'")
        .bind(publication_document(dir.path()))
        .execute(&mut *tx)
        .await
        .unwrap();
    // Readers see the committed old document while an unpublished writer holds a transaction.
    let source = corint_decision_server::repo_source::Source::Postgres(url.clone());
    assert_eq!(
        tokio::task::spawn_blocking(move || source.load())
            .await
            .unwrap()
            .unwrap()
            .identity
            .revision,
        "initial"
    );
    tx.commit().await.unwrap();
    let (status, after) = call(
        &router,
        "POST",
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        json!({"expected_revision":before["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(after["repository"]["revision"], "second");
    sqlx::query("UPDATE corint_core_publication SET document='{}' WHERE slot='published'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            json!({"expected_revision":after["revision"]})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/v1/core/target",
            Some(PUBLISHER),
            json!({})
        )
        .await
        .1["revision"],
        after["revision"]
    );
    pool.close().await;
}
