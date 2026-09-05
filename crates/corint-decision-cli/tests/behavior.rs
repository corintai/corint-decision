use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const FILES: &[&str] = &[
    "rule.yaml",
    "ruleset.yaml",
    "pipeline.yaml",
    "registry.yaml",
];
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core")
}
fn fixture(name: &str) -> String {
    std::fs::read_to_string(root().join(name)).unwrap()
}
fn suite() -> Value {
    serde_yaml::from_str(&fixture("behavior.yaml")).unwrap()
}
fn setup(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in files
        .iter()
        .copied()
        .chain(["input-schema.yaml", "behavior.yaml"])
    {
        std::fs::write(dir.path().join(name), fixture(name)).unwrap();
    }
    dir
}
fn save(dir: &Path, suite: &Value) {
    std::fs::write(dir.join("behavior.yaml"), suite.to_string()).unwrap();
}
fn command(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_corint"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
fn report(output: Output, exit: i32) -> Value {
    assert_eq!(output.status.code(), Some(exit), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scope"], "behavior");
    assert_eq!(report["valid"], exit == 0);
    assert_eq!(report["business_evaluation"], "not_performed");
    report
}
fn run(dir: &Path, files: &[&str], exit: i32) -> Value {
    let mut args = vec![
        "test",
        "--input-schema",
        "input-schema.yaml",
        "--cases",
        "behavior.yaml",
        "--format",
        "json",
    ];
    args.extend(files);
    report(command(dir, &args), exit)
}

#[test]
fn shipped_examples_run_in_both_trace_modes_without_mutation() {
    let dir = setup(FILES);
    let first = run(dir.path(), FILES, 0);
    assert_eq!(first["execution_checked"], true);
    assert_eq!(first["test_results"]["total"], 5);
    assert_eq!(first["test_results"]["executed"], 5);
    assert_eq!(first["test_results"]["passed"], 5);
    assert_eq!(first["test_results"]["failed"], 0);
    for case in first["test_results"]["cases"].as_array().unwrap() {
        assert_eq!(case["trace_parity"], true);
        assert_eq!(case["passed"], true);
        assert_eq!(case["actual"], case["trace_actual"]);
        assert!(case.get("input").is_none());
    }
    assert_eq!(
        first,
        run(dir.path(), FILES, 0),
        "No random IDs/timing in semantic reports"
    );
    for name in FILES
        .iter()
        .copied()
        .chain(["input-schema.yaml", "behavior.yaml"])
    {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(name)).unwrap(),
            fixture(name)
        );
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 6);
}

#[test]
fn existing_real_engine_cases_include_router_paths_and_skipped_calls() {
    let manifest: Value = serde_yaml::from_str(&fixture("manifest.yaml")).unwrap();
    for case in manifest["cases"].as_array().unwrap() {
        let files: Vec<_> = case["documents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        let dir = setup(&files);
        let cases: Vec<_> = case["runs"].as_array().unwrap().iter().enumerate().map(|(i, run)| {
            let locals: serde_json::Map<_,_> = run["local_scores"].as_object().unwrap().iter().map(|(id, score)| {
                let signal = if id == "branch" { "review" } else if score.as_i64().unwrap() >= 60 { "decline" } else { "approve" };
                (id.clone(), json!({"score":score,"signal":signal}))
            }).collect();
            let calls: Vec<_> = run["calls"].as_array().unwrap().iter().map(|rule| {
                let id = if rule == "branch_marker" {"branch"} else {"risk"};
                json!({"ruleset_id":id,"rule_id":rule,"triggered":run["triggered"].as_array().unwrap().contains(rule),"score":run["local_scores"][id]})
            }).collect();
            json!({"id":format!("case_{i}"),"input":{"event":{"amount":run["amount"]}},"expect":{
                "pipeline_id":"payment","score":run["score"],"signal":run["signal"],"actions":run["actions"],
                "triggered_rules":run["triggered"],"steps":run["steps"],"calls":calls,"local_results":locals
            }})
        }).collect();
        save(
            dir.path(),
            &json!({"version":"1","profile":"cdl-core-risk-draft-1","cases":cases}),
        );
        let report = run(dir.path(), &files, 0);
        assert_eq!(report["test_results"]["passed"], cases.len());
    }
}

#[test]
fn every_result_assertion_can_fail_and_later_cases_still_run() {
    let dir = setup(FILES);
    for (field, wrong) in [
        ("pipeline_id", json!("wrong")),
        ("score", json!(61)),
        ("signal", json!("approve")),
        ("actions", json!([])),
        ("triggered_rules", json!([])),
        ("explanation", json!("wrong")),
        ("steps", json!(["check", "extra"])),
        ("calls", json!([])),
        ("local_results", json!({})),
    ] {
        let mut suite = suite();
        suite["cases"][0]["expect"][field] = wrong;
        save(dir.path(), &suite);
        let result = run(dir.path(), FILES, 1);
        assert_eq!(result["test_results"]["failed"], 1);
        assert_eq!(result["test_results"]["passed"], 4);
        assert_eq!(result["test_results"]["executed"], 5);
        let error = &result["test_results"]["cases"][0]["diagnostics"][0];
        assert_eq!(error["code"], "E_TEST_MISMATCH");
        assert_eq!(error["source"], "behavior.yaml");
        assert_eq!(error["field_path"], format!("/cases/0/expect/{field}"));
    }
}

#[test]
fn expected_and_unexpected_errors_are_not_confused() {
    let dir = setup(FILES);
    let mut data = suite();
    data["cases"][3]["expect_error"] = json!({"stage":"execute","code":"E_NO_PIPELINE_MATCH"});
    save(dir.path(), &data);
    let report = run(dir.path(), FILES, 1);
    assert_eq!(
        report["test_results"]["cases"][3]["actual"]["error"]["code"],
        "E_INPUT_SCHEMA"
    );
    let expected = data["cases"][0]["expect"].take();
    data["cases"][0].as_object_mut().unwrap().remove("expect");
    data["cases"][0]["expect_error"] = json!({"stage":"input","code":"E_INPUT_SCHEMA"});
    data["cases"][3]
        .as_object_mut()
        .unwrap()
        .remove("expect_error");
    data["cases"][3]["expect"] = expected;
    save(dir.path(), &data);
    assert_eq!(run(dir.path(), FILES, 1)["test_results"]["failed"], 2);
}

#[test]
fn malformed_suites_fail_before_any_case_executes() {
    let dir = setup(FILES);
    for (pointer, value) in [
        ("/version", json!("2")),
        ("/profile", json!("legacy")),
        ("/cases", json!([])),
        ("/cases/0/input/event", json!([])),
        ("/cases/0/expect/score", json!("60")),
        ("/cases/0/expect/signal", json!("allow")),
        ("/cases/0/expect", json!({})),
        ("/cases/0/expect", Value::Null),
        (
            "/cases/3/expect_error",
            json!({"stage":"compile","code":"E_COMPILE"}),
        ),
    ] {
        let mut data = suite();
        *data.pointer_mut(pointer).unwrap() = value;
        save(dir.path(), &data);
        assert_not_run(run(dir.path(), FILES, 1), "E_TEST_SUITE");
    }
    for yaml in [
        "version: '1'\nversion: '1'\nprofile: cdl-core-risk-draft-1\ncases: []".to_string(),
        fixture("behavior.yaml") + "\n---\nversion: '1'",
        fixture("behavior.yaml").replace(
            "input: {event: {amount: 1001}}",
            "input: {event: {amount: 1001, amount: 0}}",
        ),
    ] {
        std::fs::write(dir.path().join("behavior.yaml"), yaml).unwrap();
        assert_not_run(run(dir.path(), FILES, 1), "E_TEST_SUITE");
    }
    for location in [
        "",
        "/cases/0",
        "/cases/0/input",
        "/cases/0/expect",
        "/cases/0/expect/calls/0",
        "/cases/0/expect/local_results/risk",
    ] {
        let mut data = suite();
        data.pointer_mut(location)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(true));
        save(dir.path(), &data);
        assert_not_run(run(dir.path(), FILES, 1), "E_TEST_SUITE");
    }
    let mut data = suite();
    data["cases"][0]["expect_error"] = json!({"stage":"input","code":"E_INPUT_SCHEMA"});
    save(dir.path(), &data);
    assert_not_run(run(dir.path(), FILES, 1), "E_TEST_SUITE");
    data = suite();
    data["cases"][1]["id"] = data["cases"][0]["id"].clone();
    save(dir.path(), &data);
    assert_not_run(run(dir.path(), FILES, 1), "E_DUPLICATE_ID");
}

fn assert_not_run(report: Value, code: &str) {
    assert_eq!(report["execution_checked"], false);
    assert!(report.get("test_results").is_none());
    assert_eq!(report["diagnostics"][0]["code"], code);
}

#[test]
fn compile_failures_cannot_be_swallowed_as_expected_runtime_errors() {
    let dir = setup(FILES);
    std::fs::write(
        dir.path().join("rule.yaml"),
        fixture("rule.yaml").replace("\"0.1\"", "\"9.9\""),
    )
    .unwrap();
    let report = run(dir.path(), FILES, 1);
    assert_not_run(report, "E_UNSUPPORTED_VERSION");
}

#[test]
fn no_pipeline_match_and_overflow_are_controlled_expected_errors() {
    let dir = setup(FILES);
    let mut data = suite();
    data["cases"] = json!([{"id":"no_match","input":{"event":{"amount":-1}},"expect_error":{"stage":"execute","code":"E_NO_PIPELINE_MATCH"}}]);
    save(dir.path(), &data);
    std::fs::write(
        dir.path().join("registry.yaml"),
        "version: '0.1'\nregistry: [{pipeline: payment, when: 'event.amount >= 0'}]",
    )
    .unwrap();
    run(dir.path(), FILES, 0);
    data["cases"] = json!([{"id":"overflow","input":{"event":{"amount":1001}},"expect_error":{"stage":"execute","code":"E_SCORE_OVERFLOW"}}]);
    save(dir.path(), &data);
    std::fs::write(
        dir.path().join("rule.yaml"),
        fixture("rule.yaml").replace("score: 60", "score: 2147483647"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("extra.yaml"),
        fixture("rule.yaml")
            .replace("id: large_amount", "id: extra")
            .replace("score: 60", "score: 1"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("ruleset.yaml"),
        fixture("ruleset.yaml").replace("[large_amount]", "[large_amount, extra]"),
    )
    .unwrap();
    let mut files = FILES.to_vec();
    files.push("extra.yaml");
    run(dir.path(), &files, 0);
}

#[test]
fn suite_file_io_and_command_errors_are_distinct() {
    let dir = setup(FILES);
    for args in [
        vec![
            "test",
            "--format",
            "json",
            "--input-schema",
            "input-schema.yaml",
            "rule.yaml",
        ],
        vec!["test", "--format", "json", "--cases"],
        vec![
            "test",
            "--format",
            "json",
            "--cases",
            "behavior.yaml",
            "--cases",
            "behavior.yaml",
        ],
    ] {
        assert_not_run(report(command(dir.path(), &args), 2), "E_USAGE");
    }
    let args = [
        "test",
        "--format",
        "json",
        "--input-schema",
        "input-schema.yaml",
        "--cases",
        "missing.yaml",
        "rule.yaml",
    ];
    assert_not_run(report(command(dir.path(), &args), 2), "E_IO");
    let output = command(
        dir.path(),
        &["validate", "--format", "json", "--cases", "behavior.yaml"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["diagnostics"][0]["code"],
        "E_USAGE"
    );
}

#[test]
fn numeric_integer_spellings_and_text_output_are_supported() {
    let dir = setup(FILES);
    let mut data = suite();
    data["cases"][0]["expect"]["score"] = json!(60.0);
    save(dir.path(), &data);
    run(dir.path(), FILES, 0);
    let mut args = vec![
        "test",
        "--input-schema",
        "input-schema.yaml",
        "--cases",
        "behavior.yaml",
    ];
    args.extend(FILES);
    let output = command(dir.path(), &args);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("5/5 cases passed"));
    assert!(text.contains("business effectiveness not evaluated"));
    assert!(command(dir.path(), &["test", "--help"]).status.success());
}

#[test]
fn runtime_input_diagnostics_are_stable_and_escape_json_pointers() {
    let dir = setup(FILES);
    let mut data = suite();
    data["cases"] = json!([{"id":"extra_fields","input":{"event":{"amount":1,"a/b~c":true,"z":true}},"expect_error":{"stage":"input","code":"E_INPUT_SCHEMA"}}]);
    save(dir.path(), &data);
    let first = run(dir.path(), FILES, 0);
    assert_eq!(
        first["test_results"]["cases"][0]["diagnostics"][0]["field_path"],
        "/event/a~1b~0c"
    );
    for _ in 0..3 {
        assert_eq!(first, run(dir.path(), FILES, 0));
    }
}

#[test]
fn test_capability_evidence_and_suite_contract_are_in_sync() {
    let capabilities: Value =
        serde_json::from_str(include_str!("../../../docs/cdl/schema/capabilities.json")).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../../../docs/cdl/schema/test-suite.json")).unwrap();
    assert_eq!(
        schema["properties"]["profile"]["const"],
        capabilities["profile"]
    );
    assert_eq!(schema["properties"]["version"]["const"], suite()["version"]);
    assert_eq!(capabilities["test_suite_schema"], "test-suite.json");
    let tool = &capabilities["tools"]["test_cli"];
    assert_eq!(tool["entry_point"], "DecisionEngine::from_core");
    assert_eq!(tool["scope"], "declared_examples_only");
    assert_eq!(tool["trace_modes"], json!([false, true]));
    assert_eq!(tool["business_evaluation"], "not_performed");
    assert_eq!(
        root()
            .join("../../../docs/cdl/schema")
            .join(tool["evidence"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/behavior.rs")
            .canonicalize()
            .unwrap()
    );
}
