use corint_decision_compiler::core::{CoreError, CoreSource};
use corint_decision_toolchain::phase0::{self, Contract, EvidenceTrust, FeedbackLedger, Ingest};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, path::Path};

fn value(name: &str) -> Value {
    serde_json::from_str(
        &fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/conformance/contracts/phase0")
                .join(format!("{name}.json")),
        )
        .unwrap(),
    )
    .unwrap()
}
fn load(name: &str, v: Value) -> Contract {
    Contract::load(
        name,
        &CoreSource {
            path: format!("{name}.json"),
            yaml: v.to_string(),
        },
    )
    .unwrap()
}
fn fixture(name: &str) -> Contract {
    load(name, value(name))
}
fn code<T>(result: Result<T, CoreError>, expected: &str) {
    match result {
        Err(e) => assert_eq!(e.diagnostic.code, expected, "{e}"),
        Ok(_) => panic!("expected {expected}"),
    }
}
fn trust(e: &Contract, a: &Contract) -> EvidenceTrust {
    EvidenceTrust {
        evaluations: [(
            e.sha256(),
            e.value()["provenance"]["producer"].as_str().unwrap().into(),
        )]
        .into(),
        approvals: [(a.sha256(), a.value()["approver"].as_str().unwrap().into())].into(),
        approvers: ["fixture-reviewer".into()].into(),
    }
}
fn resources(bindings: &Contract) -> Result<(), CoreError> {
    phase0::check_resources(
        &[fixture("feature-descriptor"), fixture("model-descriptor")],
        bindings,
        &"c".repeat(64),
        &["feature-read-v1".into(), "model-score-v1".into()].into(),
    )
}
fn approve(e: &Contract, a: &Contract, trust: &EvidenceTrust, now: u64) -> Result<(), CoreError> {
    let original = value("evaluation-evidence");
    phase0::check_approval(
        e,
        a,
        &original["subject"],
        original["features"].as_array().unwrap(),
        trust,
        now,
    )
}
fn ledger() -> FeedbackLedger {
    let mut ledger = FeedbackLedger::default();
    assert_eq!(
        ledger.record_decision(fixture("decision-record")).unwrap(),
        Ingest::Inserted
    );
    ledger
}

#[test]
fn every_versioned_schema_has_a_roundtrip_fixture_and_rejects_bad_envelopes() {
    for (kind, schema) in phase0::SCHEMAS {
        let v = value(kind);
        let schema: Value = serde_json::from_str(schema).unwrap();
        assert_eq!(schema["$id"], format!("urn:corint:{kind}:1"));
        let validator = jsonschema::JSONSchema::compile(&schema).unwrap();
        assert!(validator.is_valid(&v), "{kind}");
        let original = load(kind, v.clone());
        let yaml = serde_yaml::to_string(&v).unwrap();
        let moved = Contract::load(
            kind,
            &CoreSource {
                path: "moved.yaml".into(),
                yaml: yaml.clone(),
            },
        )
        .unwrap();
        assert_eq!(original.sha256(), moved.sha256());
        assert_eq!(original.value(), moved.value());
        for (field, bad, expected) in [
            ("contract_version", json!("2"), "E_CONTRACT_VERSION"),
            ("approval", json!(true), "E_CONTRACT_FORMAT"),
            ("kind", json!("unknown"), "E_CONTRACT_FORMAT"),
            ("id", json!(" "), "E_CONTRACT_FORMAT"),
        ] {
            let mut mutated = v.clone();
            mutated[field] = bad;
            code(
                Contract::load(
                    kind,
                    &CoreSource {
                        path: kind.to_string(),
                        yaml: mutated.to_string(),
                    },
                ),
                expected,
            );
        }
        let mut missing = v.clone();
        missing.as_object_mut().unwrap().remove("revision");
        code(
            Contract::load(
                kind,
                &CoreSource {
                    path: kind.to_string(),
                    yaml: missing.to_string(),
                },
            ),
            "E_CONTRACT_FORMAT",
        );
        code(
            Contract::load(
                kind,
                &CoreSource {
                    path: kind.to_string(),
                    yaml: format!("{yaml}\nid: duplicate\n"),
                },
            ),
            "E_CONTRACT_FORMAT",
        );
    }
}

#[test]
fn w04_exact_dependency_closure_and_capability_gate() {
    let bindings = fixture("resource-bindings");
    resources(&bindings).unwrap();
    let descriptors = vec![fixture("feature-descriptor"), fixture("model-descriptor")];
    // The current Core capability set cannot invoke these future resources.
    code(
        phase0::check_resources(&descriptors, &bindings, &"c".repeat(64), &BTreeSet::new()),
        "E_RESOURCE_CAPABILITY",
    );
    code(
        phase0::check_resources(
            &descriptors[1..],
            &bindings,
            &"c".repeat(64),
            &["model-score-v1".into()].into(),
        ),
        "E_RESOURCE_BINDING",
    );
    let mut feature = value("feature-descriptor");
    feature["output"]["unit"] = json!("cents");
    code(
        phase0::check_resources(
            &[
                load("feature-descriptor", feature),
                fixture("model-descriptor"),
            ],
            &bindings,
            &"c".repeat(64),
            &["feature-read-v1".into(), "model-score-v1".into()].into(),
        ),
        "E_RESOURCE_BINDING",
    );
    let mut duplicate = value("resource-bindings");
    let first = duplicate["resources"][0].clone();
    duplicate["resources"].as_array_mut().unwrap().push(first);
    code(
        Contract::load(
            "resource-bindings",
            &CoreSource {
                path: "duplicate.json".into(),
                yaml: duplicate.to_string(),
            },
        ),
        "E_CONTRACT_FORMAT",
    );
}

#[test]
fn negative_manifest_executes_all_declared_consumers() {
    let manifest = value("negative-cases");
    assert_eq!(manifest["version"], 1);
    let mut ids = BTreeSet::new();
    for case in manifest["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id));
        let name = case["base"]
            .as_str()
            .unwrap()
            .strip_suffix(".json")
            .unwrap();
        let mut v = value(name);
        *v.pointer_mut(case["pointer"].as_str().unwrap())
            .expect("existing fixture field") = case["value"].clone();
        let document = load(name, v);
        let result = match case["consumer"].as_str().unwrap() {
            "resources" => resources(&document),
            "evaluation" => phase0::check_evaluation(&document),
            "approval" => {
                let e = fixture("evaluation-evidence");
                // Even attested documents cannot bypass semantic binding/role checks.
                approve(&e, &document, &trust(&e, &document), 200)
            }
            "outcome" => ledger().ingest_outcome(document).map(|_| ()),
            other => panic!("unknown consumer {other}"),
        };
        match result {
            Err(e) => assert_eq!(e.diagnostic.code, case["expected_code"], "{id}: {e}"),
            Ok(_) => panic!("negative accepted: {id}"),
        }
    }
    assert!(ids.len() >= 16);
    for prefix in ["W04_", "W05_", "W06_", "W07_", "W08_", "W09_"] {
        assert!(ids.iter().any(|id| id.starts_with(prefix)));
    }
}

#[test]
fn w05_w06_authority_is_external_exact_scoped_and_time_bounded() {
    let e = fixture("evaluation-evidence");
    let a = fixture("approval-evidence");
    phase0::check_evaluation(&e).unwrap();
    approve(&e, &a, &trust(&e, &a), 200).unwrap();
    code(
        approve(&e, &a, &EvidenceTrust::default(), 200),
        "E_EVIDENCE_AUTHORITY",
    );
    code(approve(&e, &a, &trust(&e, &a), 159), "E_EVIDENCE_TIME");
    code(approve(&e, &a, &trust(&e, &a), 500), "E_EVIDENCE_TIME");
    approve(&e, &a, &trust(&e, &a), 160).unwrap();
    approve(&e, &a, &trust(&e, &a), 499).unwrap();
    for field in [
        "policy_sha256",
        "context_sha256",
        "target_sha256",
        "bindings_sha256",
        "checker_sha256",
    ] {
        let mut subject = e.value()["subject"].clone();
        subject[field] = json!("0".repeat(64));
        code(
            phase0::check_approval(
                &e,
                &a,
                &subject,
                e.value()["features"].as_array().unwrap(),
                &trust(&e, &a),
                200,
            ),
            "E_STALE_EVIDENCE",
        );
    }
    let mut edited = e.value().clone();
    edited["metrics"][0]["value"] = json!(0.2);
    let edited = load("evaluation-evidence", edited);
    code(
        approve(&edited, &a, &trust(&e, &a), 200),
        "E_STALE_EVIDENCE",
    );
    let mut behavior = e.value().clone();
    behavior["scope"] = json!("behavior");
    behavior["dataset"]["origin"] = json!("synthetic");
    let behavior = load("evaluation-evidence", behavior);
    phase0::check_evaluation(&behavior).unwrap();
    let mut updated_approval = a.value().clone();
    updated_approval["evaluation_sha256"] = json!(behavior.sha256());
    let updated_approval = load("approval-evidence", updated_approval);
    code(
        approve(
            &behavior,
            &updated_approval,
            &trust(&behavior, &updated_approval),
            200,
        ),
        "E_EVALUATION_SCOPE",
    );
    let mut rejected = a.value().clone();
    rejected["decision"] = json!("rejected");
    let rejected = load("approval-evidence", rejected);
    code(
        approve(&e, &rejected, &trust(&e, &rejected), 200),
        "E_APPROVAL_DENIED",
    );
    let mut renamed = a.value().clone();
    renamed["id"] = json!("forged-copy");
    let renamed = load("approval-evidence", renamed);
    code(
        approve(&e, &renamed, &trust(&e, &a), 200),
        "E_EVIDENCE_AUTHORITY",
    );
}

#[test]
fn w07_required_coverage_and_point_in_time_boundary() {
    let mut v = value("evaluation-evidence");
    v["samples"][0]["available_time_ms"] = json!(120);
    phase0::check_evaluation(&load("evaluation-evidence", v)).unwrap();
    let mut omitted = value("evaluation-evidence");
    omitted["features"] = json!([]);
    omitted["samples"] = json!([]);
    let e = load("evaluation-evidence", omitted);
    let mut a = value("approval-evidence");
    a["evaluation_sha256"] = json!(e.sha256());
    let a = load("approval-evidence", a);
    code(approve(&e, &a, &trust(&e, &a), 200), "E_FEATURE_COVERAGE");
}

#[test]
fn w08_missing_delayed_duplicate_corrected_and_historical_labels() {
    let mut ledger = ledger();
    assert_eq!(
        ledger.record_decision(fixture("decision-record")).unwrap(),
        Ingest::Duplicate
    );
    assert!(ledger
        .outcome_as_of("fixture-tenant", "decision-1", "fraud", u64::MAX)
        .is_none());
    let event = fixture("outcome-event");
    assert_eq!(
        ledger.ingest_outcome(event.clone()).unwrap(),
        Ingest::Inserted
    );
    assert_eq!(ledger.ingest_outcome(event).unwrap(), Ingest::Duplicate);
    assert!(ledger
        .outcome_as_of("fixture-tenant", "decision-1", "fraud", 309)
        .is_none());
    assert_eq!(
        ledger
            .outcome_as_of("fixture-tenant", "decision-1", "fraud", 310)
            .unwrap()
            .value()["label"],
        "positive"
    );
    let mut correction = value("outcome-event");
    correction["id"] = json!("correction-2");
    correction["revision"] = json!("r2");
    correction["label_version"] = json!(2);
    correction["label"] = json!("unknown");
    correction["observed_at_ms"] = json!(400);
    correction["available_at_ms"] = json!(410);
    correction["supersedes"] = json!("outcome-event-fixture");
    let correction = load("outcome-event", correction);
    code(
        FeedbackLedger::default().ingest_outcome(correction.clone()),
        "E_EVENT_CORRELATION",
    );
    let mut empty = self::ledger();
    code(empty.ingest_outcome(correction.clone()), "E_LABEL_VERSION");
    assert_eq!(
        ledger.ingest_outcome(correction.clone()).unwrap(),
        Ingest::Inserted
    );
    assert_eq!(
        ledger.ingest_outcome(correction).unwrap(),
        Ingest::Duplicate
    );
    assert_eq!(
        ledger
            .outcome_as_of("fixture-tenant", "decision-1", "fraud", 409)
            .unwrap()
            .value()["label_version"],
        1
    );
    assert_eq!(
        ledger
            .outcome_as_of("fixture-tenant", "decision-1", "fraud", 410)
            .unwrap()
            .value()["label"],
        "unknown"
    );
    let mut conflict = value("outcome-event");
    conflict["label"] = json!("negative");
    code(
        ledger.ingest_outcome(load("outcome-event", conflict)),
        "E_EVENT_CONFLICT",
    );
    let mut tenant = value("outcome-event");
    tenant["tenant_id"] = json!("other");
    code(
        ledger.ingest_outcome(load("outcome-event", tenant)),
        "E_EVENT_CORRELATION",
    );
    assert!(ledger
        .outcome_as_of("other", "decision-1", "fraud", u64::MAX)
        .is_none());
}

#[test]
fn action_receipts_match_intent_and_do_not_execute_actions() {
    let mut ledger = ledger();
    assert_eq!(
        ledger.ingest_receipt(fixture("action-receipt")).unwrap(),
        Ingest::Inserted
    );
    assert_eq!(
        ledger.ingest_receipt(fixture("action-receipt")).unwrap(),
        Ingest::Duplicate
    );
    let mut changed = value("action-receipt");
    changed["status"] = json!("failed");
    code(
        ledger.ingest_receipt(load("action-receipt", changed)),
        "E_EVENT_CONFLICT",
    );
    let mut changed = value("action-receipt");
    changed["idempotency_key"] = json!("unrelated");
    code(
        ledger.ingest_receipt(load("action-receipt", changed)),
        "E_ACTION_IDENTITY",
    );
    let mut changed = value("action-receipt");
    changed["executed_at_ms"] = json!(119);
    code(
        ledger.ingest_receipt(load("action-receipt", changed)),
        "E_EVENT_TIME",
    );
    let mut decision = value("decision-record");
    decision["result"] = json!("error");
    code(
        ledger.record_decision(load("decision-record", decision)),
        "E_DECISION_RESULT",
    );
}

#[test]
fn fixed_contract_handoff_keeps_resource_evidence_and_feedback_identities() {
    let bindings = fixture("resource-bindings");
    resources(&bindings).unwrap();
    let e = fixture("evaluation-evidence");
    let a = fixture("approval-evidence");
    let decision = fixture("decision-record");
    // Consumer handoff derives current identities, rather than trusting old report claims.
    let mut current = e.value()["subject"].clone();
    current["bindings_sha256"] = json!(bindings.sha256());
    current["target_sha256"] = bindings.value()["target_sha256"].clone();
    let feature_ref = bindings.value()["resources"][0]["resource"].clone();
    assert_eq!(
        feature_ref["sha256"],
        fixture("feature-descriptor").sha256()
    );
    assert_eq!(
        bindings.value()["resources"][1]["resource"]["sha256"],
        fixture("model-descriptor").sha256()
    );
    phase0::check_approval(&e, &a, &current, &[feature_ref], &trust(&e, &a), 200).unwrap();
    assert_eq!(decision.value()["subject"], current);
    let actual_resources: Vec<_> = bindings.value()["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["resource"].clone())
        .collect();
    assert_eq!(decision.value()["resources"], json!(actual_resources));
    let mut ledger = FeedbackLedger::default();
    ledger.record_decision(decision).unwrap();
    ledger.ingest_receipt(fixture("action-receipt")).unwrap();
    ledger.ingest_outcome(fixture("outcome-event")).unwrap();
    assert_eq!(
        ledger
            .outcome_as_of("fixture-tenant", "decision-1", "fraud", 310)
            .unwrap()
            .value()["label"],
        "positive"
    );
    // A failed conflicting write cannot change the accepted historical label.
    let mut conflict = value("outcome-event");
    conflict["label"] = json!("negative");
    code(
        ledger.ingest_outcome(load("outcome-event", conflict)),
        "E_EVENT_CONFLICT",
    );
    assert_eq!(
        ledger
            .outcome_as_of("fixture-tenant", "decision-1", "fraud", 310)
            .unwrap()
            .value()["label"],
        "positive"
    );
}
