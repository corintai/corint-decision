//! Experimental offline resource, evidence and feedback contracts.
//! No network, publication, runtime resource calls or implicit authority.
use crate::{contracts, package};
use corint_decision_compiler::core::{diagnostic, CoreError, CoreSource};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// All schemas are self-contained Draft 7 documents, available to other consumers.
pub const SCHEMAS: &[(&str, &str)] = &[
    (
        "feature-descriptor",
        include_str!("../../../docs/contracts/schema/feature-descriptor.json"),
    ),
    (
        "model-descriptor",
        include_str!("../../../docs/contracts/schema/model-descriptor.json"),
    ),
    (
        "resource-bindings",
        include_str!("../../../docs/contracts/schema/resource-bindings.json"),
    ),
    (
        "evaluation-evidence",
        include_str!("../../../docs/contracts/schema/evaluation-evidence.json"),
    ),
    (
        "approval-evidence",
        include_str!("../../../docs/contracts/schema/approval-evidence.json"),
    ),
    (
        "decision-record",
        include_str!("../../../docs/contracts/schema/decision-record.json"),
    ),
    (
        "outcome-event",
        include_str!("../../../docs/contracts/schema/outcome-event.json"),
    ),
    (
        "action-receipt",
        include_str!("../../../docs/contracts/schema/action-receipt.json"),
    ),
];

/// Immutable schema-validated document. Source declarations confer no authority.
#[derive(Debug, Clone)]
pub struct Contract {
    kind: String,
    value: Value,
    source: String,
}
impl Contract {
    pub fn load(kind: &str, source: &CoreSource) -> Result<Self, CoreError> {
        let schema = SCHEMAS
            .iter()
            .find(|(name, _)| *name == kind)
            .ok_or_else(|| {
                diagnostic(
                    &source.path,
                    "/kind",
                    "contract",
                    "E_CONTRACT_KIND",
                    "Unknown contract kind",
                )
            })?
            .1;
        let value = contracts::parse(source, schema)?;
        Ok(Self {
            kind: kind.into(),
            value,
            source: source.path.clone(),
        })
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    /// Canonical JSON content identity; independent of source path/formatting.
    pub fn sha256(&self) -> String {
        package::json_hash("phase0-contract-v1", self.value.clone())
    }
    fn require(&self, kind: &str) -> Result<(), CoreError> {
        self.ensure(
            self.kind == kind,
            "/kind",
            "E_CONTRACT_KIND",
            "Wrong contract for consumer",
        )
    }
    fn ensure(&self, ok: bool, path: &str, code: &str, message: &str) -> Result<(), CoreError> {
        if ok {
            Ok(())
        } else {
            Err(diagnostic(&self.source, path, "contract", code, message))
        }
    }
    fn resource_ref(&self) -> Value {
        serde_json::json!({"kind":if self.kind == "feature-descriptor" {"feature"} else {"model"},
            "id":self.value["id"], "revision":self.value["revision"], "sha256":self.sha256()})
    }
}
fn items(v: &Value) -> &[Value] {
    v.as_array().expect("validated array")
}
fn s(v: &Value) -> &str {
    v.as_str().expect("validated string")
}
// JSON Schema integers may be encoded as 120.0. Safe integers are bounded by schema.
fn time(v: &Value) -> u64 {
    v.as_f64().expect("validated bounded integer") as u64
}

/// W04: check the exact caller-supplied dependency closure against target declarations.
/// The caller must derive descriptors from the policy's actual dependency closure.
/// Success is declaration compatibility only, never live readiness or publication permission.
pub fn check_resources(
    descriptors: &[Contract],
    bindings: &Contract,
    target_sha256: &str,
    capabilities: &BTreeSet<String>,
) -> Result<(), CoreError> {
    bindings.require("resource-bindings")?;
    let b = bindings.value();
    bindings.ensure(
        b["target_sha256"] == target_sha256,
        "/target_sha256",
        "E_RESOURCE_TARGET",
        "Target identity changed",
    )?;
    let entries = items(&b["resources"]);
    let mut identities = BTreeSet::new();
    for d in descriptors {
        d.ensure(
            matches!(d.kind.as_str(), "feature-descriptor" | "model-descriptor"),
            "/kind",
            "E_CONTRACT_KIND",
            "Expected resource descriptor",
        )?;
        d.ensure(
            identities.insert((d.kind.clone(), s(&d.value["id"]).to_owned())),
            "/id",
            "E_RESOURCE_BINDING",
            "Duplicate logical resource",
        )?;
        let reference = d.resource_ref();
        let matches: Vec<_> = entries
            .iter()
            .filter(|e| e["resource"] == reference)
            .collect();
        bindings.ensure(
            matches.len() == 1,
            "/resources",
            "E_RESOURCE_BINDING",
            "Each exact resource requires one binding",
        )?;
        let entry = matches[0];
        bindings.ensure(
            entry["status"] == "ready",
            "/resources",
            "E_RESOURCE_UNAVAILABLE",
            "Resource not declared ready; no automatic deployment or version substitution",
        )?;
        bindings.ensure(
            capabilities.contains(s(&entry["capability"])),
            "/resources",
            "E_RESOURCE_CAPABILITY",
            "Target does not support required capability",
        )?;
        if d.kind == "feature-descriptor" {
            bindings.ensure(
                entry["capability"] == "feature-read-v1"
                    && entry["offline_binding"] == d.value["offline_binding"]
                    && entry["online_binding"] == d.value["online_binding"],
                "/resources",
                "E_RESOURCE_BINDING",
                "Feature backend bindings must match definition",
            )?;
        } else {
            d.ensure(
                d.value["deployment_status"] == "ready",
                "/deployment_status",
                "E_RESOURCE_UNAVAILABLE",
                "Model definition alone is not deployed",
            )?;
            bindings.ensure(
                entry["capability"] == d.value["required_capability"],
                "/resources",
                "E_RESOURCE_CAPABILITY",
                "Model service capability mismatch",
            )?;
            for feature in items(&d.value["features"]) {
                d.ensure(
                    feature["kind"] == "feature"
                        && descriptors.iter().any(|f| {
                            f.kind == "feature-descriptor" && f.resource_ref() == *feature
                        }),
                    "/features",
                    "E_RESOURCE_BINDING",
                    "Missing exact model input feature",
                )?;
            }
        }
    }
    bindings.ensure(
        entries.len() == descriptors.len(),
        "/resources",
        "E_RESOURCE_BINDING",
        "Undeclared or duplicate target bindings",
    )
}

/// W07: fixed-sample parity and historical availability checks. Digests are compared,
/// not recomputed by a live feature backend; dataset claims need trusted attestation.
pub fn check_evaluation(evaluation: &Contract) -> Result<(), CoreError> {
    evaluation.require("evaluation-evidence")?;
    let v = evaluation.value();
    if v["scope"] == "business" {
        evaluation.ensure(
            v["dataset"]["origin"] == "real",
            "/dataset/origin",
            "E_EVALUATION_SCOPE",
            "Synthetic behavior evidence cannot claim business evaluation",
        )?;
        let dataset = &v["dataset"];
        let partitions: BTreeSet<_> = [
            "train_partition_sha256",
            "tuning_partition_sha256",
            "evaluation_partition_sha256",
        ]
        .iter()
        .map(|k| s(&dataset[*k]))
        .collect();
        evaluation.ensure(
            partitions.len() == 3,
            "/dataset",
            "E_DATA_SPLIT",
            "Training, tuning and evaluation partitions must have distinct identities",
        )?;
    }
    let features = items(&v["features"]);
    let samples = items(&v["samples"]);
    let mut feature_ids = BTreeSet::new();
    for f in features {
        evaluation.ensure(
            f["kind"] == "feature" && feature_ids.insert(s(&f["id"])),
            "/features",
            "E_FEATURE_COVERAGE",
            "Feature requirements must be unique feature definitions",
        )?;
        evaluation.ensure(
            samples.iter().any(|x| x["feature"] == *f),
            "/samples",
            "E_FEATURE_COVERAGE",
            "Every required feature needs a fixed parity sample",
        )?;
    }
    for sample in samples {
        evaluation.ensure(
            features.contains(&sample["feature"]),
            "/samples",
            "E_FEATURE_COVERAGE",
            "Sample not in exact feature closure",
        )?;
        evaluation.ensure(
            sample["offline_definition_sha256"] == sample["feature"]["sha256"]
                && sample["online_definition_sha256"] == sample["feature"]["sha256"]
                && sample["offline_value_sha256"] == sample["online_value_sha256"],
            "/samples",
            "E_FEATURE_PARITY",
            "Offline/online definition or fixed output differs",
        )?;
        evaluation.ensure(
            time(&sample["event_time_ms"]) <= time(&sample["available_time_ms"])
                && time(&sample["available_time_ms"]) <= time(&sample["decision_time_ms"])
                && time(&sample["decision_time_ms"]) <= time(&v["created_at_ms"]),
            "/samples",
            "E_POINT_IN_TIME",
            "Evidence was unavailable at decision time or report predates sample",
        )?;
    }
    Ok(())
}

/// Supplied by the host's trusted configuration, never deserialized from a report.
/// Maps exact, independently verified content digests to authenticated producers.
#[derive(Default)]
pub struct EvidenceTrust {
    pub evaluations: BTreeMap<String, String>,
    pub approvals: BTreeMap<String, String>,
    pub approvers: BTreeSet<String>,
}
/// W05/W06/W09: admission of exact business evidence and approval within a local
/// trust domain. The host must authenticate attestations and obtain current state.
/// This function does not grant Core runtime capabilities or activate a policy.
pub fn check_approval(
    evaluation: &Contract,
    approval: &Contract,
    subject: &Value,
    required_features: &[Value],
    trust: &EvidenceTrust,
    now_ms: u64,
) -> Result<(), CoreError> {
    check_evaluation(evaluation)?;
    approval.require("approval-evidence")?;
    let e = evaluation.value();
    let a = approval.value();
    approval.ensure(
        e["subject"] == *subject
            && a["subject"] == *subject
            && a["evaluation_sha256"] == evaluation.sha256(),
        "/subject",
        "E_STALE_EVIDENCE",
        "Policy, context, target, binding, checker or evaluation changed",
    )?;
    let actual: BTreeSet<_> = items(&e["features"]).iter().map(Value::to_string).collect();
    let required: BTreeSet<_> = required_features.iter().map(Value::to_string).collect();
    evaluation.ensure(
        actual == required && required.len() == required_features.len(),
        "/features",
        "E_FEATURE_COVERAGE",
        "Evaluation must cover the current exact feature closure",
    )?;
    evaluation.ensure(
        e["scope"] == "business" && e["result"] == "passed",
        "/scope",
        "E_EVALUATION_SCOPE",
        "Publication evidence requires a passed business evaluation",
    )?;
    approval.ensure(
        trust
            .evaluations
            .get(&evaluation.sha256())
            .is_some_and(|p| p == s(&e["provenance"]["producer"]))
            && trust
                .approvals
                .get(&approval.sha256())
                .is_some_and(|p| p == s(&a["approver"]) && p == s(&a["provenance"]["producer"]))
            && trust.approvers.contains(s(&a["approver"])),
        "/approver",
        "E_EVIDENCE_AUTHORITY",
        "Unattested evidence or unauthorized approver",
    )?;
    approval.ensure(
        a["decision"] == "approved",
        "/decision",
        "E_APPROVAL_DENIED",
        "Approval denied",
    )?;
    approval.ensure(
        time(&e["created_at_ms"]) <= time(&a["issued_at_ms"])
            && time(&a["issued_at_ms"]) <= now_ms
            && now_ms < time(&a["expires_at_ms"]),
        "/expires_at_ms",
        "E_EVIDENCE_TIME",
        "Approval is premature or expired",
    )
}

#[derive(Debug, PartialEq, Eq)]
pub enum Ingest {
    Inserted,
    Duplicate,
}

/// W08 reference consumer. In-memory, append-only histories; no Work dependency.
/// Hosts must authenticate tenant/producers, persist state and apply retention policy.
#[derive(Default)]
pub struct FeedbackLedger {
    decisions: BTreeMap<(String, String), Contract>,
    events: BTreeMap<(String, String), Contract>,
    labels: BTreeMap<(String, String, String), Vec<Contract>>,
    receipts: BTreeMap<(String, String, String), Contract>,
}
fn decision_key(v: &Value) -> (String, String) {
    (s(&v["tenant_id"]).into(), s(&v["decision_id"]).into())
}
impl FeedbackLedger {
    pub fn record_decision(&mut self, record: Contract) -> Result<Ingest, CoreError> {
        record.require("decision-record")?;
        let v = record.value();
        record.ensure(
            (v["result"] == "error") == !v["error_code"].is_null(),
            "/error_code",
            "E_DECISION_RESULT",
            "Error result and error code must agree",
        )?;
        let mut ids = BTreeSet::new();
        let mut keys = BTreeSet::new();
        for a in items(&v["actions"]) {
            record.ensure(
                ids.insert(s(&a["action_id"])) && keys.insert(s(&a["idempotency_key"])),
                "/actions",
                "E_ACTION_IDENTITY",
                "Duplicate action identity or idempotency key",
            )?;
        }
        let key = decision_key(v);
        if let Some(old) = self.decisions.get(&key) {
            record.ensure(
                old.sha256() == record.sha256(),
                "/decision_id",
                "E_EVENT_CONFLICT",
                "Decision ID reused with different content",
            )?;
            return Ok(Ingest::Duplicate);
        }
        self.decisions.insert(key, record);
        Ok(Ingest::Inserted)
    }
    fn correlate(&self, event: &Contract) -> Result<&Contract, CoreError> {
        let decision = self.decisions.get(&decision_key(event.value()));
        event.ensure(
            decision
                .is_some_and(|d| d.value["business_event_id"] == event.value["business_event_id"]),
            "/decision_id",
            "E_EVENT_CORRELATION",
            "Unknown tenant/decision or business event mismatch; retry after decision arrives",
        )?;
        Ok(decision.expect("checked decision"))
    }
    pub fn ingest_outcome(&mut self, event: Contract) -> Result<Ingest, CoreError> {
        event.require("outcome-event")?;
        self.correlate(&event)?;
        let v = event.value();
        let key = (s(&v["tenant_id"]).to_owned(), s(&v["id"]).to_owned());
        if let Some(old) = self.events.get(&key) {
            event.ensure(
                old.sha256() == event.sha256(),
                "/id",
                "E_EVENT_CONFLICT",
                "Event ID reused with different content",
            )?;
            return Ok(Ingest::Duplicate);
        }
        event.ensure(
            time(&v["occurred_at_ms"]) <= time(&v["observed_at_ms"])
                && time(&v["observed_at_ms"]) <= time(&v["available_at_ms"]),
            "/available_at_ms",
            "E_EVENT_TIME",
            "Invalid event observation/availability order",
        )?;
        let label_key = (
            s(&v["tenant_id"]).into(),
            s(&v["decision_id"]).into(),
            s(&v["label_name"]).into(),
        );
        let previous = self.labels.get(&label_key).and_then(|h| h.last());
        if let Some(old) = previous {
            event.ensure(
                time(&v["label_version"]) == time(&old.value["label_version"]) + 1
                    && v["supersedes"] == old.value["id"],
                "/supersedes",
                "E_LABEL_VERSION",
                "Correction must follow the current label version and name its event",
            )?;
            event.ensure(
                time(&v["available_at_ms"]) >= time(&old.value["available_at_ms"]),
                "/available_at_ms",
                "E_EVENT_TIME",
                "Correction cannot rewrite historical availability",
            )?;
        } else {
            event.ensure(
                time(&v["label_version"]) == 1 && v["supersedes"].is_null(),
                "/label_version",
                "E_LABEL_VERSION",
                "First label must be version 1 without predecessor; retry out-of-order corrections",
            )?;
        }
        self.labels
            .entry(label_key)
            .or_default()
            .push(event.clone());
        self.events.insert(key, event);
        Ok(Ingest::Inserted)
    }
    /// Missing labels return None, never a fabricated negative/safe label.
    pub fn outcome_as_of(
        &self,
        tenant: &str,
        decision: &str,
        label: &str,
        available_ms: u64,
    ) -> Option<&Contract> {
        self.labels
            .get(&(tenant.into(), decision.into(), label.into()))?
            .iter()
            .rev()
            .find(|e| time(&e.value["available_at_ms"]) <= available_ms)
    }
    pub fn ingest_receipt(&mut self, receipt: Contract) -> Result<Ingest, CoreError> {
        receipt.require("action-receipt")?;
        let decision = self.correlate(&receipt)?;
        let v = receipt.value();
        receipt.ensure(
            items(&decision.value["actions"]).iter().any(|a| {
                a["action_id"] == v["action_id"] && a["idempotency_key"] == v["idempotency_key"]
            }),
            "/action_id",
            "E_ACTION_IDENTITY",
            "Receipt must match a recorded action intent",
        )?;
        receipt.ensure(
            time(&v["executed_at_ms"]) >= time(&decision.value["decided_at_ms"]),
            "/executed_at_ms",
            "E_EVENT_TIME",
            "Action receipt predates decision",
        )?;
        let key = (
            s(&v["tenant_id"]).into(),
            s(&v["decision_id"]).into(),
            s(&v["action_id"]).into(),
        );
        if let Some(old) = self.receipts.get(&key) {
            receipt.ensure(
                old.sha256() == receipt.sha256(),
                "/action_id",
                "E_EVENT_CONFLICT",
                "Conflicting terminal receipt; v1 accepts one receipt per action",
            )?;
            return Ok(Ingest::Duplicate);
        }
        self.receipts.insert(key, receipt);
        Ok(Ingest::Inserted)
    }
}
