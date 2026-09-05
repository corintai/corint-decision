use corint_decision_compiler::core::CoreSource;
use corint_decision_toolchain::{candidate, contracts::TargetContracts, repository};
use std::{fs, path::Path};

#[test]
fn candidate_uses_frozen_import_originals_after_author_edits() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance");
    let dir = tempfile::tempdir().unwrap();
    let author = dir.path().join("author");
    for name in [
        "input-schema.yaml",
        "registry.yaml",
        "pipelines/payment.yaml",
        "rulesets/risk.yaml",
        "rules/amount.yaml",
    ] {
        let path = author.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(fixtures.join("cdl_imports").join(name), path).unwrap();
    }
    let read = |name: &str| CoreSource {
        path: name.into(),
        yaml: fs::read_to_string(fixtures.join(name)).unwrap(),
    };
    let target = TargetContracts::load(
        &read("contracts/business-context.yaml"),
        &read("contracts/target-capabilities.json"),
    )
    .unwrap();
    let prepared = candidate::prepare(
        &author,
        "input-schema.yaml",
        &["registry.yaml".into()],
        "imports-v1",
        &read("cdl_core/behavior.yaml"),
        &target,
    )
    .unwrap();
    let checked = prepared.compatibility().policy_sha256.clone();
    let original = fs::read(author.join("rules/amount.yaml")).unwrap();
    fs::write(author.join("rules/amount.yaml"), "invalid subsequent edit").unwrap();
    let output = dir.path().join("candidate");
    let receipt = prepared.write(&output).unwrap();
    let loaded = repository::load(&output).unwrap();
    assert_eq!(receipt.policy_sha256, checked);
    assert_eq!(loaded.closure.receipt().policy_sha256, checked);
    assert_eq!(
        fs::read(output.join("rules/amount.yaml")).unwrap(),
        original
    );
    assert!(fs::read_to_string(output.join("registry.yaml"))
        .unwrap()
        .contains("import:"));
    assert!(prepared.write(&output).is_err());
}
