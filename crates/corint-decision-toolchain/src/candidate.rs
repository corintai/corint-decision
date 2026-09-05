//! Prepare a new, self-contained filesystem repository for operator review.
//! Never changes the active repository or grants publication authority.
use crate::{behavior, contracts, failure, package, repository, resolve};
use corint_decision_compiler::core::{parse_core_input_schema, CoreError, CoreSource};
use serde::Serialize;
use serde_json::json;
use std::{fs, io::Write, path::Path};

pub struct Candidate {
    closure: resolve::ResolvedClosure,
    revision: String,
    tests: behavior::TestResults,
    compatibility: contracts::CompatibilityReport,
}
#[derive(Serialize)]
pub struct CandidateReceipt {
    pub repository: repository::RepositoryIdentity,
    pub policy_sha256: String,
    pub publication_approval: &'static str,
    pub activated: bool,
}

/// Retain failed behavior results for the author to inspect. `write` refuses
/// candidates with failed or unexecuted cases before creating any output.
pub fn prepare(
    root: &Path,
    input_label: &str,
    entries: &[String],
    revision: &str,
    cases: &CoreSource,
    target: &contracts::TargetContracts,
) -> Result<Candidate, CoreError> {
    if revision.is_empty()
        || revision.len() > 128
        || !revision
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    {
        return Err(failure(
            "<revision>",
            "validate",
            "E_REPOSITORY_REVISION",
            "Revision requires 1..128 ASCII letters, digits, underscore, dot or dash",
        ));
    }
    let closure = resolve::resolve(root, input_label, entries)?;
    let bundle = closure.bundle();
    let compatibility = target.check(&bundle.sources, &bundle.input_schema, None)?;
    let schema = parse_core_input_schema(&bundle.input_schema)?;
    let tests = behavior::test(&bundle.sources, schema, cases)?;
    Ok(Candidate {
        closure,
        revision: revision.into(),
        tests,
        compatibility,
    })
}

impl Candidate {
    pub fn tests(&self) -> &behavior::TestResults {
        &self.tests
    }
    pub fn compatibility(&self) -> &contracts::CompatibilityReport {
        &self.compatibility
    }

    /// Output must not exist. Original bytes come from the already frozen read,
    /// so edits to the authoring directory cannot change the checked candidate.
    /// A write failure before the final rename leaves an incomplete directory
    /// without a manifest. The caller must not deploy a failed command output.
    pub fn write(&self, output: &Path) -> Result<CandidateReceipt, CoreError> {
        if self.tests.failed != 0 || self.tests.executed == 0 {
            return Err(failure(
                "<candidate>",
                "test",
                "E_CANDIDATE_BEHAVIOR",
                "Candidate behavior tests failed; no repository was written",
            ));
        }
        let io_error = |error: std::io::Error| {
            failure(
                &output.display().to_string(),
                "write",
                "E_IO",
                error.to_string(),
            )
        };
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(output).map_err(io_error)?;
        for source in self.closure.originals() {
            // All labels were checked by the resolver before any output exists.
            let path = output.join(&source.path);
            fs::create_dir_all(path.parent().expect("source parent")).map_err(io_error)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(io_error)?;
            file.write_all(source.yaml.as_bytes()).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
        }
        let manifest = json!({
            "format":"corint-core-repository", "format_version":"1",
            "revision": self.revision,
            "input_schema": self.closure.receipt().manifest.input_path,
            "entries": self.closure.receipt().manifest.entries,
            "policy_sha256": self.closure.receipt().policy_sha256,
        });
        // The server only recognizes a complete manifest. Do not mark an output
        // loadable until all checked original files have been written.
        let pending = output.join("candidate.pending.json");
        package::write_bytes(
            &serde_json::to_vec_pretty(&manifest).expect("manifest JSON"),
            &pending,
        )?;
        fs::File::open(&pending)
            .map_err(io_error)?
            .sync_all()
            .map_err(io_error)?;
        let document = repository::PublishedSources {
            manifest: fs::read_to_string(&pending).map_err(io_error)?,
            sources: self.closure.originals().to_vec(),
        };
        let bytes = serde_json::to_vec(&document).expect("publication JSON");
        let verified = repository::load_sources(&bytes)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join("publication.json"))
            .map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        // Flush directory entries for every original, not only file contents.
        let mut dirs = std::collections::BTreeSet::new();
        for source in self.closure.originals() {
            let mut current = output.join(&source.path);
            while current.pop() {
                dirs.insert(current.clone());
                if current == output {
                    break;
                }
            }
        }
        for dir in dirs.iter().rev() {
            fs::File::open(dir)
                .map_err(io_error)?
                .sync_all()
                .map_err(io_error)?;
        }
        fs::rename(pending, output.join(repository::MANIFEST)).map_err(io_error)?;
        fs::File::open(output)
            .map_err(io_error)?
            .sync_all()
            .map_err(io_error)?;
        let snapshot = repository::load(output)?;
        if verified.identity != snapshot.identity {
            return Err(failure(
                "<candidate>",
                "write",
                "E_REPOSITORY_IDENTITY",
                "Publication representations disagree",
            ));
        }
        Ok(CandidateReceipt {
            repository: snapshot.identity,
            policy_sha256: self.closure.receipt().policy_sha256.clone(),
            publication_approval: "not_granted",
            activated: false,
        })
    }
}
