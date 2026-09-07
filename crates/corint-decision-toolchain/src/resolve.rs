//! Explicit authoring/load-time imports, also used by strict repository startup
//! and reload. Runtime evaluation and legacy draft-1 parsing never read files.
use crate::{package, transfer::SourceBundle};
use corint_decision_compiler::core::{
    diagnostic, parse_core_input_schema, validate_core_document, CoreError, CoreSource,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const SOURCE_PROFILE: &str = "cdl-core-import-draft-1";
pub const HEADER_SCHEMA: &str = include_str!("../../../CDL/schema/import-header.json");
const MAX_FILES: usize = 256;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 32;

#[derive(Clone, Serialize)]
pub struct ImportEdge {
    pub kind: String,
    pub path: String,
}
#[derive(Serialize)]
pub struct SourceRecord {
    pub path: String,
    pub sha256: String,
    pub kind: String,
    pub id: String,
    pub imports: Vec<ImportEdge>,
}
#[derive(Serialize)]
pub struct ResolutionManifest {
    pub manifest_version: &'static str,
    pub source_profile: &'static str,
    pub entries: Vec<String>,
    pub input_path: String,
    pub input_sha256: String,
    pub sources: Vec<SourceRecord>,
}
#[derive(Serialize)]
pub struct ResolutionReceipt {
    pub manifest: ResolutionManifest,
    pub resolution_sha256: String,
    pub policy_sha256: String,
    pub bundle_sha256: String,
    pub execution_checked: bool,
    pub publication_approval: &'static str,
}
pub struct ResolvedClosure {
    bundle: SourceBundle,
    receipt: ResolutionReceipt,
    /// Original source bytes are available to in-process callers for an
    /// explicitly authorized provenance store. Not silently written or logged.
    originals: Vec<CoreSource>,
}
impl ResolvedClosure {
    pub fn bundle(&self) -> &SourceBundle {
        &self.bundle
    }
    pub fn receipt(&self) -> &ResolutionReceipt {
        &self.receipt
    }
    pub fn originals(&self) -> &[CoreSource] {
        &self.originals
    }
    pub fn into_receipt(self) -> ResolutionReceipt {
        self.receipt
    }
    pub fn write(&self, output: &Path) -> Result<(), CoreError> {
        package::write_bytes(&bundle_bytes(&self.bundle), output)
    }
}
fn bundle_bytes(bundle: &SourceBundle) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(bundle).expect("bundle JSON");
    bytes.push(b'\n');
    bytes
}
fn fail(path: &str, code: &str, message: impl Into<String>) -> CoreError {
    diagnostic(
        path,
        "",
        if code == "E_IO" { "load" } else { "resolve" },
        code,
        message,
    )
}
fn valid_path(path: &str) -> bool {
    let base = path
        .strip_suffix(".yaml")
        .or_else(|| path.strip_suffix(".yml"));
    path.len() <= 256
        && base.is_some_and(|base| {
            base.split('/').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            })
        })
}
fn path_check(path: &str) -> Result<(), CoreError> {
    if !valid_path(path) {
        return Err(fail(path, "E_INVALID_IMPORT", "Expected a root-relative ASCII YAML label; no absolute paths, dot segments, URLs or backslashes"));
    }
    Ok(())
}

/// Read only files reachable from explicit entries, using an anchored directory
/// descriptor on Unix. All symlink components are rejected (including internal
/// aliases), and files are opened nonblocking before checking regular-file type.
#[cfg(unix)]
pub fn resolve(root: &Path, input: &str, entries: &[String]) -> Result<ResolvedClosure, CoreError> {
    use std::{
        ffi::CString,
        fs::File,
        io::Read,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::OpenOptionsExt,
        },
    };
    let root_file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)
        .map_err(|e| fail("<root>", "E_IO", e.to_string()))?;
    resolve_with(input, entries, |path| {
        let mut directory = root_file
            .try_clone()
            .map_err(|e| fail(path, "E_IO", e.to_string()))?;
        let parts: Vec<_> = path.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            let name = CString::new(*part).expect("checked ASCII path");
            let final_part = i + 1 == parts.len();
            let flags = libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | if final_part { 0 } else { libc::O_DIRECTORY };
            // The directory descriptor remains owned throughout openat; path
            // components contain neither separators nor dot segments.
            let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                return Err(fail(
                    path,
                    "E_INVALID_IMPORT",
                    format!(
                        "Cannot open confined import: {}",
                        std::io::Error::last_os_error()
                    ),
                ));
            }
            // openat returned a new owned descriptor; RAII closes every handle.
            let file = unsafe { File::from_raw_fd(fd) };
            if final_part {
                if !file
                    .metadata()
                    .map_err(|e| fail(path, "E_IO", e.to_string()))?
                    .is_file()
                {
                    return Err(fail(
                        path,
                        "E_INVALID_IMPORT",
                        "Import must be a regular file",
                    ));
                }
                let mut bytes = Vec::new();
                file.take(MAX_FILE_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| fail(path, "E_IO", e.to_string()))?;
                if bytes.len() > MAX_FILE_BYTES {
                    return Err(fail(path, "E_IMPORT_LIMIT", "Source byte limit exceeded"));
                }
                return String::from_utf8(bytes)
                    .map_err(|_| fail(path, "E_INVALID_IMPORT", "Source must be UTF-8"));
            }
            directory = file;
        }
        unreachable!("checked nonempty label")
    })
}
#[cfg(not(unix))]
pub fn resolve(_: &Path, _: &str, _: &[String]) -> Result<ResolvedClosure, CoreError> {
    Err(fail("<root>", "E_INVALID_IMPORT", "Confined filesystem resolution currently requires Unix; use resolve_sources for an explicit in-memory repository"))
}

/// Same resolution/compilation without filesystem access, suitable for Agent
/// adapters. Paths are virtual repository labels, never local destinations.
pub fn resolve_sources(
    input: &str,
    entries: &[String],
    repository: &[CoreSource],
) -> Result<ResolvedClosure, CoreError> {
    let mut map = BTreeMap::new();
    for source in repository {
        path_check(&source.path)?;
        if map
            .insert(source.path.as_str(), source.yaml.as_str())
            .is_some()
        {
            return Err(fail(
                &source.path,
                "E_DUPLICATE_SOURCE",
                "Duplicate virtual source label",
            ));
        }
    }
    resolve_with(input, entries, |path| {
        map.get(path).map(|s| s.to_string()).ok_or_else(|| {
            fail(
                path,
                "E_INVALID_IMPORT",
                "Missing source in explicit repository",
            )
        })
    })
}

struct Parsed {
    normalized: CoreSource,
    record: SourceRecord,
    original: CoreSource,
}
fn parse(path: &str, yaml: String) -> Result<Parsed, CoreError> {
    corint_decision_dsl_parser::source_format::validate_rules_format(&yaml).map_err(|e| {
        let mut error = fail(path, "E_RULES_FORMAT", e.to_string());
        error.diagnostic.field_path = Some("/ruleset/rules".into());
        error.diagnostic.stage = Some("parse".into());
        error.diagnostic.line = Some(e.line);
        error.diagnostic.column = Some(e.column);
        error
    })?;
    let mut docs = Vec::new();
    for doc in serde_yaml::Deserializer::from_str(&yaml) {
        if docs.len() == 2 {
            return Err(fail(
                path,
                "E_INVALID_IMPORT",
                "At most an import header and one resource document are supported",
            ));
        }
        let value = serde_yaml::Value::deserialize(doc).map_err(|e| {
            let mut err = fail(path, "E_INVALID_STRUCTURE", e.to_string());
            if let Some(pos) = e.location() {
                err.diagnostic.line = Some(pos.line());
                err.diagnostic.column = Some(pos.column());
            }
            err
        })?;
        docs.push(
            serde_json::to_value(value)
                .map_err(|e| fail(path, "E_INVALID_STRUCTURE", e.to_string()))?,
        );
    }
    if docs.is_empty() {
        return Err(fail(path, "E_INVALID_STRUCTURE", "Empty YAML document"));
    }
    let version = docs[0]
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                path,
                "E_INVALID_VERSION",
                "Explicit string version is required on every source/header",
            )
        })?;
    if version != "0.1" {
        return Err(fail(
            path,
            "E_UNSUPPORTED_VERSION",
            "Only language version 0.1 is supported",
        ));
    }
    let has_import = docs[0].get("import").is_some();
    if docs.len() == 2 && !has_import {
        return Err(fail(
            path,
            "E_INVALID_IMPORT",
            "Two-document form requires an explicit import header",
        ));
    }
    let mut resource = docs.last().expect("nonempty").clone();
    let mut imports = Vec::new();
    if has_import {
        let header = if docs.len() == 1 {
            json!({"version":docs[0]["version"], "import":docs[0]["import"]})
        } else {
            docs[0].clone()
        };
        let schema: Value = serde_json::from_str(HEADER_SCHEMA).expect("header schema");
        let validator = jsonschema::JSONSchema::compile(&schema).expect("header validator");
        if let Err(mut errors) = validator.validate(&header) {
            if let Some(error) = errors.next() {
                return Err(diagnostic(
                    path,
                    &error.instance_path.to_string(),
                    "resolve",
                    "E_INVALID_IMPORT",
                    error.to_string(),
                ));
            }
        }
        for (group, kind) in [
            ("rules", "rule"),
            ("rulesets", "ruleset"),
            ("pipelines", "pipeline"),
        ] {
            if let Some(paths) = header["import"][group].as_array() {
                for path in paths {
                    imports.push(ImportEdge {
                        kind: kind.into(),
                        path: path.as_str().expect("schema string").into(),
                    });
                }
            }
        }
        let map = resource
            .as_object_mut()
            .ok_or_else(|| fail(path, "E_INVALID_STRUCTURE", "Expected a resource object"))?;
        if docs.len() == 1 {
            map.remove("import");
        }
        if docs.len() == 2 {
            if map.get("version").is_some_and(|v| v != "0.1") {
                return Err(fail(
                    path,
                    "E_UNSUPPORTED_VERSION",
                    "Header and resource versions conflict",
                ));
            }
            map.insert("version".into(), json!("0.1"));
        }
    }
    let normalized = CoreSource {
        path: path.into(),
        yaml: serde_yaml::to_string(&resource).expect("JSON YAML"),
    };
    validate_core_document(&normalized).map_err(|mut e| {
        e.diagnostic.line = None;
        e.diagnostic.column = None;
        e
    })?;
    let kind = ["rule", "ruleset", "pipeline", "registry"]
        .into_iter()
        .find(|k| resource.get(k).is_some())
        .expect("validated kind");
    let id = if kind == "registry" {
        "registry"
    } else {
        resource[kind]["id"].as_str().expect("validated ID")
    };
    Ok(Parsed {
        normalized,
        record: SourceRecord {
            path: path.into(),
            sha256: package::hash(yaml.as_bytes()),
            kind: kind.into(),
            id: id.into(),
            imports,
        },
        original: CoreSource {
            path: path.into(),
            yaml,
        },
    })
}

struct Resolver<F> {
    read: F,
    loaded: BTreeMap<String, Parsed>,
    visiting: BTreeSet<String>,
    stack: Vec<String>,
    bytes: usize,
    input: String,
}
impl<F: Fn(&str) -> Result<String, CoreError>> Resolver<F> {
    fn visit(&mut self, path: &str) -> Result<(), CoreError> {
        path_check(path)?;
        if path == self.input {
            return Err(fail(
                path,
                "E_INVALID_IMPORT",
                "Input schema cannot be imported as a resource",
            ));
        }
        if self.visiting.contains(path) {
            return Err(fail(
                path,
                "E_INVALID_IMPORT",
                format!("Import cycle: {} -> {path}", self.stack.join(" -> ")),
            ));
        }
        if self.loaded.contains_key(path) {
            return Ok(());
        }
        if self.stack.len() >= MAX_DEPTH || self.loaded.len() + self.visiting.len() >= MAX_FILES {
            return Err(fail(
                path,
                "E_IMPORT_LIMIT",
                "Import depth or file limit exceeded",
            ));
        }
        self.visiting.insert(path.into());
        self.stack.push(path.into());
        let yaml = (self.read)(path).map_err(|mut e| {
            e.diagnostic.message = format!(
                "{}; import chain: {}",
                e.diagnostic.message,
                self.stack.join(" -> ")
            );
            e
        })?;
        self.bytes += yaml.len();
        if yaml.len() > MAX_FILE_BYTES || self.bytes > MAX_TOTAL_BYTES {
            return Err(fail(path, "E_IMPORT_LIMIT", "Import byte limit exceeded"));
        }
        let parsed = parse(path, yaml)?;
        for edge in &parsed.record.imports {
            self.visit(&edge.path)?;
            if self.loaded[&edge.path].record.kind != edge.kind {
                return Err(fail(
                    path,
                    "E_IMPORT_KIND",
                    format!(
                        "{} must contain {}, not {}",
                        edge.path, edge.kind, self.loaded[&edge.path].record.kind
                    ),
                ));
            }
        }
        self.stack.pop();
        self.visiting.remove(path);
        self.loaded.insert(path.into(), parsed);
        Ok(())
    }
}
fn resolve_with(
    input: &str,
    entries: &[String],
    read: impl Fn(&str) -> Result<String, CoreError>,
) -> Result<ResolvedClosure, CoreError> {
    path_check(input)?;
    let roots: BTreeSet<_> = entries.iter().cloned().collect();
    if roots.is_empty() || roots.len() != entries.len() || roots.len() > MAX_FILES {
        return Err(fail(
            "<entries>",
            "E_INVALID_IMPORT",
            "Supply 1..256 unique explicit entry labels",
        ));
    }
    for path in &roots {
        path_check(path)?;
    }
    let input_source = CoreSource {
        path: input.into(),
        yaml: read(input)?,
    };
    if input_source.yaml.len() > MAX_FILE_BYTES {
        return Err(fail(
            input,
            "E_IMPORT_LIMIT",
            "Input schema byte limit exceeded",
        ));
    }
    parse_core_input_schema(&input_source)?;
    let mut resolver = Resolver {
        read,
        loaded: BTreeMap::new(),
        visiting: BTreeSet::new(),
        stack: Vec::new(),
        bytes: input_source.yaml.len(),
        input: input.into(),
    };
    for path in &roots {
        resolver.visit(path)?;
    }
    let mut sources = Vec::new();
    let mut records = Vec::new();
    let mut originals = vec![input_source.clone()];
    for parsed in resolver.loaded.into_values() {
        sources.push(parsed.normalized);
        records.push(parsed.record);
        originals.push(parsed.original);
    }
    let manifest = ResolutionManifest {
        manifest_version: "1",
        source_profile: SOURCE_PROFILE,
        entries: roots.into_iter().collect(),
        input_path: input.into(),
        input_sha256: package::hash(input_source.yaml.as_bytes()),
        sources: records,
    };
    let resolution_sha256 = package::json_hash("core-import-resolution-v1", json!(manifest));
    // Bind ORIGINAL bytes/graph to ordinary package identity without making the
    // runtime resolve paths or trusting comments as executable instructions.
    for source in &mut sources {
        source.yaml.push_str(&format!(
            "\n# corint-resolution-sha256: {resolution_sha256}\n"
        ));
    }
    let bundle = SourceBundle::new(input_source, sources)?;
    let policy_sha256 = package::policy_identity(&bundle.sources, &bundle.input_schema)?;
    let bundle_sha256 = package::hash(&bundle_bytes(&bundle));
    Ok(ResolvedClosure {
        bundle,
        originals,
        receipt: ResolutionReceipt {
            manifest,
            resolution_sha256,
            policy_sha256,
            bundle_sha256,
            execution_checked: false,
            publication_approval: "not_granted",
        },
    })
}
