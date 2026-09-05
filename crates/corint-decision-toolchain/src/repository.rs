//! Read a published Core policy from its sole authority: a filesystem repository.
//! The publisher owns version history and atomically replaces `published.json`.
//! Readers freeze and validate the closure; they never write a competing store.
use crate::{failure, resolve};
use corint_decision_compiler::core::CoreError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::OnceLock};

pub const MANIFEST: &str = "published.json";
pub const SCHEMA: &str = include_str!("../../../docs/contracts/schema/core-repository.json");
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedManifest {
    pub format: String,
    pub format_version: String,
    pub revision: String,
    pub input_schema: String,
    pub entries: Vec<String>,
    pub policy_sha256: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RepositoryIdentity {
    pub revision: String,
    pub manifest_sha256: String,
}

pub struct RepositorySnapshot {
    pub identity: RepositoryIdentity,
    pub closure: resolve::ResolvedClosure,
}

fn error(code: &str, message: impl Into<String>) -> CoreError {
    failure(MANIFEST, "load", code, message)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Use a fixed filename, a directory descriptor and no-follow/nonblocking opens.
/// Repository configuration is operator-owned; HTTP never supplies a local path.
#[cfg(unix)]
fn read_manifest(root: &Path) -> Result<Vec<u8>, CoreError> {
    use std::{
        fs::{File, OpenOptions},
        io::Read,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::OpenOptionsExt,
        },
    };
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)
        .map_err(|_| error("E_REPOSITORY_IO", "Cannot open repository directory"))?;
    // The fixed name has one component and the directory remains owned here.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            c"published.json".as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(error(
            "E_REPOSITORY_IO",
            "Cannot open published repository manifest",
        ));
    }
    // openat returned an owned descriptor; File closes it on every exit.
    let file = unsafe { File::from_raw_fd(fd) };
    if !file
        .metadata()
        .map_err(|_| error("E_REPOSITORY_IO", "Cannot inspect repository manifest"))?
        .is_file()
    {
        return Err(error(
            "E_REPOSITORY_IO",
            "Repository manifest must be a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("E_REPOSITORY_IO", "Cannot read repository manifest"))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(error(
            "E_REPOSITORY_MANIFEST",
            "Repository manifest exceeds 64 KiB",
        ));
    }
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_manifest(_: &Path) -> Result<Vec<u8>, CoreError> {
    Err(error(
        "E_REPOSITORY_IO",
        "Confined filesystem repositories currently require Unix",
    ))
}

fn parse_manifest(bytes: &[u8]) -> Result<PublishedManifest, CoreError> {
    // Typed decoding rejects duplicate fields before JSON Schema validation.
    let manifest: PublishedManifest = serde_json::from_slice(bytes).map_err(|_| {
        error(
            "E_REPOSITORY_MANIFEST",
            "Invalid published repository manifest",
        )
    })?;
    static VALIDATOR: OnceLock<jsonschema::JSONSchema> = OnceLock::new();
    let validator = VALIDATOR.get_or_init(|| {
        jsonschema::JSONSchema::compile(
            &serde_json::from_str(SCHEMA).expect("repository schema JSON"),
        )
        .expect("repository schema")
    });
    if !validator.is_valid(&serde_json::to_value(&manifest).expect("manifest JSON")) {
        return Err(error(
            "E_REPOSITORY_MANIFEST",
            "Unsupported or invalid published repository manifest",
        ));
    }
    Ok(manifest)
}

pub fn load(root: &Path) -> Result<RepositorySnapshot, CoreError> {
    let bytes = read_manifest(root)?;
    let manifest = parse_manifest(&bytes)?;
    let closure = resolve::resolve(root, &manifest.input_schema, &manifest.entries)?;
    if closure.receipt().policy_sha256 != manifest.policy_sha256 {
        return Err(error(
            "E_REPOSITORY_DIGEST",
            "Repository source closure differs from the published policy fingerprint",
        ));
    }
    let identity = RepositoryIdentity {
        revision: manifest.revision,
        manifest_sha256: sha256(&bytes),
    };
    verify_current(root, &identity)?;
    Ok(RepositorySnapshot { identity, closure })
}

/// Recheck after expensive compilation/acceptance tests. A changed publication
/// is retried as a new reload, never combined with an already prepared engine.
pub fn verify_current(root: &Path, identity: &RepositoryIdentity) -> Result<(), CoreError> {
    if sha256(&read_manifest(root)?) != identity.manifest_sha256 {
        return Err(error(
            "E_REPOSITORY_CHANGED",
            "Repository publication changed during preparation; reload again",
        ));
    }
    Ok(())
}
