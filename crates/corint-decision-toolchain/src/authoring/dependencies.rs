//! Resolve a file's resource references without validating unrelated neighbors.
use super::{sources, Document, Options, Reference, Report};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

type Index = BTreeMap<(String, String), Vec<PathBuf>>;

pub(super) fn options(options: &Options) -> Options {
    let mut effective = options.clone();
    if options.root.is_some() || options.files.is_empty() {
        return effective;
    }
    let files: Option<BTreeSet<_>> = options
        .files
        .iter()
        .map(|p| std::fs::canonicalize(p).ok().filter(|p| p.is_file()))
        .collect();
    let Some(files) = files.filter(|files| files.len() == 1) else {
        return effective;
    };
    let file = files.first().unwrap();
    let parent = file.parent().unwrap();
    // Prefer the nearest policy registry, and never search beyond a Git root.
    let root = parent
        .ancestors()
        .find(|dir| {
            ["registry.yaml", "registry.yml", "registry.json"]
                .iter()
                .any(|name| dir.join(name).is_file())
                || dir.join(".git").exists()
        })
        .filter(|dir| {
            ["registry.yaml", "registry.yml", "registry.json"]
                .iter()
                .any(|name| dir.join(name).is_file())
        });
    let root = root.unwrap_or_else(|| {
        // Conventional resource folders belong to their parent policy directory.
        if [
            "rules",
            "rulesets",
            "pipelines",
            "features",
            "lists",
            "services",
        ]
        .iter()
        .any(|name| parent.file_name().is_some_and(|n| n == *name))
        {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        }
    });
    effective.root = Some(root.to_path_buf());
    effective.files = files.into_iter().collect();
    effective
}

pub(super) struct Resolver {
    root: Option<PathBuf>,
    index: Option<Index>,
}

impl Resolver {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self { root, index: None }
    }

    pub fn load(&mut self, refs: &[Reference], report: &mut Report) -> Vec<Document> {
        let Some(root) = &self.root else {
            return vec![];
        };
        if refs.is_empty() {
            return vec![];
        }
        if self.index.is_none() {
            let mut files = vec![];
            // Discovery is an index only. Unrelated malformed documents and
            // symlinks do not become validation inputs.
            sources::scan(root, &mut files, &mut Report::empty());
            if files.len() > 4096 {
                report.error(
                    &root.to_string_lossy(),
                    "",
                    "load",
                    "E_LIMIT",
                    "Dependency search exceeds 4096 files; select a narrower --root",
                );
                self.index = Some(Index::new());
                return vec![];
            }
            let mut index = Index::new();
            for (path, _) in files {
                let Some(text) = sources::read(&path, &mut Report::empty()) else {
                    continue;
                };
                // Index every declaration in a shared file, including repeated kinds.
                if let Ok(values) = super::bundle::parse(&text) {
                    for value in values {
                        index_value(&value, &path, &mut index);
                    }
                }
            }
            self.index = Some(index);
        }
        let mut paths = BTreeSet::new();
        for reference in refs {
            if let Some(found) = self
                .index
                .as_ref()
                .unwrap()
                .get(&(reference.kind.clone(), reference.id.clone()))
            {
                paths.extend(found.iter().cloned());
            }
        }
        let mut documents = vec![];
        for path in paths {
            let canonical = std::fs::canonicalize(&path).unwrap_or(path);
            if report.sources.iter().any(|s| Path::new(s) == canonical) {
                continue;
            }
            if report.sources.len() >= 4096 {
                report.error(
                    &canonical.to_string_lossy(),
                    "",
                    "load",
                    "E_LIMIT",
                    "Dependency closure exceeds 4096 files",
                );
                break;
            }
            let mut loaded = Report::empty();
            let options = Options {
                root: Some(root.clone()),
                files: vec![canonical],
                input_schema: None,
            };
            let new = sources::load(&options, &mut loaded);
            for document in new {
                if !report.sources.contains(&document.source) {
                    documents.push(document);
                }
            }
            for source in loaded.sources {
                if !report.sources.contains(&source) {
                    report.sources.push(source);
                }
            }
            report.diagnostics.extend(loaded.diagnostics);
        }
        documents
    }
}

fn index_value(value: &Value, path: &Path, index: &mut Index) {
    let mut add = |kind: &str, id: Option<&str>| {
        if let Some(id) = id {
            let files = index.entry((kind.into(), id.into())).or_default();
            if !files.iter().any(|p| p == path) {
                files.push(path.to_path_buf());
            }
        }
    };
    for kind in ["rule", "ruleset", "pipeline"] {
        add(kind, value[kind]["id"].as_str());
    }
    for feature in value["features"].as_array().into_iter().flatten() {
        add("feature", feature["name"].as_str());
    }
    for list in value["lists"].as_array().into_iter().flatten() {
        add("list", list["id"].as_str());
    }
    add("list", value["id"].as_str());
    if value.get("base_url").is_some() || value.get("operations").is_some() {
        add("service", value["name"].as_str());
    }
}
