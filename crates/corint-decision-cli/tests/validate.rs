//! Exercise the shipped binary from isolated working directories, using the same
//! positive/negative policy fixtures as the real-engine conformance suite.
use corint_decision_compiler::core::{compile_core, parse_core_input_schema, CoreSource, PROFILE};
use serde::Deserialize;
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

#[derive(Deserialize)]
struct Manifest {
    cases: Vec<Case>,
    invalid: Vec<Invalid>,
}
#[derive(Deserialize)]
struct Case {
    documents: Vec<String>,
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

#[test]
fn cli_capability_and_input_artifacts_are_in_sync() {
    let capabilities: Value =
        serde_json::from_str(include_str!("../../../docs/cdl/schema/capabilities.json")).unwrap();
    assert_eq!(capabilities["profile"], PROFILE);
    assert_eq!(capabilities["input_schema"], "input.json");
    let tool = &capabilities["tools"]["validate_cli"];
    assert_eq!(tool["entry_point"], "compile_core");
    assert_eq!(tool["scope"], "compile_only");
    assert_eq!(tool["execution_checked"], false);
    assert_eq!(tool["business_evaluation"], "not_performed");
    let evidence = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/cdl/schema")
        .join(tool["evidence"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    assert_eq!(
        evidence,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/validate.rs")
            .canonicalize()
            .unwrap()
    );
    let public_schema: Value =
        serde_json::from_str(corint_decision_compiler::core::CORE_INPUT_SCHEMA).unwrap();
    assert_eq!(public_schema["additionalProperties"], false);
    assert_eq!(
        public_schema["definitions"]["field"]["additionalProperties"],
        false
    );
    assert_eq!(
        public_schema["definitions"]["field"]["properties"]["field_type"]["enum"],
        json!(["number", "string", "boolean"])
    );
}

fn manifest() -> Manifest {
    serde_yaml::from_str(&fixture("manifest.yaml")).unwrap()
}

fn setup(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in files.iter().copied().chain(["input-schema.yaml"]) {
        std::fs::write(dir.path().join(file), fixture(file)).unwrap();
    }
    dir
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_corint"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn json_report(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).expect("stdout is one JSON document");
    assert_eq!(value["report_version"], "1");
    assert_eq!(value["tool_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["profile"], PROFILE);
    assert_eq!(value["scope"], "compile");
    assert_eq!(value["valid"], code == 0);
    assert_eq!(value["execution_checked"], false);
    assert_eq!(value["business_evaluation"], "not_performed");
    assert_eq!(
        value["diagnostics"].as_array().unwrap().len(),
        usize::from(code != 0)
    );
    value
}

fn validate(dir: &Path, files: &[&str], code: i32) -> Value {
    let mut args = vec![
        "validate",
        "--input-schema",
        "input-schema.yaml",
        "--format",
        "json",
    ];
    args.extend(files);
    json_report(&run(dir, &args), code)
}

fn source(dir: &Path, name: &str) -> CoreSource {
    CoreSource {
        path: name.into(),
        yaml: std::fs::read_to_string(dir.join(name)).unwrap(),
    }
}

#[test]
fn complete_bundles_compile_without_work_and_do_not_modify_sources() {
    for case in manifest().cases {
        let files: Vec<_> = case.documents.iter().map(String::as_str).collect();
        let dir = setup(&files);
        let schema = parse_core_input_schema(&source(dir.path(), "input-schema.yaml")).unwrap();
        let sources: Vec<_> = files.iter().map(|file| source(dir.path(), file)).collect();
        let compiled = compile_core(&sources, schema).unwrap();
        assert!(!compiled.programs.is_empty());
        assert_eq!(
            compiled.registry_guards.len(),
            compiled.registry.registry.len()
        );
        let report = validate(dir.path(), &files, 0);
        assert_eq!(report["sources"], json!(files));
        assert_eq!(report["input_schema"], "input-schema.yaml");
        for src in sources {
            assert_eq!(
                std::fs::read_to_string(dir.path().join(src.path)).unwrap(),
                src.yaml
            );
        }
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            files.len() + 1
        );
    }
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
        let report = validate(dir.path(), FILES, 1);
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
        let report = validate(dir.path(), files, 1);
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
        ("name: event\nfields: {amount: {name: amount, field_type: number, required: false}}", "E_UNSUPPORTED_CAPABILITY", "type"),
        ("name: event\nfields: {amount: {name: other, field_type: number, required: true}}", "E_UNSUPPORTED_CAPABILITY", "type"),
        ("name: event\nfields: {amount: {name: amount, field_type: number, required: true, default: '0'}}", "E_UNSUPPORTED_CAPABILITY", "type"),
    ];
    let dir = setup(FILES);
    for (yaml, code, stage) in invalid {
        std::fs::write(dir.path().join("input-schema.yaml"), yaml).unwrap();
        let report = validate(dir.path(), FILES, 1);
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
fn json_schema_input_and_all_scalar_types_are_supported() {
    let dir = setup(FILES);
    let mut input: Value = serde_yaml::from_str(&fixture("input-schema.yaml")).unwrap();
    input["description"] = json!("synthetic local context only");
    input["fields"]["flag"] =
        json!({"name": "flag", "field_type": "boolean", "required": true, "default": null});
    input["fields"]["country"] =
        json!({"name": "country", "field_type": "string", "required": true});
    std::fs::write(dir.path().join("input-schema.yaml"), input.to_string()).unwrap();
    validate(dir.path(), FILES, 0);
}

#[test]
fn input_diagnostic_order_is_stable_and_paths_are_json_pointers() {
    let dir = setup(FILES);
    std::fs::write(dir.path().join("input-schema.yaml"), "name: event\nfields:\n  z: {name: z, field_type: number, required: false}\n  a/b~c: {name: 'a/b~c', field_type: number, required: true}\n").unwrap();
    let first = validate(dir.path(), FILES, 1);
    assert_eq!(first["diagnostics"][0]["field_path"], "/fields/a~1b~0c");
    for _ in 0..5 {
        assert_eq!(validate(dir.path(), FILES, 1), first);
    }
}

#[test]
fn usage_errors_are_json_and_have_exit_code_two() {
    let dir = setup(FILES);
    for args in [
        vec!["unknown", "--format", "json"],
        vec!["validate", "--format", "json", "rule.yaml"],
        vec![
            "validate",
            "--format",
            "json",
            "--input-schema",
            "input-schema.yaml",
        ],
        vec!["validate", "--format", "json", "--input-schema"],
        vec!["validate", "--format", "json", "--input-schema", "--oops"],
        vec!["validate", "--format", "json", "--unknown"],
        vec!["validate", "--format", "json", "--format", "text"],
        vec![
            "validate",
            "--format",
            "json",
            "--input-schema",
            "input-schema.yaml",
            "--input-schema",
            "input-schema.yaml",
        ],
    ] {
        let report = json_report(&run(dir.path(), &args), 2);
        assert_eq!(report["diagnostics"][0]["code"], "E_USAGE");
        assert_eq!(report["diagnostics"][0]["stage"], "usage");
    }
    for args in [
        vec![],
        vec!["validate", "--format", "xml"],
        vec!["validate", "--format"],
    ] {
        let output = run(dir.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("E_USAGE"));
    }
}

#[test]
fn io_errors_are_not_policy_errors_or_panics() {
    let dir = setup(FILES);
    std::fs::write(dir.path().join("invalid-utf8.yaml"), [0xff, 0xfe]).unwrap();
    for path in ["missing.yaml", ".", "invalid-utf8.yaml"] {
        let report = validate(dir.path(), &[path], 2);
        assert_eq!(report["diagnostics"][0]["code"], "E_IO");
        assert_eq!(report["diagnostics"][0]["source"], path);
    }
    let report = json_report(
        &run(
            dir.path(),
            &[
                "validate",
                "--input-schema",
                "missing.yaml",
                "--format",
                "json",
                "rule.yaml",
            ],
        ),
        2,
    );
    assert_eq!(report["diagnostics"][0]["source"], "missing.yaml");
}

#[test]
fn duplicate_paths_and_aliases_are_rejected() {
    let dir = setup(FILES);
    for alias in ["rule.yaml", "./rule.yaml"] {
        let mut files = FILES.to_vec();
        files.push(alias);
        let report = validate(dir.path(), &files, 1);
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
    let report = validate(dir.path(), &files, 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_DUPLICATE_SOURCE");
}

#[test]
fn paths_with_spaces_and_dash_prefixes_are_files_not_options() {
    let dir = setup(FILES);
    for name in ["-rule.yaml", "a rule.yaml", "规则.yaml"] {
        std::fs::copy(dir.path().join("rule.yaml"), dir.path().join(name)).unwrap();
        json_report(
            &run(
                dir.path(),
                &[
                    "validate",
                    "--format",
                    "json",
                    "--input-schema",
                    "input-schema.yaml",
                    "--",
                    name,
                    "ruleset.yaml",
                    "pipeline.yaml",
                    "registry.yaml",
                ],
            ),
            0,
        );
    }
    // After -- these are literal filenames, not a request for JSON output.
    let output = run(
        dir.path(),
        &[
            "validate",
            "--input-schema",
            "input-schema.yaml",
            "--",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .starts_with("INVALID"));
}

#[test]
fn human_output_help_and_version_have_no_validation_claims() {
    let dir = setup(FILES);
    for args in [vec!["--help"], vec!["-h"], vec!["validate", "--help"]] {
        let output = run(dir.path(), &args);
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout).unwrap().contains("Usage:"));
    }
    let version = run(dir.path(), &["--version"]);
    assert!(version.status.success());
    assert!(String::from_utf8(version.stdout).unwrap().contains(PROFILE));
    let mut args = vec!["validate", "--input-schema", "input-schema.yaml"];
    args.extend(FILES);
    let output = run(dir.path(), &args);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("VALID"));
    assert!(text.contains("Execution not checked"));
    assert!(text.contains("not a publication approval"));
    std::fs::write(dir.path().join("rule.yaml"), "version: [").unwrap();
    let output = run(dir.path(), &args);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("E_INVALID_STRUCTURE [parse] rule.yaml:"));
}
