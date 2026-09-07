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

fn matching_fixture() -> (Vec<CoreSource>, Schema, HashMap<String, Value>) {
    let sources = sources(
        &[
            "rule.yaml",
            "ruleset.yaml",
            "pipeline.yaml",
            "registry.yaml",
        ]
        .map(String::from),
    );
    let mut input = schema();
    for name in ["country", "text", "needle", "optional"] {
        input.fields.insert(
            name.into(),
            SchemaField {
                name: name.into(),
                field_type: FieldType::String,
                required: name != "optional",
                description: None,
                default: None,
            },
        );
    }
    let event = HashMap::from([
        ("amount".into(), Value::Number(1001.0)),
        ("country".into(), Value::String("中国".into())),
        ("text".into(), Value::String("支付🙂.COM".into())),
        ("needle".into(), Value::String("🙂".into())),
    ]);
    (sources, input, event)
}

#[tokio::test]
async fn core_membership_and_string_matching_preserve_execution_and_trace() {
    use corint_decision_compiler::{Compiler, CompilerOptions};
    use corint_decision_dsl_parser::RuleParser;
    use corint_decision_runtime::PipelineExecutor;
    let (base, input, event) = matching_fixture();
    for (expression, expected) in [
        (r#"event.country in ["US", "中国", "中国"]"#, true),
        (r#"event.country not in ["US", "中国"]"#, false),
        (r#"event.country not_in ["US"]"#, true),
        ("event.amount - 1002 in [-1, 0]", true),
        ("true in [false, true]", true),
        ("event.amount in []", false),
        ("event.country not in []", true),
        ("-0 in [0]", true),
        (r#"event.text contains event.needle"#, true),
        (r#"event.text contains "com""#, false),
        (r#"event.text starts_with "支付""#, true),
        (r#"event.text ends_with ".COM""#, true),
        (r#"event.text ends_with ".com""#, false),
        (r#"event.text contains """#, true),
        (r#""" starts_with """#, true),
        (r#""" ends_with """#, true),
        (r#""é" contains "é""#, false),
        (r#"event.text regex "支付""#, true),
        (r#"event.text regex "^支付$""#, false),
        (r#"event.text regex "(?i)com$""#, true),
        (r#"event.text regex "\\p{Han}+""#, true),
        (r#"event.text regex """#, true),
        (r#"true || event.optional contains "x""#, true),
        (r#"false && event.optional regex "x""#, false),
        (
            r#"exists(event.optional) && event.optional starts_with "x""#,
            false,
        ),
        ("false || true in [true] && 1 in [1]", true),
        (r#""in contains regex" in ["in contains regex"]"#, true),
    ] {
        let mut sources = base.clone();
        modify(&mut sources, "rule.yaml", |doc| {
            doc["rule"]["when"] = expression.into()
        });
        let engine = DecisionEngine::from_core(&sources, input.clone())
            .unwrap_or_else(|e| panic!("{expression}: {e}"));
        let mut plain = None;
        for trace in [false, true] {
            let mut req = DecisionRequest::new(event.clone());
            req.options.enable_trace = trace;
            let response = engine
                .decide(req)
                .await
                .unwrap_or_else(|e| panic!("{expression}: {e}"));
            assert_eq!(
                response.result.score,
                if expected { 60 } else { 0 },
                "{expression}"
            );
            if let Some(previous) = &plain {
                assert_eq!(&response.result, previous, "{expression}");
            } else {
                plain = Some(response.result.clone());
            }
            assert_eq!(response.trace.is_some(), trace);
        }
        // Compare the same parsed rule through optimized and unoptimized VM programs.
        // Optional input errors are a strict entry contract; skip those cases here.
        if !expression.contains("optional") {
            let rule = RuleParser::parse(&sources[0].yaml).unwrap();
            for optimized in [false, true] {
                let program = Compiler::with_options(CompilerOptions {
                    enable_constant_folding: optimized,
                    enable_dead_code_elimination: optimized,
                    ..Default::default()
                })
                .compile_rule(&rule)
                .unwrap();
                let result = PipelineExecutor::new_offline()
                    .execute(&program, event.clone())
                    .await
                    .unwrap();
                assert_eq!(
                    result.score,
                    if expected { 60 } else { 0 },
                    "{expression}, optimized={optimized}"
                );
            }
        }
    }
}

#[test]
fn core_matching_rejects_invalid_types_patterns_and_collection_limits() {
    let (base, input, _) = matching_fixture();
    let oversized_array = format!("event.amount in [{}]", vec!["1"; 1025].join(","));
    let oversized_pattern = format!("event.text regex \"{}\"", "a".repeat(4097));
    for (expression, code) in [
        ("event.amount in [1, \"1\"]", "E_TYPE"),
        ("event.amount in [true]", "E_TYPE"),
        ("event.amount in [null]", "E_TYPE"),
        ("event.amount in [[1]]", "E_TYPE"),
        ("event.amount in [1e999]", "E_TYPE"),
        ("event.amount in event.country", "E_TYPE"),
        ("event.text contains 1", "E_TYPE"),
        ("event.amount starts_with \"1\"", "E_TYPE"),
        ("event.text ends_with false", "E_TYPE"),
        ("event.text regex event.needle", "E_TYPE"),
        ("event.amount regex \"1\"", "E_TYPE"),
        ("false && event.text regex \"[\"", "E_INVALID_REGEX"),
        ("event.text regex \"(?=x)\"", "E_INVALID_REGEX"),
        ("event.text regex \"a{10000000}\"", "E_INVALID_REGEX"),
        (&oversized_array, "E_EXPRESSION_LIMIT"),
        (&oversized_pattern, "E_INVALID_REGEX"),
        ("event.text in list.blocked", "E_UNSUPPORTED_CAPABILITY"),
    ] {
        let mut sources = base.clone();
        modify(&mut sources, "rule.yaml", |doc| {
            doc["rule"]["when"] = expression.into()
        });
        let error = core_error(DecisionEngine::from_core(&sources, input.clone()));
        assert_eq!(error.diagnostic.code, code, "{expression}: {error}");
        assert_eq!(error.diagnostic.field_path.as_deref(), Some("/rule/when"));
    }
}

#[tokio::test]
async fn core_matching_works_in_all_condition_scopes_and_missing_inputs_fail() {
    let (mut sources, input, event) = matching_fixture();
    let condition = r#"event.country in ["中国"] && event.text contains "🙂" && event.text starts_with "支付" && event.text ends_with ".COM" && event.text regex "^支付" && event.country not in ["US"]"#;
    modify(&mut sources, "registry.yaml", |doc| {
        doc["registry"][0]["when"] = condition.into()
    });
    modify(&mut sources, "rule.yaml", |doc| {
        doc["rule"]["when"] = condition.into()
    });
    modify(&mut sources, "ruleset.yaml", |doc| {
        doc["ruleset"]["conclusion"] = serde_json::json!([
            {"when":condition,"signal":"decline"}, {"default":true,"signal":"approve"}
        ])
    });
    modify(&mut sources, "pipeline.yaml", |doc| {
        let pipeline = &mut doc["pipeline"];
        pipeline["when"] = condition.into();
        pipeline["steps"][0]["step"]["when"] = condition.into();
        pipeline["steps"][0]["step"]["next"] = "route".into();
        pipeline["steps"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"step":{
                "id":"route","name":"Route","type":"router",
                "routes":[{"when":condition,"next":"end"}],"default":"end"
            }}));
        pipeline["decision"] = serde_json::json!([
            {"when":condition,"result":"decline"}, {"default":true,"result":"approve"}
        ]);
    });
    let engine = DecisionEngine::from_core(&sources, input.clone()).unwrap();
    let plain = engine
        .decide(DecisionRequest::new(event.clone()))
        .await
        .unwrap();
    let traced = engine
        .decide(DecisionRequest::new(event.clone()).with_trace())
        .await
        .unwrap();
    assert_eq!(plain.result, traced.result);
    assert_eq!(plain.result.score, 60);
    assert_eq!(
        serde_json::to_value(plain.result.signal).unwrap()["type"],
        "decline"
    );
    let records = traced.trace.unwrap().core_conditions_v1.unwrap();
    for prefix in [
        "/registry/0/when",
        "/rule/when",
        "/ruleset/conclusion/0/when",
        "/pipeline/when",
        "/pipeline/steps/0/step/when",
        "/pipeline/steps/1/step/routes/0/when",
        "/pipeline/decision/0/when",
    ] {
        assert!(
            records.iter().any(|r| r.field_path == prefix),
            "Missing trace: {prefix}"
        );
    }
    for expression in [
        r#"event.optional in ["x"]"#,
        r#"event.optional contains "x""#,
        r#"event.optional regex "x""#,
    ] {
        let (mut base, _, _) = matching_fixture();
        modify(&mut base, "rule.yaml", |doc| {
            doc["rule"]["when"] = expression.into()
        });
        let engine = DecisionEngine::from_core(&base, input.clone()).unwrap();
        for trace in [false, true] {
            let mut req = DecisionRequest::new(event.clone());
            req.options.enable_trace = trace;
            let Err(EngineError::Core(error)) = engine.decide(req).await else {
                panic!("Missing field admitted");
            };
            assert_eq!(error.diagnostic.code, "E_MISSING_INPUT");
        }
    }
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
                        "../../../docs/contracts/schema/condition-trace.json"
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
async fn expression_precedence_strings_and_negation_preserve_decisions_and_trace() {
    let cases = [
        ("true || false && false", true, ""),
        ("true || false && (1 / 0 > 0)", true, ""),
        ("false && (1 / 0 > 0) || true", true, ""),
        ("(true || false) && false", false, ""),
        ("10 - 3 - 2 == 5 && 2 + 3 * 4 == 14", true, ""),
        (
            "-event.amount < 0 && event.amount > -1 && 1e-3 < 0.01",
            true,
            "",
        ),
        (r#"event.label == "中国""#, true, "中国"),
        (r#"event.label == "high-risk""#, true, "high-risk"),
        (r#"event.label == 'a/b'"#, true, "a/b"),
        (r#"event.label == "a\n\"b\\c""#, true, "a\n\"b\\c"),
        (
            r#"event.label == "\u4e2d\u56fd\uD83D\uDE00""#,
            true,
            "中国😀",
        ),
        (r#"event.label == "中国""#, false, "other"),
    ];
    for (condition, matched, label) in cases {
        let mut docs = sources(&manifest().cases[0].documents);
        modify(&mut docs, "rule.yaml", |d| {
            d["rule"]["when"] = condition.into()
        });
        let input =
            schema().add_field(SchemaField::new("label".into(), FieldType::String).required());
        let engine =
            DecisionEngine::from_core(&docs, input).unwrap_or_else(|e| panic!("{condition}: {e}"));
        let request = || {
            DecisionRequest::new(HashMap::from([
                ("amount".into(), Value::Number(1001.0)),
                ("label".into(), Value::String(label.into())),
            ]))
        };
        let plain = engine
            .decide(request())
            .await
            .unwrap_or_else(|e| panic!("{condition}: {e}"));
        let traced = engine.decide(request().with_trace()).await.unwrap();
        assert_eq!(plain.result, traced.result, "{condition}");
        assert_eq!(
            plain.result.score,
            if matched { 60 } else { 0 },
            "{condition}"
        );
        assert_eq!(
            plain
                .result
                .triggered_rules
                .contains(&"large_amount".into()),
            matched,
            "{condition}"
        );
    }
}

#[tokio::test]
async fn compatibility_string_conditions_short_circuit_and_lists_fail_closed() {
    use corint_decision_compiler::Compiler;
    use corint_decision_dsl_parser::{RuleParser, RulesetParser};
    use corint_decision_model::ast::Signal;
    use corint_decision_runtime::PipelineExecutor;
    let executor = PipelineExecutor::new_offline();
    for (condition, score) in [
        ("true || false && (1 / 0 > 0)", 60),
        ("false && (1 / 0 > 0)", 0),
        (r#""中国/high-risk" == '中国/high-risk'"#, 60),
    ] {
        let yaml = serde_yaml::to_string(&serde_json::json!({"rule": {
            "id": "test", "name": "Test", "when": condition, "score": 60
        }}))
        .unwrap();
        let rule = RuleParser::parse(&yaml).unwrap();
        let program = Compiler::new().compile_rule(&rule).unwrap();
        assert_eq!(
            executor
                .execute(&program, HashMap::new())
                .await
                .unwrap()
                .score,
            score,
            "{condition}"
        );
    }
    let ruleset = RulesetParser::parse("ruleset:\n  id: risk\n  rules: []\n  conclusion:\n    - when: 'true || false && (1 / 0 > 0)'\n      signal: decline\n    - default: true\n      signal: approve\n").unwrap();
    let program = Compiler::new().compile_ruleset(&ruleset).unwrap();
    assert_eq!(
        executor
            .execute(&program, HashMap::new())
            .await
            .unwrap()
            .signal,
        Some(Signal::Decline)
    );
    for operator in ["in", "not in"] {
        let yaml = serde_yaml::to_string(&serde_json::json!({"rule": {
            "id": "list_check", "name": "List check", "when": format!("'user-1' {operator} list.missing"), "score": 60
        }})).unwrap();
        let rule = RuleParser::parse(&yaml).unwrap();
        let program = Compiler::new().compile_rule(&rule).unwrap();
        let error = executor
            .execute(&program, HashMap::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("E_LIST_UNAVAILABLE"), "{operator}: {error}");
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
    let capabilities: Json = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/capabilities.json"
    ))
    .unwrap();
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
    assert_eq!(
        capabilities["example_registry"],
        "../../../tests/conformance/documentation/examples.json"
    );
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
    path_base: String,
    profile: String,
    language_version: String,
    pages: Vec<String>,
    compatibility_pages: Vec<String>,
    source_examples: Vec<SourceExample>,
    examples: Vec<DocumentExample>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceExample {
    path: String,
    kind: String,
    core_rejection_stage: String,
    core_rejection_code: String,
}

fn check_source_examples(
    index: &ExampleRegistry,
    files: &HashMap<String, String>,
) -> Result<(), String> {
    let mut remaining = files.clone();
    for example in &index.source_examples {
        let text = remaining
            .remove(&example.path)
            .ok_or("missing/duplicate source example")?;
        if example.kind != "compatibility-unverified"
            || !text.contains("# cdl-scope: compatibility-unverified")
            || ["production_ready", "production-ready", "supported_complete"]
                .iter()
                .any(|claim| text.to_lowercase().contains(claim))
        {
            return Err(format!("unverified source scope/claim: {}", example.path));
        }
        // Parse every YAML document, even when strict Core would reject the
        // import header before reaching the rest of this historical example.
        for document in serde_yaml::Deserializer::from_str(&text) {
            serde_yaml::Value::deserialize(document)
                .map_err(|e| format!("{}: {e}", example.path))?;
        }
        let source = CoreSource {
            path: example.path.clone(),
            yaml: text,
        };
        let error = validate_core_document(&source)
            .err()
            .ok_or("historical source unexpectedly accepted")?;
        if error.diagnostic.stage.as_deref() != Some(&example.core_rejection_stage)
            || error.diagnostic.code != example.core_rejection_code
        {
            return Err(format!(
                "unexpected Core rejection for {}: {error}",
                example.path
            ));
        }
    }
    if !remaining.is_empty() {
        return Err("unregistered source examples".into());
    }
    Ok(())
}

#[test]
fn standalone_source_examples_have_valid_yaml_and_explicit_scope() {
    let index: ExampleRegistry = serde_json::from_str(include_str!(
        "../../../tests/conformance/documentation/examples.json"
    ))
    .unwrap();
    let repository = root().join("../../..");
    let mut directories = vec![repository.join("tests/conformance/documentation/legacy")];
    let mut files = HashMap::new();
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("yaml" | "yml")
            ) {
                files.insert(
                    path.strip_prefix(&repository)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    std::fs::read_to_string(path).unwrap(),
                );
            }
        }
    }
    assert!(!files.is_empty());
    check_source_examples(&index, &files).unwrap();
    let mut invalid = index.clone();
    invalid.source_examples.clear();
    assert!(check_source_examples(&invalid, &files).is_err());
    let path = &index.source_examples[0].path;
    for mutation in [
        files[path].replace("# cdl-scope: compatibility-unverified", ""),
        format!("{}\n# production_ready\n", files[path]),
        format!("{}\n---\nbroken: [\n", files[path]),
    ] {
        let mut invalid = files.clone();
        invalid.insert(path.clone(), mutation);
        assert!(check_source_examples(&index, &invalid).is_err());
    }
    let mut invalid = files.clone();
    invalid.insert(
        "tests/conformance/documentation/legacy/unregistered.yml".into(),
        files[path].clone(),
    );
    assert!(check_source_examples(&index, &invalid).is_err());
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentExample {
    id: String,
    kind: String,
    page: String,
    fixture: String,
    source: String,
    case_id: String,
}

fn page_links_to(page: &str, text: &str, target: &str) -> bool {
    let repository = root().join("../../..");
    let Ok(target) = repository.join(target).canonicalize() else {
        return false;
    };
    let source = repository.join(page);
    text.split("](")
        .skip(1)
        .filter_map(|link| link.split_once(')'))
        .any(|(link, _)| {
            source
                .parent()
                .unwrap()
                .join(link.split('#').next().unwrap())
                .canonicalize()
                .is_ok_and(|resolved| resolved == target)
        })
}

fn check_example_mapping(
    index: &ExampleRegistry,
    pages: &HashMap<String, String>,
) -> Result<(), String> {
    use std::collections::BTreeSet;
    if index.version != 2
        || index.path_base != "repository"
        || index.profile != PROFILE
        || index.language_version != "0.1"
    {
        return Err("version/path base/profile".into());
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
            || !(text.contains("This page is an unverified compatibility reference. Its snippets are not Core support evidence.")
                || text.contains("Historical snippets on this page are unverified compatibility references. They are not Core support evidence."))
            || !page_links_to(page, text, "CDL/overall.md")
            || (!page.starts_with("CDL/") && !page_links_to(page, text, "docs/contracts/schema/capabilities.json"))
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
        if example.fixture != "tests/conformance/cdl_core/manifest.yaml"
            || !page_links_to(&example.page, &pages[&example.page], &example.source)
        {
            return Err("missing example source link".into());
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
                if !case
                    .documents
                    .iter()
                    .any(|file| example.source == format!("tests/conformance/cdl_core/{file}"))
                {
                    return Err("example source is not executed by its bound case".into());
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
                if example.source != format!("tests/conformance/cdl_core/{}", case.document) {
                    return Err("negative example source does not match its bound case".into());
                }
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
    let index: ExampleRegistry = serde_json::from_str(include_str!(
        "../../../tests/conformance/documentation/examples.json"
    ))
    .unwrap();
    let repository = root().join("../../..");
    let pages: HashMap<_, _> = index
        .pages
        .iter()
        .chain(&index.compatibility_pages)
        .map(|p| {
            (
                p.clone(),
                std::fs::read_to_string(repository.join(p)).unwrap(),
            )
        })
        .collect();
    check_example_mapping(&index, &pages).unwrap();
    let mut invalid = index.clone();
    invalid.examples[0].case_id = "unverified".into();
    assert!(check_example_mapping(&invalid, &pages).is_err());
    let mut invalid = pages.clone();
    invalid
        .get_mut("docs/runtime-validation.md")
        .unwrap()
        .push_str("\n<!-- cdl-example: unregistered -->\n");
    assert!(check_example_mapping(&index, &invalid).is_err());
    let mut invalid = pages.clone();
    *invalid.get_mut("docs/runtime-validation.md").unwrap() = pages["docs/runtime-validation.md"]
        .replace(
            "../tests/conformance/cdl_core/trace_rule.yaml",
            "missing.yaml",
        );
    assert!(check_example_mapping(&index, &invalid).is_err());
    let mut invalid = pages.clone();
    invalid
        .get_mut("docs/runtime-validation.md")
        .unwrap()
        .push_str("\n```yaml\nrule: {}\n```\n");
    assert!(check_example_mapping(&index, &invalid).is_err());
    let mut invalid = pages.clone();
    invalid
        .get_mut("docs/runtime-validation.md")
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
    invalid
        .compatibility_pages
        .push("docs/runtime-validation.md".into());
    assert!(check_example_mapping(&invalid, &pages).is_err());
    let mut invalid = index;
    invalid.examples[0].page = "CDL/api.md".into();
    assert!(check_example_mapping(&invalid, &pages).is_err());
}

#[test]
fn condition_trace_schema_distinguishes_skipped_from_false_and_is_additive() {
    let schema: Json = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/condition-trace.json"
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

fn extension_schema() -> Schema {
    Schema::new("event".into())
        .add_field(SchemaField::new("enabled".into(), FieldType::Boolean).required())
        .add_field(SchemaField::new(
            "payment".into(),
            FieldType::object_with_schema(
                Schema::new("payment".into())
                    .add_field(SchemaField::new("amount".into(), FieldType::Number)),
            ),
        ))
}

fn extension_sources() -> Vec<CoreSource> {
    [
        "rule.yaml",
        "marker.yaml",
        "ruleset.yaml",
        "child.yaml",
        "pipeline.yaml",
        "registry.yaml",
    ]
    .into_iter()
    .map(|path| CoreSource {
        path: path.into(),
        yaml: std::fs::read_to_string(root().join("../core_extensions").join(path)).unwrap(),
    })
    .collect()
}

fn extension_request(event: Json, trace: bool) -> DecisionRequest {
    let mut request = DecisionRequest::new(serde_json::from_value(event).unwrap());
    request.options.enable_trace = trace;
    request
}

#[tokio::test]
async fn core_calls_guards_and_nested_optional_inputs_share_the_vm() {
    let engine = DecisionEngine::from_core(&extension_sources(), extension_schema()).unwrap();
    for trace in [false, true] {
        for (event, score, signal, actions) in [
            (
                serde_json::json!({"enabled":true,"payment":{"amount":1001}}),
                67,
                "review",
                vec!["parent_action"],
            ),
            (
                serde_json::json!({"enabled":true,"payment":{"amount":1000}}),
                0,
                "pass",
                vec![],
            ),
            (
                serde_json::json!({"enabled":true,"payment":{}}),
                0,
                "pass",
                vec![],
            ),
            (serde_json::json!({"enabled":true}), 0, "hold", vec![]),
            (
                serde_json::json!({"enabled":false,"payment":{"amount":1001}}),
                7,
                "hold",
                vec![],
            ),
        ] {
            let result = engine
                .decide(extension_request(event, trace))
                .await
                .unwrap();
            assert_eq!(result.result.score, score);
            assert_eq!(
                serde_json::to_value(&result.result.signal).unwrap()["type"],
                signal
            );
            assert_eq!(result.result.actions, actions);
            assert!(
                !result
                    .result
                    .context
                    .contains_key("__ruleset_result__.risk"),
                "callee results must not leak"
            );
            if trace {
                let calls = result
                    .trace
                    .as_ref()
                    .unwrap()
                    .core_calls_v1
                    .as_ref()
                    .unwrap();
                let child = calls.iter().find(|c| c.resource_id == "child").unwrap();
                assert_eq!(child.call_path, ["parent", "child"]);
                if signal == "hold" {
                    assert_eq!(child.status, "skipped");
                    assert_eq!(child.score, None);
                    assert_eq!(child.signal, None);
                } else {
                    assert!(calls
                        .iter()
                        .any(|c| c.call_path == ["parent", "child", "risk"]));
                    if score > 0 {
                        assert_eq!(child.actions, ["child_only"]);
                    }
                }
            } else {
                assert!(result.trace.is_none());
            }
        }
    }
}

#[tokio::test]
async fn core_extension_errors_are_structured_and_trace_invariant() {
    let base = extension_sources();
    for (expression, event, code) in [
        (
            "event.payment.amount > 0",
            serde_json::json!({"enabled":true}),
            "E_MISSING_INPUT",
        ),
        (
            "event.payment.amount / 0 > 0",
            serde_json::json!({"enabled":true,"payment":{"amount":1}}),
            "E_DIVISION_BY_ZERO",
        ),
        (
            "event.payment.amount * event.payment.amount > 0",
            serde_json::json!({"enabled":true,"payment":{"amount":1e308}}),
            "E_NUMBER_OVERFLOW",
        ),
    ] {
        let mut docs = base.clone();
        modify(&mut docs, "marker.yaml", |d| {
            d["rule"]["when"] = expression.into()
        });
        let engine = DecisionEngine::from_core(&docs, extension_schema()).unwrap();
        let mut errors = Vec::new();
        for trace in [false, true] {
            let error = engine
                .decide(extension_request(event.clone(), trace))
                .await
                .unwrap_err();
            let EngineError::Core(error) = error else {
                panic!("Expected structured Core error: {error}")
            };
            assert_eq!(error.diagnostic.code, code);
            assert_eq!(error.diagnostic.source.as_deref(), Some("marker.yaml"));
            assert_eq!(error.diagnostic.stage.as_deref(), Some("execute"));
            errors.push(serde_json::to_value(error).unwrap());
        }
        assert_eq!(errors[0], errors[1]);
    }
    let engine = DecisionEngine::from_core(&base, extension_schema()).unwrap();
    for event in [
        serde_json::json!({"enabled":true,"payment":null}),
        serde_json::json!({"enabled":true,"payment":{"amount":"1"}}),
        serde_json::json!({"enabled":true,"payment":{"unknown":1}}),
        serde_json::json!({"payment":{}}),
    ] {
        let EngineError::Core(error) = engine
            .decide(extension_request(event, false))
            .await
            .unwrap_err()
        else {
            panic!("Expected Core error")
        };
        assert_eq!(error.diagnostic.code, "E_INPUT_SCHEMA");
    }
    let mut docs = base.clone();
    modify(&mut docs, "pipeline.yaml", |d| {
        d["pipeline"]["when"] = "false".into()
    });
    let engine = DecisionEngine::from_core(&docs, extension_schema()).unwrap();
    for trace in [false, true] {
        let EngineError::Core(error) = engine
            .decide(extension_request(
                serde_json::json!({"enabled":true}),
                trace,
            ))
            .await
            .unwrap_err()
        else {
            panic!("Expected Core error")
        };
        assert_eq!(error.diagnostic.code, "E_PIPELINE_SKIPPED");
    }
    modify(&mut docs, "pipeline.yaml", |d| {
        d["pipeline"].as_object_mut().unwrap().remove("when");
        d["pipeline"]["decision"] = serde_json::json!([{"when":"results.child.score > 0","result":"decline"},{"default":true,"result":"pass"}]);
    });
    let engine = DecisionEngine::from_core(&docs, extension_schema()).unwrap();
    let EngineError::Core(error) = engine
        .decide(extension_request(
            serde_json::json!({"enabled":true}),
            false,
        ))
        .await
        .unwrap_err()
    else {
        panic!("Expected Core error")
    };
    assert_eq!(error.diagnostic.code, "E_RESULT_UNAVAILABLE");
}

#[test]
fn core_pipeline_duplicate_calls_report_the_resource_field() {
    for (kind, target) in [
        ("rule", "marker"),
        ("ruleset", "risk"),
        ("pipeline", "child"),
    ] {
        let mut docs = pipeline_call_sources(kind, target);
        DecisionEngine::from_core(&docs, extension_schema()).unwrap();
        modify(&mut docs, "pipeline.yaml", |d| {
            let steps = d["pipeline"]["steps"].as_array_mut().unwrap();
            let mut duplicate = steps[1].clone();
            duplicate["step"]["id"] = "second".into();
            steps[1]["step"]["next"] = "second".into();
            steps.push(duplicate);
        });
        let error = core_error(DecisionEngine::from_core(&docs, extension_schema()));
        assert_eq!(error.diagnostic.code, "E_INVALID_GRAPH", "{kind}: {error}");
        assert_eq!(error.diagnostic.stage.as_deref(), Some("resolve"));
        assert_eq!(error.diagnostic.source.as_deref(), Some("pipeline.yaml"));
        assert_eq!(
            error.diagnostic.field_path.as_deref(),
            Some(format!("/pipeline/steps/2/step/{kind}").as_str()),
            "{kind}: {error}"
        );
    }
}

#[test]
fn core_pipeline_unresolved_calls_report_the_resource_field() {
    for (kind, target, wrong_kind_target) in [
        ("rule", "marker", "risk"),
        ("ruleset", "risk", "marker"),
        ("pipeline", "child", "risk"),
    ] {
        let base = pipeline_call_sources(kind, target);
        DecisionEngine::from_core(&base, extension_schema()).unwrap();
        for invalid_target in ["unknown", wrong_kind_target] {
            let mut docs = base.clone();
            modify(&mut docs, "pipeline.yaml", |d| {
                d["pipeline"]["steps"][1]["step"][kind] = invalid_target.into();
            });
            let error = core_error(DecisionEngine::from_core(&docs, extension_schema()));
            assert_eq!(
                error.diagnostic.code, "E_UNRESOLVED_REF",
                "{kind} {invalid_target}: {error}"
            );
            assert_eq!(error.diagnostic.stage.as_deref(), Some("resolve"));
            assert_eq!(error.diagnostic.source.as_deref(), Some("pipeline.yaml"));
            assert_eq!(
                error.diagnostic.field_path.as_deref(),
                Some(format!("/pipeline/steps/1/step/{kind}").as_str()),
                "{kind} {invalid_target}: {error}"
            );
        }
    }
}

fn pipeline_call_sources(kind: &str, target: &str) -> Vec<CoreSource> {
    let mut docs = extension_sources();
    modify(&mut docs, "pipeline.yaml", |d| {
        d["pipeline"]["entry"] = "route".into();
        // The source index must include router steps as well as resource calls.
        d["pipeline"]["steps"] = serde_json::json!([
            {"step": {
                "id": "route", "name": "Route", "type": "router",
                "routes": [{"when": "true", "next": "first"}], "default": "end"
            }},
            {"step": {
                "id": "first", "name": "Call", "type": kind, (kind): target,
                "next": "end"
            }}
        ]);
        d["pipeline"]["decision"] = serde_json::json!([{"default": true, "result": "pass"}]);
    });
    docs
}

#[test]
fn core_call_graph_and_expression_extensions_fail_closed() {
    for (file, field, expression, code) in [
        (
            "marker.yaml",
            "rule",
            "exists(event.unknown)",
            "E_INVALID_REF",
        ),
        (
            "marker.yaml",
            "rule",
            "exists(event.payment, event.enabled)",
            "E_TYPE",
        ),
        (
            "marker.yaml",
            "rule",
            "event.payment == event.payment",
            "E_TYPE",
        ),
        ("marker.yaml", "rule", "event.enabled + 1 > 0", "E_TYPE"),
    ] {
        let mut docs = extension_sources();
        modify(&mut docs, file, |d| d[field]["when"] = expression.into());
        assert_eq!(
            core_error(DecisionEngine::from_core(&docs, extension_schema()))
                .diagnostic
                .code,
            code
        );
    }
    let mut docs = extension_sources();
    modify(&mut docs, "child.yaml", |d| {
        d["pipeline"]["steps"][0]["step"] = serde_json::json!({"id":"check","name":"Recurse","type":"pipeline","pipeline":"parent","next":"end"})
    });
    assert_eq!(
        core_error(DecisionEngine::from_core(&docs, extension_schema()))
            .diagnostic
            .code,
        "E_CALL_CYCLE"
    );
    let mut docs = extension_sources();
    modify(&mut docs, "pipeline.yaml", |d| {
        d["pipeline"]["steps"][1]["step"]["pipeline"] = "unknown".into()
    });
    assert_eq!(
        core_error(DecisionEngine::from_core(&docs, extension_schema()))
            .diagnostic
            .code,
        "E_UNRESOLVED_REF"
    );
    let mut docs = extension_sources();
    modify(&mut docs, "pipeline.yaml", |d| {
        d["pipeline"]["decision"][0]["when"] = "results.risk.score > 0".into()
    });
    assert_eq!(
        core_error(DecisionEngine::from_core(&docs, extension_schema()))
            .diagnostic
            .code,
        "E_INVALID_REF"
    );
    let mut nested = extension_schema();
    for _ in 0..17 {
        nested = Schema::new("nested".into()).add_field(SchemaField::new(
            "child".into(),
            FieldType::object_with_schema(nested),
        ));
    }
    assert_eq!(
        core_error(DecisionEngine::from_core(&extension_sources(), nested))
            .diagnostic
            .code,
        "E_INPUT_SCHEMA"
    );
}

#[tokio::test]
async fn core_zero_score_match_and_guarded_router_do_not_fall_through() {
    let mut docs = extension_sources();
    modify(&mut docs, "marker.yaml", |d| d["rule"]["score"] = 0.into());
    modify(&mut docs, "pipeline.yaml", |d| {
        d["pipeline"]["entry"] = "route".into();
        d["pipeline"]["steps"].as_array_mut().unwrap().push(serde_json::json!({"step":{"id":"route","name":"Guarded router","type":"router","when":"event.enabled","routes":[{"when":"true","next":"single"}],"default":"end"}}));
        d["pipeline"]["decision"] = serde_json::json!([{"default":true,"result":"review"}]);
    });
    let engine = DecisionEngine::from_core(&docs, extension_schema()).unwrap();
    for trace in [false, true] {
        let response = engine
            .decide(extension_request(
                serde_json::json!({"enabled":false,"payment":{"amount":1001}}),
                trace,
            ))
            .await
            .unwrap();
        assert_eq!(response.result.score, 0);
        assert!(response.result.triggered_rules.is_empty());
        assert_eq!(
            response.result.context["__core_skipped_steps__"],
            Value::Array(vec![Value::String("route".into())])
        );
        let response = engine
            .decide(extension_request(
                serde_json::json!({"enabled":true,"payment":{"amount":1001}}),
                trace,
            ))
            .await
            .unwrap();
        assert!(response.result.triggered_rules.contains(&"marker".into()));
        assert_eq!(response.result.score, 60);
        if trace {
            assert!(response
                .trace
                .unwrap()
                .core_conditions_v1
                .unwrap()
                .iter()
                .any(|r| r.field_path.ends_with("/step/when")));
        }
    }
}

#[test]
fn core_call_expansion_and_depth_are_bounded_before_execution() {
    for branching in [false, true] {
        let mut docs = extension_sources();
        let levels = if branching { 13 } else { 17 };
        for level in 0..levels {
            for sibling in 0..if branching { 2 } else { 1 } {
                let id = format!("bounded_{level}_{sibling}");
                let steps = if level + 1 == levels {
                    serde_json::json!([{"step":{"id":"last","name":"End","type":"router","routes":[{"when":"true","next":"end"}],"default":"end"}}])
                } else {
                    let mut steps = vec![
                        serde_json::json!({"step":{"id":"first","name":"Call","type":"pipeline","pipeline":format!("bounded_{}_0",level+1),"next":if branching {"second"} else {"end"}}}),
                    ];
                    if branching {
                        steps.push(serde_json::json!({"step":{"id":"second","name":"Call","type":"pipeline","pipeline":format!("bounded_{}_1",level+1),"next":"end"}}));
                    }
                    serde_json::json!(steps)
                };
                docs.push(CoreSource { path:format!("{id}.yaml"), yaml:serde_json::json!({"version":"0.1","pipeline":{"id":id,"name":"Bounded","entry":if level+1==levels {"last"} else {"first"},"steps":steps,"decision":[{"default":true,"result":"pass"}]}}).to_string() });
            }
        }
        assert_eq!(
            core_error(DecisionEngine::from_core(&docs, extension_schema()))
                .diagnostic
                .code,
            "E_CALL_LIMIT"
        );
    }
}

#[tokio::test]
async fn core_call_trace_schema_rejects_fabricated_skipped_outputs() {
    let schema: Json = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/call-trace.json"
    ))
    .unwrap();
    let validator = jsonschema::JSONSchema::compile(&schema).unwrap();
    let engine = DecisionEngine::from_core(&extension_sources(), extension_schema()).unwrap();
    let response = engine
        .decide(extension_request(serde_json::json!({"enabled":true}), true))
        .await
        .unwrap();
    let mut trace = serde_json::to_value(response.trace.unwrap().core_calls_v1.unwrap()).unwrap();
    assert!(validator.is_valid(&trace));
    let skipped = trace
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|t| t["status"] == "skipped")
        .unwrap();
    skipped["score"] = 0.into();
    assert!(!validator.is_valid(&trace));
}

#[test]
fn optional_object_still_enforces_required_children_and_schema_size() {
    use corint_decision_compiler::core::{compile_core, validate_core_input};
    let schema = Schema::new("event".into()).add_field(SchemaField::new(
        "payment".into(),
        FieldType::object_with_schema(
            Schema::new("payment".into())
                .add_field(SchemaField::new("amount".into(), FieldType::Number).required()),
        ),
    ));
    assert!(validate_core_input(&schema, &HashMap::new()).is_ok());
    let error = validate_core_input(
        &schema,
        &HashMap::from([("payment".into(), Value::Object(HashMap::new()))]),
    )
    .unwrap_err();
    assert_eq!(error.diagnostic.code, "E_INPUT_SCHEMA");
    assert_eq!(
        error.diagnostic.field_path.as_deref(),
        Some("/event/payment/amount")
    );
    let mut schema = Schema::new("oversized".into());
    for i in 0..1025 {
        schema = schema.add_field(SchemaField::new(format!("f_{i}"), FieldType::Boolean));
    }
    assert_eq!(
        compile_core(&extension_sources(), schema)
            .unwrap_err()
            .diagnostic
            .code,
        "E_INPUT_SCHEMA"
    );
}

#[test]
fn multiple_guard_source_maps_produce_deterministic_programs() {
    use corint_decision_compiler::core::compile_core;
    let mut docs = extension_sources();
    modify(&mut docs, "pipeline.yaml", |d| {
        d["pipeline"]["steps"][0]["step"]["when"] = "true".into()
    });
    let expected = compile_core(&docs, extension_schema()).unwrap().programs;
    for _ in 0..12 {
        assert_eq!(
            compile_core(&docs, extension_schema()).unwrap().programs,
            expected
        );
    }
}
