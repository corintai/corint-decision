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

#[test]
fn ruleset_rule_lists_require_block_style_in_files_directories_and_imports() {
    let dir = setup();
    let path = dir.path().join("rulesets/payment.yaml");
    let original = std::fs::read_to_string(&path).unwrap();
    for flow in ["[blocked]", "[\n    blocked\n  ]", "[]", "&ids [blocked]"] {
        std::fs::write(
            &path,
            original.replace("rules:\n    - blocked", &format!("rules: {flow}")),
        )
        .unwrap();
        for args in [
            vec!["rulesets/payment.yaml"],
            vec!["rulesets"],
            vec!["--root", "."],
        ] {
            let report = run(dir.path(), &args, 1);
            has(&report, "E_RULES_FORMAT");
            let diagnostic = report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["code"] == "E_RULES_FORMAT")
                .unwrap();
            assert_eq!(diagnostic["field_path"], "/ruleset/rules");
            assert_eq!(diagnostic["line"], 4);
        }
    }
    std::fs::write(path, original).unwrap();
    run(dir.path(), &["."], 0);
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
        assert_eq!(report["references_checked"], true);
        assert_eq!(report["input_schema_checked"], false);
    }
}

#[test]
fn selected_collections_reject_misspelled_rule_ids_without_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("rules")).unwrap();
    std::fs::create_dir(dir.path().join("rulesets")).unwrap();
    // Filenames deliberately differ from IDs: references resolve by rule.id.
    std::fs::write(dir.path().join("rules/history.yaml"),
        "version: '0.1'\nrule:\n  id: customer_amount_spike_7d\n  name: History\n  when: 'true'\n  score: 50\n").unwrap();
    let path = dir.path().join("rulesets/risk.yaml");
    let correct = "version: '0.1'\nruleset:\n  id: risk\n  rules:\n    - customer_amount_spike_7d\n  conclusion:\n    - default: true\n      signal: pass\n";
    for args in [
        vec!["."],
        vec!["rules", "rulesets"],
        vec!["rules/history.yaml", "rulesets/risk.yaml"],
        vec!["rulesets/risk.yaml", "rules/history.yaml"],
    ] {
        std::fs::write(&path, correct).unwrap();
        let good = run(dir.path(), &args, 0);
        assert_eq!(good["references_checked"], true);
        assert!(!good["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "cross_file_references"));
        std::fs::write(
            &path,
            correct.replace("customer_amount_spike_7d", "customer_amount_spike_7"),
        )
        .unwrap();
        let bad = run(dir.path(), &args, 1);
        let diagnostic = &bad["diagnostics"][0];
        assert_eq!(diagnostic["code"], "E_UNRESOLVED_REFERENCE");
        assert_eq!(diagnostic["field_path"], "/ruleset/rules/0");
        assert!(diagnostic["message"]
            .as_str()
            .unwrap()
            .contains("customer_amount_spike_7"));
    }
    let text = text_report(dir.path(), &["."], 1);
    assert!(text.contains("E_UNRESOLVED_REFERENCE"));
    assert!(!text.contains("[PASS]"));
    // A directory containing only the referring ruleset is still a collection.
    has(&run(dir.path(), &["rulesets"], 1), "E_UNRESOLVED_REFERENCE");
    // Repeating one file does not supply the missing dependencies.
    let single = run(dir.path(), &["rulesets/risk.yaml", "rulesets/risk.yaml"], 1);
    assert_eq!(single["references_checked"], true);
    let text = text_report(dir.path(), &["rulesets/risk.yaml"], 1);
    assert!(text.contains("Unknown rule: customer_amount_spike_7"));
    // Correct IDs load their definitions automatically, even without a registry.
    std::fs::write(&path, correct).unwrap();
    let single = run(dir.path(), &["rulesets/risk.yaml"], 0);
    assert_eq!(single["sources"].as_array().unwrap().len(), 2);
}

#[test]
fn single_file_validates_transitive_dependencies_without_unrelated_files() {
    let dir = setup();
    std::fs::write(dir.path().join("unrelated.yaml"), "rule: [").unwrap();
    let report = run(dir.path(), &["pipelines/payment.yaml"], 0);
    assert_eq!(report["references_checked"], true);
    assert_eq!(report["sources"].as_array().unwrap().len(), 6);
    assert!(report["reference_root"]
        .as_str()
        .unwrap()
        .ends_with(dir.path().file_name().unwrap().to_str().unwrap()));
    assert!(!report["sources"].as_array().unwrap().iter().any(|p| {
        let p = p.as_str().unwrap();
        p.ends_with("unrelated.yaml") || p.ends_with("registry.yaml")
    }));
    for (file, from, to, code) in [
        (
            "rules/blocked.yaml",
            "score: 100",
            "score: invalid",
            "E_INVALID_STRUCTURE",
        ),
        (
            "rulesets/payment.yaml",
            "rules:\n    - blocked",
            "rules: [blocked]",
            "E_RULES_FORMAT",
        ),
        (
            "services/risk.yaml",
            "assess:",
            "other:",
            "E_UNKNOWN_OPERATION",
        ),
        (
            "features/payment.yaml",
            "features.count_1h +",
            "features.typo +",
            "E_UNRESOLVED_REFERENCE",
        ),
        (
            "lists/blocked.yaml",
            "id: blocked_customers",
            "id: typo",
            "E_UNRESOLVED_REFERENCE",
        ),
    ] {
        let source = std::fs::read_to_string(dir.path().join(file)).unwrap();
        mutate(dir.path(), file, from, to);
        has(&run(dir.path(), &["pipelines/payment.yaml"], 1), code);
        std::fs::write(dir.path().join(file), source).unwrap();
    }
    std::fs::copy(
        dir.path().join("rules/blocked.yaml"),
        dir.path().join("duplicate.yaml"),
    )
    .unwrap();
    has(
        &run(dir.path(), &["pipelines/payment.yaml"], 1),
        "E_DUPLICATE_ID",
    );
}

#[test]
fn single_file_loads_declared_imports_and_checks_cycles() {
    let dir = setup();
    mutate(
        dir.path(),
        "rules/blocked.yaml",
        "version: \"0.1\"",
        "version: \"0.1\"\nimport: {rules: [broken.yaml]}",
    );
    std::fs::write(dir.path().join("broken.yaml"), "rule: [").unwrap();
    has(&run(dir.path(), &["rules/blocked.yaml"], 1), "E_YAML");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.yaml"),
        "version: '0.1'\nruleset:\n  id: a\n  extends: b\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.yaml"),
        "version: '0.1'\nruleset:\n  id: b\n  extends: a\n",
    )
    .unwrap();
    has(&run(dir.path(), &["a.yaml"], 1), "E_CYCLE");
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
            "rules:\n    - blocked",
            "rules:\n    - missing",
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
    assert_eq!(single["sources"].as_array().unwrap().len(), 7);
    assert_eq!(single["references_checked"], true);
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
fn documented_online_examples_require_their_host_list_bindings() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CDL/examples/payment-review/online");
    let dir = tempfile::tempdir().unwrap();
    let files = [
        "features.yaml",
        "prepare.yaml",
        "markers.yaml",
        "customer-risk.yaml",
        "blocked-email.yaml",
        "loyal-customer.yaml",
    ];
    for file in files {
        std::fs::copy(root.join(file), dir.path().join(file)).unwrap();
    }
    // SDK-provided objects still need declarative bindings for static resolution.
    has(
        &run(dir.path(), &["prepare.yaml"], 1),
        "E_UNRESOLVED_REFERENCE",
    );
    std::fs::write(dir.path().join("lists.yaml"), "lists:\n  - id: blocked_emails\n    backend: memory\n  - id: trusted_customers\n    backend: memory\n").unwrap();
    for file in files {
        run(dir.path(), &[file], 0);
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
    std::fs::write(dir.path().join("rulesets/base.yaml"), "version: '0.1'\nruleset:\n  id: base\n  rules:\n    - blocked\n  conclusion:\n    - default: true\n      signal: pass\n      reason: Base policy\n      actions: [notify]\n").unwrap();
    std::fs::write(
        dir.path().join("rulesets/payment.yaml"),
        "version: '0.1'\nruleset:\n  id: payment\n  extends: base\n",
    )
    .unwrap();
    run(dir.path(), &["--root", "."], 0);
    mutate(
        dir.path(),
        "rulesets/base.yaml",
        "rules:\n    - blocked",
        "extends: payment\n  rules:\n    - blocked",
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
    assert_eq!(report["references_checked"], true);
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
    let report = run(dir.path(), &["rules/blocked.yaml", "services/risk.yaml"], 1);
    assert_eq!(report["sources"].as_array().unwrap().len(), 2);
    has(&report, "E_UNRESOLVED_REFERENCE");
    assert!(!report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "E_READ"));
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

fn write_auxiliary_files(dir: &Path) {
    let files = [
        ("schema.yaml", "name: event\nfields: {}"),
        (
            "cases.json",
            r#"{"version":"1","profile":"core","cases":[]}"#,
        ),
        (
            "validation.json",
            r#"{"report_version":"1","profile":"cdl-static-1","diagnostics":[],"valid":true}"#,
        ),
        (
            "quality.json",
            r#"{"rows":20,"columns":["amount"],"source":"sample.csv","sha256":"sample"}"#,
        ),
        (
            "summary.json",
            r#"{"metrics":[],"source_sha256":"sample","status":"complete"}"#,
        ),
    ];
    for (name, text) in files {
        std::fs::write(dir.join(name), text).unwrap();
    }
}

#[test]
fn directory_discovery_reports_auxiliary_documents_as_skipped() {
    let dir = setup();
    write_auxiliary_files(dir.path());
    let report = run(dir.path(), &["."], 0);
    assert_eq!(report["sources"].as_array().unwrap().len(), 7);
    // The existing input-schema fixture is also discovered and skipped.
    assert_eq!(report["skipped_sources"].as_array().unwrap().len(), 6);
    assert_eq!(report["input_schema_checked"], false);
    assert!(report["skipped_sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["reason"] == "analysis_report"));
    // Selecting the same directory twice must not duplicate skip records.
    let repeated = run(dir.path(), &[".", "."], 0);
    assert_eq!(repeated["skipped_sources"], report["skipped_sources"]);
}

#[test]
fn explicit_files_override_discovery_skips_in_either_argument_order() {
    let dir = setup();
    write_auxiliary_files(dir.path());
    for args in [
        vec![".", "validation.json"],
        vec!["validation.json", "."],
        vec!["input-schema.yaml"],
    ] {
        let report = run(dir.path(), &args, 1);
        has(&report, "E_NOT_CDL");
        assert!(!report["skipped_sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["source"]
                .as_str()
                .unwrap()
                .ends_with(args.iter().find(|arg| **arg != ".").unwrap())));
    }
}

#[test]
fn unknown_malformed_or_resource_shaped_files_are_never_skipped() {
    for (text, code) in [
        ("rule: [", "E_YAML"),
        ("rulle: {id: typo, name: Typo, when: 'true', score: 1}", "E_UNKNOWN_FIELD"),
        ("name: broken_service", "E_MISSING_FIELD"),
        ("features: [{name: f, type: typo}]", "E_INVALID_STRUCTURE"),
        ("rule: {id: r, name: R, when: 'true', score: 1}\nmetrics: {}\nstatus: complete\nsource_sha256: sample", "E_UNKNOWN_FIELD"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("reports")).unwrap();
        std::fs::write(dir.path().join("reports/summary.yaml"), text).unwrap();
        let report = run(dir.path(), &["."], 1);
        has(&report,code);
        assert!(report["skipped_sources"].as_array().unwrap().is_empty());
    }
}

#[test]
fn imported_auxiliary_documents_fail_instead_of_disappearing() {
    let dir = setup();
    write_auxiliary_files(dir.path());
    mutate(
        dir.path(),
        "registry.yaml",
        "version: \"0.1\"",
        "version: \"0.1\"\nimport: {rules: [cases.json]}",
    );
    let report = run(dir.path(), &["--root", ".", "registry.yaml"], 1);
    has(&report, "E_IMPORT_KIND");
    has(&report, "E_NOT_CDL");
    assert!(report["skipped_sources"].as_array().unwrap().is_empty());
}

#[test]
fn an_auxiliary_only_directory_does_not_claim_validated_cdl() {
    let dir = tempfile::tempdir().unwrap();
    write_auxiliary_files(dir.path());
    let report = run(dir.path(), &["."], 2);
    has(&report, "E_NO_SOURCES");
    assert!(report["sources"].as_array().unwrap().is_empty());
    assert_eq!(report["skipped_sources"].as_array().unwrap().len(), 5);
}

fn text_report(dir: &Path, args: &[&str], code: i32) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_corint"))
        .current_dir(dir)
        .arg("validate")
        .args(args)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn directory_text_lists_every_passed_resource_once() {
    let dir = setup();
    for args in [vec!["."], vec![".", "rules"], vec!["--root", "."]] {
        let text = text_report(dir.path(), &args, 0);
        let passed: Vec<_> = text
            .lines()
            .filter_map(|line| line.strip_prefix("  [PASS] "))
            .collect();
        let report = run(dir.path(), &args, 0);
        let expected: Vec<_> = report["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(passed, expected);
        assert_eq!(passed.len(), 7);
        assert!(!passed
            .iter()
            .any(|path| path.ends_with("input-schema.yaml")));
    }
}

#[test]
fn directory_text_lists_good_files_alongside_file_local_failures() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("good.yaml"), "id: good\nbackend: memory").unwrap();
    std::fs::write(dir.path().join("bad.yaml"), "rule: [").unwrap();
    let text = text_report(dir.path(), &["."], 1);
    let passed: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with("  [PASS] "))
        .collect();
    assert_eq!(passed.len(), 1);
    assert!(passed[0].ends_with("good.yaml"));
    assert!(text.contains("E_YAML"));
    assert!(text.contains("FAIL (2 files)"));
}

#[test]
fn global_errors_do_not_print_misleading_file_passes() {
    let dir = setup();
    std::fs::write(
        dir.path().join("input-schema.yaml"),
        "name: event\nfields: typo",
    )
    .unwrap();
    let text = text_report(
        dir.path(),
        &["rules", "--input-schema", "input-schema.yaml"],
        1,
    );
    assert!(!text.contains("[PASS]"));
    mutate(
        dir.path(),
        "rulesets/payment.yaml",
        "rules:\n    - blocked",
        "rules:\n    - missing",
    );
    let text = text_report(dir.path(), &["--root", "."], 1);
    assert!(!text.contains("[PASS]"));
    assert!(text.contains("E_UNRESOLVED_REFERENCE"));
}
