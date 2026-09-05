//! Real CLI + real Core server process + TCP, with a fixed (not live) model.
//! Run via tests/scripts/run_core_e2e_tests.sh; no database or ambient credentials.
use corint_decision_compiler::core::{CoreSource, PROFILE};
#[path = "../../../tests/support/core_repository.rs"]
mod repository_fixture;
use corint_decision_llm::{CoreGenerator, MockProvider, RuleGeneratorConfig};
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
use tempfile::TempDir;

const FILES: [&str; 4] = [
    "rule.yaml",
    "ruleset.yaml",
    "pipeline.yaml",
    "registry.yaml",
];
// Source packages intentionally canonicalize labels by resource kind and ID.
const EXPORTED_RULE: &str = "rule/large_amount.yaml";
const DECISION: &str = "process-e2e-only-decision-token-1234567890";
const PUBLISHER: &str = "process-e2e-only-publisher-token-0987654321";
const PROCESS_TIMEOUT: Duration = Duration::from_secs(45);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}
fn fixture(name: &str) -> String {
    fs::read_to_string(root().join(name)).unwrap()
}
fn save(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// RAII also covers assertion panics and startup/CLI timeouts. Never kill by name.
struct Process {
    child: Child,
    stdout: PathBuf,
    stderr: PathBuf,
}
impl Process {
    fn spawn(mut command: Command, dir: &Path, label: &str) -> Self {
        let stdout = dir.join(format!("{label}.stdout.log"));
        let stderr = dir.join(format!("{label}.stderr.log"));
        let child = command
            .stdin(Stdio::null())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        Self {
            child,
            stdout,
            stderr,
        }
    }
    fn logs(&self) -> String {
        format!(
            "stdout:\n{}\nstderr:\n{}",
            fs::read_to_string(&self.stdout).unwrap_or_default(),
            fs::read_to_string(&self.stderr).unwrap_or_default()
        )
        .replace(DECISION, "<decision-token>")
        .replace(PUBLISHER, "<publisher-token>")
    }
    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + PROCESS_TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "Child process timed out: {}",
                self.logs()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn stop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.stop();
        if std::thread::panicking() {
            eprintln!("Child process diagnostics (sanitized): {}", self.logs());
        }
    }
}

fn cli(dir: &Path, label: &str, args: &[&str], expected: i32) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_corint"));
    command
        .env_clear()
        .current_dir(dir)
        .args(args)
        .args(["--format", "json"]);
    let mut process = Process::spawn(command, dir, label);
    assert_eq!(process.wait().code(), Some(expected), "{}", process.logs());
    assert!(
        fs::read(&process.stderr).unwrap().is_empty(),
        "{}",
        process.logs()
    );
    let value = read(&process.stdout);
    assert_eq!(value["valid"], expected == 0);
    assert_eq!(value["business_evaluation"], "not_performed");
    value
}

struct Delivery {
    bundle: Value,
    policy: Value,
}
/// Provider output is untrusted synthetic data; use the real generator gate,
/// then write accepted sources for the real CLI. No generator report is trusted by HTTP.
async fn prepare(dir: &Path, condition: &str, weak_cases: bool) -> Delivery {
    fs::create_dir_all(dir).unwrap();
    let input = CoreSource {
        path: "input-schema.yaml".into(),
        yaml: fixture("cdl_core/input-schema.yaml"),
    };
    let mut cases: Value = serde_yaml::from_str(&fixture("cdl_core/behavior.yaml")).unwrap();
    if weak_cases {
        cases["cases"]
            .as_array_mut()
            .unwrap()
            .retain(|case| case["id"] != "equal_threshold");
    }
    let suite = CoreSource {
        path: "author-cases.yaml".into(),
        yaml: serde_yaml::to_string(&cases).unwrap(),
    };
    let sources: Vec<_> = FILES
        .iter()
        .map(|name| {
            let mut yaml = fixture(&format!("cdl_core/{name}"));
            if *name == "rule.yaml" {
                yaml = yaml.replace("event.amount > 1000", condition);
            }
            CoreSource {
                path: (*name).into(),
                yaml,
            }
        })
        .collect();
    let generator = CoreGenerator::new(
        Arc::new(MockProvider::with_response(
            json!({"profile":PROFILE,"sources":sources}).to_string(),
        )),
        RuleGeneratorConfig::new("fixed-process-e2e"),
    );
    let generated = generator
        .generate("Synthetic payment risk strategy", &input, &suite)
        .await
        .unwrap();
    assert!(generated.accepted());
    assert_eq!(generated.tests.passed, if weak_cases { 4 } else { 5 });
    assert!(generated.tests.cases.iter().all(|case| case.trace_parity));
    for source in generated.sources {
        assert!(FILES.contains(&source.path.as_str()));
        fs::write(dir.join(source.path), source.yaml).unwrap();
    }
    fs::write(dir.join(&input.path), input.yaml).unwrap();
    fs::write(dir.join(&suite.path), suite.yaml).unwrap();
    cli(
        dir,
        "validate",
        &[
            "validate",
            "--input-schema",
            "input-schema.yaml",
            FILES[0],
            FILES[1],
            FILES[2],
            FILES[3],
        ],
        0,
    );
    let built = cli(
        dir,
        "build",
        &[
            "build",
            "--input-schema",
            "input-schema.yaml",
            "--cases",
            "author-cases.yaml",
            "--output",
            "package.json",
            FILES[0],
            FILES[1],
            FILES[2],
            FILES[3],
        ],
        0,
    );
    assert_eq!(
        built["test_results"]["passed"],
        if weak_cases { 4 } else { 5 }
    );
    cli(
        dir,
        "verify",
        &[
            "verify",
            "--package",
            "package.json",
            "--cases",
            "author-cases.yaml",
        ],
        0,
    );
    cli(
        dir,
        "export",
        &[
            "export",
            "--package",
            "package.json",
            "--output",
            "bundle.json",
        ],
        0,
    );
    let package = read(&dir.join("package.json"));
    assert_eq!(package["evidence"]["publication_approval"], "not_granted");
    let bundle = read(&dir.join("bundle.json"));
    assert_eq!(bundle["sources"].as_array().unwrap().len(), FILES.len());
    assert_eq!(
        bundle["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|source| source["path"] == EXPORTED_RULE)
            .count(),
        1
    );
    let typed = serde_json::from_value(bundle.clone()).unwrap();
    let policy = json!(repository_fixture::identity(&typed));
    Delivery { bundle, policy }
}

fn publish(dir: &Path, bundle: &Value, revision: &str) {
    repository_fixture::publish(
        dir,
        &serde_json::from_value(bundle.clone()).unwrap(),
        revision,
    );
}

fn operator_config(dir: &Path, initial: &Delivery, approved: &[&Delivery]) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    for (name, source) in [
        ("context.yaml", "contracts/business-context.yaml"),
        ("target.json", "contracts/target-capabilities.json"),
        ("cases.yaml", "cdl_core/behavior.yaml"),
    ] {
        fs::write(dir.join(name), fixture(source)).unwrap();
    }
    publish(dir, &initial.bundle, "initial");
    let config = json!({"config_version":"2", "listen":"127.0.0.1:0",
        "context":"context.yaml", "target":"target.json", "cases":"cases.yaml", "repository":"repository",
        "decision_token_env":"E2E_DECISION_TOKEN", "publisher_token_env":"E2E_PUBLISHER_TOKEN",
        "approvals":approved.iter().map(|delivery| json!({"policy_sha256":delivery.policy,
            "context_sha256":hash(&fs::read(dir.join("context.yaml")).unwrap()),
            "target_sha256":hash(&fs::read(dir.join("target.json")).unwrap()),
            "cases_sha256":hash(&fs::read(dir.join("cases.yaml")).unwrap()),
        })).collect::<Vec<_>>()});
    let path = dir.join("core.json");
    save(&path, &config);
    path
}

fn server_command(config: &Path) -> Command {
    let path = std::env::var_os("CORINT_E2E_SERVER")
        .expect("Use bash tests/scripts/run_core_e2e_tests.sh (server binary is required)");
    let path = fs::canonicalize(path).unwrap();
    assert!(path.is_file());
    let mut command = Command::new(path);
    command
        .env_clear()
        .current_dir(config.parent().unwrap())
        .env("CORINT_CORE_CONFIG", config)
        .env("E2E_DECISION_TOKEN", DECISION)
        .env("E2E_PUBLISHER_TOKEN", PUBLISHER)
        .env("RUST_LOG", "corint_decision_server=info")
        .env("NO_COLOR", "1");
    command
}

struct Server {
    process: Process,
    url: String,
    client: Client,
}
impl Server {
    async fn start(config: &Path, label: &str) -> Self {
        let mut process = Process::spawn(server_command(config), config.parent().unwrap(), label);
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
        let deadline = Instant::now() + PROCESS_TIMEOUT;
        loop {
            assert!(
                process.child.try_wait().unwrap().is_none(),
                "Server exited before ready: {}",
                process.logs()
            );
            // Port 0 is bound by the server itself: no reserve/release port race.
            // Read only this child's log; authenticated HTTP confirms readiness/identity.
            let log = fs::read_to_string(&process.stdout).unwrap();
            if let Some(address) = log.lines().find_map(|line| {
                line.split_once("Experimental strict Core server listening on ")
                    .map(|(_, rest)| rest.trim())
            }) {
                let address: SocketAddr = address.parse().expect("server's bound socket address");
                assert!(address.ip().is_loopback() && address.port() != 0);
                let url = format!("http://{address}");
                if let Ok(response) = client
                    .get(format!("{url}/v1/core/target"))
                    .bearer_auth(PUBLISHER)
                    .send()
                    .await
                {
                    if response.status() == StatusCode::OK {
                        return Self {
                            process,
                            url,
                            client,
                        };
                    }
                }
            }
            assert!(
                Instant::now() < deadline,
                "Server readiness timed out: {}",
                process.logs()
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    async fn call(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: &Value,
    ) -> (u16, Value) {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.url))
            .json(body);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        let body = response.bytes().await.unwrap();
        (
            status,
            serde_json::from_slice(&body)
                .unwrap_or_else(|_| json!({"text":String::from_utf8_lossy(&body)})),
        )
    }
    async fn state(&self) -> Value {
        let (status, result) = self
            .call(
                Method::GET,
                "/v1/core/target",
                Some(PUBLISHER),
                &json!(null),
            )
            .await;
        assert_eq!(status, 200);
        result
    }
    async fn assert_decision(&self, snapshot: &Value, amount: i32, decline: bool) {
        let mut baseline = None;
        for trace in [false, true] {
            let (status, body) = self
                .call(
                    Method::POST,
                    "/v1/core/decide",
                    Some(DECISION),
                    &json!({"event":{"amount":amount},"enable_trace":trace}),
                )
                .await;
            assert_eq!(status, 200, "{body}");
            assert_eq!(&body["snapshot"], snapshot);
            let decision = &body["decision"];
            let result = &decision["result"];
            assert_eq!(decision["pipeline_id"], "payment");
            assert_eq!(result["score"], if decline { 60 } else { 0 });
            assert_eq!(
                result["signal"]["type"],
                if decline { "decline" } else { "approve" }
            );
            assert_eq!(
                result["actions"],
                if decline { json!(["BLOCK"]) } else { json!([]) }
            );
            assert_eq!(
                result["triggered_rules"],
                if decline {
                    json!(["large_amount"])
                } else {
                    json!([])
                }
            );
            assert_eq!(
                result["explanation"],
                if decline {
                    "Large amount"
                } else {
                    "Below threshold"
                }
            );
            let ctx = &result["context"];
            assert_eq!(
                ctx["__core_rule_executions__"],
                json!([{"rule_id":"large_amount","ruleset_id":"risk","triggered":decline,"score":if decline {60.0} else {0.0}}])
            );
            let steps = ctx["__executed_steps__"].as_array().unwrap();
            assert_eq!(steps.len(), 1);
            assert_eq!(
                serde_json::from_str::<Value>(steps[0].as_str().unwrap()).unwrap()["step_id"],
                "check"
            );
            if let Some(baseline) = &baseline {
                assert_eq!(result, baseline);
            } else {
                baseline = Some(result.clone());
            }
            if trace {
                let records = decision["trace"]["core_conditions_v1"].as_array().unwrap();
                assert!(
                    records.iter().any(|r| r["source"] == EXPORTED_RULE
                        && r["field_path"] == "/rule/when"
                        && r["node_path"] == ""
                        && r["outcome"] == json!({"status":"evaluated","result":decline})),
                    "Unexpected condition trace: {records:?}"
                );
            } else {
                assert!(decision["trace"].is_null());
            }
        }
    }
}

#[test]
fn generated_policy_crosses_cli_and_real_http_with_fail_closed_activation() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(check_delivery_scenario());
}

async fn check_delivery_scenario() {
    let temp = TempDir::new().unwrap();
    let initial = prepare(&temp.path().join("initial"), "event.amount > 1000", false).await;
    let next = prepare(
        &temp.path().join("candidate"),
        "event.amount > 1000 && event.amount < 2000",
        false,
    )
    .await;
    let wrong = prepare(
        &temp.path().join("weak-author-tests"),
        "event.amount >= 1000",
        true,
    )
    .await;
    let config = operator_config(
        &temp.path().join("operator"),
        &initial,
        &[&initial, &next, &wrong],
    );
    let mut server = Server::start(&config, "server-first").await;
    let original = server.state().await;
    assert_eq!(original["policy_sha256"], initial.policy);
    assert_eq!(original["server_owned_cases_passed"], true);
    assert_eq!(original["business_evaluation"], "not_performed");
    for (amount, decline) in [(999, false), (1000, false), (1001, true), (2500, true)] {
        server.assert_decision(&original, amount, decline).await;
    }
    publish(config.parent().unwrap(), &next.bundle, "next");
    let activation = json!({"expected_revision":original["revision"]});
    for token in [None, Some(DECISION), Some("wrong-token")] {
        let (status, body) = server
            .call(Method::POST, "/v1/core/repo/reload", token, &activation)
            .await;
        assert_eq!(status, 401);
        assert_eq!(body["error"], "E_CORE_UNAUTHORIZED");
        assert_eq!(server.state().await, original);
        server.assert_decision(&original, 2500, true).await;
    }
    let (status, updated) = server
        .call(
            Method::POST,
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            &activation,
        )
        .await;
    assert_eq!(status, 200, "{updated}");
    assert_ne!(updated["revision"], original["revision"]);
    assert_eq!(updated["policy_sha256"], next.policy);
    server.assert_decision(&updated, 2500, false).await;
    server.assert_decision(&updated, 1001, true).await;

    let mut unapproved = next.bundle.clone();
    for source in unapproved["sources"].as_array_mut().unwrap() {
        if source["path"] == EXPORTED_RULE {
            source["yaml"] = source["yaml"]
                .as_str()
                .unwrap()
                .replace("< 2000", "< 3000")
                .into();
        }
    }
    assert_ne!(unapproved, next.bundle);
    for (contents, expected_status, code, revision) in [
        (&wrong.bundle, 422, "E_CORE_BEHAVIOR_REJECTED", "wrong"),
        (
            &unapproved,
            403,
            "E_OPERATOR_APPROVAL_REQUIRED",
            "unapproved",
        ),
    ] {
        publish(config.parent().unwrap(), contents, revision);
        let (status, body) = server
            .call(
                Method::POST,
                "/v1/core/repo/reload",
                Some(PUBLISHER),
                &json!({"expected_revision":updated["revision"]}),
            )
            .await;
        assert_eq!(status, expected_status, "{body}");
        assert_eq!(body["error"], code);
        assert_eq!(server.state().await, updated);
        server.assert_decision(&updated, 2500, false).await;
    }
    // An invalid externally published repo keeps the old process alive, but
    // cannot be accepted on restart. Restore the validated repo before restart.
    publish(config.parent().unwrap(), &next.bundle, "next");
    for (candidate, expected_status, code) in [
        (activation, 409, "E_ACTIVE_REVISION"),
        (
            json!({"expected_revision":updated["revision"],"bundle":initial.bundle}),
            400,
            "E_CORE_REQUEST",
        ),
        (
            json!({"expected_revision":updated["revision"],"approvals":[]}),
            400,
            "E_CORE_REQUEST",
        ),
    ] {
        let (status, body) = server
            .call(
                Method::POST,
                "/v1/core/repo/reload",
                Some(PUBLISHER),
                &candidate,
            )
            .await;
        assert_eq!(status, expected_status, "{body}");
        assert_eq!(body["error"], code);
        assert_eq!(server.state().await, updated);
        server.assert_decision(&updated, 2500, false).await;
    }
    assert_eq!(
        server
            .call(
                Method::POST,
                "/v1/core/policies/activate",
                Some(PUBLISHER),
                &json!({"expected_revision":updated["revision"],"bundle":initial.bundle})
            )
            .await
            .0,
        404
    );
    let (status, _) = server
        .call(
            Method::POST,
            "/v1/core/decide",
            Some(DECISION),
            &json!({"event":{"amount":"1001"}}),
        )
        .await;
    assert_eq!(status, 422);
    assert_eq!(
        server
            .call(Method::POST, "/v1/decide", Some(DECISION), &json!({}))
            .await
            .0,
        404
    );
    // Real process restart, not construction of another Router in the same process.
    server.process.stop();
    let restarted = Server::start(&config, "server-restarted").await;
    let state = restarted.state().await;
    assert_ne!(state["revision"], updated["revision"]);
    assert_ne!(state["revision"], original["revision"]);
    assert_eq!(state["policy_sha256"], next.policy);
    assert_eq!(state["repository"], updated["repository"]);
    restarted.assert_decision(&state, 2500, false).await;
    let (status, _) = restarted
        .call(
            Method::POST,
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            &json!({"expected_revision":updated["revision"]}),
        )
        .await;
    assert_eq!(status, 409);
    assert_eq!(restarted.state().await, state);
    restarted.assert_decision(&state, 2500, false).await;
    publish(config.parent().unwrap(), &initial.bundle, "initial");
    let (status, rolled_back) = restarted
        .call(
            Method::POST,
            "/v1/core/repo/reload",
            Some(PUBLISHER),
            &json!({"expected_revision":state["revision"]}),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(rolled_back["policy_sha256"], initial.policy);
    assert_eq!(rolled_back["repository"]["revision"], "initial");
    restarted.assert_decision(&rolled_back, 2500, true).await;
    for process in [&server.process, &restarted.process] {
        for path in [&process.stdout, &process.stderr] {
            let log = fs::read_to_string(path).unwrap();
            assert!(!log.contains(DECISION) && !log.contains(PUBLISHER));
            assert!(!log.contains("Loaded configuration"));
        }
    }
}

#[test]
fn rejected_bootstrap_exits_without_listening_or_legacy_fallback() {
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("core.json");
    save(
        &config,
        &json!({
            "config_version":"unsupported", "listen":"127.0.0.1:0",
            "context":"context.yaml", "target":"target.json", "cases":"cases.yaml",
            "repository":"repository", "approvals":[],
            "decision_token_env":"E2E_DECISION_TOKEN", "publisher_token_env":"E2E_PUBLISHER_TOKEN"
        }),
    );
    let mut process = Process::spawn(server_command(&config), temp.path(), "rejected-startup");
    assert!(!process.wait().success());
    let log = process.logs();
    assert!(
        log.contains("Unsupported Core server config version"),
        "{log}"
    );
    assert!(!log.contains("listening") && !log.contains("Loaded configuration"));
}
