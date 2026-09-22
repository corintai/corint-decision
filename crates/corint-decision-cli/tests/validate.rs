//! Public validation has one full-CDL static path, with no profile selection.
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

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core")
}

fn setup() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in FILES.iter().copied().chain(["input-schema.yaml"]) {
        std::fs::copy(fixture_root().join(name), dir.path().join(name)).unwrap();
    }
    dir
}

fn command(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_corint"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn validate(dir: &Path, args: &[&str], code: i32) -> Value {
    let mut command_args = vec!["validate", "--format", "json"];
    command_args.extend(args);
    let output = command(dir, &command_args);
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["profile"], "cdl-static-1");
    assert_eq!(report["scope"], "static");
    assert_eq!(report["valid"], code == 0);
    assert_eq!(report["execution_checked"], false);
    report
}

#[test]
fn validation_capability_only_advertises_full_cdl_static_checks() {
    let capabilities: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/capabilities.json"
    ))
    .unwrap();
    let tool = &capabilities["tools"]["validate_cli"];
    assert!(capabilities["tools"].get("core_validate_cli").is_none());
    assert_eq!(tool["scope"], "static");
    assert_eq!(tool["execution_checked"], false);
    assert_eq!(
        tool["resource_kinds"],
        json!(["rule", "ruleset", "pipeline", "registry", "feature", "list", "service"])
    );
    assert!(!tool["command"].as_str().unwrap().contains("--profile"));
    assert!(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/contracts/schema")
        .join(tool["evidence"].as_str().unwrap())
        .is_file());
}

#[test]
fn profile_selection_is_rejected_without_falling_back_to_core_compilation() {
    let dir = setup();
    for profile in ["cdl-core-risk-draft-1", "cdl-static-1", "unknown"] {
        for args in [
            vec!["--profile", profile, "rule.yaml"],
            vec!["rule.yaml", "--profile", profile],
            vec!["--profile", profile, "--profile", profile, "rule.yaml"],
        ] {
            let report = validate(dir.path(), &args, 2);
            assert_eq!(report["diagnostics"][0]["code"], "E_ARGUMENT");
            assert_eq!(report["diagnostics"][0]["stage"], "usage");
            assert!(report["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .contains("Unknown option: --profile"));
        }
    }
}

#[test]
fn core_fixtures_use_static_validation_with_optional_input_schema() {
    let dir = setup();
    let report = validate(dir.path(), &["rule.yaml"], 0);
    assert_eq!(report["input_schema_checked"], false);
    assert_eq!(report["sources"].as_array().unwrap().len(), 1);
    let mut args = vec!["--input-schema", "input-schema.yaml"];
    args.extend(FILES);
    let report = validate(dir.path(), &args, 0);
    assert_eq!(report["input_schema_checked"], true);
    assert_eq!(report["references_checked"], true);
    assert_eq!(report["sources"].as_array().unwrap().len(), FILES.len());
    for name in FILES.iter().copied().chain(["input-schema.yaml"]) {
        assert_eq!(
            std::fs::read(dir.path().join(name)).unwrap(),
            std::fs::read(fixture_root().join(name)).unwrap()
        );
    }
}

#[test]
fn profile_tokens_after_terminator_are_literal_source_paths() {
    let dir = setup();
    for name in [
        "--profile",
        "cdl-core-risk-draft-1",
        "a rule.yaml",
        "规则.yaml",
    ] {
        std::fs::copy(dir.path().join("rule.yaml"), dir.path().join(name)).unwrap();
        let report = validate(dir.path(), &["--", name], 0);
        assert_eq!(
            report["sources"],
            json!([dir.path().join(name).canonicalize().unwrap()])
        );
        std::fs::remove_file(dir.path().join(name)).unwrap();
    }
    std::fs::copy(dir.path().join("rule.yaml"), dir.path().join("--profile")).unwrap();
    std::fs::copy(
        dir.path().join("ruleset.yaml"),
        dir.path().join("cdl-core-risk-draft-1"),
    )
    .unwrap();
    validate(dir.path(), &["--", "--profile", "cdl-core-risk-draft-1"], 0);
}

#[test]
fn io_errors_and_argument_errors_keep_static_json_reports() {
    let dir = setup();
    std::fs::write(dir.path().join("invalid-utf8.yaml"), [0xff, 0xfe]).unwrap();
    for path in ["missing.yaml", "invalid-utf8.yaml"] {
        let report = validate(dir.path(), &[path], 2);
        assert_eq!(report["diagnostics"][0]["code"], "E_READ");
    }
    assert_eq!(
        validate(dir.path(), &[], 2)["diagnostics"][0]["code"],
        "E_NO_SOURCES"
    );
    for args in [
        vec!["--input-schema"],
        vec!["--unknown"],
        vec!["--format", "text"],
    ] {
        let report = validate(dir.path(), &args, 2);
        assert_eq!(report["diagnostics"][0]["code"], "E_ARGUMENT");
    }
}

#[test]
fn help_and_text_output_describe_static_validation_without_profile_selection() {
    let dir = setup();
    for args in [vec!["--help"], vec!["-h"], vec!["validate", "--help"]] {
        let output = command(dir.path(), &args);
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("Usage:"));
        assert!(!help.contains("--profile"));
        assert!(!help.contains("Validate compiles only"));
    }
    let version = command(dir.path(), &["--version"]);
    assert!(version.status.success());
    assert!(String::from_utf8(version.stdout)
        .unwrap()
        .contains("cdl-static-1"));
    let output = command(dir.path(), &["validate", "rule.yaml"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("CDL static validation: PASS"));
    assert!(text.contains("execution checked: false"));
    std::fs::write(dir.path().join("rule.yaml"), "version: [").unwrap();
    let output = command(dir.path(), &["validate", "rule.yaml"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stdout).unwrap().contains("E_YAML"));
}
