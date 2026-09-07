use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
fn setup() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in FILES
        .iter()
        .copied()
        .chain(["input-schema.yaml", "behavior.yaml"])
    {
        std::fs::write(dir.path().join(name), fixture(name)).unwrap();
    }
    dir
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn cmd(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_corint"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
fn report(output: Output, exit: i32, scope: &str) -> Value {
    assert_eq!(output.status.code(), Some(exit), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["scope"], scope);
    assert_eq!(value["valid"], exit == 0);
    assert_eq!(value["business_evaluation"], "not_performed");
    value
}
fn build(dir: &Path, output: &str, files: &[&str], exit: i32) -> Value {
    let mut args = vec![
        "build",
        "--input-schema",
        "input-schema.yaml",
        "--cases",
        "behavior.yaml",
        "--output",
        output,
        "--format",
        "json",
    ];
    args.extend(files);
    report(cmd(dir, &args), exit, "build")
}
fn verify(dir: &Path, file: &str, exit: i32) -> Value {
    report(
        cmd(
            dir,
            &[
                "verify",
                "--package",
                file,
                "--cases",
                "behavior.yaml",
                "--format",
                "json",
            ],
        ),
        exit,
        "verify",
    )
}
fn read(dir: &Path, name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(dir.join(name)).unwrap()).unwrap()
}
fn save(dir: &Path, name: &str, value: &Value) {
    std::fs::write(dir.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn error(report: &Value, code: &str, executed: bool) {
    assert_eq!(report["diagnostics"][0]["code"], code, "{report}");
    assert_eq!(report["execution_checked"], executed);
    assert!(report.get("artifact").is_none());
}
// Independent hash-format reference: serde_json here uses recursively sorted
// object maps, exactly as the documented canonical-json-v1 encoding requires.
fn rebind_policy(package: &mut Value) {
    let sources: Vec<_> = package["policy"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| json!({"path":d["path"],"sha256":d["sha256"]}))
        .collect();
    let value = json!({"domain":"core-source-policy-v1","value":{
        "profile":package["profile"],"language_version":package["language_version"],
        "input_schema_sha256":package["policy"]["input_schema"]["sha256"],"sources":sources
    }});
    let mut bytes = b"corint-canonical-json-v1\0".to_vec();
    bytes.extend(serde_json::to_vec(&value).unwrap());
    let digest = hash(&bytes);
    package["policy"]["sha256"] = json!(digest);
    package["evidence"]["policy_sha256"] = json!(digest);
}

#[test]
fn builds_content_bound_snapshot_without_exporting_case_inputs() {
    let dir = setup();
    let report = build(dir.path(), "package.json", FILES, 0);
    let package = read(dir.path(), "package.json");
    assert_eq!(report["execution_checked"], true);
    assert_eq!(report["test_results"]["passed"], 5);
    assert_eq!(
        report["artifact"]["package_sha256"],
        hash(&std::fs::read(dir.path().join("package.json")).unwrap())
    );
    assert_eq!(
        package["evidence"]["suite_sha256"],
        hash(fixture("behavior.yaml").as_bytes())
    );
    assert_eq!(
        package["evidence"]["tool"]["executable_sha256"],
        hash(&std::fs::read(env!("CARGO_BIN_EXE_corint")).unwrap())
    );
    assert_eq!(
        package["policy"]["sha256"],
        package["evidence"]["policy_sha256"]
    );
    for doc in package["policy"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .chain([&package["policy"]["input_schema"]])
    {
        assert_eq!(
            doc["sha256"],
            hash(doc["yaml"].as_str().unwrap().as_bytes())
        );
    }
    let mut independent = package.clone();
    rebind_policy(&mut independent);
    assert_eq!(independent["policy"]["sha256"], package["policy"]["sha256"]);
    assert_eq!(package["evidence"]["business_evaluation"], "not_performed");
    assert_eq!(package["evidence"]["publication_approval"], "not_granted");
    assert_eq!(package["evidence"]["authenticity"], "unsigned");
    let text = std::fs::read_to_string(dir.path().join("package.json")).unwrap();
    assert!(!text.contains("above_threshold"));
    assert!(!text.contains("amount: 1001"));
    assert!(!text.contains(&dir.path().to_string_lossy().to_string()));
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
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 7);
}

#[test]
fn relocated_packages_verify_without_original_source_files() {
    let dir = setup();
    build(dir.path(), "package.json", FILES, 0);
    let relocated = tempfile::tempdir().unwrap();
    for file in ["package.json", "behavior.yaml"] {
        std::fs::copy(dir.path().join(file), relocated.path().join(file)).unwrap();
    }
    let verified = verify(relocated.path(), "package.json", 0);
    assert_eq!(verified["test_results"]["executed"], 5);
    assert_eq!(
        verified["artifact"]["policy_sha256"],
        read(dir.path(), "package.json")["policy"]["sha256"]
    );
    assert_eq!(std::fs::read_dir(relocated.path()).unwrap().count(), 2);
}

#[test]
fn source_locations_order_and_output_name_do_not_change_package_bytes() {
    let a = setup();
    let b = setup();
    build(a.path(), "first.json", FILES, 0);
    std::fs::rename(b.path().join("rule.yaml"), b.path().join("renamed.txt")).unwrap();
    build(
        b.path(),
        "second.json",
        &[
            "registry.yaml",
            "pipeline.yaml",
            "renamed.txt",
            "ruleset.yaml",
        ],
        0,
    );
    assert_eq!(
        std::fs::read(a.path().join("first.json")).unwrap(),
        std::fs::read(b.path().join("second.json")).unwrap()
    );
}

#[test]
fn fingerprints_separate_policy_test_and_report_versions() {
    let dir = setup();
    build(dir.path(), "base.json", FILES, 0);
    let base = read(dir.path(), "base.json");
    std::fs::write(
        dir.path().join("behavior.yaml"),
        fixture("behavior.yaml") + "\n# test-only change\n",
    )
    .unwrap();
    build(dir.path(), "test-change.json", FILES, 0);
    let changed = read(dir.path(), "test-change.json");
    assert_eq!(base["policy"], changed["policy"]);
    assert_ne!(
        base["evidence"]["suite_sha256"],
        changed["evidence"]["suite_sha256"]
    );
    assert_eq!(
        base["evidence"]["test_report_sha256"],
        changed["evidence"]["test_report_sha256"]
    );
    error(
        &verify(dir.path(), "base.json", 1),
        "E_SUITE_MISMATCH",
        false,
    );
    std::fs::write(
        dir.path().join("input-schema.yaml"),
        fixture("input-schema.yaml") + "\n# contract source change\n",
    )
    .unwrap();
    build(dir.path(), "schema-change.json", FILES, 0);
    assert_ne!(
        read(dir.path(), "schema-change.json")["policy"]["sha256"],
        changed["policy"]["sha256"]
    );
    std::fs::write(
        dir.path().join("rule.yaml"),
        fixture("rule.yaml") + "\n# source change\n",
    )
    .unwrap();
    build(dir.path(), "rule-change.json", FILES, 0);
    assert_ne!(
        read(dir.path(), "rule-change.json")["policy"]["sha256"],
        read(dir.path(), "schema-change.json")["policy"]["sha256"]
    );
}

#[test]
fn stale_forged_or_mismatched_evidence_is_rejected() {
    let dir = setup();
    build(dir.path(), "base.json", FILES, 0);
    let base = read(dir.path(), "base.json");
    for (pointer, value, code, executed) in [
        (
            "/evidence/policy_sha256",
            json!("0".repeat(64)),
            "E_POLICY_BINDING",
            false,
        ),
        (
            "/evidence/suite_sha256",
            json!("0".repeat(64)),
            "E_SUITE_MISMATCH",
            false,
        ),
        (
            "/evidence/tool/executable_sha256",
            json!("0".repeat(64)),
            "E_TOOL_MISMATCH",
            false,
        ),
        (
            "/evidence/tool/version",
            json!("other"),
            "E_TOOL_MISMATCH",
            false,
        ),
        (
            "/evidence/test_report_sha256",
            json!("0".repeat(64)),
            "E_REPORT_MISMATCH",
            true,
        ),
        ("/evidence/passed", json!(4), "E_REPORT_MISMATCH", true),
        (
            "/evidence/business_evaluation",
            json!("passed"),
            "E_PACKAGE_FORMAT",
            false,
        ),
        (
            "/evidence/publication_approval",
            json!("granted"),
            "E_PACKAGE_FORMAT",
            false,
        ),
    ] {
        let mut edited = base.clone();
        *edited.pointer_mut(pointer).unwrap() = value;
        save(dir.path(), "edited.json", &edited);
        error(&verify(dir.path(), "edited.json", 1), code, executed);
    }
    // Old evidence cannot attest to another policy, even if its example outputs agree.
    std::fs::write(
        dir.path().join("rule.yaml"),
        fixture("rule.yaml") + "\n# another version\n",
    )
    .unwrap();
    build(dir.path(), "other.json", FILES, 0);
    let mut other = read(dir.path(), "other.json");
    other["evidence"] = base["evidence"].clone();
    save(dir.path(), "edited.json", &other);
    error(
        &verify(dir.path(), "edited.json", 1),
        "E_POLICY_BINDING",
        false,
    );
}

#[test]
fn rehashing_changed_sources_cannot_skip_real_execution() {
    let dir = setup();
    build(dir.path(), "base.json", FILES, 0);
    let mut package = read(dir.path(), "base.json");
    let index = package["policy"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .position(|d| d["path"] == "rule/large_amount.yaml")
        .unwrap();
    let doc = &mut package["policy"]["sources"][index];
    doc["yaml"] = json!(doc["yaml"]
        .as_str()
        .unwrap()
        .replace("score: 60", "score: 61"));
    save(dir.path(), "edited.json", &package);
    error(
        &verify(dir.path(), "edited.json", 1),
        "E_PACKAGE_INTEGRITY",
        false,
    );
    let doc = &mut package["policy"]["sources"][index];
    doc["sha256"] = json!(hash(doc["yaml"].as_str().unwrap().as_bytes()));
    save(dir.path(), "edited.json", &package);
    error(
        &verify(dir.path(), "edited.json", 1),
        "E_POLICY_BINDING",
        false,
    );
    rebind_policy(&mut package);
    save(dir.path(), "edited.json", &package);
    let report = verify(dir.path(), "edited.json", 1);
    error(&report, "E_PACKAGE_TEST_FAILED", true);
    assert_eq!(report["test_results"]["failed"], 1);
}

#[test]
fn invalid_inputs_or_failed_cases_never_create_an_artifact() {
    let dir = setup();
    let mut suite: Value = serde_yaml::from_str(&fixture("behavior.yaml")).unwrap();
    suite["cases"][0]["expect"]["score"] = json!(999);
    std::fs::write(dir.path().join("behavior.yaml"), suite.to_string()).unwrap();
    let report = build(dir.path(), "failed.json", FILES, 1);
    assert_eq!(report["execution_checked"], true);
    assert_eq!(report["test_results"]["failed"], 1);
    assert!(report.get("artifact").is_none());
    assert!(!dir.path().join("failed.json").exists());
    std::fs::write(dir.path().join("rule.yaml"), "version: '9.9'").unwrap();
    error(
        &build(dir.path(), "invalid.json", FILES, 1),
        "E_UNSUPPORTED_VERSION",
        false,
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 6);
}

#[test]
fn output_creation_is_no_clobber_and_failure_leaves_no_temporary_files() {
    let dir = setup();
    for target in ["rule.yaml", "."] {
        error(
            &build(dir.path(), target, FILES, 2),
            "E_OUTPUT_EXISTS",
            false,
        );
    }
    build(dir.path(), "package.json", FILES, 0);
    let before = std::fs::read(dir.path().join("package.json")).unwrap();
    error(
        &build(dir.path(), "package.json", FILES, 2),
        "E_OUTPUT_EXISTS",
        false,
    );
    assert_eq!(
        std::fs::read(dir.path().join("package.json")).unwrap(),
        before
    );
    error(
        &build(dir.path(), "missing/failed.json", FILES, 2),
        "E_IO",
        true,
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 7);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("rule.yaml")).unwrap(),
        fixture("rule.yaml")
    );
}

#[cfg(unix)]
#[test]
fn existing_and_dangling_symlinks_are_never_followed_for_output() {
    let dir = setup();
    for (link, target) in [
        ("existing.json", "rule.yaml"),
        ("dangling.json", "not-created.yaml"),
    ] {
        std::os::unix::fs::symlink(target, dir.path().join(link)).unwrap();
        error(&build(dir.path(), link, FILES, 2), "E_OUTPUT_EXISTS", false);
    }
    assert!(!dir.path().join("not-created.yaml").exists());
}

#[test]
fn malformed_packages_and_unsafe_embedded_labels_fail_without_extraction() {
    let dir = setup();
    build(dir.path(), "base.json", FILES, 0);
    let base = read(dir.path(), "base.json");
    for (pointer, value) in [
        ("/format_version", json!("2")),
        ("/profile", json!("legacy")),
        ("/policy/sources/0/path", json!("../../escape.yaml")),
        ("/policy/sources/0/path", json!("/tmp/escape.yaml")),
        ("/policy/sources", json!([])),
    ] {
        let mut edited = base.clone();
        *edited.pointer_mut(pointer).unwrap() = value;
        save(dir.path(), "edited.json", &edited);
        error(
            &verify(dir.path(), "edited.json", 1),
            "E_PACKAGE_FORMAT",
            false,
        );
    }
    for pointer in [
        "",
        "/policy",
        "/policy/input_schema",
        "/policy/sources/0",
        "/evidence",
        "/evidence/tool",
    ] {
        let mut edited = base.clone();
        edited
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(true));
        save(dir.path(), "edited.json", &edited);
        error(
            &verify(dir.path(), "edited.json", 1),
            "E_PACKAGE_FORMAT",
            false,
        );
    }
    let text = serde_json::to_string(&base).unwrap();
    for malformed in [
        text.replace(
            "\"validation\":\"passed\"",
            "\"validation\":\"passed\",\"validation\":\"passed\"",
        ),
        text.clone() + &text,
    ] {
        std::fs::write(dir.path().join("edited.json"), malformed).unwrap();
        error(
            &verify(dir.path(), "edited.json", 1),
            "E_PACKAGE_FORMAT",
            false,
        );
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 8);
}

#[test]
fn build_verify_options_and_missing_files_fail_with_clear_exit_codes() {
    let dir = setup();
    for (scope, args) in [
        (
            "build",
            vec![
                "build",
                "--format",
                "json",
                "--input-schema",
                "input-schema.yaml",
                "--cases",
                "behavior.yaml",
                "rule.yaml",
            ],
        ),
        (
            "verify",
            vec!["verify", "--format", "json", "--package", "missing.json"],
        ),
        (
            "verify",
            vec![
                "verify",
                "--format",
                "json",
                "--package",
                "missing.json",
                "--cases",
                "behavior.yaml",
                "rule.yaml",
            ],
        ),
        (
            "verify",
            vec![
                "verify",
                "--format",
                "json",
                "--input-schema",
                "input-schema.yaml",
            ],
        ),
        (
            "build",
            vec![
                "build", "--format", "json", "--output", "a", "--output", "b",
            ],
        ),
        (
            "verify",
            vec![
                "verify",
                "--format",
                "json",
                "--package",
                "a",
                "--package",
                "b",
            ],
        ),
    ] {
        error(&report(cmd(dir.path(), &args), 2, scope), "E_USAGE", false);
    }
    error(&verify(dir.path(), "missing.json", 2), "E_IO", false);
    for command in ["build", "verify"] {
        assert!(cmd(dir.path(), &[command, "--help"]).status.success());
    }
}

#[test]
fn concurrent_builds_cannot_replace_each_others_output() {
    use std::process::Stdio;
    let dir = setup();
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_corint"))
            .current_dir(dir.path())
            .args([
                "build",
                "--input-schema",
                "input-schema.yaml",
                "--cases",
                "behavior.yaml",
                "--output",
                "race.json",
                "--format",
                "json",
            ])
            .args(FILES)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let first = spawn();
    let second = spawn();
    let outputs = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    let mut codes: Vec<_> = outputs.iter().map(|o| o.status.code().unwrap()).collect();
    codes.sort();
    assert_eq!(codes, vec![0, 2]);
    for output in outputs {
        let code = output.status.code().unwrap();
        let result = report(output, code, "build");
        if code == 2 {
            assert_eq!(result["diagnostics"][0]["code"], "E_OUTPUT_EXISTS");
        }
    }
    verify(dir.path(), "race.json", 0);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 7);
}

#[test]
fn rehashed_bundles_still_require_complete_unique_canonical_resources() {
    let dir = setup();
    build(dir.path(), "base.json", FILES, 0);
    let base = read(dir.path(), "base.json");
    let mut package = base.clone();
    package["policy"]["sources"]
        .as_array_mut()
        .unwrap()
        .retain(|d| d["path"] != "rule/large_amount.yaml");
    rebind_policy(&mut package);
    save(dir.path(), "edited.json", &package);
    error(
        &verify(dir.path(), "edited.json", 1),
        "E_UNRESOLVED_REF",
        false,
    );
    package = base.clone();
    let duplicate = package["policy"]["sources"][0].clone();
    package["policy"]["sources"]
        .as_array_mut()
        .unwrap()
        .insert(0, duplicate);
    rebind_policy(&mut package);
    save(dir.path(), "edited.json", &package);
    error(
        &verify(dir.path(), "edited.json", 1),
        "E_DUPLICATE_ID",
        false,
    );
    package = base;
    package["policy"]["sources"][0]["path"] = json!("pipeline/wrong.yaml");
    rebind_policy(&mut package);
    save(dir.path(), "edited.json", &package);
    error(
        &verify(dir.path(), "edited.json", 1),
        "E_PACKAGE_FORMAT",
        false,
    );
}

#[test]
fn package_schema_and_capability_artifacts_are_in_sync() {
    let capabilities: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/capabilities.json"
    ))
    .unwrap();
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/source-package.json"
    ))
    .unwrap();
    assert_eq!(capabilities["source_package_schema"], "source-package.json");
    assert_eq!(
        schema["properties"]["profile"]["const"],
        capabilities["profile"]
    );
    let tool = &capabilities["tools"]["source_package_cli"];
    assert_eq!(tool["authenticity"], "unsigned");
    assert_eq!(tool["publication_approval"], "not_granted");
    assert_eq!(tool["business_evaluation"], "not_performed");
    assert_eq!(tool["commands"], json!(["corint build", "corint verify"]));
    assert_eq!(
        root()
            .join("../../../docs/contracts/schema")
            .join(tool["evidence"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/package.rs")
            .canonicalize()
            .unwrap()
    );
    let dir = setup();
    build(dir.path(), "base.json", FILES, 0);
    let mut package = read(dir.path(), "base.json");
    package["evidence"]["total"] = json!(5.0);
    package["evidence"]["passed"] = json!(5.0);
    save(dir.path(), "counts.json", &package);
    verify(dir.path(), "counts.json", 0);
    package["evidence"]["total"] = json!(5.5);
    save(dir.path(), "counts.json", &package);
    error(
        &verify(dir.path(), "counts.json", 1),
        "E_PACKAGE_FORMAT",
        false,
    );
}
