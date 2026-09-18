//! Operator-owned catalog. Tool arguments select IDs, never filesystem paths.
use anyhow::{Context, Result};
use corint_decision_compiler::core::CoreSource;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

const MAX_FILE: usize = 4 * 1024 * 1024;
const MAX_TOTAL: usize = 16 * 1024 * 1024;
pub const MAX_CASES_BYTES: usize = 1024 * 1024;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub policies: Vec<Policy>,
    #[serde(skip)]
    pub repository: Option<PathBuf>,
}

pub enum CatalogSource {
    Configured(Catalog),
    Repository(PathBuf),
}

impl CatalogSource {
    pub fn load(&self) -> Result<Catalog> {
        match self {
            Self::Configured(catalog) => Ok(catalog.clone()),
            Self::Repository(root) => Catalog::from_repository(root),
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub root: PathBuf,
    /// Complete explicit source closure; imports must also be listed.
    pub files: Vec<String>,
    pub input_schema: Option<String>,
    pub cases: Option<String>,
    #[serde(skip)]
    pub kind: Option<String>,
    #[serde(skip)]
    pub resource_id: Option<String>,
    #[serde(skip)]
    repository_sources: Option<Arc<Vec<CoreSource>>>,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub policy_id: String,
    /// Adapter-specific content identity, not a repository publication fingerprint.
    pub snapshot_sha256: String,
    pub sources: Vec<CoreSource>,
    pub input_schema: Option<CoreSource>,
}

impl Catalog {
    pub fn from_repository(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .context("Cannot locate policy repository")?;
        anyhow::ensure!(root.is_dir(), "Policy repository must be a directory");
        let mut labels = Vec::new();
        // Only policy resource trees; never capture datasource/auth configuration.
        for name in [
            "rules",
            "rulesets",
            "pipelines",
            "features",
            "lists",
            "services",
        ] {
            let path = root.join(name);
            if path.try_exists()? {
                scan(&root, &path, &mut labels, 0)?;
            }
        }
        labels.sort();
        let reader = Policy {
            id: String::new(),
            description: String::new(),
            root: root.clone(),
            files: vec![],
            input_schema: None,
            cases: None,
            kind: None,
            resource_id: None,
            repository_sources: None,
        };
        let mut sources = Vec::new();
        let mut policies = Vec::new();
        let mut ids = BTreeSet::new();
        let mut total = 0;
        for label in labels {
            let source = reader.read(&label)?;
            total += source.yaml.len();
            anyhow::ensure!(total <= MAX_TOTAL, "Repository sources exceed 16 MiB");
            for value in corint_decision_toolchain::authoring::parse_source(&source.yaml)
                .with_context(|| format!("Invalid repository CDL: {label}"))?
            {
                for kind in ["pipeline", "ruleset"] {
                    let Some(resource) = value.get(kind) else {
                        continue;
                    };
                    let id = resource
                        .get("id")
                        .and_then(|v| v.as_str())
                        .with_context(|| format!("Missing {kind}.id in {label}"))?;
                    anyhow::ensure!(
                        !id.is_empty()
                            && id.len() <= 256
                            && id
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric()
                                    || matches!(b, b'_' | b'-' | b'.')),
                        "Unsupported repository resource ID: {id}"
                    );
                    let policy_id = format!("{kind}/{id}");
                    anyhow::ensure!(
                        ids.insert(policy_id.clone()),
                        "Duplicate repository resource: {policy_id}"
                    );
                    policies.push(Policy {
                        id: policy_id,
                        description: resource
                            .get("description")
                            .or_else(|| resource.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .into(),
                        root: root.clone(),
                        files: vec![label.clone()],
                        input_schema: None,
                        cases: None,
                        kind: Some(kind.into()),
                        resource_id: Some(id.into()),
                        repository_sources: None,
                    });
                }
            }
            sources.push(source);
        }
        let sources = Arc::new(sources);
        for policy in &mut policies {
            policy.repository_sources = Some(sources.clone());
        }
        policies.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self {
            policies,
            repository: Some(root),
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let path = path.canonicalize().context("Cannot locate MCP config")?;
        let mut text = String::new();
        std::fs::File::open(&path)?
            .take(MAX_CASES_BYTES as u64 + 1)
            .read_to_string(&mut text)?;
        anyhow::ensure!(text.len() <= MAX_CASES_BYTES, "MCP config exceeds 1 MiB");
        let mut catalog: Self = serde_json::from_str(&text).context("Invalid MCP config JSON")?;
        anyhow::ensure!(
            !catalog.policies.is_empty() && catalog.policies.len() <= 64,
            "Configure 1–64 policies"
        );
        let mut ids = BTreeSet::new();
        for policy in &mut catalog.policies {
            anyhow::ensure!(
                !policy.id.is_empty()
                    && policy.id.len() <= 80
                    && policy
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                "Policy IDs must be 1–80 ASCII letters, digits, '-' or '_'"
            );
            anyhow::ensure!(
                ids.insert(policy.id.clone()),
                "Duplicate policy ID: {}",
                policy.id
            );
            anyhow::ensure!(
                !policy.files.is_empty() && policy.files.len() <= 256,
                "Policy {} must list 1–256 source files",
                policy.id
            );
            let mut labels = BTreeSet::new();
            for label in policy
                .files
                .iter()
                .chain(policy.input_schema.iter())
                .chain(policy.cases.iter())
            {
                check_label(label)?;
                anyhow::ensure!(
                    labels.insert(label),
                    "Duplicate file label in {}: {label}",
                    policy.id
                );
            }
            policy.root = path
                .parent()
                .expect("absolute config path")
                .join(&policy.root)
                .canonicalize()
                .with_context(|| format!("Cannot locate root for {}", policy.id))?;
            anyhow::ensure!(policy.root.is_dir(), "Policy root must be a directory");
        }
        Ok(catalog)
    }

    pub fn policy(&self, id: &str) -> Result<&Policy> {
        self.policies
            .iter()
            .find(|policy| policy.id == id)
            .with_context(|| format!("Unknown policy_id: {id}. Call list_policies first."))
    }
}

fn scan(root: &Path, path: &Path, labels: &mut Vec<String>, depth: usize) -> Result<()> {
    anyhow::ensure!(depth <= 32, "Repository directory nesting exceeds 32");
    let metadata = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "Repository symlink is not allowed: {}",
        path.display()
    );
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            scan(root, &entry?.path(), labels, depth + 1)?;
        }
    } else if path
        .extension()
        .is_some_and(|ext| ext == "yaml" || ext == "yml" || ext == "json")
    {
        anyhow::ensure!(metadata.is_file(), "Expected a regular policy file");
        let label = path
            .strip_prefix(root)?
            .to_str()
            .context("Non-UTF-8 source path")?
            .to_owned();
        check_label(&label)?;
        labels.push(label);
        anyhow::ensure!(labels.len() <= 4096, "Repository exceeds 4096 source files");
    }
    Ok(())
}

fn check_label(label: &str) -> Result<()> {
    anyhow::ensure!(
        !label.is_empty()
            && label.len() <= 256
            && label.split('/').all(|part| !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))),
        "Expected a root-relative file label without traversal: {label}"
    );
    Ok(())
}

impl Policy {
    pub fn snapshot(&self) -> Result<Snapshot> {
        if let Some(sources) = &self.repository_sources {
            // Resolve dependencies with the existing CDL validator, against only
            // captured policy bytes. Source reads do not require valid compilation.
            let dir = tempfile::tempdir()?;
            for source in sources.iter() {
                let path = dir.path().join(&source.path);
                std::fs::create_dir_all(path.parent().expect("source parent"))?;
                std::fs::write(path, &source.yaml)?;
            }
            let report = corint_decision_toolchain::authoring::validate(
                &corint_decision_toolchain::authoring::Options {
                    files: self.files.iter().map(|f| dir.path().join(f)).collect(),
                    root: Some(dir.path().to_owned()),
                    input_schema: None,
                },
            );
            let root = dir.path().canonicalize()?;
            let selected: BTreeSet<_> = report
                .sources
                .iter()
                .filter_map(|s| Path::new(s).strip_prefix(&root).ok())
                .map(|p| p.to_string_lossy().into_owned())
                .chain(self.files.iter().cloned())
                .collect();
            let captured = sources
                .iter()
                .filter(|s| selected.contains(&s.path))
                .cloned()
                .collect();
            return make_snapshot(&self.id, captured, None);
        }
        let mut total = 0;
        let sources = self
            .files
            .iter()
            .map(|file| {
                let source = self.read(file)?;
                total += source.yaml.len();
                anyhow::ensure!(total <= MAX_TOTAL, "Policy snapshot exceeds 16 MiB");
                Ok(source)
            })
            .collect::<Result<Vec<_>>>()?;
        let input_schema = self
            .input_schema
            .as_ref()
            .map(|file| self.read(file))
            .transpose()?;
        anyhow::ensure!(
            sources
                .iter()
                .chain(input_schema.iter())
                .map(|s| s.yaml.len())
                .sum::<usize>()
                <= MAX_TOTAL,
            "Policy snapshot exceeds 16 MiB"
        );
        make_snapshot(&self.id, sources, input_schema)
    }

    pub fn suite(&self, inline: Option<String>) -> Result<CoreSource> {
        let source = match inline {
            Some(yaml) => CoreSource {
                path: "<caller-cases>".into(),
                yaml,
            },
            None => self.read(
                self.cases
                    .as_deref()
                    .context("No configured cases; provide cases_yaml")?,
            )?,
        };
        anyhow::ensure!(
            source.yaml.len() <= MAX_CASES_BYTES,
            "Behavior suite exceeds 1 MiB"
        );
        Ok(source)
    }

    fn read(&self, label: &str) -> Result<CoreSource> {
        check_label(label)?;
        let file = open_file(&self.root, label).with_context(|| format!("Cannot read {label}"))?;
        let metadata = file.metadata()?;
        anyhow::ensure!(
            metadata.is_file() && metadata.len() <= MAX_FILE as u64,
            "{label}: expected a regular file no larger than 4 MiB"
        );
        let mut yaml = String::new();
        file.take(MAX_FILE as u64 + 1).read_to_string(&mut yaml)?;
        anyhow::ensure!(yaml.len() <= MAX_FILE, "{label}: file exceeds 4 MiB");
        Ok(CoreSource {
            path: label.into(),
            yaml,
        })
    }
}

fn make_snapshot(
    id: &str,
    sources: Vec<CoreSource>,
    input_schema: Option<CoreSource>,
) -> Result<Snapshot> {
    let bytes = serde_json::to_vec(&(&sources, &input_schema))?;
    let mut hash = Sha256::new();
    hash.update(b"corint-mcp-snapshot-v1\0");
    hash.update(bytes);
    Ok(Snapshot {
        policy_id: id.into(),
        snapshot_sha256: format!("{:x}", hash.finalize()),
        sources,
        input_schema,
    })
}

/// Anchor every component to a directory descriptor. No symlink following or FIFO blocking.
#[cfg(unix)]
fn open_file(root: &Path, label: &str) -> Result<std::fs::File> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::OpenOptionsExt,
        },
    };
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    let parts: Vec<_> = label.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        let name = CString::new(*part)?;
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if index + 1 == parts.len() {
                0
            } else {
                libc::O_DIRECTORY
            };
        // SAFETY: file owns a live directory descriptor and name is a valid C string.
        let fd = unsafe { libc::openat(file.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: openat returned a new, uniquely owned descriptor.
        file = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_file(_root: &Path, _label: &str) -> Result<std::fs::File> {
    anyhow::bail!("The local MCP file adapter currently requires Unix")
}

impl Snapshot {
    pub fn schema(&self) -> Result<corint_decision_engine::Schema> {
        Ok(corint_decision_compiler::core::parse_core_input_schema(
            self.input_schema
                .as_ref()
                .context("Core execution requires a configured input_schema")?,
        )?)
    }

    pub fn validate(&self) -> Result<serde_json::Value> {
        // Run the existing validator on exactly the captured source bytes. Its
        // import/reference resolver cannot read files outside this temporary root.
        let dir = tempfile::tempdir()?;
        for source in self.sources.iter().chain(self.input_schema.iter()) {
            let path = dir.path().join(&source.path);
            std::fs::create_dir_all(path.parent().expect("source parent"))?;
            std::fs::write(path, &source.yaml)?;
        }
        let report = corint_decision_toolchain::authoring::validate(
            &corint_decision_toolchain::authoring::Options {
                files: self
                    .sources
                    .iter()
                    .map(|source| dir.path().join(&source.path))
                    .collect(),
                root: Some(dir.path().to_owned()),
                input_schema: self
                    .input_schema
                    .as_ref()
                    .map(|source| dir.path().join(&source.path)),
            },
        );
        let mut value = serde_json::to_value(report)?;
        let prefix = dir.path().canonicalize()?.to_string_lossy().into_owned();
        remap_paths(&mut value, &prefix);
        value["snapshot_sha256"] = self.snapshot_sha256.clone().into();
        Ok(value)
    }
}

fn remap_paths(value: &mut serde_json::Value, prefix: &str) {
    match value {
        serde_json::Value::String(text) => {
            if text == prefix {
                *text = ".".into();
            } else if let Some(label) = text.strip_prefix(&format!("{prefix}/")) {
                *text = label.into();
            }
        }
        serde_json::Value::Array(values) => values
            .iter_mut()
            .for_each(|value| remap_paths(value, prefix)),
        serde_json::Value::Object(values) => values
            .values_mut()
            .for_each(|value| remap_paths(value, prefix)),
        _ => {}
    }
}
