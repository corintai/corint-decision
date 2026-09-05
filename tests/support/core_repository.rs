//! Synthetic publisher used by HTTP and process tests. Production publication
//! is owned by the repository operator, never by the Decision HTTP service.
use corint_decision_compiler::core::CoreSource;
use corint_decision_toolchain::{resolve, transfer::SourceBundle};
use serde_json::json;
use std::path::Path;

fn resolved(bundle: &SourceBundle) -> resolve::ResolvedClosure {
    let mut sources = bundle.sources.clone();
    sources.push(bundle.input_schema.clone());
    resolve::resolve_sources(
        &bundle.input_schema.path,
        &bundle
            .sources
            .iter()
            .map(|source| source.path.clone())
            .collect::<Vec<_>>(),
        &sources,
    )
    .unwrap()
}

pub fn identity(bundle: &SourceBundle) -> String {
    resolved(bundle).receipt().policy_sha256.clone()
}

pub fn publish(dir: &Path, bundle: &SourceBundle, revision: &str) {
    // Resolve first, so even this fixture cannot extract arbitrary bundle paths.
    let resolved = resolved(bundle);
    let root = dir.join("repository");
    std::fs::create_dir_all(&root).unwrap();
    for CoreSource { path, yaml } in bundle.sources.iter().chain([&bundle.input_schema]) {
        let destination = root.join(path);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(destination, yaml).unwrap();
    }
    let manifest = json!({
        "format":"corint-core-repository", "format_version":"1", "revision":revision,
        "input_schema":bundle.input_schema.path,
        "entries":bundle.sources.iter().map(|s| &s.path).collect::<Vec<_>>(),
        "policy_sha256":resolved.receipt().policy_sha256,
    });
    let pending = root.join("published.pending.json");
    std::fs::write(&pending, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    std::fs::rename(pending, root.join("published.json")).unwrap();
}
