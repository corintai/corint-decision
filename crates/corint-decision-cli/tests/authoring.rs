//! Full CDL syntax checks through the shipped CLI, with no server or credentials.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const FILES: &[&str] = &[
    "features/payment.yaml",
    "lists/blocked.yaml",
    "services/risk.yaml",
    "rules/blocked.yaml",
    "rulesets/payment.yaml",
    "pipelines/payment.yaml",
    "registry.yaml",
    "input-schema.yaml",
];
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_authoring")
}
fn setup() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in FILES {
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(fixtures().join(name), path).unwrap();
    }
    dir
}
fn run(dir: &Path, args: &[&str], code: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_corint"))
        .current_dir(dir)
        .args(["validate", "--format", "json"])
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["profile"], "cdl-static-1");
    assert_eq!(report["scope"], "static");
    assert_eq!(report["execution_checked"], false);
    assert_eq!(report["valid"], code == 0);
    report
}
fn mutate(dir: &Path, file: &str, from: &str, to: &str) {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from));
    std::fs::write(path, text.replacen(from, to, 1)).unwrap();
}
fn has(report: &Value, code: &str) {
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == code),
        "{report:#}"
    );
}
#[test]
fn all_seven_resources_and_four_feature_types_validate_offline() {
    let dir = setup();
    let report = run(
        dir.path(),
        &["--root", ".", "--input-schema", "input-schema.yaml"],
        0,
    );
    assert_eq!(report["references_checked"], true);
    assert_eq!(report["input_schema_checked"], true);
    assert_eq!(report["sources"].as_array().unwrap().len(), 7);
    let capabilities: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/capabilities.json"
    ))
    .unwrap();
    assert_eq!(
        capabilities["tools"]["validate_cli"]["profile"],
        report["profile"]
    );
}
#[test]
fn individual_resources_do_not_require_a_registry_or_schema() {
    let dir = setup();
    for file in &FILES[..7] {
        let report = run(dir.path(), &[file], 0);
        assert_eq!(report["references_checked"], false);
        assert_eq!(report["input_schema_checked"], false);
    }
}
#[test]
fn static_success_does_not_admit_extensions_to_core() {
    let dir = setup();
    let out = Command::new(env!("CARGO_BIN_EXE_corint"))
        .current_dir(dir.path())
        .args([
            "validate",
            "--profile",
            "cdl-core-risk-draft-1",
            "--format",
            "json",
            "--input-schema",
            "input-schema.yaml",
            "features/payment.yaml",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["profile"], "cdl-core-risk-draft-1");
}
#[test]
fn schema_errors_have_actionable_paths_without_echoing_service_secrets() {
    let dir = setup();
    mutate(
        dir.path(),
        "services/risk.yaml",
        "    method: POST",
        "    secret_typo: do-not-print-this-secret\n    method: POST",
    );
    let report = run(dir.path(), &["services/risk.yaml"], 1);
    has(&report, "E_UNKNOWN_FIELD");
    assert!(!report.to_string().contains("do-not-print-this-secret"));
    assert_eq!(report["diagnostics"][0]["field_path"], "/operations/assess");
}
#[test]
fn invalid_resource_structures_expressions_types_and_templates_fail() {
    for (file, from, to, code) in [
        (
            "features/payment.yaml",
            "window: 1h",
            "window: yesterday",
            "E_INVALID_FEATURE",
        ),
        (
            "features/payment.yaml",
            "method: count",
            "method: typo",
            "E_INVALID_STRUCTURE",
        ),
        (
            "features/payment.yaml",
            "type: lookup",
            "tyep: lookup",
            "E_MISSING_FIELD",
        ),
        (
            "features/payment.yaml",
            "features.count_1h + max(event.amount, 1)",
            "features.count_1h +",
            "E_INVALID_EXPRESSION",
        ),
        (
            "features/payment.yaml",
            "${event.customer_id}",
            "${event.customer_id",
            "E_INVALID_TEMPLATE",
        ),
        (
            "rules/blocked.yaml",
            "features.adjusted_count > 100",
            "event.amount > 'large'",
            "E_TYPE_MISMATCH",
        ),
        (
            "rules/blocked.yaml",
            "features.adjusted_count > 100",
            "event.typo > 100",
            "E_UNKNOWN_FIELD",
        ),
        (
            "rules/blocked.yaml",
            "features.adjusted_count > 100",
            "event.amount >",
            "E_INVALID_EXPRESSION",
        ),
        (
            "rules/blocked.yaml",
            "score: 100",
            "score: 1.5",
            "E_INVALID_STRUCTURE",
        ),
        (
            "lists/blocked.yaml",
            "backend: memory",
            "backend: file",
            "E_MISSING_FIELD",
        ),
        (
            "lists/blocked.yaml",
            "backend: memory",
            "backend: memory\ncache_ttl: 1.0e30",
            "E_INVALID_STRUCTURE",
        ),
        (
            "services/risk.yaml",
            "https://risk.example.invalid/v1",
            "file:///tmp/a",
            "E_INVALID_SERVICE",
        ),
        (
            "services/risk.yaml",
            "method: POST",
            "method: GET",
            "E_INVALID_SERVICE",
        ),
        (
            "services/risk.yaml",
            "/customers/{customer_id}/assess",
            "/customers/{customer_id/assess",
            "E_INVALID_TEMPLATE",
        ),
        (
            "services/risk.yaml",
            "${amount}",
            "prefix${amount}",
            "E_INVALID_TEMPLATE",
        ),
        (
            "pipelines/payment.yaml",
            "output: vars.customer_risk",
            "output: event.customer_risk",
            "E_INVALID_STRUCTURE",
        ),
        (
            "pipelines/payment.yaml",
            "event.amount + 1",
            "event.amount +",
            "E_INVALID_EXPRESSION",
        ),
    ] {
        let dir = setup();
        mutate(dir.path(), file, from, to);
        let report = run(
            dir.path(),
            &["--root", ".", "--input-schema", "input-schema.yaml"],
            1,
        );
        has(&report, code);
    }
}
#[test]
fn missing_references_duplicate_ids_and_graph_cycles_fail() {
    for (file, from, to, code) in [
        (
            "rulesets/payment.yaml",
            "rules: [blocked]",
            "rules: [missing]",
            "E_UNRESOLVED_REFERENCE",
        ),
        (
            "rules/blocked.yaml",
            "list.blocked_customers",
            "list.missing",
            "E_UNRESOLVED_REFERENCE",
        ),
        (
            "features/payment.yaml",
            "features.count_1h +",
            "features.missing +",
            "E_UNRESOLVED_REFERENCE",
        ),
        (
            "features/payment.yaml",
            "features.count_1h +",
            "features.adjusted_count +",
            "E_CYCLE",
        ),
        (
            "pipelines/payment.yaml",
            "operation: assess",
            "operation: missing",
            "E_UNKNOWN_OPERATION",
        ),
        (
            "pipelines/payment.yaml",
            "next: end",
            "next: missing",
            "E_UNKNOWN_STEP",
        ),
        (
            "pipelines/payment.yaml",
            "next: end",
            "next: assess",
            "E_CYCLE",
        ),
        (
            "pipelines/payment.yaml",
            "id: check",
            "id: assess",
            "E_DUPLICATE_ID",
        ),
    ] {
        let dir = setup();
        mutate(dir.path(), file, from, to);
        has(&run(dir.path(), &["--root", "."], 1), code);
    }
    let dir = setup();
    std::fs::copy(
        dir.path().join("lists/blocked.yaml"),
        dir.path().join("lists/duplicate.yaml"),
    )
    .unwrap();
    has(&run(dir.path(), &["--root", "."], 1), "E_DUPLICATE_ID");
}
#[test]
fn yaml_duplicates_multiple_bodies_and_unknown_versions_fail() {
    for text in [
        "rule: {}\nrule: {}",
        "rule: {}\n---\nrule: {}",
        "version: '99'\nrule: {id: r, name: R, when: 'true', score: 1}",
    ] {
        let dir = setup();
        std::fs::write(dir.path().join("bad.yaml"), text).unwrap();
        let report = run(dir.path(), &["bad.yaml"], 1);
        assert!(!report["diagnostics"].as_array().unwrap().is_empty());
    }
    let dir = setup();
    std::fs::write(dir.path().join("bad.yaml"), "a: [\n").unwrap();
    let report = run(dir.path(), &["bad.yaml"], 1);
    assert!(report["diagnostics"][0]["line"].is_number());
}
#[test]
fn imports_resolve_all_resource_kinds_and_support_header_documents() {
    let dir = setup();
    let imports="import:\n  features: [features/payment.yaml]\n  lists: [lists/blocked.yaml]\n  services: [services/risk.yaml]\n  rules: [rules/blocked.yaml]\n  rulesets: [rulesets/payment.yaml]\n  pipelines: [pipelines/payment.yaml]\n---\n";
    let registry = std::fs::read_to_string(dir.path().join("registry.yaml")).unwrap();
    std::fs::write(
        dir.path().join("registry.yaml"),
        format!("{imports}{registry}"),
    )
    .unwrap();
    let report = run(dir.path(), &["--root", ".", "registry.yaml"], 0);
    assert_eq!(report["sources"].as_array().unwrap().len(), 7);
    let single = run(dir.path(), &["registry.yaml"], 0);
    assert_eq!(single["sources"].as_array().unwrap().len(), 1);
    assert_eq!(single["references_checked"], false);
    mutate(
        dir.path(),
        "registry.yaml",
        "features/payment.yaml",
        "lists/blocked.yaml",
    );
    has(
        &run(dir.path(), &["--root", ".", "registry.yaml"], 1),
        "E_IMPORT_KIND",
    );
}
#[test]
fn circular_missing_and_escaping_imports_report_errors() {
    let dir = setup();
    mutate(
        dir.path(),
        "registry.yaml",
        "version: \"0.1\"",
        "import: {pipelines: [registry.yaml]}\nversion: \"0.1\"",
    );
    has(
        &run(dir.path(), &["--root", ".", "registry.yaml"], 1),
        "E_IMPORT_CYCLE",
    );
    mutate(
        dir.path(),
        "registry.yaml",
        "[registry.yaml]",
        "[absent.yaml]",
    );
    has(
        &run(dir.path(), &["--root", ".", "registry.yaml"], 2),
        "E_READ",
    );
    let outside = tempfile::NamedTempFile::new().unwrap();
    mutate(
        dir.path(),
        "registry.yaml",
        "[absent.yaml]",
        &format!("['{}']", outside.path().display()),
    );
    has(
        &run(dir.path(), &["--root", ".", "registry.yaml"], 2),
        "E_PATH",
    );
}
#[test]
fn service_checks_do_not_contact_the_declared_endpoint() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let dir = setup();
    mutate(
        dir.path(),
        "services/risk.yaml",
        "https://risk.example.invalid/v1",
        &format!("http://{}/v1", listener.local_addr().unwrap()),
    );
    run(dir.path(), &["--root", "."], 0);
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
#[test]
fn documented_online_examples_pass_the_static_language_gate() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CDL/examples/payment-review/online");
    for file in [
        "features.yaml",
        "prepare.yaml",
        "markers.yaml",
        "customer-risk.yaml",
        "blocked-email.yaml",
        "loyal-customer.yaml",
    ] {
        run(&root, &[file], 0);
    }
}
#[test]
fn usage_and_io_errors_always_return_json_and_exit_two() {
    let dir = setup();
    for args in [
        vec![],
        vec!["--root"],
        vec!["--profile", "unknown"],
        vec!["--typo"],
        vec!["--format", "text"],
        vec!["missing.yaml"],
        vec!["--input-schema", "missing.yaml", "rules/blocked.yaml"],
    ] {
        run(dir.path(), &args, 2);
    }
}

#[test]
fn inheritance_and_conclusion_annotations_are_full_cdl_syntax() {
    let dir = setup();
    std::fs::write(dir.path().join("rulesets/base.yaml"), "version: '0.1'\nruleset:\n  id: base\n  rules: [blocked]\n  conclusion:\n    - default: true\n      signal: pass\n      reason: Base policy\n      actions: [notify]\n").unwrap();
    std::fs::write(
        dir.path().join("rulesets/payment.yaml"),
        "version: '0.1'\nruleset:\n  id: payment\n  extends: base\n",
    )
    .unwrap();
    run(dir.path(), &["--root", "."], 0);
    mutate(
        dir.path(),
        "rulesets/base.yaml",
        "rules: [blocked]",
        "extends: payment\n  rules: [blocked]",
    );
    has(&run(dir.path(), &["--root", "."], 1), "E_CYCLE");
}

#[test]
fn expression_diagnostics_identify_the_exact_condition() {
    let dir = setup();
    mutate(
        dir.path(),
        "rules/blocked.yaml",
        "features.adjusted_count > 100",
        "event.amount >",
    );
    let report = run(dir.path(), &["rules/blocked.yaml"], 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_INVALID_EXPRESSION");
    assert_eq!(report["diagnostics"][0]["field_path"], "/rule/when/any/1");
}

#[test]
fn positional_directories_recurse_through_arbitrary_layouts() {
    let dir = tempfile::tempdir().unwrap();
    for relative in [
        "policies/a.yaml",
        "policies/custom/deep/b.YML",
        "other/c.json",
    ] {
        let path = dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let id = path.file_stem().unwrap().to_str().unwrap();
        std::fs::write(&path, format!(r#"{{"id":"{id}","backend":"memory"}}"#)).unwrap();
    }
    std::fs::write(
        dir.path().join("policies/README.md"),
        "Documentation, not CDL",
    )
    .unwrap();
    let report = run(dir.path(), &["policies", "other"], 0);
    assert_eq!(report["sources"].as_array().unwrap().len(), 3);
    assert_eq!(report["references_checked"], false);
    // Overlapping directories and explicit files load the same resource once.
    let mixed = run(
        dir.path(),
        &[
            "policies",
            "policies/custom",
            "policies/a.yaml",
            "other/c.json",
        ],
        0,
    );
    assert_eq!(mixed["sources"], report["sources"]);
}

#[test]
fn explicit_files_do_not_discover_neighbors_or_follow_imports() {
    let dir = setup();
    std::fs::write(dir.path().join("bad.yaml"), "rule: [").unwrap();
    mutate(
        dir.path(),
        "rules/blocked.yaml",
        "version: \"0.1\"",
        "version: \"0.1\"\nimport: {rules: [bad.yaml]}",
    );
    let report = run(dir.path(), &["rules/blocked.yaml", "services/risk.yaml"], 0);
    assert_eq!(report["sources"].as_array().unwrap().len(), 2);
    // The declaration itself must still have valid syntax.
    mutate(dir.path(), "rules/blocked.yaml", "[bad.yaml]", "123");
    has(
        &run(dir.path(), &["rules/blocked.yaml"], 1),
        "E_INVALID_STRUCTURE",
    );
}

#[test]
fn directories_report_invalid_files_at_any_depth_and_io_errors() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("custom/deep")).unwrap();
    std::fs::write(
        dir.path().join("custom/good.yaml"),
        "id: good\nbackend: memory",
    )
    .unwrap();
    std::fs::write(dir.path().join("custom/deep/bad.yaml"), "rule: [").unwrap();
    let report = run(dir.path(), &["custom"], 1);
    has(&report, "E_YAML");
    assert_eq!(report["sources"].as_array().unwrap().len(), 2);
    has(
        &run(dir.path(), &["custom/good.yaml", "missing"], 2),
        "E_READ",
    );
    std::fs::create_dir(dir.path().join("empty")).unwrap();
    has(&run(dir.path(), &["empty"], 2), "E_NO_SOURCES");
    // An explicitly selected empty directory must not fall back to --root discovery.
    has(
        &run(dir.path(), &["--root", ".", "empty"], 2),
        "E_NO_SOURCES",
    );
}

#[test]
fn directory_selection_can_opt_into_reference_checks_with_root() {
    let dir = setup();
    let report = run(
        dir.path(),
        &[
            "--root",
            ".",
            "rules",
            "rulesets",
            "pipelines",
            "features",
            "lists",
            "services",
            "registry.yaml",
        ],
        0,
    );
    assert_eq!(report["sources"].as_array().unwrap().len(), 7);
    assert_eq!(report["references_checked"], true);
}

#[cfg(unix)]
#[test]
fn directory_symlink_cycles_do_not_recurse() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("policies")).unwrap();
    symlink(
        dir.path().join("policies"),
        dir.path().join("policies/loop"),
    )
    .unwrap();
    has(&run(dir.path(), &["policies"], 2), "E_PATH");
}
