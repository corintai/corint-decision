#![cfg(unix)]
use corint_decision_compiler::core::{compile_core, parse_core_input_schema, CoreSource};
use corint_decision_toolchain::{behavior, contracts::TargetContracts, package, resolve, transfer};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;
const FILES: &[&str] = &[
    "input-schema.yaml",
    "registry.yaml",
    "pipelines/payment.yaml",
    "rulesets/risk.yaml",
    "rules/amount.yaml",
];
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}
fn fixture(path: &str) -> CoreSource {
    CoreSource {
        path: path.into(),
        yaml: std::fs::read_to_string(root().join(path)).unwrap(),
    }
}
fn repo() -> Vec<CoreSource> {
    FILES
        .iter()
        .map(|p| CoreSource {
            path: p.to_string(),
            yaml: fixture(&format!("cdl_imports/{p}")).yaml,
        })
        .collect()
}
fn setup() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in repo() {
        let path = dir.path().join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.yaml).unwrap();
    }
    dir
}
fn entries() -> Vec<String> {
    vec!["registry.yaml".into()]
}
fn run(args: &[&str], exit: i32) -> Value {
    let result = Command::new(env!("CARGO_BIN_EXE_corint"))
        .args(args.iter().take(1))
        .args(if args.first() == Some(&"validate") {
            vec!["--profile", "cdl-core-risk-draft-1"]
        } else {
            vec![]
        })
        .args(args.iter().skip(1))
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(result.stderr.is_empty());
    serde_json::from_slice(&result.stdout).unwrap()
}
fn cli(dir: &Path, output: &Path, exit: i32) -> Value {
    run(
        &[
            "resolve",
            "--source-profile",
            resolve::SOURCE_PROFILE,
            "--root",
            dir.to_str().unwrap(),
            "--input-schema",
            "input-schema.yaml",
            "--output",
            output.to_str().unwrap(),
            "registry.yaml",
        ],
        exit,
    )
}
fn memory(
    sources: &[CoreSource],
) -> Result<resolve::ResolvedClosure, corint_decision_compiler::core::CoreError> {
    resolve::resolve_sources("input-schema.yaml", &entries(), sources)
}
fn mutate(sources: &mut [CoreSource], path: &str, from: &str, to: &str) {
    let file = sources.iter_mut().find(|s| s.path == path).unwrap();
    assert!(file.yaml.contains(from));
    file.yaml = file.yaml.replacen(from, to, 1);
}
fn code(
    result: Result<resolve::ResolvedClosure, corint_decision_compiler::core::CoreError>,
    expected: &str,
) {
    match result {
        Err(e) => assert_eq!(e.diagnostic.code, expected, "{e}"),
        Ok(_) => panic!("expected {expected}"),
    }
}

#[test]
fn real_file_resolution_matches_virtual_repository_and_existing_engine_behavior() {
    let dir = setup();
    let resolved = resolve::resolve(dir.path(), "input-schema.yaml", &entries()).unwrap();
    let memory = memory(&repo()).unwrap();
    assert_eq!(json!(resolved.receipt()), json!(memory.receipt()));
    assert_eq!(resolved.receipt().manifest.sources.len(), 4); // diamond dependency loaded once
    assert_eq!(resolved.originals().len(), 5);
    assert!(resolved
        .originals()
        .iter()
        .any(|s| s.yaml.contains("import:")));
    assert!(resolved
        .bundle()
        .sources
        .iter()
        .all(|s| !s.yaml.contains("import:")));
    let cases = fixture("cdl_core/behavior.yaml");
    let expected_sources: Vec<_> = [
        "rule.yaml",
        "ruleset.yaml",
        "pipeline.yaml",
        "registry.yaml",
    ]
    .iter()
    .map(|p| fixture(&format!("cdl_core/{p}")))
    .collect();
    let expected = behavior::test(
        &expected_sources,
        parse_core_input_schema(&fixture("cdl_core/input-schema.yaml")).unwrap(),
        &cases,
    )
    .unwrap();
    let actual = behavior::test(
        &memory.bundle().sources,
        parse_core_input_schema(&memory.bundle().input_schema).unwrap(),
        &cases,
    )
    .unwrap();
    assert_eq!(json!(actual), json!(expected));
    assert_eq!(actual.passed, 5);
    let output = dir.path().join("resolved.json");
    let report = cli(dir.path(), &output, 0);
    assert_eq!(report["scope"], "resolve");
    assert_eq!(report["execution_checked"], false);
    assert_eq!(report["resolution"], json!(resolved.receipt()));
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(
        report["resolution"]["bundle_sha256"],
        format!("{:x}", Sha256::digest(&bytes))
    );
    let frozen = transfer::read_bundle(&CoreSource {
        path: "resolved.json".into(),
        yaml: String::from_utf8(bytes).unwrap(),
    })
    .unwrap();
    assert_eq!(
        package::policy_identity(&frozen.sources, &frozen.input_schema).unwrap(),
        resolved.receipt().policy_sha256
    );
}

#[test]
fn frozen_bundle_imports_builds_verifies_and_target_checks_without_original_files() {
    let dir = setup();
    let out = tempfile::tempdir().unwrap();
    let bundle = out.path().join("resolved.json");
    let report = cli(dir.path(), &bundle, 0);
    drop(dir);
    let cases = root().join("cdl_core/behavior.yaml");
    let package = out.path().join("policy.json");
    run(
        &[
            "import",
            "--bundle",
            bundle.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
            "--output",
            package.to_str().unwrap(),
        ],
        0,
    );
    run(
        &[
            "verify",
            "--package",
            package.to_str().unwrap(),
            "--cases",
            cases.to_str().unwrap(),
        ],
        0,
    );
    let frozen = transfer::read_bundle(&CoreSource {
        path: "resolved.json".into(),
        yaml: std::fs::read_to_string(bundle).unwrap(),
    })
    .unwrap();
    let contracts = TargetContracts::load(
        &fixture("contracts/business-context.yaml"),
        &fixture("contracts/target-capabilities.json"),
    )
    .unwrap();
    let checked = contracts
        .check(&frozen.sources, &frozen.input_schema, None)
        .unwrap();
    assert_eq!(checked.policy_sha256, report["resolution"]["policy_sha256"]);
    let exported = transfer::export_sources(&CoreSource {
        path: "package.json".into(),
        yaml: std::fs::read_to_string(package).unwrap(),
    })
    .unwrap();
    assert_eq!(
        package::policy_identity(&exported.sources, &exported.input_schema).unwrap(),
        checked.policy_sha256
    );
}

#[test]
fn raw_dependency_changes_invalidate_identity_and_binding_even_without_behavior_change() {
    let original = memory(&repo()).unwrap();
    let contracts = TargetContracts::load(
        &fixture("contracts/business-context.yaml"),
        &fixture("contracts/target-capabilities.json"),
    )
    .unwrap();
    let checked = contracts
        .check(
            &original.bundle().sources,
            &original.bundle().input_schema,
            None,
        )
        .unwrap();
    for path in FILES {
        let mut files = repo();
        files
            .iter_mut()
            .find(|s| &s.path == path)
            .unwrap()
            .yaml
            .push_str("\n# dependency changed\n");
        let changed = memory(&files).unwrap();
        assert_ne!(
            original.receipt().resolution_sha256,
            changed.receipt().resolution_sha256
        );
        assert_ne!(
            original.receipt().policy_sha256,
            changed.receipt().policy_sha256
        );
        assert_eq!(
            contracts
                .check(
                    &changed.bundle().sources,
                    &changed.bundle().input_schema,
                    Some(&checked.binding_sha256)
                )
                .unwrap_err()
                .diagnostic
                .code,
            "E_STALE_BINDING"
        );
    }
    let mut files = repo();
    mutate(&mut files, "rules/amount.yaml", "> 1000", ">= 1000");
    let changed = memory(&files).unwrap();
    assert!(package::prepare(
        &changed.bundle().sources,
        &changed.bundle().input_schema,
        &fixture("cdl_core/behavior.yaml")
    )
    .unwrap()
    .0
    .is_none());
}

#[test]
fn relocation_entry_order_and_repository_order_do_not_change_resolution() {
    let a = setup();
    let b = setup();
    let roots = vec!["registry.yaml".into(), "pipelines/payment.yaml".into()];
    let mut reversed = roots.clone();
    reversed.reverse();
    let first = resolve::resolve(a.path(), "input-schema.yaml", &roots).unwrap();
    let second = resolve::resolve(b.path(), "input-schema.yaml", &reversed).unwrap();
    assert_eq!(json!(first.receipt()), json!(second.receipt()));
    let mut files = repo();
    files.reverse();
    assert_eq!(
        json!(first.receipt()),
        json!(
            resolve::resolve_sources("input-schema.yaml", &roots, &files)
                .unwrap()
                .receipt()
        )
    );
}

#[test]
fn malformed_headers_versions_extra_documents_and_duplicate_keys_are_rejected() {
    for (from, to, expected) in [
        ("pipelines:", "pipline:", "E_INVALID_IMPORT"),
        ("[pipelines/payment.yaml]", "[]", "E_INVALID_IMPORT"),
        ("[pipelines/payment.yaml]", "[7]", "E_INVALID_IMPORT"),
        (
            "[pipelines/payment.yaml]",
            "[pipelines/payment.yaml, pipelines/payment.yaml]",
            "E_INVALID_IMPORT",
        ),
        (
            "version: \"0.1\"",
            "version: \"0.2\"",
            "E_UNSUPPORTED_VERSION",
        ),
        ("version: \"0.1\"", "version: 0.1", "E_INVALID_VERSION"),
        ("version: \"0.1\"", "", "E_INVALID_VERSION"),
        ("---", "---\nversion: \"0.2\"", "E_UNSUPPORTED_VERSION"),
        ("import:", "unknown: true\nimport:", "E_INVALID_IMPORT"),
        (
            "import:",
            "version: \"0.1\"\nimport:",
            "E_INVALID_STRUCTURE",
        ),
    ] {
        let mut files = repo();
        mutate(&mut files, "registry.yaml", "registry:", "---\nregistry:");
        mutate(&mut files, "registry.yaml", from, to);
        code(memory(&files), expected);
    }
    let mut files = repo();
    mutate(&mut files, "registry.yaml", "registry:", "---\nregistry:");
    files[1].yaml.push_str("\n---\nrule: {}\n");
    code(memory(&files), "E_INVALID_IMPORT");
    let mut files = repo();
    files[1].yaml = "version: '0.1'\n---\nregistry: []\n".into();
    code(memory(&files), "E_INVALID_IMPORT");
}

#[test]
fn missing_wrong_kind_cycles_and_duplicate_ids_fail_with_no_fallback() {
    for (from, to, expected) in [
        (
            "pipelines/payment.yaml",
            "pipelines/missing.yaml",
            "E_INVALID_IMPORT",
        ),
        (
            "pipelines/payment.yaml",
            "rules/amount.yaml",
            "E_IMPORT_KIND",
        ),
        (
            "pipelines/payment.yaml",
            "registry.yaml",
            "E_INVALID_IMPORT",
        ),
        (
            "pipelines/payment.yaml",
            "input-schema.yaml",
            "E_INVALID_IMPORT",
        ),
    ] {
        let mut files = repo();
        mutate(&mut files, "registry.yaml", from, to);
        code(memory(&files), expected);
    }
    let mut files = repo();
    mutate(
        &mut files,
        "rules/amount.yaml",
        "rule:",
        "import:\n  pipelines: [pipelines/payment.yaml]\nrule:",
    );
    let e = memory(&files).err().unwrap();
    assert!(e.diagnostic.message.contains("cycle"));
    assert!(e.diagnostic.message.contains("rules/amount.yaml"));
    let mut files = repo();
    let mut duplicate = files[4].clone();
    duplicate.path = "rules/duplicate.yaml".into();
    files.push(duplicate);
    mutate(
        &mut files,
        "rulesets/risk.yaml",
        "[rules/amount.yaml]",
        "[rules/amount.yaml, rules/duplicate.yaml]",
    );
    code(memory(&files), "E_DUPLICATE_ID");
    let mut files = repo();
    files.push(files[4].clone());
    code(memory(&files), "E_DUPLICATE_SOURCE");
}

#[test]
fn paths_symlinks_directories_and_depth_limits_are_confined() {
    for invalid in [
        "../outside.yaml",
        "./registry.yaml",
        "/tmp/outside.yaml",
        "https://x/rule.yaml",
        "rules\\amount.yaml",
    ] {
        let mut files = repo();
        mutate(
            &mut files,
            "registry.yaml",
            "pipelines/payment.yaml",
            invalid,
        );
        assert!(memory(&files).is_err());
        code(
            resolve::resolve_sources("input-schema.yaml", &[invalid.into()], &repo()),
            "E_INVALID_IMPORT",
        );
    }
    for kind in ["file", "directory", "internal"] {
        let dir = setup();
        let outside = setup();
        let path = dir.path().join("rules/amount.yaml");
        if kind == "directory" {
            std::fs::rename(dir.path().join("rules"), dir.path().join("old-rules")).unwrap();
            std::os::unix::fs::symlink(outside.path().join("rules"), dir.path().join("rules"))
                .unwrap();
        } else {
            std::fs::rename(&path, dir.path().join("old.yaml")).unwrap();
            let destination = if kind == "internal" {
                dir.path().join("old.yaml")
            } else {
                outside.path().join("rules/amount.yaml")
            };
            std::os::unix::fs::symlink(destination, &path).unwrap();
        }
        code(
            resolve::resolve(dir.path(), "input-schema.yaml", &entries()),
            "E_INVALID_IMPORT",
        );
    }
    let dir = setup();
    std::fs::rename(
        dir.path().join("rules/amount.yaml"),
        dir.path().join("old.yaml"),
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("rules/amount.yaml")).unwrap();
    code(
        resolve::resolve(dir.path(), "input-schema.yaml", &entries()),
        "E_INVALID_IMPORT",
    );
    let mut files = repo();
    for i in 0..34 {
        files.push(CoreSource {path:format!("chain{i}.yaml"), yaml:format!("version: '0.1'\nimport:\n  rules: [chain{}.yaml]\nrule:\n  id: chain{i}\n  name: chain\n  when: 'true'\n  score: 1\n", i+1)});
    }
    code(
        resolve::resolve_sources("input-schema.yaml", &["chain0.yaml".into()], &files),
        "E_IMPORT_LIMIT",
    );
    let mut files = repo();
    files[4].yaml.push_str(&" ".repeat(1024 * 1024));
    code(memory(&files), "E_IMPORT_LIMIT");
}

#[test]
fn cli_requires_explicit_profile_is_no_clobber_and_old_core_still_rejects_imports() {
    let dir = setup();
    let output = dir.path().join("resolved.json");
    cli(dir.path(), &output, 0);
    let before = std::fs::read(&output).unwrap();
    cli(dir.path(), &output, 2);
    assert_eq!(std::fs::read(&output).unwrap(), before);
    run(
        &[
            "resolve",
            "--root",
            dir.path().to_str().unwrap(),
            "--input-schema",
            "input-schema.yaml",
            "--output",
            "unused.json",
            "registry.yaml",
        ],
        2,
    );
    run(&["resolve", "--source-profile", "unknown"], 2);
    run(&["validate", "--root", dir.path().to_str().unwrap()], 2);
    let files = repo();
    let resources: Vec<_> = files[1..].to_vec();
    assert!(compile_core(&resources, parse_core_input_schema(&files[0]).unwrap()).is_err());
    let dir = setup();
    std::fs::write(dir.path().join("rules/amount.yaml"), "invalid").unwrap();
    let output = dir.path().join("rejected.json");
    cli(dir.path(), &output, 1);
    assert!(!output.exists());
}

#[test]
fn published_profile_schema_and_c08_fixture_match_actual_resolution() {
    let inventory: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/capabilities.json"
    ))
    .unwrap();
    let tool = &inventory["tools"]["resolve_cli"];
    assert_eq!(tool["source_profile"], resolve::SOURCE_PROFILE);
    assert_eq!(tool["cases"], json!(["C08"]));
    assert_eq!(tool["runtime_filesystem_access"], false);
    assert_eq!(tool["execution_checked"], false);
    assert_eq!(
        tool["evidence"],
        "../../../crates/corint-decision-cli/tests/resolve.rs"
    );
    let manifest: Value = serde_json::from_str(&fixture("cdl_imports/manifest.json").yaml).unwrap();
    assert_eq!(manifest["source_profile"], resolve::SOURCE_PROFILE);
    assert_eq!(manifest["entries"], json!(entries()));
    assert_eq!(manifest["expected_behavior"], "../cdl_core/behavior.yaml");
    let actual = memory(&repo()).unwrap();
    let mut expected: Vec<_> = manifest["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    expected.sort();
    assert_eq!(
        actual
            .receipt()
            .manifest
            .sources
            .iter()
            .map(|s| s.path.as_str())
            .collect::<Vec<_>>(),
        expected
    );
    let schema: Value = serde_json::from_str(resolve::HEADER_SCHEMA).unwrap();
    let validator = jsonschema::JSONSchema::compile(&schema).unwrap();
    assert!(validator.is_valid(&json!({"version":"0.1", "import":{"rules":["rules/amount.yaml"]}})));
    assert!(!validator.is_valid(&json!({"version":"0.1", "import":{"rules":["../secret.yaml"]}})));
}

#[test]
fn file_count_and_total_byte_limits_fail_before_closure_compilation() {
    for (count, padding) in [(256, 0), (24, 750_000)] {
        let mut files = repo();
        let paths: Vec<_> = (0..count).map(|i| format!("extra{i}.yaml")).collect();
        mutate(
            &mut files,
            "registry.yaml",
            "import:",
            &format!(
                "import:\n  rules: {}",
                serde_json::to_string(&paths).unwrap()
            ),
        );
        for (i, path) in paths.into_iter().enumerate() {
            files.push(CoreSource { path, yaml: format!("version: '0.1'\nrule:\n  id: extra{i}\n  name: extra\n  when: 'true'\n  score: 1\n# {}\n", "x".repeat(padding)) });
        }
        code(memory(&files), "E_IMPORT_LIMIT");
    }
}
