//! Editable source exchange, intentionally separate from historical evidence.
//! Never extracts embedded labels to disk or imports authorization.
use crate::{behavior, failure, label, package};
use corint_decision_compiler::core::{
    compile_core, diagnostic, parse_core_input_schema, CoreError, CoreSource, PROFILE,
};
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

pub const BUNDLE_SCHEMA: &str = include_str!("../../../docs/contracts/schema/source-bundle.json");

/// Source content only. Edit the public source fields in memory, then re-import to
/// test and rebuild. There is deliberately no validation/approval flag here.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBundle {
    format: String,
    format_version: String,
    profile: String,
    language_version: String,
    pub input_schema: CoreSource,
    pub sources: Vec<CoreSource>,
}

#[derive(Serialize)]
pub struct ExportReceipt {
    pub path: String,
    pub bundle_sha256: String,
    pub source_count: usize,
    pub source_package_evidence: &'static str,
    pub evidence_exported: bool,
    pub publication_approval: &'static str,
}

impl SourceBundle {
    pub fn new(input_schema: CoreSource, sources: Vec<CoreSource>) -> Result<Self, CoreError> {
        let bundle = Self {
            format: "corint-core-source-bundle".into(),
            format_version: "1".into(),
            profile: PROFILE.into(),
            language_version: "0.1".into(),
            input_schema,
            sources,
        };
        bundle.validate("<bundle>")?;
        Ok(bundle)
    }

    fn validate(&self, origin: &str) -> Result<(), CoreError> {
        let schema =
            JSONSchema::compile(&serde_json::from_str(BUNDLE_SCHEMA).expect("bundle schema JSON"))
                .expect("embedded bundle schema");
        let value = serde_json::to_value(self).expect("bundle JSON");
        if let Err(mut errors) = schema.validate(&value) {
            if let Some(error) = errors.next() {
                return Err(diagnostic(
                    origin,
                    &error.instance_path.to_string(),
                    "package",
                    "E_BUNDLE_FORMAT",
                    error.to_string(),
                ));
            }
        }
        let mut labels = BTreeSet::from([&self.input_schema.path]);
        for (i, source) in self.sources.iter().enumerate() {
            if !labels.insert(&source.path) {
                return Err(diagnostic(
                    origin,
                    &format!("/sources/{i}/path"),
                    "resolve",
                    "E_DUPLICATE_SOURCE",
                    "Bundle source labels must be unique, including input schema",
                ));
            }
        }
        compile_core(&self.sources, parse_core_input_schema(&self.input_schema)?)?;
        Ok(())
    }
}

/// Check source-package format, content bindings, canonical labels and current
/// Core compilation. Historical test report/tool/suite evidence is NOT verified.
pub fn export_sources(stored: &CoreSource) -> Result<SourceBundle, CoreError> {
    let (sources, input) = package::source_snapshot(stored)?;
    SourceBundle::new(input, sources)
}

/// Deserialize the entire strict JSON bundle. No directory scanning, imports,
/// fallback YAML parser or interpretation of source labels as filesystem paths.
pub fn read_bundle(stored: &CoreSource) -> Result<SourceBundle, CoreError> {
    let bundle: SourceBundle = serde_json::from_str(&stored.yaml).map_err(|e| {
        let mut error = failure(&stored.path, "package", "E_BUNDLE_FORMAT", e.to_string());
        error.diagnostic.line = Some(e.line());
        error.diagnostic.column = Some(e.column());
        error
    })?;
    bundle.validate(&stored.path)?;
    Ok(bundle)
}

/// Fresh execution under this host and the caller's explicitly supplied suite.
/// No requirement that it be the original suite: this is NEW evidence, not verify.
pub fn import_sources(
    stored: &CoreSource,
    suite: &CoreSource,
) -> Result<(Option<package::Package>, behavior::TestResults), CoreError> {
    let bundle = read_bundle(stored)?;
    package::prepare(&bundle.sources, &bundle.input_schema, suite)
}

/// Write one editable JSON source bundle, never a directory/archive extraction.
pub fn export(stored: &CoreSource, output: &Path) -> Result<ExportReceipt, CoreError> {
    package::check_output(output)?;
    let bundle = export_sources(stored)?;
    let mut bytes = serde_json::to_vec_pretty(&bundle).expect("source bundle JSON");
    bytes.push(b'\n');
    package::write_bytes(&bytes, output)?;
    Ok(ExportReceipt {
        path: label(output),
        bundle_sha256: package::hash(&bytes),
        source_count: bundle.sources.len(),
        source_package_evidence: "not_verified",
        evidence_exported: false,
        publication_approval: "not_granted",
    })
}
