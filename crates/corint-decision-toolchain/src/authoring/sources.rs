use super::{Document, Options, Report};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) fn read(path: &Path, report: &mut Report) -> Option<String> {
    let result = std::fs::metadata(path).and_then(|meta| {
        if !meta.is_file() || meta.len() > 4 * 1024 * 1024 {
            return Err(std::io::Error::other(
                "Expected a regular file no larger than 4 MiB",
            ));
        }
        std::fs::read_to_string(path)
    });
    match result {
        Ok(text) => Some(text),
        Err(error) => {
            report.error(
                &path.to_string_lossy(),
                "",
                "load",
                "E_READ",
                error.to_string(),
            );
            None
        }
    }
}

pub(super) fn yaml(text: &str, source: &str, report: &mut Report) -> Option<Value> {
    let mut documents = vec![];
    for document in serde_yaml::Deserializer::from_str(text) {
        match serde_yaml::Value::deserialize(document).and_then(serde_yaml::from_value::<Value>) {
            Ok(value) => documents.push(value),
            Err(error) => {
                report.error(source, "", "parse", "E_YAML", error.to_string());
                if let Some(location) = error.location() {
                    let diag = report.diagnostics.last_mut().unwrap();
                    diag.line = Some(location.line());
                    diag.column = Some(location.column());
                }
                return None;
            }
        }
    }
    if documents.len() == 1 {
        return documents.pop();
    }
    if documents.len() == 2 {
        let mut body = documents.pop().unwrap();
        let header = documents.pop().unwrap();
        if let (Some(header), Some(body)) = (header.as_object(), body.as_object_mut()) {
            if !header.is_empty() && header.keys().all(|k| k == "version" || k == "import") {
                for (key, value) in header {
                    if body.contains_key(key) {
                        report.error(
                            source,
                            "",
                            "parse",
                            "E_DUPLICATE_FIELD",
                            format!("Field {key} occurs in both header and body"),
                        );
                        return None;
                    }
                    body.insert(key.clone(), value.clone());
                }
                return Some(Value::Object(body.clone()));
            }
        }
    }
    report.error(
        source,
        "",
        "parse",
        "E_DOCUMENT_COUNT",
        "Expected one resource document, optionally preceded by a version/import header",
    );
    None
}

fn scan(dir: &Path, files: &mut Vec<PathBuf>, report: &mut Report) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            report.error(
                &dir.to_string_lossy(),
                "",
                "load",
                "E_READ",
                error.to_string(),
            );
            return;
        }
    };
    let mut paths = vec![];
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(error) => report.error(
                &dir.to_string_lossy(),
                "",
                "load",
                "E_READ",
                error.to_string(),
            ),
        }
    }
    paths.sort();
    for path in paths {
        if path.is_symlink() {
            report.error(
                &path.to_string_lossy(),
                "",
                "load",
                "E_PATH",
                "Directory discovery does not follow symlinks",
            );
        } else if path.is_dir() {
            scan(&path, files, report);
        } else if path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| {
                ["yaml", "yml", "json"]
                    .iter()
                    .any(|candidate| ext.eq_ignore_ascii_case(candidate))
            })
        {
            files.push(path);
        }
    }
}

pub(super) fn load(options: &Options, report: &mut Report) -> Vec<Document> {
    let root = if let Some(path) = &options.root {
        match std::fs::canonicalize(path) {
            Ok(path) if path.is_dir() => Some(path),
            _ => {
                report.error(
                    &path.to_string_lossy(),
                    "",
                    "load",
                    "E_ROOT",
                    "Root must be an existing directory",
                );
                return vec![];
            }
        }
    } else {
        None
    };
    let mut files = Vec::new();
    for input in &options.files {
        let path = root
            .as_ref()
            .map_or_else(|| input.clone(), |root| root.join(input));
        if path.is_dir() {
            if path.is_symlink() {
                report.error(
                    &path.to_string_lossy(),
                    "",
                    "load",
                    "E_PATH",
                    "Directory discovery does not follow symlinks",
                );
            } else {
                scan(&path, &mut files, report);
            }
        } else {
            files.push(path);
        }
    }
    // Keep the existing root-only repository shortcut. Explicit paths always
    // select their own scope, including directories with arbitrary layouts.
    if options.files.is_empty() {
        if let Some(root) = &root {
            for dir in [
                "rules",
                "rulesets",
                "pipelines",
                "features",
                "lists",
                "services",
            ] {
                let dir = root.join(dir);
                if dir.exists() {
                    scan(&dir, &mut files, report);
                }
            }
            for file in ["registry.yaml", "registry.yml", "registry.json"] {
                let path = root.join(file);
                if path.exists() {
                    files.push(path);
                }
            }
        }
    }
    if files.is_empty() {
        report.error(
            "",
            "",
            "usage",
            "E_NO_SOURCES",
            "Supply CDL files or directories containing YAML/JSON resources",
        );
        return vec![];
    }
    let mut loader = Loader {
        root,
        documents: vec![],
        loaded: BTreeMap::new(),
        visiting: vec![],
        report,
    };
    for path in files {
        loader.load(&path, None);
    }
    loader.documents
}

struct Loader<'a> {
    root: Option<PathBuf>,
    documents: Vec<Document>,
    loaded: BTreeMap<PathBuf, Value>,
    visiting: Vec<PathBuf>,
    report: &'a mut Report,
}
impl Loader<'_> {
    fn load(&mut self, path: &Path, expected: Option<&str>) {
        let canonical = match std::fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) => {
                self.report.error(
                    &path.to_string_lossy(),
                    "",
                    "load",
                    "E_READ",
                    error.to_string(),
                );
                return;
            }
        };
        let source = canonical.to_string_lossy().into_owned();
        if self
            .root
            .as_ref()
            .is_some_and(|root| !canonical.starts_with(root))
        {
            self.report.error(
                &source,
                "",
                "load",
                "E_PATH",
                "Source/import is outside --root",
            );
            return;
        }
        if self.visiting.contains(&canonical) {
            self.report.error(
                &source,
                "/import",
                "reference",
                "E_IMPORT_CYCLE",
                "Circular import",
            );
            return;
        }
        if self.visiting.len() >= 128 || self.loaded.len() >= 4096 {
            self.report.error(
                &source,
                "/import",
                "load",
                "E_LIMIT",
                "Source graph exceeds 128 import levels or 4096 files",
            );
            return;
        }
        if let Some(value) = self.loaded.get(&canonical) {
            check_kind(value, expected, &source, self.report);
            return;
        }
        let Some(text) = read(&canonical, self.report) else {
            return;
        };
        if !self.report.sources.contains(&source) {
            self.report.sources.push(source.clone());
        }
        let Some(mut value) = yaml(&text, &source, self.report) else {
            return;
        };
        check_kind(&value, expected, &source, self.report);
        self.visiting.push(canonical.clone());
        if let Some(imports) = value.get("import") {
            if let Some(groups) = imports.as_object() {
                for (group, paths) in groups {
                    let kind = match group.as_str() {
                        "rules" => "rule",
                        "rulesets" => "ruleset",
                        "pipelines" => "pipeline",
                        "features" => "features",
                        "lists" => "list",
                        "services" => "service",
                        _ => {
                            self.report.error(
                                &source,
                                "/import",
                                "schema",
                                "E_UNKNOWN_FIELD",
                                format!("Unknown import group: {group}"),
                            );
                            continue;
                        }
                    };
                    if let Some(paths) = paths.as_array() {
                        for path in paths {
                            if let Some(path) = path.as_str().filter(|p| !p.trim().is_empty()) {
                                if let Some(root) = &self.root {
                                    let path = root.join(path);
                                    self.load(&path, Some(kind));
                                }
                                // Without --root, only validate the import declaration;
                                // explicit file/directory selection must not load siblings.
                            } else {
                                self.report.error(
                                    &source,
                                    "/import",
                                    "schema",
                                    "E_INVALID_STRUCTURE",
                                    "Import paths must be nonempty strings",
                                );
                            }
                        }
                    } else {
                        self.report.error(
                            &source,
                            "/import",
                            "schema",
                            "E_INVALID_STRUCTURE",
                            "Import group must be an array of paths",
                        );
                    }
                }
            } else {
                self.report.error(
                    &source,
                    "/import",
                    "schema",
                    "E_INVALID_STRUCTURE",
                    "Import must be a mapping",
                );
            }
            value.as_object_mut().unwrap().remove("import");
        }
        self.visiting.pop();
        self.loaded.insert(canonical, value.clone());
        self.documents.push(Document { source, value });
    }
}
fn check_kind(value: &Value, expected: Option<&str>, source: &str, report: &mut Report) {
    if let Some(expected) = expected {
        let matches = match expected {
            "list" => value.get("lists").is_some() || value.get("id").is_some(),
            "service" => value.get("base_url").is_some(),
            other => value.get(other).is_some(),
        };
        if !matches {
            report.error(
                source,
                "/import",
                "reference",
                "E_IMPORT_KIND",
                format!("Expected imported {expected} resource"),
            );
        }
    }
}
