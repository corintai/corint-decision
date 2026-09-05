//! Offline fixtures exercise public validation, compilation and DecisionEngine execution.
use corint_decision_compiler::core::{
    parse_core_input_schema, validate_core_document, CoreSource, PROFILE,
};
use corint_decision_engine::{DecisionEngine, DecisionRequest, EngineError, Value};
use corint_decision_model::types::{FieldType, Schema, SchemaField};
use serde::Deserialize;
use serde_json::Value as Json;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    profile: String,
    cases: Vec<Case>,
    invalid: Vec<Invalid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    requirements: Vec<String>,
    documents: Vec<String>,
    runs: Vec<Run>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Run {
    #[serde(default)]
    conditions: Vec<ExpectedCondition>,
    amount: f64,
    score: i32,
    signal: String,
    actions: Vec<String>,
    triggered: Vec<String>,
    steps: Vec<String>,
    local_scores: HashMap<String, i32>,
    calls: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedCondition {
    source: String,
    field_path: String,
    node_path: String,
    outcome: corint_decision_runtime::result::CoreConditionOutcome,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Invalid {
    id: String,
    document: String,
    find: String,
    replace: String,
    stage: String,
    code: String,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core")
}
fn manifest() -> Manifest {
    serde_yaml::from_str(&std::fs::read_to_string(root().join("manifest.yaml")).unwrap()).unwrap()
}
fn schema() -> Schema {
    parse_core_input_schema(&CoreSource {
        path: "input-schema.yaml".into(),
        yaml: std::fs::read_to_string(root().join("input-schema.yaml")).unwrap(),
    })
    .unwrap()
}
fn sources(files: &[String]) -> Vec<CoreSource> {
    files
        .iter()
        .map(|file| CoreSource {
            path: file.clone(),
            yaml: std::fs::read_to_string(root().join(file)).unwrap(),
        })
        .collect()
}
fn request(amount: f64) -> DecisionRequest {
    DecisionRequest::new(HashMap::from([("amount".into(), Value::Number(amount))]))
}
fn modify(sources: &mut [CoreSource], file: &str, f: impl FnOnce(&mut Json)) {
    let doc = sources.iter_mut().find(|s| s.path == file).unwrap();
    let mut json: Json = serde_yaml::from_str(&doc.yaml).unwrap();
    f(&mut json);
    doc.yaml = serde_yaml::to_string(&json).unwrap();
}
fn core_error(
    result: Result<DecisionEngine, EngineError>,
) -> corint_decision_compiler::core::CoreError {
    match result {
        Err(EngineError::Core(e)) => e,
        Err(e) => panic!("Unexpected error: {e}"),
        Ok(_) => panic!("Invalid bundle accepted"),
    }
}

#[tokio::test]
async fn manifest_behavior_and_trace_parity() {
    let manifest = manifest();
    assert_eq!(manifest.profile, PROFILE);
    for case in manifest.cases {
        assert!(!case.requirements.is_empty());
        let engine = DecisionEngine::from_core(&sources(&case.documents), schema())
            .unwrap_or_else(|e| panic!("{}: {e}", case.id));
        for run in case.runs {
            let mut baseline = None;
            for trace in [false, true] {
                let mut req = request(run.amount);
                req.options.enable_trace = trace;
                let response = engine.decide(req).await.unwrap();
                let result = &response.result;
                assert_eq!(response.pipeline_id.as_deref(), Some("payment"));
                assert_eq!(result.score, run.score, "{} amount={}", case.id, run.amount);
                assert_eq!(
                    serde_json::to_value(&result.signal).unwrap()["type"],
                    run.signal
                );
                assert_eq!(result.actions, run.actions);
                assert_eq!(
                    result.explanation,
                    if run.signal == "decline" {
                        "Large amount"
                    } else {
                        "Below threshold"
                    }
                );
                assert_eq!(result.triggered_rules, run.triggered);
                let context = serde_json::to_value(&result.context).unwrap();
                let steps: Vec<String> = context["__executed_steps__"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        serde_json::from_str::<Json>(v.as_str().unwrap()).unwrap()["step_id"]
                            .as_str()
                            .unwrap()
                            .into()
                    })
                    .collect();
                assert_eq!(steps, run.steps);
                let calls: Vec<String> = context["__core_rule_executions__"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v["rule_id"].as_str().unwrap().into())
                    .collect();
                assert_eq!(calls, run.calls, "Unselected branches must not execute");
                let local: HashMap<String, i32> = context
                    .as_object()
                    .unwrap()
                    .iter()
                    .filter_map(|(key, value)| {
                        key.strip_prefix("__ruleset_result__.")
                            .map(|id| (id.to_owned(), value["score"].as_f64().unwrap() as i32))
                    })
                    .collect();
                assert_eq!(local, run.local_scores);
                if let Some(baseline) = &baseline {
                    assert_eq!(result, baseline, "Trace must not change semantics");
                } else {
                    baseline = Some(result.clone());
                }
                assert_eq!(response.trace.is_some(), trace);
                if let Some(trace) = response.trace {
                    let records = trace.core_conditions_v1.as_ref().unwrap();
                    let trace_schema: Json = serde_json::from_str(include_str!(
                        "../../../docs/cdl/schema/condition-trace.json"
                    ))
                    .unwrap();
                    let validator = jsonschema::JSONSchema::compile(&trace_schema).unwrap();
                    assert!(validator.is_valid(&serde_json::to_value(records).unwrap()));
                    for expected in &run.conditions {
                        let record = records
                            .iter()
                            .find(|r| {
                                r.source == expected.source
                                    && r.field_path == expected.field_path
                                    && r.node_path == expected.node_path
                            })
                            .unwrap_or_else(|| {
                                panic!(
                                    "Missing condition {} {} {}",
                                    expected.source, expected.field_path, expected.node_path
                                )
                            });
                        assert_eq!(record.outcome, expected.outcome);
                    }
                    let pipeline = trace.pipeline.unwrap();
                    for step in pipeline.steps {
                        assert_eq!(step.executed, run.steps.contains(&step.step_id));
                    }
                    assert_eq!(trace.rules_evaluated, run.calls.len());
                }
            }
        }
    }
}

#[test]
fn manifest_invalid_documents_fail_at_expected_stage() {
    let manifest = manifest();
    for case in manifest.invalid {
        let mut docs = sources(&manifest.cases[0].documents);
        let doc = docs.iter_mut().find(|d| d.path == case.document).unwrap();
        assert!(
            doc.yaml.contains(&case.find),
            "{}: stale fixture replacement",
            case.id
        );
        doc.yaml = doc.yaml.replace(&case.find, &case.replace);
        let error = core_error(DecisionEngine::from_core(&docs, schema()));
        assert_eq!(error.diagnostic.code, case.code, "{}: {error}", case.id);
        assert_eq!(
            error.diagnostic.stage.as_deref(),
            Some(case.stage.as_str()),
            "{}",
            case.id
        );
        assert_eq!(
            error.diagnostic.source.as_deref(),
            Some(case.document.as_str()),
            "{}",
            case.id
        );
        assert!(error.diagnostic.field_path.is_some());
    }
}

#[tokio::test]
async fn input_errors_and_registry_no_match_are_not_approval() {
    let mut docs = sources(&manifest().cases[0].documents);
    modify(&mut docs, "registry.yaml", |d| {
        d["registry"] = serde_json::json!([{"pipeline":"payment", "when":"event.amount > 0"}]);
    });
    let engine = DecisionEngine::from_core(&docs, schema()).unwrap();
    for event in [
        HashMap::new(),
        HashMap::from([("amount".into(), Value::Null)]),
        HashMap::from([("amount".into(), Value::String("1001".into()))]),
        HashMap::from([("amount".into(), Value::Number(f64::NAN))]),
    ] {
        let error = engine
            .decide(DecisionRequest::new(event))
            .await
            .unwrap_err();
        assert!(matches!(error, EngineError::Core(ref e) if e.diagnostic.code == "E_INPUT_SCHEMA"));
    }
    let error = engine.decide(request(0.0)).await.unwrap_err();
    assert!(
        matches!(error, EngineError::Core(ref e) if e.diagnostic.code == "E_NO_PIPELINE_MATCH")
    );
    let error = engine
        .decide(request(1001.0).with_vars(HashMap::new()))
        .await
        .unwrap_err();
    assert!(
        matches!(error, EngineError::Core(ref e) if e.diagnostic.code == "E_UNSUPPORTED_CAPABILITY")
    );
}

#[tokio::test]
async fn node_order_does_not_change_control_flow() {
    let mut docs = sources(&manifest().cases[1].documents);
    modify(&mut docs, "router.yaml", |d| {
        d["pipeline"]["steps"].as_array_mut().unwrap().reverse()
    });
    let engine = DecisionEngine::from_core(&docs, schema()).unwrap();
    assert_eq!(
        engine.decide(request(1001.0)).await.unwrap().result.score,
        67
    );
    assert_eq!(engine.decide(request(999.0)).await.unwrap().result.score, 0);
}

#[test]
fn graph_and_reference_guards() {
    let original = sources(&manifest().cases[1].documents);
    let mut docs = original.clone();
    modify(&mut docs, "router.yaml", |d| {
        d["pipeline"]["decision"][0]["when"] = "results.branch.signal == \"review\"".into()
    });
    assert_eq!(
        core_error(DecisionEngine::from_core(&docs, schema()))
            .diagnostic
            .code,
        "E_INVALID_REF"
    );
    let mut docs = original.clone();
    let mut duplicate = docs[0].clone();
    duplicate.path = "duplicate.yaml".into();
    docs.push(duplicate);
    assert_eq!(
        core_error(DecisionEngine::from_core(&docs, schema()))
            .diagnostic
            .code,
        "E_DUPLICATE_ID"
    );
    let mut docs = original;
    modify(&mut docs, "router.yaml", |d| {
        d["pipeline"]["steps"][1]["step"]["default"] = "missing".into()
    });
    assert_eq!(
        core_error(DecisionEngine::from_core(&docs, schema()))
            .diagnostic
            .code,
        "E_INVALID_GRAPH"
    );
}

#[tokio::test]
async fn score_overflow_is_a_controlled_error() {
    let mut docs = sources(&manifest().cases[1].documents);
    modify(&mut docs, "rule.yaml", |d| {
        d["rule"]["score"] = i32::MAX.into()
    });
    let engine = DecisionEngine::from_core(&docs, schema()).unwrap();
    let plain_error = engine
        .decide(request(1001.0))
        .await
        .unwrap_err()
        .to_string();
    let traced_error = engine
        .decide(request(1001.0).with_trace())
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(plain_error, traced_error);
    assert!(engine
        .decide(request(1001.0))
        .await
        .unwrap_err()
        .to_string()
        .contains("E_SCORE_OVERFLOW"));
    modify(&mut docs, "ruleset.yaml", |d| {
        d["ruleset"]["rules"] = serde_json::json!(["large_amount", "branch_marker"]);
    });
    let engine = DecisionEngine::from_core(&docs, schema()).unwrap();
    assert!(engine
        .decide(request(1001.0))
        .await
        .unwrap_err()
        .to_string()
        .contains("i32 score overflow"));
}

#[tokio::test]
async fn condition_trace_preserves_zero_score_rule_and_unselected_steps() {
    let mut docs = sources(&manifest().cases[1].documents);
    modify(&mut docs, "rule.yaml", |d| d["rule"]["score"] = 0.into());
    // Source order changes pointer positions, not control flow or observation results.
    modify(&mut docs, "router.yaml", |d| {
        d["pipeline"]["steps"].as_array_mut().unwrap().reverse()
    });
    let engine = DecisionEngine::from_core(&docs, schema()).unwrap();
    for amount in [1000.0, 1001.0] {
        let plain = engine.decide(request(amount)).await.unwrap();
        let traced = engine.decide(request(amount).with_trace()).await.unwrap();
        assert_eq!(plain.result, traced.result);
        assert_eq!(traced.result.score, 0);
        assert_eq!(
            traced
                .result
                .triggered_rules
                .contains(&"large_amount".into()),
            amount > 1000.0
        );
        let records = traced.trace.unwrap().core_conditions_v1.unwrap();
        assert!(!records.iter().any(|r| r.resource_id == "branch_marker"));
        assert!(records
            .iter()
            .any(|r| r.field_path == "/pipeline/steps/1/step/routes/0/when"
                && r.node_path.is_empty()));
    }
}

#[test]
fn imports_and_legacy_generator_shapes_are_not_silently_accepted() {
    let mut docs = sources(&manifest().cases[0].documents);
    docs[0]
        .yaml
        .push_str("import: {rules: ['../../private.yaml']}\n");
    let error = match validate_core_document(&docs[0]) {
        Err(e) => e,
        Ok(_) => panic!("import accepted"),
    };
    assert_eq!(error.diagnostic.code, "E_UNSUPPORTED_CAPABILITY");
    let source = CoreSource {
        path: "agent-output.yaml".into(),
        yaml: "version: '0.1'\nrule:\n  id: generated\n  when: 'true'\n  score: 60\n".into(),
    };
    let error = match validate_core_document(&source) {
        Err(e) => e,
        Ok(_) => panic!("missing name accepted"),
    };
    assert_eq!(error.diagnostic.code, "E_MISSING_FIELD");
    let serialized = serde_json::to_value(error).unwrap();
    assert_eq!(serialized["source"], "agent-output.yaml");
}

#[tokio::test]
async fn boolean_forms_share_short_circuit_execution() {
    use corint_decision_compiler::core::compile_core;
    use corint_decision_runtime::PipelineExecutor;
    let forms = [
        (serde_json::json!("false && !event.flag"), false, false),
        (serde_json::json!("true || !event.flag"), true, false),
        (
            serde_json::json!({"all":["false", "!event.flag"]}),
            false,
            false,
        ),
        (
            serde_json::json!({"any":["true", "!event.flag"]}),
            true,
            false,
        ),
        (
            serde_json::json!({"not":[{"all":["false", "!event.flag"]}]}),
            true,
            false,
        ),
        (serde_json::json!("true && !event.flag"), false, true),
        (serde_json::json!("false || !event.flag"), false, true),
    ];
    for (condition, matched, must_evaluate) in forms {
        let mut docs = sources(&manifest().cases[0].documents);
        modify(&mut docs, "rule.yaml", |d| {
            d["rule"]["when"] = condition.clone()
        });
        let input_schema =
            schema().add_field(SchemaField::new("flag".into(), FieldType::Boolean).required());
        let compiled = compile_core(&docs, input_schema).unwrap();
        let rule = compiled
            .programs
            .iter()
            .find(|p| p.metadata.source_id == "large_amount")
            .unwrap();
        // VM-level fault injection proves the RHS is not evaluated. Such a
        // malformed request is rejected by the public engine input gate (N09).
        for enabled in [false, true] {
            let mut state = corint_decision_runtime::result::ExecutionResult::new();
            if enabled {
                state.variables.insert(
                    corint_decision_model::ir::condition_map::TRACE_ENABLED.into(),
                    Value::Bool(true),
                );
            }
            let result = PipelineExecutor::new_offline()
                .execute_with_result(
                    rule,
                    corint_decision_runtime::ContextInput::new(HashMap::from([
                        ("amount".into(), Value::Number(1001.0)),
                        (
                            "flag".into(),
                            Value::String("invalid boolean operand".into()),
                        ),
                    ])),
                    state,
                )
                .await;
            if must_evaluate {
                assert!(result.is_err(), "{condition}");
            } else {
                let result = result.unwrap();
                assert_eq!(result.score, if matched { 60 } else { 0 }, "{condition}");
                if enabled {
                    let records =
                        &result.context[corint_decision_model::ir::condition_map::CONDITION_TRACE];
                    assert!(serde_json::to_string(records)
                        .unwrap()
                        .contains("short_circuit"));
                }
            }
        }
    }
}

#[tokio::test]
async fn boolean_conditions_work_across_public_scopes() {
    for condition in [
        serde_json::json!("event.amount > 1000 && !(event.amount < 0)"),
        serde_json::json!({"all": ["event.amount > 1000", {"not": ["event.amount < 0"]}]}),
    ] {
        let mut docs = sources(&manifest().cases[0].documents);
        modify(&mut docs, "rule.yaml", |d| {
            d["rule"]["when"] = condition.clone()
        });
        modify(&mut docs, "registry.yaml", |d| {
            d["registry"][0]["when"] = condition.clone()
        });
        modify(&mut docs, "pipeline.yaml", |d| {
            d["pipeline"]["decision"][0]["when"] = condition.clone()
        });
        let engine = DecisionEngine::from_core(&docs, schema()).unwrap();
        for (amount, expected) in [(1001.0, 60), (1000.0, 0)] {
            let plain = engine.decide(request(amount)).await.unwrap();
            let traced = engine.decide(request(amount).with_trace()).await.unwrap();
            assert_eq!(plain.result.score, expected);
            assert_eq!(plain.result, traced.result);
        }
    }
}

#[tokio::test]
async fn condition_trace_covers_scopes_skips_and_shared_rule_invocations() {
    use corint_decision_runtime::result::{CoreConditionOutcome as Outcome, CoreSkipReason};
    let mut docs = sources(&manifest().cases[1].documents);
    let condition = serde_json::json!({"any": ["true", "event.amount > 1000"]});
    modify(&mut docs, "rule.yaml", |d| {
        d["rule"]["when"] = condition.clone()
    });
    modify(&mut docs, "registry.yaml", |d| {
        d["registry"][0]["when"] = "false".into();
        d["registry"][1]["when"] = condition.clone();
        let unused = d["registry"][1].clone();
        d["registry"].as_array_mut().unwrap().push(unused);
    });
    modify(&mut docs, "ruleset.yaml", |d| {
        d["ruleset"]["conclusion"][0]["when"] = "true || total_score > 0".into();
    });
    modify(&mut docs, "branch_ruleset.yaml", |d| {
        d["ruleset"]["rules"] = serde_json::json!(["large_amount"])
    });
    modify(&mut docs, "router.yaml", |d| {
        d["pipeline"]["steps"][1]["step"]["routes"][0]["when"] = condition.clone();
        let unused = d["pipeline"]["steps"][1]["step"]["routes"][0].clone();
        d["pipeline"]["steps"][1]["step"]["routes"]
            .as_array_mut()
            .unwrap()
            .push(unused);
        d["pipeline"]["decision"][0]["when"] = condition.clone();
        let unused = d["pipeline"]["decision"][0].clone();
        d["pipeline"]["decision"]
            .as_array_mut()
            .unwrap()
            .insert(1, unused);
    });
    let engine = DecisionEngine::from_core(&docs, schema()).unwrap();
    let plain = engine.decide(request(0.0)).await.unwrap();
    let traced = engine.decide(request(0.0).with_trace()).await.unwrap();
    assert_eq!(plain.result, traced.result);
    assert!(plain.trace.is_none());
    assert_eq!(traced.result.score, 120);
    assert!(!traced
        .result
        .context
        .keys()
        .any(|k| k.starts_with("__core_trace") || k == "__core_condition_trace__"));
    let records = traced.trace.unwrap().core_conditions_v1.unwrap();
    for path in [
        "/registry/1/when",
        "/rule/when",
        "/ruleset/conclusion/0/when",
        "/pipeline/steps/1/step/routes/0/when",
        "/pipeline/decision/0/when",
    ] {
        assert!(
            records.iter().any(|r| r.field_path == path
                && r.node_path.is_empty()
                && r.outcome == Outcome::Evaluated { result: true }),
            "root {path}"
        );
        assert!(
            records.iter().any(|r| r.field_path == path
                && r.outcome
                    == Outcome::Skipped {
                        reason: CoreSkipReason::ShortCircuit
                    }),
            "skip {path}"
        );
    }
    for path in [
        "/ruleset/conclusion/1/when",
        "/pipeline/steps/1/step/routes/1/when",
        "/pipeline/decision/1/when",
    ] {
        assert!(
            records.iter().any(|r| r.field_path == path
                && r.node_path.is_empty()
                && r.outcome
                    == Outcome::Skipped {
                        reason: CoreSkipReason::NotReached
                    }),
            "unreached {path}"
        );
    }
    assert!(records.iter().any(|r| r.field_path == "/registry/0/when"
        && r.node_path.is_empty()
        && r.outcome == Outcome::Evaluated { result: false }));
    assert!(!records.iter().any(|r| r.field_path == "/registry/2/when"));
    let calls: std::collections::BTreeSet<_> = records
        .iter()
        .filter(|r| r.resource_id == "large_amount")
        .map(|r| r.invocation)
        .collect();
    assert_eq!(calls.len(), 2);
    assert!(records
        .iter()
        .all(|r| docs.iter().any(|s| s.path == r.source)));
    let again = engine.decide(request(0.0).with_trace()).await.unwrap();
    assert_eq!(records, again.trace.unwrap().core_conditions_v1.unwrap());
}

#[test]
fn capability_evidence_and_schema_are_in_sync() {
    let capabilities: Json =
        serde_json::from_str(include_str!("../../../docs/cdl/schema/capabilities.json")).unwrap();
    assert_eq!(capabilities["profile"], PROFILE);
    assert_eq!(capabilities["language_version"], "0.1");
    assert_eq!(
        capabilities["condition_trace"]["schema"],
        "condition-trace.json"
    );
    assert_eq!(
        capabilities["condition_trace"]["field"],
        "trace.core_conditions_v1"
    );
    assert_eq!(capabilities["condition_trace"]["raw_operand_values"], false);
    assert_eq!(capabilities["example_registry"], "../examples.json");
    let manifest = manifest();
    let test_source = include_str!("cdl_core_conformance.rs");
    let mut ids = std::collections::BTreeSet::new();
    for capability in capabilities["capabilities"].as_array().unwrap() {
        assert!(ids.insert(capability["id"].as_str().unwrap()));
        assert_eq!(capability["status"], "supported");
        let cases = capability["cases"].as_array().unwrap();
        let tests = capability["tests"].as_array().unwrap();
        assert!(!cases.is_empty() || !tests.is_empty());
        for case in cases {
            assert!(manifest
                .cases
                .iter()
                .any(|c| c.id == case.as_str().unwrap()));
        }
        for test in tests {
            assert!(
                test_source.contains(&format!("fn {}(", test.as_str().unwrap())),
                "Missing evidence test: {test}"
            );
        }
    }
    assert!(
        !test_source.contains(concat!("#[", "ignore")),
        "Core evidence must not be ignored"
    );
    let schema: Json = serde_json::from_str(corint_decision_compiler::core::CORE_SCHEMA).unwrap();
    assert_eq!(
        schema["properties"]["version"]["const"],
        capabilities["language_version"]
    );
    let mut case_ids = std::collections::BTreeSet::new();
    for case in &manifest.cases {
        assert!(case_ids.insert(&case.id));
        assert!(!case.runs.is_empty());
        for doc in sources(&case.documents) {
            validate_core_document(&doc).unwrap();
        }
    }
    for case in &manifest.invalid {
        assert!(case_ids.insert(&case.id));
    }
    for case in capabilities["condition_trace"]["cases"].as_array().unwrap() {
        assert!(manifest
            .cases
            .iter()
            .any(|c| c.id == case.as_str().unwrap()
                && c.runs.iter().any(|r| !r.conditions.is_empty())));
    }
    for test in capabilities["condition_trace"]["tests"].as_array().unwrap() {
        assert!(test_source.contains(&format!("fn {}(", test.as_str().unwrap())));
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExampleRegistry {
    version: u32,
    profile: String,
    language_version: String,
    pages: Vec<String>,
    compatibility_pages: Vec<String>,
    examples: Vec<DocumentExample>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentExample {
    id: String,
    kind: String,
    page: String,
    fixture: String,
    case_id: String,
}

fn check_example_mapping(
    index: &ExampleRegistry,
    pages: &HashMap<String, String>,
) -> Result<(), String> {
    use std::collections::BTreeSet;
    if index.version != 1 || index.profile != PROFILE || index.language_version != "0.1" {
        return Err("version/profile".into());
    }
    let registered: BTreeSet<_> = index.pages.iter().collect();
    if registered.len() != index.pages.len() || registered.is_empty() {
        return Err("duplicate/empty pages".into());
    }
    let compatibility: BTreeSet<_> = index.compatibility_pages.iter().collect();
    if compatibility.len() != index.compatibility_pages.len()
        || !registered.is_disjoint(&compatibility)
    {
        return Err("duplicate/conflicting page classification".into());
    }
    for page in &index.compatibility_pages {
        let text = pages.get(page).ok_or("missing compatibility page")?;
        if !text.contains("<!-- cdl-scope: compatibility-unverified -->")
            || !text.contains("This page is an unverified compatibility reference. Its snippets are not Core support evidence.")
            || !text.contains("](cdl-core.md)")
            || !text.contains("](schema/capabilities.json)")
        {
            return Err(format!("missing compatibility scope: {page}"));
        }
        let lower = text.to_lowercase();
        if [
            "✅",
            "🟢",
            "production-ready",
            "production ready",
            "ready for production",
            "available now",
            "cdl-example:",
            "supported_complete",
        ]
        .iter()
        .any(|claim| lower.contains(claim))
        {
            return Err(format!("unverified support claim: {page}"));
        }
    }
    let mut markers = BTreeSet::new();
    for page in &index.pages {
        let text = pages.get(page).ok_or("missing page")?;
        if text.lines().any(|line| {
            let line = line.trim();
            (line.starts_with("```") || line.starts_with("~~~"))
                && matches!(
                    line.trim_start_matches(['`', '~'])
                        .trim()
                        .to_ascii_lowercase()
                        .as_str(),
                    "yaml" | "yml"
                )
        }) {
            return Err("inline YAML must use a fixture".into());
        }
        for line in text.lines().filter(|l| l.contains("cdl-example:")) {
            let id = line
                .strip_prefix("<!-- cdl-example: ")
                .and_then(|l| l.strip_suffix(" -->"))
                .ok_or("malformed marker")?;
            if !markers.insert((page.clone(), id.to_owned())) {
                return Err("duplicate marker".into());
            }
        }
    }
    let cases = manifest();
    let mut ids = BTreeSet::new();
    for example in &index.examples {
        if !registered.contains(&example.page) || !ids.insert(&example.id) {
            return Err("duplicate/unregistered example".into());
        }
        if !markers.remove(&(example.page.clone(), example.id.clone())) {
            return Err("missing marker".into());
        }
        if example.fixture != "../../tests/conformance/cdl_core/manifest.yaml"
            || !pages[&example.page].contains(&format!("]({})", example.fixture))
        {
            return Err("missing fixture link".into());
        }
        match example.kind.as_str() {
            "supported_complete" => {
                let case = cases
                    .cases
                    .iter()
                    .find(|c| c.id == example.case_id)
                    .ok_or("missing behavior case")?;
                if case.runs.is_empty() || case.documents.is_empty() {
                    return Err("incomplete example".into());
                }
                for file in &case.documents {
                    if !root().join(file).is_file() {
                        return Err("missing source".into());
                    }
                }
            }
            "negative" => {
                let case = cases
                    .invalid
                    .iter()
                    .find(|c| c.id == example.case_id)
                    .ok_or("missing negative case")?;
                if case.code.is_empty() || case.stage.is_empty() {
                    return Err("missing expected error".into());
                }
            }
            _ => return Err("unclassified example".into()),
        }
    }
    if !markers.is_empty() {
        return Err("unmapped marker".into());
    }
    Ok(())
}

#[test]
fn documentation_examples_are_classified_and_bound_to_executed_fixtures() {
    let index: ExampleRegistry =
        serde_json::from_str(include_str!("../../../docs/cdl/examples.json")).unwrap();
    let docs = root().join("../../../docs/cdl");
    let pages: HashMap<_, _> = index
        .pages
        .iter()
        .chain(&index.compatibility_pages)
        .map(|p| (p.clone(), std::fs::read_to_string(docs.join(p)).unwrap()))
        .collect();
    check_example_mapping(&index, &pages).unwrap();
    let mut invalid = index.clone();
    invalid.examples[0].case_id = "unverified".into();
    assert!(check_example_mapping(&invalid, &pages).is_err());
    let mut invalid = pages.clone();
    invalid
        .get_mut("cdl-core.md")
        .unwrap()
        .push_str("\n<!-- cdl-example: unregistered -->\n");
    assert!(check_example_mapping(&index, &invalid).is_err());
    let mut invalid = pages.clone();
    *invalid.get_mut("condition-trace.md").unwrap() = pages["condition-trace.md"].replace(
        "../../tests/conformance/cdl_core/manifest.yaml",
        "missing.yaml",
    );
    assert!(check_example_mapping(&index, &invalid).is_err());
    let mut invalid = pages.clone();
    invalid
        .get_mut("cdl-core.md")
        .unwrap()
        .push_str("\n```yaml\nrule: {}\n```\n");
    assert!(check_example_mapping(&index, &invalid).is_err());
    let mut invalid = pages.clone();
    invalid
        .get_mut("pipeline.md")
        .unwrap()
        .push_str("\n~~~YAML\npipeline: {}\n~~~\n");
    assert!(check_example_mapping(&index, &invalid).is_err());
    for page in &index.compatibility_pages {
        let mut invalid = pages.clone();
        *invalid.get_mut(page).unwrap() =
            pages[page].replace("<!-- cdl-scope: compatibility-unverified -->", "");
        assert!(check_example_mapping(&index, &invalid).is_err(), "{page}");
        for claim in [
            "✅ Implemented",
            "Production-ready",
            "<!-- cdl-example: fake -->",
        ] {
            let mut invalid = pages.clone();
            invalid.get_mut(page).unwrap().push_str(claim);
            assert!(
                check_example_mapping(&index, &invalid).is_err(),
                "{page}: {claim}"
            );
        }
    }
    let mut invalid = index.clone();
    invalid.compatibility_pages.push("pipeline.md".into());
    assert!(check_example_mapping(&invalid, &pages).is_err());
    let mut invalid = index;
    invalid.examples[0].page = "api.md".into();
    assert!(check_example_mapping(&invalid, &pages).is_err());
}

#[test]
fn condition_trace_schema_distinguishes_skipped_from_false_and_is_additive() {
    let schema: Json = serde_json::from_str(include_str!(
        "../../../docs/cdl/schema/condition-trace.json"
    ))
    .unwrap();
    let validator = jsonschema::JSONSchema::compile(&schema).unwrap();
    let mut records = serde_json::json!([{"source":"rule.yaml","resource_type":"rule","resource_id":"r","invocation":0,
        "field_path":"/rule/when","node_path":"","outcome":{"status":"evaluated","result":false}}]);
    assert!(validator.is_valid(&records));
    records[0]["outcome"] = serde_json::json!({"status":"skipped","reason":"short_circuit"});
    assert!(validator.is_valid(&records));
    records[0]["outcome"]["result"] = false.into();
    assert!(!validator.is_valid(&records));
    assert!(
        serde_json::from_value::<Vec<corint_decision_runtime::result::CoreConditionTrace>>(records)
            .is_err()
    );
    let old = corint_decision_runtime::ExecutionTrace::new();
    let json = serde_json::to_value(old).unwrap();
    assert!(json.get("core_conditions_v1").is_none());
    let old: corint_decision_runtime::ExecutionTrace = serde_json::from_value(json).unwrap();
    assert!(old.core_conditions_v1.is_none());
}

#[tokio::test]
async fn scalar_input_and_integral_score_contract() {
    let mut docs = sources(&manifest().cases[0].documents);
    modify(&mut docs, "rule.yaml", |d| {
        d["rule"]["when"] = "event.flag && event.channel == \"mobile\"".into();
        d["rule"]["score"] = serde_json::json!(60.0);
    });
    let schema = schema()
        .add_field(SchemaField::new("flag".into(), FieldType::Boolean).required())
        .add_field(SchemaField::new("channel".into(), FieldType::String).required());
    let engine = DecisionEngine::from_core(&docs, schema).unwrap();
    for (flag, expected) in [(true, 60), (false, 0)] {
        let mut req = request(1001.0);
        req.event_data.insert("flag".into(), Value::Bool(flag));
        req.event_data
            .insert("channel".into(), Value::String("mobile".into()));
        assert_eq!(engine.decide(req).await.unwrap().result.score, expected);
    }
}

#[test]
fn malformed_defaults_and_ids_are_rejected() {
    for (name, field, code) in [
        ("missing_step_name", "name", "E_MISSING_FIELD"),
        ("missing_step_id", "id", "E_MISSING_FIELD"),
        ("missing_next", "next", "E_MISSING_FIELD"),
    ] {
        let mut docs = sources(&manifest().cases[0].documents);
        modify(&mut docs, "pipeline.yaml", |d| {
            d["pipeline"]["steps"][0]["step"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        });
        assert_eq!(
            core_error(DecisionEngine::from_core(&docs, schema()))
                .diagnostic
                .code,
            code,
            "{name}"
        );
    }
    for duplicate in [false, true] {
        let mut docs = sources(&manifest().cases[0].documents);
        modify(&mut docs, "pipeline.yaml", |d| {
            let rows = d["pipeline"]["decision"].as_array_mut().unwrap();
            if duplicate {
                rows.push(rows.last().unwrap().clone());
            } else {
                rows.rotate_right(1);
            }
        });
        assert_eq!(
            core_error(DecisionEngine::from_core(&docs, schema()))
                .diagnostic
                .code,
            "E_INVALID_STRUCTURE"
        );
    }
}
