use corint_decision_compiler::core::{CoreSource, PROFILE};
use corint_decision_llm::{CoreGenerator, MockProvider, RuleGeneratorConfig};
use corint_decision_toolchain::{package, transfer};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    process::{Command, Output},
    sync::Arc,
};
use tempfile::TempDir;

const FILES: &[&str] = &[
    "rule.yaml",
    "ruleset.yaml",
    "pipeline.yaml",
    "registry.yaml",
];

#[test]
fn export_does_not_apply_filesystem_length_limits_to_valid_resource_ids() {
    let id = "long_rule_".to_owned() + &"x".repeat(500);
    let docs: Vec<_> = FILES
        .iter()
        .map(|file| {
            let mut doc = source(file);
            doc.yaml = doc.yaml.replace("large_amount", &id);
            doc
        })
        .collect();
    let mut cases = source("behavior.yaml");
    cases.yaml = cases.yaml.replace("large_amount", &id);
    let (original, _) = package::prepare(&docs, &source("input-schema.yaml"), &cases).unwrap();
    let stored = CoreSource {
        path: "long-id.json".into(),
        yaml: serde_json::to_string(&original.unwrap()).unwrap(),
    };
    let bundle = transfer::export_sources(&stored).unwrap();
    assert!(bundle.sources.iter().any(|doc| doc.path.len() > 500));
    let bundle_source = CoreSource {
        path: "editable.json".into(),
        yaml: serde_json::to_string(&bundle).unwrap(),
    };
    let (rebuilt, tests) = transfer::import_sources(&bundle_source, &cases).unwrap();
    assert_eq!(tests.passed, 5);
    assert_eq!(
        serde_json::to_value(rebuilt.unwrap()).unwrap(),
        serde_json::from_str::<Value>(&stored.yaml).unwrap()
    );
}
fn source(name: &str) -> CoreSource {
    CoreSource {
        path: name.into(),
        yaml: std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/conformance/cdl_core")
                .join(name),
        )
        .unwrap(),
    }
}
fn setup() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in FILES
        .iter()
        .copied()
        .chain(["input-schema.yaml", "behavior.yaml"])
    {
        std::fs::write(dir.path().join(name), source(name).yaml).unwrap();
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
fn report(dir: &Path, args: &[&str], exit: i32) -> Value {
    let mut args = args.to_vec();
    args.extend(["--format", "json"]);
    let output = command(dir, &args);
    assert_eq!(output.status.code(), Some(exit), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["valid"], exit == 0);
    assert_eq!(value["business_evaluation"], "not_performed");
    value
}
fn build(dir: &Path) -> Value {
    let mut args = vec![
        "build",
        "--input-schema",
        "input-schema.yaml",
        "--cases",
        "behavior.yaml",
        "--output",
        "original.json",
    ];
    args.extend(FILES);
    report(dir, &args, 0)
}
fn export(dir: &Path, package: &str, output: &str, exit: i32) -> Value {
    let result = report(
        dir,
        &["export", "--package", package, "--output", output],
        exit,
    );
    assert_eq!(result["scope"], "export");
    assert_eq!(result["execution_checked"], false);
    assert!(result.get("artifact").is_none());
    assert!(result.get("test_results").is_none());
    result
}
fn import(dir: &Path, bundle: &str, output: &str, exit: i32) -> Value {
    let result = report(
        dir,
        &[
            "import",
            "--bundle",
            bundle,
            "--cases",
            "behavior.yaml",
            "--output",
            output,
        ],
        exit,
    );
    assert_eq!(result["scope"], "import");
    result
}
fn read(dir: &Path, name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(dir.join(name)).unwrap()).unwrap()
}
fn save(dir: &Path, name: &str, value: &Value) {
    std::fs::write(dir.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn error(value: &Value, code: &str) {
    assert_eq!(value["diagnostics"][0]["code"], code, "{value}");
}

#[test]
fn cli_roundtrip_preserves_source_bytes_policy_identity_and_behavior() {
    let dir = setup();
    std::fs::write(
        dir.path().join("rule.yaml"),
        source("rule.yaml").yaml + "\n# 保留人工注释和字节顺序\n",
    )
    .unwrap();
    let built = build(dir.path());
    let exported = export(dir.path(), "original.json", "editable.json", 0);
    let receipt = &exported["exported_bundle"];
    assert_eq!(receipt["source_package_evidence"], "not_verified");
    assert_eq!(receipt["evidence_exported"], false);
    assert_eq!(receipt["publication_approval"], "not_granted");
    assert_eq!(
        receipt["bundle_sha256"],
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(dir.path().join("editable.json")).unwrap())
        )
    );
    let original = read(dir.path(), "original.json");
    let editable = read(dir.path(), "editable.json");
    assert!(editable.get("evidence").is_none());
    for doc in editable["sources"].as_array().unwrap() {
        let prior = original["policy"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["path"] == doc["path"])
            .unwrap();
        assert_eq!(doc["yaml"], prior["yaml"]);
    }
    assert_eq!(
        editable["input_schema"]["yaml"],
        original["policy"]["input_schema"]["yaml"]
    );
    let imported = import(dir.path(), "editable.json", "rebuilt.json", 0);
    assert_eq!(imported["test_results"], built["test_results"]);
    assert_eq!(
        imported["artifact"]["policy_sha256"],
        built["artifact"]["policy_sha256"]
    );
    assert_eq!(
        std::fs::read(dir.path().join("original.json")).unwrap(),
        std::fs::read(dir.path().join("rebuilt.json")).unwrap()
    );
    export(dir.path(), "rebuilt.json", "again.json", 0);
    assert_eq!(
        std::fs::read(dir.path().join("editable.json")).unwrap(),
        std::fs::read(dir.path().join("again.json")).unwrap()
    );
    report(
        dir.path(),
        &[
            "verify",
            "--package",
            "rebuilt.json",
            "--cases",
            "behavior.yaml",
        ],
        0,
    );
    assert!(!dir.path().join("rule").exists()); // Labels never become directories.
}

#[test]
fn every_core_negative_fixture_retains_shared_diagnostics_on_import() {
    let dir = setup();
    let manifest: Value = serde_yaml::from_str(&source("manifest.yaml").yaml).unwrap();
    // Preserve the original 24 rejections plus five new Pipeline boundary cases;
    // future manifest additions must also run through this adapter.
    assert!(manifest["invalid"].as_array().unwrap().len() >= 29);
    for case in manifest["invalid"].as_array().unwrap() {
        let docs: Vec<_> = FILES.iter().map(|f| source(f)).collect();
        let mut bundle = transfer::SourceBundle::new(source("input-schema.yaml"), docs).unwrap();
        let doc = bundle
            .sources
            .iter_mut()
            .find(|s| s.path == case["document"].as_str().unwrap())
            .unwrap();
        let find = case["find"].as_str().unwrap();
        assert_eq!(doc.yaml.matches(find).count(), 1);
        doc.yaml = doc
            .yaml
            .replacen(find, case["replace"].as_str().unwrap(), 1);
        let expected = corint_decision_compiler::core::compile_core(
            &bundle.sources,
            corint_decision_compiler::core::parse_core_input_schema(&bundle.input_schema).unwrap(),
        )
        .unwrap_err();
        save(
            dir.path(),
            "bad.json",
            &serde_json::to_value(&bundle).unwrap(),
        );
        let result = import(dir.path(), "bad.json", "rejected.json", 1);
        assert_eq!(
            result["diagnostics"][0],
            serde_json::to_value(expected).unwrap(),
            "{case}"
        );
        assert_eq!(result["execution_checked"], false);
        assert!(!dir.path().join("rejected.json").exists());
    }
}

#[test]
fn relocated_reordered_and_relabelled_bundle_does_not_load_external_files() {
    let dir = setup();
    let built = build(dir.path());
    export(dir.path(), "original.json", "editable.json", 0);
    let mut bundle = read(dir.path(), "editable.json");
    bundle["sources"].as_array_mut().unwrap().reverse();
    bundle["input_schema"]["path"] = json!("renamed-input.yaml");
    for (i, doc) in bundle["sources"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        doc["path"] = json!(format!("nonexistent/source_{i}.yaml"));
    }
    let relocated = tempfile::tempdir().unwrap();
    save(relocated.path(), "moved.json", &bundle);
    std::fs::write(
        relocated.path().join("behavior.yaml"),
        source("behavior.yaml").yaml,
    )
    .unwrap();
    let result = import(relocated.path(), "moved.json", "new.json", 0);
    assert_eq!(result["test_results"], built["test_results"]);
    assert_eq!(
        result["artifact"]["policy_sha256"],
        built["artifact"]["policy_sha256"]
    );
    assert_eq!(std::fs::read_dir(relocated.path()).unwrap().count(), 3);
    assert!(!relocated.path().join("nonexistent").exists());
}

#[test]
fn bundle_schema_and_capability_inventory_match_the_implemented_gate() {
    let schema: Value = serde_json::from_str(transfer::BUNDLE_SCHEMA).unwrap();
    let validator = jsonschema::JSONSchema::compile(&schema).unwrap();
    let bundle = transfer::SourceBundle::new(
        source("input-schema.yaml"),
        FILES.iter().map(|f| source(f)).collect(),
    )
    .unwrap();
    let value = serde_json::to_value(bundle).unwrap();
    assert!(validator.is_valid(&value));
    let inventory: Value =
        serde_json::from_str(include_str!("../../../docs/cdl/schema/capabilities.json")).unwrap();
    assert_eq!(inventory["source_bundle_schema"], "source-bundle.json");
    let tool = &inventory["tools"]["source_exchange_cli"];
    assert_eq!(tool["commands"], json!(["corint export", "corint import"]));
    assert_eq!(tool["historical_evidence_transferred"], false);
    assert_eq!(
        tool["evidence"],
        "../../../crates/corint-decision-cli/tests/transfer.rs"
    );
    let dir = setup();
    let mut bad = value.clone();
    bad["sources"] = json!(vec![value["sources"][0].clone(); 10001]);
    save(dir.path(), "oversized.json", &bad);
    error(
        &import(dir.path(), "oversized.json", "rejected.json", 1),
        "E_BUNDLE_FORMAT",
    );
    bad = value;
    bad["sources"][0]["path"] = bad["input_schema"]["path"].clone();
    save(dir.path(), "collision.json", &bad);
    error(
        &import(dir.path(), "collision.json", "rejected.json", 1),
        "E_DUPLICATE_SOURCE",
    );
}

#[test]
fn invalid_cases_and_output_failures_never_leave_partial_artifacts() {
    let dir = setup();
    build(dir.path());
    export(dir.path(), "original.json", "editable.json", 0);
    std::fs::write(dir.path().join("behavior.yaml"), "broken: true").unwrap();
    let result = import(dir.path(), "editable.json", "rejected.json", 1);
    error(&result, "E_TEST_SUITE");
    assert_eq!(result["execution_checked"], false);
    assert!(!dir.path().join("rejected.json").exists());
    error(
        &export(dir.path(), "original.json", "missing/out.json", 2),
        "E_IO",
    );
    assert!(!dir.path().join("missing").exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 8);
}

#[test]
fn actual_generator_host_and_cli_exchange_sources_not_old_evidence() {
    let dir = setup();
    let docs: Vec<_> = FILES.iter().map(|f| source(f)).collect();
    let generator = CoreGenerator::new(
        Arc::new(MockProvider::with_response(
            json!({"profile":PROFILE,"sources":docs}).to_string(),
        )),
        RuleGeneratorConfig::new("offline-mock"),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let generated = runtime
        .block_on(generator.generate(
            "Decline amounts over 1000",
            &source("input-schema.yaml"),
            &source("behavior.yaml"),
        ))
        .unwrap();
    let old = generated.package.unwrap();
    package::write(&old, &dir.path().join("generator.json")).unwrap();
    let denied = report(
        dir.path(),
        &[
            "verify",
            "--package",
            "generator.json",
            "--cases",
            "behavior.yaml",
        ],
        1,
    );
    error(&denied, "E_TOOL_MISMATCH");
    export(dir.path(), "generator.json", "editable.json", 0);
    let imported = import(dir.path(), "editable.json", "cli.json", 0);
    assert_eq!(
        imported["test_results"],
        serde_json::to_value(generated.tests).unwrap()
    );
    let new = read(dir.path(), "cli.json");
    let old = read(dir.path(), "generator.json");
    assert_eq!(new["policy"], old["policy"]);
    assert_ne!(
        new["evidence"]["tool"]["executable_sha256"],
        old["evidence"]["tool"]["executable_sha256"]
    );
    report(
        dir.path(),
        &[
            "verify",
            "--package",
            "cli.json",
            "--cases",
            "behavior.yaml",
        ],
        0,
    );
    // Reverse direction uses the public library, again rebuilding under this test host.
    let snapshot = transfer::export_sources(&CoreSource {
        path: "cli.json".into(),
        yaml: new.to_string(),
    })
    .unwrap();
    let (back, tests) = transfer::import_sources(
        &CoreSource {
            path: "back.json".into(),
            yaml: serde_json::to_string(&snapshot).unwrap(),
        },
        &source("behavior.yaml"),
    )
    .unwrap();
    assert_eq!(tests.passed, 5);
    assert_eq!(serde_json::to_value(back.unwrap()).unwrap(), old);
}

#[test]
fn explicit_edits_require_new_behavior_evidence_and_do_not_change_originals() {
    let dir = setup();
    build(dir.path());
    export(dir.path(), "original.json", "editable.json", 0);
    let original = std::fs::read(dir.path().join("original.json")).unwrap();
    let mut bundle = read(dir.path(), "editable.json");
    let rule = bundle["sources"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["path"] == "rule/large_amount.yaml")
        .unwrap();
    rule["yaml"] = json!(rule["yaml"].as_str().unwrap().replace("> 1000", "> 500"));
    save(dir.path(), "modified.json", &bundle);
    let failed = import(dir.path(), "modified.json", "failed.json", 1);
    assert_eq!(failed["execution_checked"], true);
    assert_eq!(failed["test_results"]["failed"], 2);
    assert!(!dir.path().join("failed.json").exists());
    assert!(failed.get("artifact").is_none());
    let revised_cases = source("behavior.yaml")
        .yaml
        .replace("1001", "501")
        .replace("1000", "500")
        .replace("999", "499");
    std::fs::write(dir.path().join("behavior.yaml"), revised_cases).unwrap();
    let passed = import(dir.path(), "modified.json", "changed.json", 0);
    assert_eq!(passed["test_results"]["passed"], 5);
    assert_ne!(
        read(dir.path(), "changed.json")["policy"]["sha256"],
        read(dir.path(), "original.json")["policy"]["sha256"]
    );
    assert_eq!(
        std::fs::read(dir.path().join("original.json")).unwrap(),
        original
    );
}

#[test]
fn stale_or_fabricated_report_is_not_transferred_as_trusted_evidence() {
    let dir = setup();
    build(dir.path());
    let mut forged = read(dir.path(), "original.json");
    forged["evidence"]["test_report_sha256"] = json!("f".repeat(64));
    forged["evidence"]["tool"]["executable_sha256"] = json!("0".repeat(64));
    save(dir.path(), "old-evidence.json", &forged);
    let result = export(dir.path(), "old-evidence.json", "editable.json", 0);
    assert_eq!(
        result["exported_bundle"]["source_package_evidence"],
        "not_verified"
    );
    let rebuilt = import(dir.path(), "editable.json", "new.json", 0);
    assert_eq!(rebuilt["test_results"]["executed"], 5);
    assert_eq!(
        read(dir.path(), "new.json"),
        read(dir.path(), "original.json")
    );
    let mut tampered = read(dir.path(), "original.json");
    tampered["policy"]["sources"][0]["yaml"] = json!("malicious: true");
    save(dir.path(), "tampered.json", &tampered);
    error(
        &export(dir.path(), "tampered.json", "bad.json", 1),
        "E_PACKAGE_INTEGRITY",
    );
    assert!(!dir.path().join("bad.json").exists());
}

#[test]
fn bundle_cannot_smuggle_evidence_or_drop_dependencies() {
    let dir = setup();
    build(dir.path());
    export(dir.path(), "original.json", "editable.json", 0);
    let original = read(dir.path(), "editable.json");
    for field in ["evidence", "approval", "imports", "tool", "validated"] {
        let mut bad = original.clone();
        bad[field] = json!({"passed":true});
        save(dir.path(), "bad.json", &bad);
        error(
            &import(dir.path(), "bad.json", "rejected.json", 1),
            "E_BUNDLE_FORMAT",
        );
        assert!(!dir.path().join("rejected.json").exists());
    }
    let mut missing = original.clone();
    missing["sources"]
        .as_array_mut()
        .unwrap()
        .retain(|s| s["path"] != "rule/large_amount.yaml");
    save(dir.path(), "bad.json", &missing);
    error(
        &import(dir.path(), "bad.json", "rejected.json", 1),
        "E_UNRESOLVED_REF",
    );
    let mut duplicate = original;
    duplicate["sources"][1]["path"] = duplicate["sources"][0]["path"].clone();
    save(dir.path(), "bad.json", &duplicate);
    error(
        &import(dir.path(), "bad.json", "rejected.json", 1),
        "E_DUPLICATE_SOURCE",
    );
}

#[test]
fn export_and_import_never_overwrite_existing_targets() {
    let dir = setup();
    build(dir.path());
    export(dir.path(), "original.json", "editable.json", 0);
    let sentinel = b"user data";
    std::fs::write(dir.path().join("existing.json"), sentinel).unwrap();
    error(
        &export(dir.path(), "original.json", "existing.json", 2),
        "E_OUTPUT_EXISTS",
    );
    error(
        &import(dir.path(), "editable.json", "existing.json", 2),
        "E_OUTPUT_EXISTS",
    );
    assert_eq!(
        std::fs::read(dir.path().join("existing.json")).unwrap(),
        sentinel
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("absent.json", dir.path().join("link.json")).unwrap();
        error(
            &export(dir.path(), "original.json", "link.json", 2),
            "E_OUTPUT_EXISTS",
        );
        error(
            &import(dir.path(), "editable.json", "link.json", 2),
            "E_OUTPUT_EXISTS",
        );
        assert!(!dir.path().join("absent.json").exists());
    }
}

#[test]
fn malformed_bundle_versions_duplicate_keys_and_unsafe_labels_fail_closed() {
    let dir = setup();
    build(dir.path());
    export(dir.path(), "original.json", "editable.json", 0);
    let original = read(dir.path(), "editable.json");
    for (field, value) in [
        ("format_version", json!("2")),
        ("profile", json!("next")),
        ("language_version", json!("0.2")),
        ("sources", json!([])),
    ] {
        let mut bad = original.clone();
        bad[field] = value;
        save(dir.path(), "bad.json", &bad);
        error(
            &import(dir.path(), "bad.json", "rejected.json", 1),
            "E_BUNDLE_FORMAT",
        );
    }
    for path in [
        "../escape.yaml",
        "/tmp/escape.yaml",
        "a/../b.yaml",
        "https://host/p.yaml",
        "a\\b.yaml",
    ] {
        let mut bad = original.clone();
        bad["sources"][0]["path"] = json!(path);
        save(dir.path(), "bad.json", &bad);
        error(
            &import(dir.path(), "bad.json", "rejected.json", 1),
            "E_BUNDLE_FORMAT",
        );
    }
    for raw in [
        format!("{{\"format\":\"bad\",{}", &original.to_string()[1..]),
        format!("{} trailing", original),
        format!("{}{}", original, original),
    ] {
        std::fs::write(dir.path().join("bad.json"), raw).unwrap();
        error(
            &import(dir.path(), "bad.json", "rejected.json", 1),
            "E_BUNDLE_FORMAT",
        );
    }
    assert!(!dir.path().join("rejected.json").exists());
}

#[test]
fn command_grammar_and_io_errors_are_machine_readable() {
    let dir = setup();
    for args in [
        vec!["export"],
        vec!["import"],
        vec!["export", "--package", "p", "--output", "x", "--cases", "c"],
        vec!["import", "--bundle", "b", "--output", "x"],
        vec![
            "import",
            "--bundle",
            "b",
            "--cases",
            "c",
            "--output",
            "x",
            "extra.yaml",
        ],
        vec![
            "export",
            "--package",
            "p",
            "--package",
            "q",
            "--output",
            "x",
        ],
        vec![
            "import", "--bundle", "b", "--bundle", "c", "--cases", "c", "--output", "x",
        ],
        vec![
            "import",
            "--bundle",
            "b",
            "--cases",
            "c",
            "--output",
            "x",
            "--input-schema",
            "s",
        ],
        vec!["export", "--bundle", "b", "--output", "x"],
    ] {
        error(&report(dir.path(), &args, 2), "E_USAGE");
    }
    error(&export(dir.path(), "absent.json", "out.json", 2), "E_IO");
    error(&import(dir.path(), "absent.json", "out.json", 2), "E_IO");
    for cmd in ["export", "import"] {
        let output = command(dir.path(), &[cmd, "--help"]);
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("fresh evidence"));
    }
}
