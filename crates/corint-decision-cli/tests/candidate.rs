use corint_decision_toolchain::repository;
use serde_json::Value;
use std::{fs, path::Path, process::Command};
use tempfile::TempDir;

fn setup() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance");
    fs::create_dir(dir.path().join("author")).unwrap();
    for name in [
        "rule.yaml",
        "ruleset.yaml",
        "pipeline.yaml",
        "registry.yaml",
        "input-schema.yaml",
    ] {
        fs::copy(
            fixtures.join("cdl_core").join(name),
            dir.path().join("author").join(name),
        )
        .unwrap();
    }
    for (src, dst) in [
        ("cdl_core/behavior.yaml", "cases.yaml"),
        ("contracts/business-context.yaml", "context.yaml"),
        ("contracts/target-capabilities.json", "target.json"),
    ] {
        fs::copy(fixtures.join(src), dir.path().join(dst)).unwrap();
    }
    dir
}
fn run(root: &Path, extra: &[&str], exit: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_corint"))
        .current_dir(root)
        .args([
            "prepare-repository",
            "--root",
            "author",
            "--input-schema",
            "input-schema.yaml",
            "--cases",
            "cases.yaml",
            "--context",
            "context.yaml",
            "--target",
            "target.json",
            "--revision",
            "v1",
            "--output",
            "candidate",
            "--format",
            "json",
            "rule.yaml",
            "ruleset.yaml",
            "pipeline.yaml",
            "registry.yaml",
        ])
        .args(extra)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], exit == 0);
    assert_eq!(report["publication_approval"], "not_granted");
    assert_eq!(report["activated"], false);
    report
}

#[test]
fn candidate_contains_checked_originals_and_server_can_load_it() {
    let dir = setup();
    let before = fs::read(dir.path().join("author/rule.yaml")).unwrap();
    let report = run(dir.path(), &[], 0);
    let inventory: Value =
        serde_json::from_str(include_str!("../../../docs/cdl/schema/capabilities.json")).unwrap();
    let declared = &inventory["tools"]["prepare_repository_cli"];
    assert_eq!(declared["scope"], report["scope"]);
    assert_eq!(declared["activates_policy"], report["activated"]);
    assert_eq!(
        declared["publication_approval"],
        report["publication_approval"]
    );
    assert_eq!(report["test_results"]["passed"], 5);
    let candidate = repository::load(&dir.path().join("candidate")).unwrap();
    assert_eq!(
        report["candidate"]["policy_sha256"],
        candidate.closure.receipt().policy_sha256
    );
    assert_eq!(candidate.identity.revision, "v1");
    assert_eq!(
        fs::read(dir.path().join("candidate/rule.yaml")).unwrap(),
        before
    );
    assert_eq!(
        fs::read(dir.path().join("author/rule.yaml")).unwrap(),
        before
    );
    fs::write(dir.path().join("author/rule.yaml"), "invalid author edit").unwrap();
    assert_eq!(
        repository::load(&dir.path().join("candidate"))
            .unwrap()
            .identity,
        candidate.identity
    );
    let manifest = fs::read(dir.path().join("candidate/published.json")).unwrap();
    run(dir.path(), &[], 2);
    assert_eq!(
        fs::read(dir.path().join("candidate/published.json")).unwrap(),
        manifest
    );
}

#[test]
fn bad_behavior_or_target_does_not_write_candidate() {
    let dir = setup();
    let rule = dir.path().join("author/rule.yaml");
    let original = fs::read_to_string(&rule).unwrap();
    fs::write(&rule, original.replace("score: 60", "score: 61")).unwrap();
    let report = run(dir.path(), &[], 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_CANDIDATE_BEHAVIOR");
    assert_eq!(report["execution_checked"], true);
    assert!(report["test_results"]["failed"].as_u64().unwrap() > 0);
    assert!(report["test_results"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|case| case["passed"] == false));
    assert!(!dir.path().join("candidate").exists());
    fs::write(rule, original).unwrap();
    let target = dir.path().join("target.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    value["actions"] = serde_json::json!([]);
    fs::write(target, serde_json::to_vec(&value).unwrap()).unwrap();
    let report = run(dir.path(), &[], 1);
    assert_eq!(report["diagnostics"][0]["code"], "E_ACTION_UNAVAILABLE");
    assert!(!dir.path().join("candidate").exists());
}

#[test]
fn unknown_duplicate_and_escaping_inputs_fail_without_writes() {
    for (extra, exit) in [
        (vec!["--trust-me"], 2),
        (vec!["--revision", "v2"], 2),
        (vec!["../escape.yaml"], 1),
    ] {
        let dir = setup();
        run(dir.path(), &extra, exit);
        assert!(!dir.path().join("candidate").exists());
    }
}

#[cfg(unix)]
#[test]
fn candidate_cannot_overwrite_or_follow_existing_symlink() {
    let dir = setup();
    let protected = dir.path().join("protected");
    fs::create_dir(&protected).unwrap();
    fs::write(protected.join("marker"), "keep").unwrap();
    std::os::unix::fs::symlink(&protected, dir.path().join("candidate")).unwrap();
    run(dir.path(), &[], 2);
    assert_eq!(
        fs::read_to_string(protected.join("marker")).unwrap(),
        "keep"
    );
    assert!(!protected.join("published.json").exists());
}

#[test]
fn extended_runtime_candidate_runs_public_cli_and_repo_loader() {
    let dir = setup();
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/core_extensions");
    for name in [
        "rule.yaml",
        "marker.yaml",
        "ruleset.yaml",
        "pipeline.yaml",
        "child.yaml",
        "registry.yaml",
        "input-schema.yaml",
    ] {
        fs::copy(fixtures.join(name), dir.path().join("author").join(name)).unwrap();
    }
    fs::copy(
        fixtures.join("behavior.yaml"),
        dir.path().join("cases.yaml"),
    )
    .unwrap();
    fs::copy(
        fixtures.join("business-context.yaml"),
        dir.path().join("context.yaml"),
    )
    .unwrap();
    fs::copy(
        fixtures.join("target-capabilities.json"),
        dir.path().join("target.json"),
    )
    .unwrap();
    let report = run(dir.path(), &["marker.yaml", "child.yaml"], 0);
    assert_eq!(report["business_evaluation"], "not_performed");
    let loaded = repository::load(&dir.path().join("candidate")).unwrap();
    assert_eq!(
        loaded.closure.receipt().policy_sha256,
        report["candidate"]["policy_sha256"]
    );
}
