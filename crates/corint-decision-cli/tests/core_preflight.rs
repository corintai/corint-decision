//! Strict compilation remains an execution preflight check for `corint cdl test`.
use corint_decision_compiler::core::{compile_core, parse_core_input_schema, CoreSource, PROFILE};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const FILES: &[&str] = &[
    "rule.yaml",
    "ruleset.yaml",
    "pipeline.yaml",
    "registry.yaml",
];

#[derive(Deserialize)]
struct Manifest {
    invalid: Vec<Invalid>,
}
#[derive(Deserialize)]
struct Invalid {
    id: String,
    document: String,
    find: String,
    replace: String,
    stage: String,
    code: String,
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core")
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_root().join(name)).unwrap()
}

fn manifest() -> Manifest {
    serde_yaml::from_str(&fixture("manifest.yaml")).unwrap()
}

fn setup(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in files
        .iter()
        .copied()
        .chain(["input-schema.yaml", "behavior.yaml"])
    {
        std::fs::write(dir.path().join(file), fixture(file)).unwrap();
    }
    dir
}

fn source(dir: &Path, name: &str) -> CoreSource {
    CoreSource {
        path: name.into(),
        yaml: std::fs::read_to_string(dir.join(name)).unwrap(),
    }
}

fn preflight(dir: &Path, files: &[&str], code: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_corint"))
        .arg("cdl")
        .current_dir(dir)
        .args([
            "test",
            "--input-schema",
            "input-schema.yaml",
            "--cases",
            "behavior.yaml",
            "--format",
            "json",
        ])
        .args(files)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["profile"], PROFILE);
    assert_eq!(report["scope"], "behavior");
    assert_eq!(report["valid"], false);
    assert_eq!(report["execution_checked"], false);
    assert!(report.get("test_results").is_none());
    assert_eq!(report["diagnostics"].as_array().unwrap().len(), 1);
    report
}

#[test]
fn core_rejects_flow_rule_lists_before_compilation() {
    let dir = setup(FILES);
    let yaml =
        fixture("ruleset.yaml").replace("rules:\n    - large_amount", "rules: [large_amount]");
    std::fs::write(dir.path().join("ruleset.yaml"), yaml).unwrap();
    let report = preflight(dir.path(), FILES, 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_RULES_FORMAT");
    assert_eq!(report["diagnostics"][0]["field_path"], "/ruleset/rules");
    assert_eq!(report["diagnostics"][0]["line"], 4);
}

#[test]
fn all_manifest_failures_preserve_shared_core_diagnostics() {
    let invalid = manifest().invalid;
    assert!(!invalid.is_empty());
    for case in invalid {
        let dir = setup(FILES);
        let original = fixture(&case.document);
        assert_eq!(original.matches(&case.find).count(), 1, "{}", case.id);
        std::fs::write(
            dir.path().join(&case.document),
            original.replacen(&case.find, &case.replace, 1),
        )
        .unwrap();
        let schema = parse_core_input_schema(&source(dir.path(), "input-schema.yaml")).unwrap();
        let sources: Vec<_> = FILES.iter().map(|name| source(dir.path(), name)).collect();
        let error = compile_core(&sources, schema).unwrap_err();
        let report = preflight(dir.path(), FILES, 1);
        assert_eq!(
            report["diagnostics"][0],
            serde_json::to_value(&error).unwrap(),
            "{}",
            case.id
        );
        assert_eq!(report["diagnostics"][0]["stage"], case.stage, "{}", case.id);
        assert_eq!(report["diagnostics"][0]["code"], case.code, "{}", case.id);
    }
}

#[test]
fn incomplete_closure_is_not_reported_as_valid_yaml() {
    let dir = setup(FILES);
    // A missing required Registry is a missing field; a present Registry with
    // absent resources is an unresolved reference. Preserve the Core distinction.
    for (files, code) in [
        (&FILES[..1], "E_MISSING_FIELD"),
        (&FILES[1..], "E_UNRESOLVED_REF"),
        (&FILES[..3], "E_MISSING_FIELD"),
    ] {
        let report = preflight(dir.path(), files, 1);
        assert_eq!(report["diagnostics"][0]["stage"], "resolve");
        assert_eq!(report["diagnostics"][0]["code"], code);
    }
}

#[test]
fn input_schema_rejects_typos_duplicates_and_unsupported_semantics() {
    let invalid = [
        ("name: event\nfields: {}\nextra: true", "E_UNKNOWN_FIELD", "validate"),
        ("name: event\nfields: {}\nfields: {}", "E_INVALID_STRUCTURE", "parse"),
        ("name: event\nfields: {}\n---\nname: second\nfields: {}", "E_INVALID_STRUCTURE", "parse"),
        ("name: event\nfields: {amount: {name: amount, field_type: number, requird: true}}", "E_UNKNOWN_FIELD", "validate"),
        ("name: event\nfields: {amount: {name: amount, field_type: number, required: true, required: false}}", "E_INVALID_STRUCTURE", "parse"),
        ("name: event\nfields: {amount: {name: amount, field_type: number}}", "E_MISSING_FIELD", "validate"),
        ("name: event\nfields: {amount: {name: amount, field_type: number, required: 'true'}}", "E_INVALID_STRUCTURE", "validate"),
        ("name: event\nfields: {amount: {name: amount, field_type: any, required: true}}", "E_INVALID_STRUCTURE", "validate"),
        ("name: event\nfields: {amount: {name: other, field_type: number, required: true}}", "E_INPUT_SCHEMA", "type"),
        ("name: event\nfields: {amount: {name: amount, field_type: number, required: true, default: '0'}}", "E_INPUT_SCHEMA", "type"),
    ];
    let dir = setup(FILES);
    for (yaml, code, stage) in invalid {
        std::fs::write(dir.path().join("input-schema.yaml"), yaml).unwrap();
        let report = preflight(dir.path(), FILES, 1);
        let error = parse_core_input_schema(&source(dir.path(), "input-schema.yaml")).unwrap_err();
        assert_eq!(
            report["diagnostics"][0],
            serde_json::to_value(error).unwrap()
        );
        assert_eq!(report["diagnostics"][0]["source"], "input-schema.yaml");
        assert_eq!(report["diagnostics"][0]["code"], code, "{yaml}");
        assert_eq!(report["diagnostics"][0]["stage"], stage, "{yaml}");
    }
}

#[test]
fn input_diagnostic_order_is_stable_and_paths_are_json_pointers() {
    let dir = setup(FILES);
    std::fs::write(dir.path().join("input-schema.yaml"), "name: event\nfields:\n  z: {name: z, field_type: number, required: false}\n  a/b~c: {name: 'a/b~c', field_type: number, required: true}\n").unwrap();
    let first = preflight(dir.path(), FILES, 1);
    assert_eq!(first["diagnostics"][0]["field_path"], "/fields/a~1b~0c");
    for _ in 0..5 {
        assert_eq!(preflight(dir.path(), FILES, 1), first);
    }
}

#[test]
fn duplicate_paths_and_aliases_are_rejected() {
    let dir = setup(FILES);
    for alias in ["rule.yaml", "./rule.yaml"] {
        let mut files = FILES.to_vec();
        files.push(alias);
        let report = preflight(dir.path(), &files, 1);
        assert_eq!(report["diagnostics"][0]["code"], "E_DUPLICATE_SOURCE");
        assert_eq!(report["diagnostics"][0]["source"], alias);
    }
}

#[cfg(unix)]
#[test]
fn symlink_aliases_do_not_bypass_duplicate_detection() {
    let dir = setup(FILES);
    std::os::unix::fs::symlink("rule.yaml", dir.path().join("alias.yaml")).unwrap();
    let mut files = FILES.to_vec();
    files.push("alias.yaml");
    let report = preflight(dir.path(), &files, 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_DUPLICATE_SOURCE");
}

#[test]
fn long_flat_conditions_return_diagnostics_instead_of_aborting() {
    let dir = setup(FILES);
    for condition in [
        vec!["true"; 1024].join(" && "),
        vec!["false"; 1024].join(" || "),
        format!("{} > 0", vec!["1"; 1024].join(" + ")),
    ] {
        let mut rule: Value = serde_yaml::from_str(&fixture("rule.yaml")).unwrap();
        rule["rule"]["when"] = condition.into();
        std::fs::write(
            dir.path().join("rule.yaml"),
            serde_yaml::to_string(&rule).unwrap(),
        )
        .unwrap();
        let report = preflight(dir.path(), FILES, 1);
        assert!(report["diagnostics"].to_string().contains("depth"));
    }
}
