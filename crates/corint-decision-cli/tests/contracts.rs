use corint_decision_compiler::core::{compile_core, parse_core_input_schema, CoreSource};
use corint_decision_toolchain::{contracts::TargetContracts, package};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};
use tempfile::TempDir;

const FILES: &[&str] = &[
    "rule.yaml",
    "ruleset.yaml",
    "pipeline.yaml",
    "registry.yaml",
];
fn fixture(file: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/conformance")
            .join(file),
    )
    .unwrap()
}
fn source(file: &str) -> CoreSource {
    CoreSource {
        path: file.into(),
        yaml: fixture(&format!("cdl_core/{file}")),
    }
}
fn sources() -> Vec<CoreSource> {
    FILES.iter().map(|f| source(f)).collect()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn setup() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in FILES
        .iter()
        .copied()
        .chain(["input-schema.yaml", "behavior.yaml"])
    {
        std::fs::write(dir.path().join(file), source(file).yaml).unwrap();
    }
    std::fs::write(
        dir.path().join("context.yaml"),
        fixture("contracts/business-context.yaml"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("target.json"),
        fixture("contracts/target-capabilities.json"),
    )
    .unwrap();
    dir
}
fn read(dir: &Path, file: &str) -> Value {
    serde_yaml::from_str(&std::fs::read_to_string(dir.join(file)).unwrap()).unwrap()
}
fn save(dir: &Path, file: &str, value: &Value) {
    std::fs::write(dir.join(file), serde_json::to_string_pretty(value).unwrap()).unwrap();
}
fn bind_context(dir: &Path) {
    let context = read(dir, "context.yaml");
    let mut target = read(dir, "target.json");
    target["context"] = json!({"id":context["id"],"revision":context["revision"],
        "sha256":hash(&std::fs::read(dir.join("context.yaml")).unwrap())});
    save(dir, "target.json", &target);
}
fn report(dir: &Path, args: &[&str], exit: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_corint"))
        .current_dir(dir)
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
    assert_eq!(output.status.code(), Some(exit), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}
fn check(dir: &Path, binding: Option<&str>, exit: i32) -> Value {
    let mut args = vec![
        "check-target",
        "--input-schema",
        "input-schema.yaml",
        "--context",
        "context.yaml",
        "--target",
        "target.json",
    ];
    if let Some(binding) = binding {
        args.extend(["--expected-binding", binding]);
    }
    args.extend(FILES);
    let result = report(dir, &args, exit);
    assert_eq!(result["scope"], "compatibility");
    assert_eq!(result["valid"], exit == 0);
    assert_eq!(result["execution_checked"], false);
    assert_eq!(result["business_evaluation"], "not_performed");
    result
}
fn error(report: &Value, code: &str) {
    assert_eq!(report["diagnostics"][0]["code"], code, "{report}");
    assert!(report.get("compatibility").is_none());
}

#[test]
fn check_binds_exact_sources_contracts_and_checker_without_claiming_authority() {
    let inventory: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/capabilities.json"
    ))
    .unwrap();
    let tool = &inventory["tools"]["target_check"];
    assert_eq!(
        tool["entry_point"],
        "corint-decision-toolchain::contracts::TargetContracts"
    );
    assert_eq!(tool["scope"], "declared_target_compatibility");
    assert_eq!(tool["execution_checked"], false);
    assert_eq!(tool["live_target_verified"], false);
    assert_eq!(tool["business_semantics_checked"], false);
    assert_eq!(tool["publication_approval"], "not_granted");
    assert_eq!(
        tool["evidence"],
        "../../../crates/corint-decision-cli/tests/contracts.rs"
    );
    let dir = setup();
    let report = check(dir.path(), None, 0);
    let evidence = &report["compatibility"];
    assert_eq!(
        evidence["policy_sha256"],
        package::policy_identity(&sources(), &source("input-schema.yaml")).unwrap()
    );
    assert_eq!(
        evidence["context"]["sha256"],
        hash(&std::fs::read(dir.path().join("context.yaml")).unwrap())
    );
    assert_eq!(
        evidence["target"]["sha256"],
        hash(&std::fs::read(dir.path().join("target.json")).unwrap())
    );
    assert_eq!(
        evidence["checker_sha256"],
        hash(&std::fs::read(env!("CARGO_BIN_EXE_corint")).unwrap())
    );
    for field in [
        "execution_checked",
        "business_semantics_checked",
        "live_target_verified",
    ] {
        assert_eq!(evidence[field], false);
    }
    assert_eq!(evidence["publication_approval"], "not_granted");
    assert_eq!(evidence["authenticity"], "unsigned");
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/compatibility-report.json"
    ))
    .unwrap();
    assert!(jsonschema::JSONSchema::compile(&schema)
        .unwrap()
        .is_valid(evidence));
    // Independent canonical-json-v1 reference: serde_json's default maps sort keys.
    let value = json!({"domain":"core-target-binding-v1","value":{
        "policy_sha256":evidence["policy_sha256"],"context_sha256":evidence["context"]["sha256"],
        "target_sha256":evidence["target"]["sha256"],"checker_version":evidence["checker_version"],
        "checker_sha256":evidence["checker_sha256"]}});
    let mut bytes = b"corint-canonical-json-v1\0".to_vec();
    bytes.extend(serde_json::to_vec(&value).unwrap());
    assert_eq!(evidence["binding_sha256"], hash(&bytes));
    let fresh = check(dir.path(), evidence["binding_sha256"].as_str(), 0);
    assert_eq!(fresh["compatibility"], *evidence);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 8);
}

#[test]
fn source_context_and_target_changes_invalidate_previous_binding() {
    for change in ["source", "unit", "target"] {
        let dir = setup();
        let original = check(dir.path(), None, 0);
        let binding = original["compatibility"]["binding_sha256"]
            .as_str()
            .unwrap();
        match change {
            "source" => std::fs::write(
                dir.path().join("rule.yaml"),
                source("rule.yaml").yaml.replace("> 1000", "> 2000"),
            )
            .unwrap(),
            "unit" => {
                let mut context = read(dir.path(), "context.yaml");
                context["fields"]["amount"]["unit"] = json!("CNY_fen");
                save(dir.path(), "context.yaml", &context);
                bind_context(dir.path());
            }
            _ => {
                let mut target = read(dir.path(), "target.json");
                target["revision"] = json!("2");
                save(dir.path(), "target.json", &target);
            }
        }
        error(&check(dir.path(), Some(binding), 1), "E_STALE_BINDING");
        let fresh = check(dir.path(), None, 0);
        assert_ne!(fresh["compatibility"]["binding_sha256"], binding);
        if change == "unit" {
            // Metadata is bound but its business truth is deliberately NOT proven.
            assert_eq!(fresh["compatibility"]["business_semantics_checked"], false);
        }
    }
}

#[test]
fn context_is_pinned_by_id_revision_and_exact_bytes() {
    for field in ["id", "revision", "sha256"] {
        let dir = setup();
        let mut target = read(dir.path(), "target.json");
        target["context"][field] = if field == "sha256" {
            json!("f".repeat(64))
        } else {
            json!("other")
        };
        save(dir.path(), "target.json", &target);
        error(&check(dir.path(), None, 1), "E_CONTEXT_BINDING");
    }
    let dir = setup();
    let text = std::fs::read_to_string(dir.path().join("context.yaml")).unwrap();
    std::fs::write(
        dir.path().join("context.yaml"),
        text + "\n# changed metadata\n",
    )
    .unwrap();
    error(&check(dir.path(), None, 1), "E_CONTEXT_BINDING");
}

#[test]
fn malformed_contexts_and_forged_permissions_fail_closed() {
    for which in [
        "version",
        "permission",
        "missing_meaning",
        "extra_meaning",
        "unknown_entity",
        "unit",
        "nested_input",
        "duplicate",
    ] {
        let dir = setup();
        let mut context = read(dir.path(), "context.yaml");
        let code = match which {
            "version" => {
                context["contract_version"] = json!("2");
                "E_CONTRACT_VERSION"
            }
            "permission" => {
                context["permissions"] = json!({"publish":true});
                "E_CONTRACT_FORMAT"
            }
            "missing_meaning" => {
                context["fields"] = json!({});
                "E_CONTEXT_FIELDS"
            }
            "extra_meaning" => {
                context["fields"]["other"] = context["fields"]["amount"].clone();
                "E_CONTEXT_FIELDS"
            }
            "unknown_entity" => {
                context["fields"]["amount"]["entity"] = json!("unknown");
                "E_CONTEXT_ENTITY"
            }
            "unit" => {
                context["fields"]["amount"]["unit"] = json!("  ");
                "E_CONTRACT_FORMAT"
            }
            "nested_input" => {
                context["input_schema"]["fields"]["amount"]["field_type"] = json!("any");
                "E_CONTRACT_FORMAT"
            }
            _ => "E_CONTRACT_FORMAT",
        };
        if which == "duplicate" {
            std::fs::write(
                dir.path().join("context.yaml"),
                "contract_version: \"1\"\ncontract_version: \"1\"",
            )
            .unwrap();
        } else {
            save(dir.path(), "context.yaml", &context);
        }
        error(&check(dir.path(), None, 1), code);
    }
}

#[test]
fn target_versions_capabilities_readiness_and_resources_cannot_be_invented() {
    for which in [
        "version",
        "profile",
        "language",
        "missing",
        "unknown",
        "unavailable",
        "resources",
        "permissions",
        "contract",
        "zero",
        "float",
    ] {
        let dir = setup();
        let mut target = read(dir.path(), "target.json");
        let code = match which {
            "version" => {
                target["engine"]["version"] = json!("999.0");
                "E_TARGET_VERSION"
            }
            "profile" => {
                target["engine"]["profile"] = json!("unknown");
                "E_TARGET_VERSION"
            }
            "language" => {
                target["engine"]["language_version"] = json!("0.2");
                "E_TARGET_VERSION"
            }
            "missing" => {
                target["engine"]["capabilities"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
                "E_TARGET_CAPABILITIES"
            }
            "unknown" => {
                target["engine"]["capabilities"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("connector"));
                "E_TARGET_CAPABILITIES"
            }
            "unavailable" => {
                target["status"] = json!("unavailable");
                "E_TARGET_UNAVAILABLE"
            }
            "resources" => {
                target["resources"] = json!([{"id":"model","ready":true}]);
                "E_CONTRACT_FORMAT"
            }
            "permissions" => {
                target["permissions"] = json!({"publish":true});
                "E_CONTRACT_FORMAT"
            }
            "contract" => {
                target["contract_version"] = json!("2");
                "E_CONTRACT_VERSION"
            }
            "zero" => {
                target["max_sources"] = json!(0);
                "E_CONTRACT_FORMAT"
            }
            _ => {
                target["max_sources"] = json!(4.5);
                "E_CONTRACT_FORMAT"
            }
        };
        save(dir.path(), "target.json", &target);
        error(&check(dir.path(), None, 1), code);
    }
}

#[test]
fn input_mismatch_action_branches_and_target_budget_are_checked() {
    let dir = setup();
    let mut input = read(dir.path(), "input-schema.yaml");
    input["fields"]["amount"]["field_type"] = json!("string");
    save(dir.path(), "input-schema.yaml", &input);
    error(&check(dir.path(), None, 1), "E_CONTEXT_INPUT");
    for origin in ["context.yaml", "target.json"] {
        let dir = setup();
        let mut value = read(dir.path(), origin);
        value["actions"] = json!(["BLOCK"]);
        save(dir.path(), origin, &value);
        if origin == "context.yaml" {
            bind_context(dir.path());
        }
        let report = check(dir.path(), None, 1);
        error(&report, "E_ACTION_UNAVAILABLE");
        assert_eq!(
            report["diagnostics"][0]["field_path"],
            "/pipeline/decision/1/actions/0"
        );
    }
    let dir = setup();
    let mut target = read(dir.path(), "target.json");
    target["max_sources"] = json!(3);
    save(dir.path(), "target.json", &target);
    error(&check(dir.path(), None, 1), "E_TARGET_LIMIT");
    target["max_sources"] = json!(4.0);
    save(dir.path(), "target.json", &target);
    check(dir.path(), None, 0);
}

#[test]
fn all_core_negative_fixtures_preserve_compiler_diagnostics() {
    let dir = setup();
    let manifest: Value = serde_yaml::from_str(&source("manifest.yaml").yaml).unwrap();
    // Preserve the original 24 rejections plus five new Pipeline boundary cases;
    // future manifest additions must also run through this adapter.
    assert!(manifest["invalid"].as_array().unwrap().len() >= 29);
    for case in manifest["invalid"].as_array().unwrap() {
        let mut docs = sources();
        let doc = docs
            .iter_mut()
            .find(|d| d.path == case["document"].as_str().unwrap())
            .unwrap();
        doc.yaml = doc.yaml.replacen(
            case["find"].as_str().unwrap(),
            case["replace"].as_str().unwrap(),
            1,
        );
        for doc in &docs {
            std::fs::write(dir.path().join(&doc.path), &doc.yaml).unwrap();
        }
        let expected = compile_core(
            &docs,
            parse_core_input_schema(&source("input-schema.yaml")).unwrap(),
        )
        .unwrap_err();
        let report = check(dir.path(), None, 1);
        assert_eq!(
            report["diagnostics"][0],
            serde_json::to_value(expected).unwrap(),
            "{case}"
        );
    }
}

#[test]
fn relocation_preserves_binding_and_direct_library_uses_same_policy_identity() {
    let dir = setup();
    let before = check(dir.path(), None, 0);
    let moved = setup();
    let after = check(moved.path(), None, 0);
    assert_eq!(before["compatibility"], after["compatibility"]);
    let context = CoreSource {
        path: "renamed.yaml".into(),
        yaml: fixture("contracts/business-context.yaml"),
    };
    let target = CoreSource {
        path: "renamed-target.json".into(),
        yaml: fixture("contracts/target-capabilities.json"),
    };
    let checked = TargetContracts::load(&context, &target)
        .unwrap()
        .check(&sources(), &source("input-schema.yaml"), None)
        .unwrap();
    assert_eq!(
        checked.policy_sha256,
        before["compatibility"]["policy_sha256"]
    );
    assert_ne!(
        checked.checker_sha256,
        before["compatibility"]["checker_sha256"]
    );
    assert_ne!(
        checked.binding_sha256,
        before["compatibility"]["binding_sha256"]
    );
}

#[test]
fn cli_flags_and_failures_are_explicit_and_do_not_change_legacy_validate() {
    let dir = setup();
    for args in [
        vec!["check-target"],
        vec![
            "check-target",
            "--context",
            "context.yaml",
            "--target",
            "target.json",
        ],
        vec!["check-target", "--context", "x", "--context", "y"],
        vec!["check-target", "--expected-binding", "not-a-hash"],
        vec!["validate", "--context", "context.yaml"],
        vec!["check-target", "--cases", "behavior.yaml"],
        vec!["check-target", "--output", "out.json"],
    ] {
        error(&report(dir.path(), &args, 2), "E_USAGE");
    }
    let mut args = vec!["validate", "--input-schema", "input-schema.yaml"];
    args.extend(FILES);
    let legacy = report(dir.path(), &args, 0);
    assert!(legacy.get("compatibility").is_none());
    std::fs::remove_file(dir.path().join("target.json")).unwrap();
    error(&check(dir.path(), None, 2), "E_IO");
}
