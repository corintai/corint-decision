//! Real subprocess + official SDK client: protocol and engine behavior together.
use corint_decision_mcp::CorintMcp;
use rmcp::{model::*, service::RunningService, RoleClient, ServiceExt};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

type Client = RunningService<RoleClient, ClientConfig>;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core")
}

fn policy(id: &str, root: &Path) -> Value {
    json!({"id": id, "root": root, "files": ["rule.yaml", "ruleset.yaml", "pipeline.yaml", "registry.yaml"],
        "input_schema": "input-schema.yaml", "cases": "behavior.yaml"})
}

fn config(dir: &Path, policies: Vec<Value>) -> PathBuf {
    let path = dir.join("mcp.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({"policies":policies})).unwrap(),
    )
    .unwrap();
    path
}

fn copy_policy(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    for file in [
        "rule.yaml",
        "ruleset.yaml",
        "pipeline.yaml",
        "registry.yaml",
        "input-schema.yaml",
        "behavior.yaml",
    ] {
        std::fs::copy(fixture_root().join(file), dir.join(file)).unwrap();
    }
}

async fn start(path: &Path) -> (Client, tokio::process::Child) {
    start_mode("--config", path).await
}

async fn start_mode(mode: &str, path: &Path) -> (Client, tokio::process::Child) {
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_corint-mcp"))
        .arg(mode)
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let io = (child.stdout.take().unwrap(), child.stdin.take().unwrap());
    let client = tokio::time::timeout(Duration::from_secs(20), ClientConfig::default().serve(io))
        .await
        .unwrap()
        .unwrap();
    (client, child)
}

async fn call(client: &Client, name: &str, args: Value, failed: bool) -> Value {
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        client.call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(args.as_object().unwrap().clone()),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.is_error, Some(failed), "{name}: {result:?}");
    let value = result.structured_content.unwrap();
    // Both modern structured clients and text-only clients receive the same report.
    let text = serde_json::to_value(&result.content).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(text[0]["text"].as_str().unwrap()).unwrap(),
        value
    );
    value
}

async fn stop(client: Client, mut child: tokio::process::Child) {
    client.cancel().await.unwrap();
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(
        status.success(),
        "server failed on client disconnect: {status}"
    );
}

#[tokio::test]
async fn discover_read_validate_test_and_evaluate_over_stdio() {
    let dir = tempfile::tempdir().unwrap();
    // Relative roots resolve against the config, independent of the child cwd.
    copy_policy(&dir.path().join("policy"));
    let path = config(dir.path(), vec![policy("payment", Path::new("policy"))]);
    let (client, child) = start(&path).await;
    let tools = client.list_all_tools().await.unwrap();
    let mut names: Vec<_> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "compare_policy_versions",
            "evaluate_decision",
            "get_policy",
            "list_policies",
            "test_policy",
            "validate_policy"
        ]
    );
    for tool in &tools {
        assert_eq!(
            tool.annotations.as_ref().unwrap().read_only_hint,
            Some(true)
        );
    }
    let listed = call(&client, "list_policies", json!({}), false).await;
    assert_eq!(listed["policies"][0]["policy_id"], "payment");
    let sources = call(&client, "get_policy", json!({"policy_id":"payment"}), false).await;
    assert_eq!(sources["sources"].as_array().unwrap().len(), 4);
    assert!(sources["input_schema"]["yaml"]
        .as_str()
        .unwrap()
        .contains("amount"));
    let resources = client.list_all_resources().await.unwrap();
    assert!(resources
        .iter()
        .any(|resource| resource.uri == "corint://cdl/authoring-schema"));
    let resource = client
        .read_resource(ReadResourceRequestParams::new("corint://policies/payment"))
        .await
        .unwrap();
    let resource = serde_json::to_value(resource).unwrap();
    let snapshot: Value =
        serde_json::from_str(resource["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(snapshot, sources);
    let document = client
        .read_resource(ReadResourceRequestParams::new(
            "corint://cdl/behavior-suite-schema",
        ))
        .await
        .unwrap();
    assert!(!serde_json::to_value(document).unwrap()["contents"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(client
        .read_resource(ReadResourceRequestParams::new("file:///etc/passwd"))
        .await
        .is_err());
    let report = call(
        &client,
        "validate_policy",
        json!({"policy_id":"payment"}),
        false,
    )
    .await;
    assert_eq!(report["valid"], true);
    assert_eq!(report["execution_checked"], false);
    assert_eq!(report["snapshot_sha256"], sources["snapshot_sha256"]);
    assert_eq!(report["reference_root"], ".");
    let tests = call(
        &client,
        "test_policy",
        json!({"policy_id":"payment"}),
        false,
    )
    .await;
    assert_eq!(tests["tests"]["passed"], 5);
    let decision = call(
        &client,
        "evaluate_decision",
        json!({"policy_id":"payment", "event":{"amount":1001}, "trace":true}),
        false,
    )
    .await;
    assert_eq!(decision["response"]["result"]["score"], 60);
    assert_eq!(decision["actions_executed"], false);
    assert!(decision["response"]["trace"].is_object());
    let error = call(
        &client,
        "evaluate_decision",
        json!({"policy_id":"payment", "event":{"amount":"1001"}}),
        true,
    )
    .await;
    assert_eq!(error["error"]["diagnostic"]["code"], "E_INPUT_SCHEMA");
    call(
        &client,
        "get_policy",
        json!({"policy_id":"../secret"}),
        true,
    )
    .await;
    let bad = client
        .call_tool(
            CallToolRequestParams::new("evaluate_decision").with_arguments(
                json!({"policy_id":"payment", "event":[], "extra":true})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    assert!(bad.is_err() || bad.unwrap().is_error == Some(true));
    // Subsequent requests still work after invalid parameters and execution errors.
    call(&client, "list_policies", json!({}), false).await;
    stop(client, child).await;
}

#[tokio::test]
async fn edits_and_comparison_use_fresh_sources_and_one_suite() {
    let dir = tempfile::tempdir().unwrap();
    let candidate = dir.path().join("candidate");
    copy_policy(&candidate);
    let path = config(
        dir.path(),
        vec![
            policy("baseline", &fixture_root()),
            policy("candidate", &candidate),
        ],
    );
    let (client, child) = start(&path).await;
    let args = json!({"baseline_policy_id":"baseline", "candidate_policy_id":"candidate"});
    let same = call(&client, "compare_policy_versions", args.clone(), false).await;
    assert_eq!(same["changed_cases"], 0);
    let original = std::fs::read_to_string(candidate.join("rule.yaml")).unwrap();
    std::fs::write(
        candidate.join("rule.yaml"),
        original.replace("> 1000", "> 2000"),
    )
    .unwrap();
    // Candidate suite must not be consulted by comparison.
    std::fs::write(candidate.join("behavior.yaml"), "invalid suite").unwrap();
    let changed = call(&client, "compare_policy_versions", args, false).await;
    assert_eq!(changed["changed_cases"], 1);
    assert_eq!(changed["changes"][0]["case_id"], "above_threshold");
    assert_eq!(changed["changes"][0]["before"]["result"]["score"], 60);
    assert_eq!(changed["changes"][0]["after"]["result"]["score"], 0);
    assert_eq!(changed["candidate"]["tests"]["failed"], 1);
    assert_ne!(
        same["candidate"]["snapshot_sha256"],
        changed["candidate"]["snapshot_sha256"]
    );
    let suite = std::fs::read_to_string(fixture_root().join("behavior.yaml")).unwrap();
    let failing = call(
        &client,
        "test_policy",
        json!({"policy_id":"candidate", "cases_yaml":suite}),
        true,
    )
    .await;
    assert_eq!(failing["tests"]["failed"], 1);
    // A malformed draft remains readable and yields diagnostics, not a server crash.
    std::fs::write(candidate.join("rule.yaml"), "rule: [").unwrap();
    call(
        &client,
        "get_policy",
        json!({"policy_id":"candidate"}),
        false,
    )
    .await;
    let invalid = call(
        &client,
        "validate_policy",
        json!({"policy_id":"candidate"}),
        true,
    )
    .await;
    assert_eq!(invalid["valid"], false);
    assert!(!invalid["diagnostics"].as_array().unwrap().is_empty());
    call(
        &client,
        "evaluate_decision",
        json!({"policy_id":"candidate", "event":{"amount":1001}}),
        true,
    )
    .await;
    stop(client, child).await;
}

#[tokio::test]
async fn static_authoring_is_separate_from_core_execution() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("service.yaml"),
        "name: lookup\nbase_url: http://127.0.0.1:1\noperations:\n  get:\n    method: GET\n    path: /customer\n").unwrap();
    let path = config(
        dir.path(),
        vec![json!({"id":"service", "root":".", "files":["service.yaml"]})],
    );
    let (client, child) = start(&path).await;
    call(
        &client,
        "validate_policy",
        json!({"policy_id":"service"}),
        false,
    )
    .await;
    call(
        &client,
        "evaluate_decision",
        json!({"policy_id":"service", "event":{}}),
        true,
    )
    .await;
    call(
        &client,
        "test_policy",
        json!({"policy_id":"service", "cases_yaml":"cases: []"}),
        true,
    )
    .await;
    stop(client, child).await;
}

#[test]
fn catalog_rejects_ambiguous_ids_and_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let path = config(
        dir.path(),
        vec![
            policy("duplicate", &fixture_root()),
            policy("duplicate", &fixture_root()),
        ],
    );
    assert!(CorintMcp::from_config(&path).is_err());
    for file in [
        "../rule.yaml",
        "/etc/passwd",
        "a/../../rule.yaml",
        "a//b.yaml",
    ] {
        let mut item = policy("payment", &fixture_root());
        item["files"] = json!([file]);
        let path = config(dir.path(), vec![item]);
        assert!(CorintMcp::from_config(&path).is_err(), "accepted {file}");
    }
}

#[tokio::test]
async fn validation_reads_only_declared_sources_and_rejects_oversize_files() {
    let dir = tempfile::tempdir().unwrap();
    copy_policy(dir.path());
    let mut item = policy("payment", dir.path());
    // The missing rule still exists in the original root. It must not be read
    // implicitly by the static dependency resolver outside the captured set.
    item["files"] = json!(["ruleset.yaml", "pipeline.yaml", "registry.yaml"]);
    let path = config(dir.path(), vec![item]);
    let (client, child) = start(&path).await;
    let report = call(
        &client,
        "validate_policy",
        json!({"policy_id":"payment"}),
        true,
    )
    .await;
    assert!(report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "E_UNRESOLVED_REFERENCE"));
    let file = std::fs::File::create(dir.path().join("ruleset.yaml")).unwrap();
    file.set_len(4 * 1024 * 1024 + 1).unwrap();
    let error = call(&client, "get_policy", json!({"policy_id":"payment"}), true).await;
    assert!(error["error"]["message"]
        .as_str()
        .unwrap()
        .contains("4 MiB"));
    stop(client, child).await;
}

#[cfg(unix)]
#[tokio::test]
async fn symlinks_and_nonregular_files_cannot_be_read() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("policy");
    copy_policy(&root);
    std::fs::remove_file(root.join("rule.yaml")).unwrap();
    symlink(fixture_root().join("rule.yaml"), root.join("rule.yaml")).unwrap();
    let path = config(dir.path(), vec![policy("payment", &root)]);
    let (client, child) = start(&path).await;
    call(&client, "get_policy", json!({"policy_id":"payment"}), true).await;
    call(
        &client,
        "validate_policy",
        json!({"policy_id":"payment"}),
        true,
    )
    .await;
    std::fs::remove_file(root.join("rule.yaml")).unwrap();
    std::fs::create_dir(root.join("rule.yaml")).unwrap();
    call(&client, "get_policy", json!({"policy_id":"payment"}), true).await;
    stop(client, child).await;
}

#[tokio::test]
async fn explicit_catalog_fixture_starts_and_runs() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mcp/catalog.json");
    let (client, child) = start(&path).await;
    let tested = call(
        &client,
        "test_policy",
        json!({"policy_id":"payment-demo"}),
        false,
    )
    .await;
    assert_eq!(tested["tests"]["passed"], 5);
    stop(client, child).await;
}

#[tokio::test]
async fn repository_discovery_tracks_changes_and_resolves_real_dependencies() {
    let dir = tempfile::tempdir().unwrap();
    for (folder, file) in [
        ("pipelines", "pipeline.yaml"),
        ("rulesets", "ruleset.yaml"),
        ("rules", "rule.yaml"),
    ] {
        std::fs::create_dir(dir.path().join(folder)).unwrap();
        std::fs::copy(
            fixture_root().join(file),
            dir.path().join(folder).join(file),
        )
        .unwrap();
    }
    // Non-policy configuration must never be captured or returned.
    std::fs::write(dir.path().join("credentials.yaml"), "secret: do-not-read").unwrap();
    let (client, child) = start_mode("--repository", dir.path()).await;
    let listed = call(&client, "list_policies", json!({}), false).await;
    assert_eq!(listed["source"], "repository");
    assert_eq!(listed["activation_status"], "not_checked");
    assert_eq!(listed["policies"].as_array().unwrap().len(), 2);
    assert_eq!(listed["policies"][0]["policy_id"], "pipeline/payment");
    let args = json!({"policy_id": "pipeline/payment"});
    let before = call(&client, "get_policy", args.clone(), false).await;
    let labels: Vec<_> = before["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        [
            "pipelines/pipeline.yaml",
            "rules/rule.yaml",
            "rulesets/ruleset.yaml"
        ]
    );
    let uri = "corint://policies/pipeline/payment";
    let resources = client.list_all_resources().await.unwrap();
    assert!(resources.iter().any(|r| r.uri == uri));
    let resource = client
        .read_resource(ReadResourceRequestParams::new(uri))
        .await
        .unwrap();
    let value = serde_json::to_value(resource).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(value["contents"][0]["text"].as_str().unwrap()).unwrap(),
        before
    );
    let rule = dir.path().join("rules/rule.yaml");
    std::fs::write(
        &rule,
        std::fs::read_to_string(&rule)
            .unwrap()
            .replace("1000", "2000"),
    )
    .unwrap();
    let after = call(&client, "get_policy", args.clone(), false).await;
    assert_ne!(before["snapshot_sha256"], after["snapshot_sha256"]);
    let new_path = dir.path().join("rulesets/new.yaml");
    std::fs::write(
        &new_path,
        std::fs::read_to_string(dir.path().join("rulesets/ruleset.yaml"))
            .unwrap()
            .replace("id: risk", "id: next"),
    )
    .unwrap();
    let listed = call(&client, "list_policies", json!({}), false).await;
    assert_eq!(listed["policies"].as_array().unwrap().len(), 3);
    assert!(client
        .list_all_resources()
        .await
        .unwrap()
        .iter()
        .any(|r| r.uri == "corint://policies/ruleset/next"));
    std::fs::remove_file(new_path).unwrap();
    call(
        &client,
        "get_policy",
        json!({"policy_id": "ruleset/next"}),
        true,
    )
    .await;
    call(
        &client,
        "get_policy",
        json!({"policy_id": "payment-demo"}),
        true,
    )
    .await;
    // Repository reading must not fabricate an execution schema.
    let execution = call(
        &client,
        "evaluate_decision",
        json!({"policy_id":"pipeline/payment", "event":{"amount":3000}}),
        true,
    )
    .await;
    assert!(execution["error"]["message"]
        .as_str()
        .unwrap()
        .contains("input_schema"));
    stop(client, child).await;
}

#[test]
fn repository_errors_never_fall_back_to_example_catalog() {
    let dir = tempfile::tempdir().unwrap();
    assert!(CorintMcp::from_repository(&dir.path().join("missing")).is_err());
    std::fs::create_dir(dir.path().join("rulesets")).unwrap();
    std::fs::write(dir.path().join("rulesets/bad.yaml"), "ruleset: [invalid").unwrap();
    assert!(CorintMcp::from_repository(dir.path()).is_err());
    #[cfg(unix)]
    {
        std::fs::remove_file(dir.path().join("rulesets/bad.yaml")).unwrap();
        std::os::unix::fs::symlink(
            fixture_root().join("ruleset.yaml"),
            dir.path().join("rulesets/link.yaml"),
        )
        .unwrap();
        assert!(CorintMcp::from_repository(dir.path()).is_err());
    }
}

#[tokio::test]
async fn repository_supports_shared_cdl_files_without_losing_declarations() {
    let rule = std::fs::read_to_string(fixture_root().join("rule.yaml")).unwrap();
    let ruleset = std::fs::read_to_string(fixture_root().join("ruleset.yaml")).unwrap();
    let pipeline = std::fs::read_to_string(fixture_root().join("pipeline.yaml")).unwrap();
    for separator in ["\n", "\n---\n"] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("pipelines")).unwrap();
        let documents = [
            pipeline.clone(),
            rule.clone(),
            rule.replace("large_amount", "second_amount"),
            ruleset.clone(),
            ruleset.replace("id: risk", "id: other_risk"),
        ];
        let shared = String::from("version: \"0.1\"\n")
            + &documents
                .iter()
                .map(|s| s.replace("version: \"0.1\"\n", ""))
                .collect::<Vec<_>>()
                .join(separator);
        std::fs::write(dir.path().join("pipelines/shared.yaml"), shared).unwrap();
        let (client, child) = start_mode("--repository", dir.path()).await;
        let listed = call(&client, "list_policies", json!({}), false).await;
        let ids: Vec<_> = listed["policies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["policy_id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            ["pipeline/payment", "ruleset/other_risk", "ruleset/risk"]
        );
        for id in ids {
            let snapshot = call(&client, "get_policy", json!({"policy_id": id}), false).await;
            assert_eq!(snapshot["sources"].as_array().unwrap().len(), 1);
            let report = call(&client, "validate_policy", json!({"policy_id": id}), false).await;
            assert_eq!(report["valid"], true);
        }
        stop(client, child).await;
    }
}
