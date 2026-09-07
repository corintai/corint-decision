//! The Registry reference is bound to complete policies, not generated Pipeline stubs.
use corint_decision_compiler::core::{parse_core_input_schema, CoreSource};
use corint_decision_engine::{DecisionEngine, DecisionRequest, EngineError};
use serde_json::{json, Value as Json};
use std::{collections::BTreeSet, path::PathBuf};

const RESOURCES: [&str; 7] = [
    "registry.yaml",
    "large_amount.yaml",
    "risk.yaml",
    "payment_shadow.yaml",
    "payment_br.yaml",
    "payment_main.yaml",
    "unhandled_event.yaml",
];

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/conformance/cdl_registry")
            .join(name),
    )
    .unwrap()
}

fn source(name: &str) -> CoreSource {
    CoreSource {
        path: name.into(),
        yaml: fixture(name),
    }
}

fn resources() -> Vec<CoreSource> {
    RESOURCES.into_iter().map(source).collect()
}

fn engine(resources: &[CoreSource]) -> Result<DecisionEngine, EngineError> {
    DecisionEngine::from_core(
        resources,
        parse_core_input_schema(&source("input-schema.yaml")).unwrap(),
    )
}

fn cases() -> Vec<Json> {
    serde_json::from_str(&fixture("cases.json")).unwrap()
}

fn event(case_id: &str) -> Json {
    cases()
        .into_iter()
        .find(|case| case["id"] == case_id)
        .unwrap()["event"]
        .clone()
}

fn request(event: Json, trace: bool) -> DecisionRequest {
    let request = DecisionRequest::new(serde_json::from_value(event).unwrap());
    if trace {
        request.with_trace()
    } else {
        request
    }
}

fn change_registry(resources: &mut [CoreSource], change: impl FnOnce(&mut Json)) {
    let source = resources
        .iter_mut()
        .find(|source| source.path == "registry.yaml")
        .unwrap();
    let mut document = serde_yaml::from_str(&source.yaml).unwrap();
    change(&mut document);
    source.yaml = document.to_string();
}

fn diagnostic(error: EngineError) -> Json {
    let EngineError::Core(error) = error else {
        panic!("Expected a Core diagnostic: {error}")
    };
    serde_json::to_value(error.diagnostic).unwrap()
}

#[tokio::test]
async fn documented_registry_routes_complete_policies() {
    let sources = resources();
    let engine = engine(&sources).unwrap();
    for case in cases() {
        for trace in [false, true] {
            let response = engine
                .decide(request(case["event"].clone(), trace))
                .await
                .unwrap();
            assert_eq!(response.pipeline_id.as_deref(), case["pipeline"].as_str());
            assert_eq!(
                json!(response.result.score),
                case["score"],
                "{}",
                case["id"]
            );
            assert_eq!(
                serde_json::to_value(response.result.signal).unwrap()["type"],
                case["signal"],
                "{}",
                case["id"]
            );
            assert_eq!(
                json!(response.result.triggered_rules),
                case["triggered_rules"],
                "{}",
                case["id"]
            );
            assert!(response.result.actions.is_empty());
            if trace {
                let records = response.trace.unwrap().core_conditions_v1.unwrap();
                let observed: BTreeSet<_> = records
                    .iter()
                    .filter(|record| record.resource_type == "registry")
                    .map(|record| record.field_path.clone())
                    .collect();
                let expected: BTreeSet<_> = (0..=case["entry"].as_u64().unwrap())
                    .map(|index| format!("/registry/{index}/when"))
                    .collect();
                assert_eq!(
                    observed, expected,
                    "Only entries through the first match are evaluated"
                );
            } else {
                assert!(response.trace.is_none());
            }
        }
    }
}

#[tokio::test]
async fn registry_order_repeated_targets_and_short_circuit_are_preserved() {
    let mut sources = resources();
    change_registry(&mut sources, |document| {
        document["registry"].as_array_mut().unwrap().swap(0, 2)
    });
    let reordered = engine(&sources).unwrap();
    // General-before-specific really changes selection; there is no automatic specificity sort.
    let response = reordered
        .decide(request(event("shadow_us"), false))
        .await
        .unwrap();
    assert_eq!(response.pipeline_id.as_deref(), Some("payment_main"));
    assert_eq!(response.result.score, 60);

    change_registry(&mut sources, |document| {
        document["registry"] = json!([
            {"pipeline":"payment_main","when":"event.type == \"payment\""},
            {"pipeline":"payment_main","when":"true"},
            {"pipeline":"unhandled_event","when":"event.amount / 0 > 1"}
        ]);
    });
    let repeated = engine(&sources).unwrap();
    for trace in [false, true] {
        let response = repeated
            .decide(request(event("main_large"), trace))
            .await
            .unwrap();
        assert_eq!(response.pipeline_id.as_deref(), Some("payment_main"));
        assert_eq!(response.result.score, 60);
        assert_eq!(response.result.triggered_rules, ["large_amount"]);
    }

    change_registry(&mut sources, |document| {
        document["registry"][0]["when"] =
            json!({"any":["event.type == \"payment\"", "event.amount / 0 > 1"]});
    });
    let short_circuit = engine(&sources).unwrap();
    for trace in [false, true] {
        assert_eq!(
            short_circuit
                .decide(request(event("main_large"), trace))
                .await
                .unwrap()
                .result
                .score,
            60
        );
    }
}

#[tokio::test]
async fn registry_failures_do_not_fall_through_to_a_business_decision() {
    for failure in [
        "no_match",
        "pipeline_guard",
        "condition_error",
        "missing_input",
    ] {
        let mut sources = resources();
        let mut input = event("main_large");
        let (code, stage) = match failure {
            "no_match" => {
                change_registry(&mut sources, |document| {
                    document["registry"].as_array_mut().unwrap().pop();
                });
                input = event("fallback");
                ("E_NO_PIPELINE_MATCH", "execute")
            }
            "pipeline_guard" => {
                input["amount"] = json!(0);
                ("E_PIPELINE_SKIPPED", "execute")
            }
            "condition_error" => {
                change_registry(&mut sources, |document| {
                    document["registry"][0]["when"] = json!("event.amount / 0 > 1")
                });
                ("E_DIVISION_BY_ZERO", "execute")
            }
            _ => {
                input.as_object_mut().unwrap().remove("country");
                ("E_INPUT_SCHEMA", "input")
            }
        };
        let engine = engine(&sources).unwrap();
        let mut errors = Vec::new();
        for trace in [false, true] {
            let error = diagnostic(
                engine
                    .decide(request(input.clone(), trace))
                    .await
                    .unwrap_err(),
            );
            assert_eq!(error["code"], code, "{failure}: {error}");
            assert_eq!(error["stage"], stage, "{failure}: {error}");
            errors.push(error);
        }
        assert_eq!(errors[0], errors[1], "Trace does not change {failure}");
    }
}

#[test]
fn every_registry_reference_and_condition_is_checked_before_execution() {
    let base = resources();
    engine(&base).unwrap();
    // These entries follow the always-true fallback and will never execute.
    // They still have to pass the complete admission gate.
    for (entry, code, stage) in [
        (
            json!({"pipeline":"missing_pipeline","when":"false"}),
            "E_UNRESOLVED_REF",
            "resolve",
        ),
        (
            json!({"pipeline":"payment_risk","when":"false"}),
            "E_UNRESOLVED_REF",
            "resolve",
        ),
        (
            json!({"pipeline":"payment_main","when":"event.amount >"}),
            "E_INVALID_EXPRESSION",
            "parse",
        ),
        (
            json!({"pipeline":"payment_main","when":"amount > 0"}),
            "E_INVALID_REF",
            "type",
        ),
        (
            json!({"pipeline":"payment_main","when":"results.payment_risk.score > 0"}),
            "E_INVALID_REF",
            "type",
        ),
        (
            json!({"pipeline":"payment_main","when":"total_score > 0"}),
            "E_INVALID_REF",
            "type",
        ),
        (
            json!({"pipeline":"payment_main","when":"event.amount"}),
            "E_TYPE",
            "type",
        ),
        (
            json!({"pipeline":"payment_main","when":"true","priority":1}),
            "E_UNKNOWN_FIELD",
            "validate",
        ),
    ] {
        let mut sources = base.clone();
        change_registry(&mut sources, |document| {
            document["registry"]
                .as_array_mut()
                .unwrap()
                .push(entry.clone())
        });
        let error = diagnostic(
            engine(&sources)
                .err()
                .expect("Invalid Registry must fail admission"),
        );
        assert_eq!(error["code"], code, "{entry}: {error}");
        assert_eq!(error["stage"], stage, "{entry}: {error}");
        assert_eq!(error["source"], "registry.yaml");
        assert!(error["field_path"]
            .as_str()
            .unwrap()
            .starts_with("/registry/4"));
    }
    // Missing and duplicate Registry resources are different from repeated target IDs.
    for duplicate in [false, true] {
        let mut sources = base.clone();
        let code = if duplicate {
            let mut registry = source("registry.yaml");
            registry.path = "second_registry.yaml".into();
            sources.push(registry);
            "E_DUPLICATE_ID"
        } else {
            sources.remove(0);
            "E_MISSING_FIELD"
        };
        let error = diagnostic(
            engine(&sources)
                .err()
                .expect("Exactly one Registry is required"),
        );
        assert_eq!(error["code"], code);
        assert_eq!(error["stage"], "resolve");
    }
}
