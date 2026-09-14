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
        "arithmetic" => {
            sources[0].yaml = sources[0].yaml.replace(
                "event.amount > 1000",
                "event.amount > 1000 && 1 / (event.amount - 2000) != 0",
            )
        }
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
async fn wait_for_writes(app: &Router) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (status, value) = call(
                app,
                "GET",
                "/v1/core/persistence",
                Some(PUBLISHER),
                json!(null),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{value}");
            if value["pending"] == 0 {
                return value;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("background writes finished")
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
    let inventory: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/capabilities.json"
    ))
    .unwrap();
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
    for endpoint in ["outcome", "receipt", "query"] {
        assert_eq!(
            call(
                &router,
                "POST",
                &format!("/v1/core/feedback/{endpoint}"),
                Some(consumer),
                json!({})
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(wait_for_writes(&router).await["failed"], 0);
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
    assert_eq!(claimed["events"].as_array().unwrap().len(), 2);
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

#[tokio::test]
async fn core_runtime_error_details_are_bound_to_durable_records() {
    let (dir, mut config) = setup(&["arithmetic"]);
    let consumer = "test-runtime-error-consumer-000000000000000";
    std::env::set_var("CORE_RUNTIME_ERROR_CONSUMER", consumer);
    config["config_version"] = json!("3");
    config["journal"] = json!({"path":"events.sqlite","tenant_id":"test","max_records":10,"max_bytes":1_000_000,"consumer_token_env":"CORE_RUNTIME_ERROR_CONSUMER"});
    publish(dir.path(), &bundle("arithmetic"), "arithmetic");
    let router = app(dir.path(), &config).await;
    for trace in [false, true] {
        let (status, response) = call(
            &router,
            "POST",
            "/v1/core/decide",
            Some(DECISION),
            json!({"business_event_id":"fault-2000","event":{"amount":2000},"enable_trace":trace}),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response["diagnostic"]["cause"]["code"],
            "E_DIVISION_BY_ZERO"
        );
        assert_eq!(response["diagnostic"]["cause"]["source"], "rule.yaml");
        assert_eq!(response["diagnostic"]["record"]["result"], "error");
        assert_eq!(
            response["diagnostic"]["record"]["error_code"],
            "E_DIVISION_BY_ZERO"
        );
        assert_eq!(
            response["diagnostic"]["record"]["runtime"]["repository_revision"],
            "arithmetic"
        );
        assert_eq!(response["diagnostic"]["record"]["actions"], json!([]));
    }
    assert_eq!(wait_for_writes(&router).await["failed"], 0);
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
    assert_eq!(claimed["events"].as_array().unwrap().len(), 2);
    assert!(claimed["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["event"]["error_code"] == "E_DIVISION_BY_ZERO"));
}

#[tokio::test]
async fn nested_agent_repository_executes_through_core_http() {
    let (dir, mut config) = setup(&[]);
    let fixtures = root().join("core_extensions");
    let read = |name: &str| CoreSource {
        path: name.into(),
        yaml: std::fs::read_to_string(fixtures.join(name)).unwrap(),
    };
    let bundle = SourceBundle::new(
        read("input-schema.yaml"),
        [
            "rule.yaml",
            "marker.yaml",
            "ruleset.yaml",
            "child.yaml",
            "pipeline.yaml",
            "registry.yaml",
        ]
        .into_iter()
        .map(read)
        .collect(),
    )
    .unwrap();
    let context = read("business-context.yaml").yaml;
    let target = read("target-capabilities.json").yaml;
    let cases = read("behavior.yaml").yaml;
    for (name, content) in [
        ("context.yaml", &context),
        ("target.json", &target),
        ("cases.yaml", &cases),
    ] {
        std::fs::write(dir.path().join(name), content).unwrap();
    }
    config["approvals"] = json!([{"policy_sha256":identity(&bundle),"context_sha256":hash(&context),"target_sha256":hash(&target),"cases_sha256":hash(&cases)}]);
    publish(dir.path(), &bundle, "nested-v1");
    let router = app(dir.path(), &config).await;
    for enabled in [true, false] {
        let (status, response) = call(
            &router,
            "POST",
            "/v1/core/decide",
            Some(DECISION),
            json!({"event":{"enabled":enabled,"payment":{"amount":1001}},"enable_trace":true}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(
            response["decision"]["result"]["score"],
            if enabled { 67 } else { 7 }
        );
        assert_eq!(
            response["decision"]["result"]["actions"],
            if enabled {
                json!(["parent_action"])
            } else {
                json!([])
            }
        );
        let child = response["decision"]["trace"]["core_calls_v1"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["resource_id"] == "child")
            .unwrap();
        assert_eq!(child["call_path"], json!(["parent", "child"]));
        assert_eq!(
            child["status"],
            if enabled { "completed" } else { "skipped" }
        );
    }
}

#[tokio::test]
async fn online_journal_requires_no_feedback_or_export_credential() {
    let (dir, mut config) = setup(&["initial"]);
    config["config_version"] = json!("3");
    config["journal"] =
        json!({"path":"online.sqlite","tenant_id":"test","max_records":1,"max_bytes":1_000_000});
    let router = app(dir.path(), &config).await;
    let (status, result) = call(
        &router,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"business_event_id":"payment-only","event":{"amount":1500}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["record"]["business_event_id"], "payment-only");
    assert_eq!(result["persistence"], "queued");
    assert_eq!(wait_for_writes(&router).await["written"], 1);
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new().filename(dir.path().join("online.sqlite")),
    )
    .await
    .unwrap();
    let row: (String, String) = sqlx::query_as("SELECT body,input FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&row.0).unwrap(),
        result["record"]
    );
    assert_eq!(
        serde_json::from_str::<Value>(&row.1).unwrap(),
        json!({"amount":1500.0})
    );
    let (status, failure) = call(
        &router,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"business_event_id":"cannot-store","event":{"amount":1500}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{failure}");
    assert_eq!(failure["persistence"], "queued");
    let writes = wait_for_writes(&router).await;
    assert_eq!(writes["failed"], 1);
    assert_eq!(writes["last_failed_id"], failure["record"]["decision_id"]);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    for path in [
        "/v1/core/outbox/claim",
        "/v1/core/feedback/outcome",
        "/v1/core/feedback/receipt",
        "/v1/core/feedback/query",
    ] {
        assert_eq!(
            call(&router, "POST", path, Some(DECISION), json!({}))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
}

#[tokio::test]
async fn decision_response_does_not_wait_for_database_write_lock() {
    let (dir, mut config) = setup(&["initial"]);
    config["config_version"] = json!("3");
    config["journal"] =
        json!({"path":"locked.sqlite","tenant_id":"test","max_records":10,"max_bytes":1_000_000});
    let router = app(dir.path(), &config).await;
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new().filename(dir.path().join("locked.sqlite")),
    )
    .await
    .unwrap();
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let (status, response) = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        call(
            &router,
            "POST",
            "/v1/core/decide",
            Some(DECISION),
            json!({"business_event_id":"while-locked","event":{"amount":1500}}),
        ),
    )
    .await
    .expect("decision must return while SQLite is locked");
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["persistence"], "queued");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let (status, _) = call(
        &router,
        "GET",
        "/v1/core/persistence",
        Some(DECISION),
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, pending) = call(
        &router,
        "GET",
        "/v1/core/persistence",
        Some(PUBLISHER),
        json!(null),
    )
    .await;
    assert_eq!(pending["pending"], 1);
    tx.commit().await.unwrap();
    let complete = wait_for_writes(&router).await;
    assert_eq!(complete["written"], 1);
    assert_eq!(complete["failed"], 0);
    let stored: String = sqlx::query_scalar("SELECT body FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&stored).unwrap(),
        response["record"]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_drains_queued_decision_before_server_exits() {
    use std::process::{Command, Stdio};
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let (dir, mut config) = setup(&["initial"]);
    config["config_version"] = json!("3");
    config["listen"] = json!("127.0.0.1:0");
    config["journal"] =
        json!({"path":"shutdown.sqlite","tenant_id":"test","max_records":10,"max_bytes":1_000_000});
    let config_path = dir.path().join("core.json");
    std::fs::write(&config_path, config.to_string()).unwrap();
    let log_path = dir.path().join("server.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_corint-decision-server"))
            .env("CORINT_CORE_CONFIG", &config_path)
            .env(config["decision_token_env"].as_str().unwrap(), DECISION)
            .env(config["publisher_token_env"].as_str().unwrap(), PUBLISHER)
            .env("RUST_LOG", "corint_decision_server=info")
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    let address = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "server exited: {}",
                std::fs::read_to_string(&log_path).unwrap()
            );
            let log = std::fs::read_to_string(&log_path).unwrap();
            if let Some(start) = log.find("Experimental strict Core server listening on 127.0.0.1:")
            {
                let tail = &log[start + "Experimental strict Core server listening on ".len()..];
                break tail
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ':')
                    .collect::<String>();
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("server ready");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new().filename(dir.path().join("shutdown.sqlite")),
    )
    .await
    .unwrap();
    let tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let response = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap()
        .post(format!("http://{address}/v1/core/decide"))
        .bearer_auth(DECISION)
        .json(&json!({"business_event_id":"before-stop","event":{"amount":1500}}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let response: Value = response.json().await.unwrap();
    assert_eq!(response["persistence"], "queued");
    assert!(Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .unwrap()
        .success());
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "server must drain the blocked write"
    );
    tx.commit().await.unwrap();
    let status = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("graceful shutdown");
    assert!(
        status.success(),
        "{}",
        std::fs::read_to_string(&log_path).unwrap()
    );
    let record: String = sqlx::query_scalar("SELECT body FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&record).unwrap(),
        response["record"]
    );
}

#[tokio::test]
async fn core_metrics_export_respects_operator_switch_and_reload() {
    for enabled in [true, false] {
        let (dir, mut config) = setup(&["initial", "second"]);
        if !enabled {
            config["enable_metrics"] = false.into();
        }
        let app = app(dir.path(), &config).await;
        for round in 0..2 {
            if round == 1 {
                let state = current(&app).await;
                let (status, _) = call(
                    &app,
                    "POST",
                    "/v1/core/repo/reload",
                    Some(PUBLISHER),
                    candidate(dir.path(), &state, "second"),
                )
                .await;
                assert_eq!(status, StatusCode::OK);
            }
            assert_eq!(
                call(
                    &app,
                    "POST",
                    "/v1/core/decide",
                    Some(DECISION),
                    json!({"event":{"amount":1001}})
                )
                .await
                .0,
                StatusCode::OK
            );
            for token in [None, Some(DECISION), Some(PUBLISHER)] {
                let (status, body) = call(&app, "GET", "/v1/core/metrics", token, json!({})).await;
                if token != Some(PUBLISHER) {
                    assert_eq!(status, StatusCode::UNAUTHORIZED);
                    continue;
                }
                assert_eq!(status, StatusCode::OK);
                assert_eq!(body["revision"], current(&app).await["revision"]);
                assert_eq!(body["metrics"]["enabled"], enabled);
                let histograms = body["metrics"]["histograms"].as_array().unwrap();
                assert_eq!(histograms.is_empty(), !enabled);
                for histogram in histograms {
                    assert!(histogram["count"].as_u64().unwrap() > 0);
                    assert_eq!(histogram["buckets"].as_array().unwrap().len(), 23);
                }
            }
        }
    }
}

// A real SQLite input resource feeding the same strict policies as the other
// server tests. Optional user_id keeps pure Core's existing boundary cases valid.
async fn feature_host_setup() -> (
    TempDir,
    Value,
    corint_decision_engine::decision_host::FeatureHostConfig,
    sqlx::SqlitePool,
    SourceBundle,
) {
    use corint_decision_engine::{
        decision_host::FeatureHostConfig,
        feature_pipeline::{FeatureInput, FeaturePlan},
    };
    use std::collections::BTreeMap;
    let (dir, mut config) = setup(&["initial"]);
    let mut sources = bundle("initial");
    let mut schema: Value = serde_yaml::from_str(&sources.input_schema.yaml).unwrap();
    schema["fields"]["user_id"] = json!({"name":"user_id","field_type":"string","required":false});
    sources = SourceBundle::new(
        CoreSource {
            path: sources.input_schema.path,
            yaml: serde_json::to_string(&schema).unwrap(),
        },
        sources.sources,
    )
    .unwrap();
    publish(dir.path(), &sources, "feature-policy");
    let mut context: Value =
        serde_yaml::from_str(&std::fs::read_to_string(dir.path().join("context.yaml")).unwrap())
            .unwrap();
    context["input_schema"] = schema;
    context["fields"]["user_id"] = json!({"description":"Synthetic account key", "unit":"identifier", "entity":"transaction", "time_basis":"Request time"});
    context["fields"]["amount"]["description"] =
        json!("Synthetic account volume from the bound feature plan");
    let context = serde_json::to_string(&context).unwrap();
    std::fs::write(dir.path().join("context.yaml"), &context).unwrap();
    let mut target: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("target.json")).unwrap())
            .unwrap();
    target["context"]["sha256"] = hash(&context).into();
    let target = serde_json::to_string(&target).unwrap();
    std::fs::write(dir.path().join("target.json"), &target).unwrap();
    config["approvals"][0]["policy_sha256"] = identity(&sources).into();
    config["approvals"][0]["context_sha256"] = hash(&context).into();
    config["approvals"][0]["target_sha256"] = hash(&target).into();
    let db = dir.path().join("features.sqlite");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE events(user_id TEXT, amount REAL, occurred_at TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    for (user, amount, offset) in [
        ("u1", 400, -2),
        ("u1", 700, -2),
        ("u2", 500, -2),
        ("u1", 99999, 3600),
        ("u1", 99999, -3600),
    ] {
        let timestamp = (chrono::Utc::now() + chrono::Duration::seconds(offset))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        sqlx::query("INSERT INTO events VALUES(?,?,?)")
            .bind(user)
            .bind(amount)
            .bind(timestamp)
            .execute(&pool)
            .await
            .unwrap();
    }
    let plan = FeaturePlan { format_version:"1".into(), revision:"volume-v1".into(), datasource_revisions:BTreeMap::from([("events".into(), "db-v1".into())]), timeout_ms:1000,
        outputs:vec![FeatureInput { field:"amount".into(), definition:serde_yaml::from_str("name: volume\ntype: aggregation\nmethod: sum\ndatasource: events\nentity: events\ndimension: user_id\ndimension_value: '${event.user_id}'\nfield: amount\nwindow: 60s\ntimestamp_field: occurred_at\n").unwrap() }] };
    let features: FeatureHostConfig = serde_json::from_value(json!({"plan":plan,"datasources":{"events":{"revision":"db-v1","config":{"name":"events","type":"sql","provider":"sqlite","connection_string":db,"database":"test","pool_size":1,"timeout_ms":1000,"query_cache_ttl_secs":0}}}})).unwrap();
    config["feature_pipeline"] = "features.json".into();
    config["approvals"][0]["feature_binding_sha256"] = features.binding_sha256().into();
    config["config_version"] = "3".into();
    config["journal"] = json!({"path":"journal.sqlite","tenant_id":"test","max_records":1000,"max_bytes":10_000_000});
    save(&dir.path().join("features.json"), &json!(features));
    (dir, config, features, pool, sources)
}
async fn feature_request(router: &Router, event: Value) -> (StatusCode, Value) {
    call(
        router,
        "POST",
        "/v1/core/decide",
        Some(DECISION),
        json!({"business_event_id":"feature-test","event":event,"enable_trace":true}),
    )
    .await
}
async fn stored_feature_input(dir: &Path, id: &str) -> Value {
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new().filename(dir.join("journal.sqlite")),
    )
    .await
    .unwrap();
    let input: String =
        sqlx::query_scalar("SELECT input FROM events WHERE json_extract(body,'$.decision_id')=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    serde_json::from_str(&input).unwrap()
}

#[tokio::test]
async fn feature_host_http_records_actual_inputs_and_replays_without_database() {
    let (dir, config, features, pool, sources) = feature_host_setup().await;
    let router = app(dir.path(), &config).await;
    let before = chrono::Utc::now().timestamp();
    let (status, response) = feature_request(&router, json!({"user_id":"u1"})).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["decision"]["result"]["score"], 60);
    assert_eq!(response["feature_evidence"]["values"]["amount"], 1100.0);
    assert_eq!(response["feature_evidence"]["plan_revision"], "volume-v1");
    assert_eq!(
        response["feature_evidence"]["datasource_revisions"],
        json!({"events":"db-v1"})
    );
    assert!(response["feature_evidence"]["as_of"].as_i64().unwrap() >= before);
    assert!(
        response["feature_evidence"]["as_of"].as_i64().unwrap() <= chrono::Utc::now().timestamp()
    );
    assert_eq!(
        response["snapshot"]["feature_binding_sha256"],
        features.binding_sha256()
    );
    assert_eq!(response["record"]["resources"], json!(features.resources()));
    assert_eq!(response["persistence"], "queued");
    assert_eq!(wait_for_writes(&router).await["failed"], 0);
    let input = stored_feature_input(
        dir.path(),
        response["record"]["decision_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(input["raw_event"], json!({"user_id":"u1"}));
    assert_eq!(input["event"], json!({"user_id":"u1","amount":1100.0}));
    assert_eq!(input["feature_evidence"], response["feature_evidence"]);
    let mut canonical = input.clone();
    canonical.sort_all_objects();
    assert_eq!(
        response["record"]["input_evidence"]["sha256"],
        hash(&canonical.to_string())
    );
    sqlx::query("DROP TABLE events")
        .execute(&pool)
        .await
        .unwrap();
    let engine = corint_decision_engine::DecisionEngine::from_core(
        &sources.sources,
        corint_decision_compiler::core::parse_core_input_schema(&sources.input_schema).unwrap(),
    )
    .unwrap();
    let replay = engine
        .decide(corint_decision_engine::DecisionRequest::new(
            serde_json::from_value(input["event"].clone()).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(replay.result.score, 60);
    assert_eq!(replay.result.actions, vec!["BLOCK"]);
}

#[tokio::test]
async fn feature_host_rejects_injection_and_records_source_failure_without_core_execution() {
    let (dir, config, _, pool, _) = feature_host_setup().await;
    let router = app(dir.path(), &config).await;
    sqlx::query("DROP TABLE events")
        .execute(&pool)
        .await
        .unwrap();
    for (event, code) in [
        (json!({"user_id":"u1","amount":0.0}), "E_INPUT_SCHEMA"),
        (json!({"user_id":"u1"}), "E_FEATURE_EXECUTION"),
    ] {
        let (status, response) = feature_request(&router, event.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{response}");
        assert_eq!(response["diagnostic"]["record"]["result"], "error");
        assert_eq!(response["diagnostic"]["record"]["error_code"], code);
        assert_eq!(response["diagnostic"]["record"]["resources"], json!([]));
        assert_eq!(wait_for_writes(&router).await["failed"], 0);
        let input = stored_feature_input(
            dir.path(),
            response["diagnostic"]["record"]["decision_id"]
                .as_str()
                .unwrap(),
        )
        .await;
        assert_eq!(input["raw_event"], event);
        assert!(input["event"].is_null());
    }
    let (_, metrics) = call(
        &router,
        "GET",
        "/v1/core/metrics",
        Some(PUBLISHER),
        json!({}),
    )
    .await;
    assert_eq!(metrics["metrics"]["histograms"], json!([]));
    assert_eq!(
        call(
            &router,
            "POST",
            "/v1/core/decide",
            Some(DECISION),
            json!({"business_event_id":"injected-time","event":{"user_id":"u1"},"as_of":0})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn feature_host_resource_change_requires_approval_and_preserves_live_snapshot() {
    let (dir, config, mut features, _pool, _) = feature_host_setup().await;
    let router = app(dir.path(), &config).await;
    let original = current(&router).await;
    features.plan.revision = "unapproved-v2".into();
    save(&dir.path().join("features.json"), &json!(features));
    let (status, response) = call(
        &router,
        "POST",
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        json!({"expected_revision":original["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(response["error"], "E_OPERATOR_APPROVAL_REQUIRED");
    assert_eq!(current(&router).await, original);
    let (status, response) = feature_request(&router, json!({"user_id":"u1"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["feature_evidence"]["plan_revision"], "volume-v1");
    assert!(core::create_router(
        serde_json::from_value(config).unwrap(),
        dir.path(),
        DECISION,
        PUBLISHER
    )
    .await
    .is_err());
}

#[tokio::test]
async fn feature_host_timeout_is_recorded_and_does_not_run_core() {
    let (dir, mut config, mut features, pool, _) = feature_host_setup().await;
    features.plan.timeout_ms = 10;
    config["approvals"][0]["feature_binding_sha256"] = features.binding_sha256().into();
    save(&dir.path().join("features.json"), &json!(features));
    let router = app(dir.path(), &config).await;
    let mut writer = pool.acquire().await.unwrap();
    sqlx::query("BEGIN EXCLUSIVE")
        .execute(&mut *writer)
        .await
        .unwrap();
    let (status, response) = feature_request(&router, json!({"user_id":"u1"})).await;
    sqlx::query("ROLLBACK").execute(&mut *writer).await.unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{response}");
    assert_eq!(
        response["diagnostic"]["record"]["error_code"],
        "E_FEATURE_TIMEOUT"
    );
    assert_eq!(wait_for_writes(&router).await["failed"], 0);
    let (_, metrics) = call(
        &router,
        "GET",
        "/v1/core/metrics",
        Some(PUBLISHER),
        json!({}),
    )
    .await;
    assert_eq!(metrics["metrics"]["histograms"], json!([]));
}

#[tokio::test]
async fn feature_host_inflight_request_keeps_resource_snapshot_during_reload() {
    use corint_decision_engine::DataSourceType;
    let (dir, mut config, mut features, pool, _) = feature_host_setup().await;
    features.plan.timeout_ms = 10_000;
    features
        .datasources
        .get_mut("events")
        .unwrap()
        .config
        .timeout_ms = 10_000;
    config["approvals"][0]["feature_binding_sha256"] = features.binding_sha256().into();
    save(&dir.path().join("features.json"), &json!(features));
    let mut next = features.clone();
    let second_path = dir.path().join("next.sqlite");
    let second = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&second_path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE events(user_id TEXT, amount REAL, occurred_at TEXT)")
        .execute(&second)
        .await
        .unwrap();
    sqlx::query("INSERT INTO events VALUES('u1',500,?)")
        .bind(
            (chrono::Utc::now() - chrono::Duration::seconds(2))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
        .execute(&second)
        .await
        .unwrap();
    if let DataSourceType::SQL(sql) = &mut next
        .datasources
        .get_mut("events")
        .unwrap()
        .config
        .source_type
    {
        sql.connection_string = second_path.to_string_lossy().into();
    }
    // Even changing only the connection target (keeping declared revisions)
    // produces a different required approval binding.
    assert_ne!(next.binding_sha256(), features.binding_sha256());
    let mut approval = config["approvals"][0].clone();
    approval["feature_binding_sha256"] = next.binding_sha256().into();
    config["approvals"].as_array_mut().unwrap().push(approval);
    let router = app(dir.path(), &config).await;
    let original = current(&router).await;
    let mut writer = pool.acquire().await.unwrap();
    sqlx::query("BEGIN EXCLUSIVE")
        .execute(&mut *writer)
        .await
        .unwrap();
    let pending = feature_request(&router, json!({"user_id":"u1"}));
    tokio::pin!(pending);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut pending)
            .await
            .is_err()
    );
    save(&dir.path().join("features.json"), &json!(next));
    let (status, updated) = call(
        &router,
        "POST",
        "/v1/core/repo/reload",
        Some(PUBLISHER),
        json!({"expected_revision":original["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    sqlx::query("ROLLBACK").execute(&mut *writer).await.unwrap();
    let (status, previous) = pending.await;
    assert_eq!(status, StatusCode::OK, "{previous}");
    assert_eq!(previous["snapshot"], original);
    assert_eq!(previous["feature_evidence"]["values"]["amount"], 1100.0);
    let (status, latest) = feature_request(&router, json!({"user_id":"u1"})).await;
    assert_eq!(status, StatusCode::OK, "{latest}");
    assert_eq!(latest["snapshot"], updated);
    assert_eq!(latest["feature_evidence"]["values"]["amount"], 500.0);
    assert_ne!(
        original["subject"]["bindings_sha256"],
        updated["subject"]["bindings_sha256"]
    );
    assert_eq!(wait_for_writes(&router).await["failed"], 0);
}

#[tokio::test]
async fn feature_host_retains_enrichment_when_core_fails() {
    let (dir, mut config, features, pool, sources) = feature_host_setup().await;
    let broken = SourceBundle::new(sources.input_schema, bundle("arithmetic").sources).unwrap();
    publish(dir.path(), &broken, "arithmetic");
    config["approvals"][0]["policy_sha256"] = identity(&broken).into();
    sqlx::query("UPDATE events SET amount=1000 WHERE user_id='u1' AND amount IN (400,700)")
        .execute(&pool)
        .await
        .unwrap();
    let router = app(dir.path(), &config).await;
    let (status, response) = feature_request(&router, json!({"user_id":"u1"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{response}");
    let diagnostic = &response["diagnostic"];
    assert_eq!(diagnostic["record"]["error_code"], "E_DIVISION_BY_ZERO");
    assert_eq!(
        diagnostic["record"]["resources"],
        json!(features.resources())
    );
    assert_eq!(diagnostic["feature_evidence"]["values"]["amount"], 2000.0);
    assert_eq!(wait_for_writes(&router).await["failed"], 0);
    let input = stored_feature_input(
        dir.path(),
        diagnostic["record"]["decision_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(input["event"]["amount"], 2000.0);
}

#[tokio::test]
async fn feature_host_business_evidence_must_cover_exact_features_and_binding() {
    use corint_decision_server::journal::contract;
    let (dir, mut config, features, _pool, _) = feature_host_setup().await;
    let router = app(dir.path(), &config).await;
    let target = current(&router).await;
    drop(router);
    let fixture = |name: &str| -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(root().join(format!("contracts/phase0/{name}.json"))).unwrap(),
        )
        .unwrap()
    };
    let attest = |resources: Vec<Value>| {
        // Synthetic fixture attestations exercise the gate, not real business evaluation.
        let mut evaluation = fixture("evaluation-evidence");
        let sample = evaluation["samples"][0].clone();
        evaluation["subject"] = target["subject"].clone();
        evaluation["features"] = json!(resources);
        evaluation["samples"] = json!(resources
            .iter()
            .map(|resource| {
                let mut sample = sample.clone();
                sample["feature"] = resource.clone();
                sample["offline_definition_sha256"] = resource["sha256"].clone();
                sample["online_definition_sha256"] = resource["sha256"].clone();
                sample
            })
            .collect::<Vec<_>>());
        let mut approval = fixture("approval-evidence");
        approval["subject"] = target["subject"].clone();
        approval["evaluation_sha256"] = contract("evaluation-evidence", &evaluation)
            .unwrap()
            .sha256()
            .into();
        approval["expires_at_ms"] = json!(chrono::Utc::now().timestamp_millis() + 60_000);
        let approval_hash = contract("approval-evidence", &approval).unwrap().sha256();
        save(&dir.path().join("evaluation.json"), &evaluation);
        save(&dir.path().join("approval.json"), &approval);
        save(
            &dir.path().join("trust.json"),
            &json!({"evaluations":{approval["evaluation_sha256"].as_str().unwrap():"fixture-author"},"approvals":{approval_hash:"fixture-reviewer"},"approvers":["fixture-reviewer"]}),
        );
    };
    config["business_evidence"] =
        json!({"evaluation":"evaluation.json","approval":"approval.json","trust":"trust.json"});
    attest(vec![]);
    assert!(core::create_router(
        serde_json::from_value(config.clone()).unwrap(),
        dir.path(),
        DECISION,
        PUBLISHER
    )
    .await
    .is_err());
    attest(features.resources());
    let router = app(dir.path(), &config).await;
    assert_eq!(
        feature_request(&router, json!({"user_id":"u1"})).await.0,
        StatusCode::OK
    );
    let mut changed = features;
    changed.plan.revision = "new-feature-revision".into();
    save(&dir.path().join("features.json"), &json!(changed));
    config["approvals"][0]["feature_binding_sha256"] = changed.binding_sha256().into();
    // A new local allowlist entry cannot reuse old business evidence.
    assert!(core::create_router(
        serde_json::from_value(config).unwrap(),
        dir.path(),
        DECISION,
        PUBLISHER
    )
    .await
    .is_err());
    // Revoked/missing coverage is also checked for already-running requests.
    attest(vec![]);
    assert_eq!(
        feature_request(&router, json!({"user_id":"u1"})).await.0,
        StatusCode::FORBIDDEN
    );
}
