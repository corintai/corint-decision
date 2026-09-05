use corint_decision_compiler::core::CoreSource;
use corint_decision_toolchain::{repository, transfer::SourceBundle};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[path = "../../../tests/support/core_repository.rs"]
mod repository_fixture;

fn fixture() -> SourceBundle {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core");
    let source = |name: &str| CoreSource {
        path: name.into(),
        yaml: fs::read_to_string(root.join(name)).unwrap(),
    };
    SourceBundle::new(
        source("input-schema.yaml"),
        [
            "rule.yaml",
            "ruleset.yaml",
            "pipeline.yaml",
            "registry.yaml",
        ]
        .iter()
        .map(|name| source(name))
        .collect(),
    )
    .unwrap()
}
fn setup() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    repository_fixture::publish(dir.path(), &fixture(), "release-1");
    let root = dir.path().join("repository");
    (dir, root)
}
fn failed(root: &Path, code: &str) {
    match repository::load(root) {
        Err(error) => assert_eq!(error.diagnostic.code, code, "{error}"),
        Ok(_) => panic!("unexpected repository acceptance"),
    }
}

#[test]
fn published_closure_is_pinned_and_loading_does_not_mutate_repository() {
    let (_dir, root) = setup();
    let before = fs::read(root.join(repository::MANIFEST)).unwrap();
    let snapshot = repository::load(&root).unwrap();
    assert_eq!(snapshot.identity.revision, "release-1");
    assert_eq!(
        snapshot.closure.receipt().policy_sha256,
        repository_fixture::identity(&fixture())
    );
    assert_eq!(fs::read(root.join(repository::MANIFEST)).unwrap(), before);
    fs::write(root.join("unrelated.yaml"), "not a policy").unwrap();
    assert_eq!(repository::load(&root).unwrap().identity, snapshot.identity);
    // A valid but unpublished source edit must not silently change execution.
    let rule = root.join("rule.yaml");
    fs::write(
        &rule,
        fs::read_to_string(&rule)
            .unwrap()
            .replace("> 1000", "> 1200"),
    )
    .unwrap();
    failed(&root, "E_REPOSITORY_DIGEST");
}

#[test]
fn manifest_versions_duplicates_paths_and_missing_dependencies_are_rejected() {
    let (_dir, root) = setup();
    let path = root.join(repository::MANIFEST);
    let original = fs::read_to_string(&path).unwrap();
    for (key, value, code) in [
        ("format_version", json!("2"), "E_REPOSITORY_MANIFEST"),
        ("approval", json!(true), "E_REPOSITORY_MANIFEST"),
        ("revision", json!(""), "E_REPOSITORY_MANIFEST"),
        (
            "policy_sha256",
            json!("0".repeat(64)),
            "E_REPOSITORY_DIGEST",
        ),
        ("entries", json!(["../outside.yaml"]), "E_INVALID_IMPORT"),
        ("input_schema", json!("/tmp/input.yaml"), "E_INVALID_IMPORT"),
    ] {
        let mut manifest: Value = serde_json::from_str(&original).unwrap();
        manifest[key] = value;
        fs::write(&path, manifest.to_string()).unwrap();
        failed(&root, code);
    }
    fs::write(
        &path,
        original.replacen('{', "{\"revision\":\"duplicate\",", 1),
    )
    .unwrap();
    failed(&root, "E_REPOSITORY_MANIFEST");
    fs::write(&path, &original).unwrap();
    fs::remove_file(root.join("rule.yaml")).unwrap();
    failed(&root, "E_INVALID_IMPORT");
    fs::remove_file(path).unwrap();
    failed(&root, "E_REPOSITORY_IO");
}

#[test]
fn changed_publication_is_detected_after_preparation_and_rollback_is_repo_owned() {
    let (dir, root) = setup();
    let before = repository::load(&root).unwrap().identity;
    repository_fixture::publish(dir.path(), &fixture(), "release-2");
    assert_eq!(
        repository::verify_current(&root, &before)
            .unwrap_err()
            .diagnostic
            .code,
        "E_REPOSITORY_CHANGED"
    );
    assert_eq!(
        repository::load(&root).unwrap().identity.revision,
        "release-2"
    );
    repository_fixture::publish(dir.path(), &fixture(), "release-1");
    assert_eq!(repository::load(&root).unwrap().identity, before);
}

#[cfg(unix)]
#[test]
fn manifest_symlink_fifo_and_oversized_content_are_rejected_without_blocking() {
    let (_dir, root) = setup();
    let path = root.join(repository::MANIFEST);
    fs::rename(&path, root.join("real.json")).unwrap();
    std::os::unix::fs::symlink("real.json", &path).unwrap();
    failed(&root, "E_REPOSITORY_IO");
    fs::remove_file(&path).unwrap();
    let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    failed(&root, "E_REPOSITORY_IO");
    fs::remove_file(&path).unwrap();
    fs::write(&path, " ".repeat(64 * 1024 + 1)).unwrap();
    failed(&root, "E_REPOSITORY_MANIFEST");
}
