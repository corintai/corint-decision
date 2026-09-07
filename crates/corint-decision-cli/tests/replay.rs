use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
fn run(args: &[&str], expected: i32) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_corint"))
        .args(args)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(expected),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn bundle(path: &Path) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core");
    let read =
        |name: &str| json!({"path":name,"yaml":std::fs::read_to_string(root.join(name)).unwrap()});
    let value = json!({"format":"corint-core-source-bundle","format_version":"1","profile":"cdl-core-risk-draft-1","language_version":"0.1","input_schema":read("input-schema.yaml"),"sources":[read("rule.yaml"),read("ruleset.yaml"),read("pipeline.yaml"),read("registry.yaml")]});
    std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}
#[test]
fn cli_records_replays_and_refuses_redaction_and_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("bundle.json");
    bundle(&source);
    let event = dir.path().join("event.json");
    std::fs::write(&event, r#"{"amount":1001}"#).unwrap();
    let record = dir.path().join("record.json");
    let args = [
        "record",
        "--bundle",
        source.to_str().unwrap(),
        "--event",
        event.to_str().unwrap(),
        "--output",
        record.to_str().unwrap(),
        "--visible-fields",
        "amount",
        "--retain-input",
        "--trace",
    ];
    assert_eq!(run(&args, 0)["replayable"], true);
    assert_eq!(
        run(
            &[
                "replay",
                "--bundle",
                source.to_str().unwrap(),
                "--record",
                record.to_str().unwrap()
            ],
            0
        )["matched"],
        true
    );
    run(&args, 2);
    let redacted = dir.path().join("redacted.json");
    run(
        &[
            "record",
            "--bundle",
            source.to_str().unwrap(),
            "--event",
            event.to_str().unwrap(),
            "--output",
            redacted.to_str().unwrap(),
        ],
        0,
    );
    let result = run(
        &[
            "replay",
            "--bundle",
            source.to_str().unwrap(),
            "--record",
            redacted.to_str().unwrap(),
        ],
        1,
    );
    assert_eq!(result["diagnostics"][0]["code"], "E_REPLAY_INCOMPLETE");
    assert_eq!(
        run(&["replay", "--trace"], 2)["diagnostics"][0]["code"],
        "E_USAGE"
    );
}
