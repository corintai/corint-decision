//! Local operator attestations; no report can authenticate itself.
use corint_decision_toolchain::{
    contracts::CompatibilityReport,
    phase0::{self, Contract, EvidenceTrust},
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceConfig {
    pub evaluation: PathBuf,
    pub approval: PathBuf,
    pub trust: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustFile {
    evaluations: BTreeMap<String, String>,
    approvals: BTreeMap<String, String>,
    approvers: BTreeSet<String>,
}
/// Core has no feature/model runtime dependencies. Hash an actual empty binding
/// contract, not the unrelated target compatibility binding fingerprint.
pub fn subject(report: &CompatibilityReport) -> anyhow::Result<Value> {
    corint_decision_toolchain::contracts::core_evidence_subject(report).map_err(Into::into)
}
/// Bind external input preparation as well as the unchanged pure Core subject.
pub fn subject_with_features(
    report: &CompatibilityReport,
    feature_binding: Option<&str>,
) -> anyhow::Result<Value> {
    let mut subject = subject(report)?;
    if let Some(binding) = feature_binding {
        subject["bindings_sha256"] = corint_decision_engine::decision_host::canonical_sha256(
            &serde_json::json!({"core_bindings_sha256":subject["bindings_sha256"], "feature_host_sha256":binding})
        ).into();
    }
    Ok(subject)
}
fn read(path: &Path) -> anyhow::Result<String> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    anyhow::ensure!(file.metadata()?.is_file(), "Expected regular evidence file");
    let mut bytes = Vec::new();
    file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 8 * 1024 * 1024, "Evidence too large");
    Ok(String::from_utf8(bytes)?)
}
pub fn check(
    root: &Path,
    config: &EvidenceConfig,
    subject: &Value,
    now: u64,
) -> anyhow::Result<()> {
    check_with_features(root, config, subject, &[], now)
}
pub fn check_with_features(
    root: &Path,
    config: &EvidenceConfig,
    subject: &Value,
    required_features: &[Value],
    now: u64,
) -> anyhow::Result<()> {
    check_scoped(root, config, subject, required_features, now, None)
}

pub fn check_scoped(
    root: &Path,
    config: &EvidenceConfig,
    subject: &Value,
    required_features: &[Value],
    now: u64,
    scope: Option<&crate::tenancy::Scope>,
) -> anyhow::Result<()> {
    let contract = |kind, path: &Path| -> anyhow::Result<Contract> {
        Ok(Contract::load(
            kind,
            &corint_decision_compiler::core::CoreSource {
                path: "operator-evidence".into(),
                yaml: read(&root.join(path))?,
            },
        )?)
    };
    let evaluation = contract("evaluation-evidence", &config.evaluation)?;
    let approval = contract("approval-evidence", &config.approval)?;
    if let Some(scope) = scope {
        for value in [evaluation.value(), approval.value()] {
            anyhow::ensure!(
                value["audience"] == serde_json::to_value(scope)?,
                "Evidence audience differs from tenant deployment"
            );
        }
    }
    let trust: TrustFile = serde_json::from_str(&read(&root.join(&config.trust))?)?;
    phase0::check_approval(
        &evaluation,
        &approval,
        subject,
        required_features,
        &EvidenceTrust {
            evaluations: trust.evaluations,
            approvals: trust.approvals,
            approvers: trust.approvers,
        },
        now,
    )?;
    Ok(())
}
