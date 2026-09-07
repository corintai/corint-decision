//! Expression semantics through the compiler and the real VM in each profile.
use corint_decision_compiler::{
    codegen::PipelineCompiler,
    core::{parse_core_input_schema, CoreSource},
};
use corint_decision_dsl_parser::PipelineParser;
use corint_decision_engine::{DecisionEngine, DecisionRequest};
use corint_decision_model::{
    ast::{Operator, Signal},
    ir::{Instruction, Program, ProgramMetadata},
    Value,
};
use corint_decision_runtime::PipelineExecutor;
use serde_json::{json, Value as Json};
use std::collections::HashMap;

fn pipeline(condition: Json) -> Json {
    json!({"pipeline": {
        "id": "expression_check", "name": "Expression check", "entry": "route",
        "steps": [{"step": {
            "id": "route", "name": "Route", "type": "router",
            "routes": [{"when": "true", "next": "end"}], "default": "end"
        }}],
        "decision": [
            {"when": condition, "result": "review"},
            {"default": true, "result": "approve"}
        ]
    }})
}

fn compile(document: &Json) -> Program {
    PipelineCompiler::compile(&PipelineParser::parse(&document.to_string()).unwrap()).unwrap()
}

#[tokio::test]
async fn compatibility_decisions_execute_all_compiled_expression_operators() {
    let event = HashMap::from([
        ("amount".into(), Value::Number(6.0)),
        ("denominator".into(), Value::Number(0.0)),
    ]);
    let mut failures = Vec::new();
    for (expression, expected) in [
        ("event.amount + 1 == 7", Signal::Review),
        ("event.amount % 4 == 2", Signal::Review),
        ("-event.amount == -6", Signal::Review),
        ("!false", Signal::Review),
        ("false || true", Signal::Review),
        ("true || 1 / 0 > 0", Signal::Review),
        (
            "exists(event.optional) && event.optional > 0",
            Signal::Approve,
        ),
        (
            "event.denominator != 0 && event.amount / event.denominator > 1",
            Signal::Approve,
        ),
    ] {
        let result = PipelineExecutor::new_offline()
            .execute(&compile(&pipeline(json!(expression))), event.clone())
            .await;
        if !result.as_ref().is_ok_and(|r| r.signal == Some(expected)) {
            failures.push(format!("{expression}: {result:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test]
async fn pipeline_yaml_groups_short_circuit_in_guards_routes_and_decisions() {
    // Deprecated source syntax stays rejected; this fix changes evaluation,
    // not the Pipeline parser's admission rules.
    assert!(PipelineParser::parse(
        &pipeline(json!({"conditions": ["false", "1 / 0 > 0"]})).to_string()
    )
    .is_err());
    for (condition, expected) in [
        (json!({"all": ["false", "1 / 0 > 0"]}), Some(false)),
        (json!({"any": ["true", "1 / 0 > 0"]}), Some(true)),
        (
            json!({"not": [{"all": ["false", "1 / 0 > 0"]}]}),
            Some(true),
        ),
        (
            json!({"event_type": "payment", "all": ["1 / 0 > 0"]}),
            Some(false),
        ),
        (json!({"all": ["true", "1 / 0 > 0"]}), None),
        (json!({"any": ["false", "1 / 0 > 0"]}), None),
    ] {
        for location in ["guard", "route", "decision"] {
            let mut document = pipeline(json!("true"));
            match location {
                "guard" => document["pipeline"]["when"] = condition.clone(),
                "route" => {
                    document["pipeline"]["steps"][0]["step"]["routes"][0]["when"] =
                        condition.clone();
                }
                _ => document["pipeline"]["decision"][0]["when"] = condition.clone(),
            }
            let event = HashMap::from([("type".into(), Value::String("login".into()))]);
            let result = PipelineExecutor::new_offline()
                .execute(&compile(&document), event)
                .await;
            if let Some(expected) = expected {
                let result = result.unwrap_or_else(|e| panic!("{location}: {condition}: {e}"));
                if location == "decision" {
                    assert_eq!(
                        result.signal,
                        Some(if expected {
                            Signal::Review
                        } else {
                            Signal::Approve
                        })
                    );
                }
            } else {
                assert!(
                    matches!(
                        result,
                        Err(corint_decision_runtime::RuntimeError::DivisionByZero)
                    ),
                    "{location}: {condition}: {result:?}"
                );
            }
        }
    }
}

#[tokio::test]
async fn compatibility_decision_keeps_service_and_variable_outputs() {
    // The main instructions model the compiler's service output stores.
    let mut program = compile(&pipeline(json!(
        "service.lookup.value + vars.enriched.value == 83 && !exists(event.optional)"
    )));
    program.instructions = vec![
        Instruction::LoadConst {
            value: Value::Object(HashMap::from([("value".into(), Value::Number(41.0))])),
        },
        Instruction::Store {
            name: "service.lookup".into(),
        },
        Instruction::LoadConst {
            value: Value::Object(HashMap::from([("value".into(), Value::Number(42.0))])),
        },
        Instruction::Store {
            name: "vars.enriched".into(),
        },
        Instruction::Return,
    ];
    let result = PipelineExecutor::new_offline()
        .execute(&program, HashMap::new())
        .await
        .unwrap();
    assert_eq!(result.signal, Some(Signal::Review));
}

fn source(name: &str, document: Json) -> CoreSource {
    CoreSource {
        path: name.into(),
        yaml: serde_yaml::to_string(&document).unwrap(),
    }
}

#[tokio::test]
async fn core_total_score_cannot_be_shadowed_by_an_event_field() {
    let input = parse_core_input_schema(&source(
        "input.json",
        json!({
            "name": "score-event",
            "fields": {
                "amount": {"name": "amount", "field_type": "number", "required": true},
                "total_score": {"name": "total_score", "field_type": "number", "required": true}
            }
        }),
    ))
    .unwrap();
    let documents = vec![
        source(
            "rule.json",
            json!({"version":"0.1", "rule":{
                "id":"score_rule", "name":"Score rule", "when":"event.amount > 0", "score":7
            }}),
        ),
        source(
            "ruleset.yaml",
            json!({"version":"0.1", "ruleset":{
                "id":"risk", "rules":["score_rule"], "conclusion":[
                    {"when":"total_score == 7 && event.total_score != 7", "signal":"review"},
                    {"default":true, "signal":"decline"}
                ]
            }}),
        ),
        source(
            "pipeline.json",
            json!({"version":"0.1", "pipeline":{
                "id":"score_pipeline", "name":"Score pipeline", "entry":"check",
                "steps":[{"step":{"id":"check", "name":"Check", "type":"ruleset", "ruleset":"risk", "next":"end"}}],
                "decision":[
                    {"when":"total_score == 7 && results.risk.total_score == 7 && results.risk.signal == 'review'", "result":"approve"},
                    {"default":true, "result":"decline"}
                ]
            }}),
        ),
        source(
            "registry.json",
            json!({"version":"0.1", "registry":[{"pipeline":"score_pipeline", "when":"true"}]}),
        ),
    ];
    let engine = DecisionEngine::from_core(&documents, input).unwrap();
    for (amount, input_score, score, signal) in [
        (1.0, 999.0, 7, "approve"),
        (1.0, -999.0, 7, "approve"),
        (0.0, 7.0, 0, "decline"),
    ] {
        let mut previous = None;
        for trace in [false, true] {
            let mut request = DecisionRequest::new(HashMap::from([
                ("amount".into(), Value::Number(amount)),
                ("total_score".into(), Value::Number(input_score)),
            ]));
            request.options.enable_trace = trace;
            let response = engine.decide(request).await.unwrap();
            assert_eq!(response.result.score, score);
            assert_eq!(
                serde_json::to_value(&response.result.signal).unwrap()["type"],
                signal
            );
            if let Some(previous) = &previous {
                assert_eq!(&response.result, previous);
            }
            previous = Some(response.result);
            assert_eq!(response.trace.is_some(), trace);
        }
    }
}

fn counting_loop(iterations: usize) -> Vec<Instruction> {
    // Each iteration consumes eight instructions and leaves the stack empty.
    vec![
        Instruction::LoadConst {
            value: Value::Number(iterations as f64),
        },
        Instruction::Store {
            name: "counter".into(),
        },
        Instruction::Load {
            name: "counter".into(),
        },
        Instruction::LoadConst {
            value: Value::Number(1.0),
        },
        Instruction::BinaryOp { op: Operator::Sub },
        Instruction::Dup,
        Instruction::Store {
            name: "counter".into(),
        },
        Instruction::LoadConst {
            value: Value::Number(0.0),
        },
        Instruction::Compare { op: Operator::Gt },
        Instruction::JumpIfTrue { offset: -7 },
        Instruction::Return,
    ]
}

#[tokio::test]
async fn main_and_decision_share_one_instruction_budget() {
    for core in [true, false] {
        let mut executor = PipelineExecutor::new_offline();
        if core {
            executor = executor.with_ruleset_programs(HashMap::new());
        }
        // Both individual blocks fit, but their combined execution exceeds one million.
        let program = Program::new_with_decision(
            counting_loop(62_500),
            ProgramMetadata::for_pipeline("budget".into()),
            counting_loop(62_500),
        );
        let error = executor
            .execute(&program, HashMap::new())
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("E_RESOURCE_LIMIT"),
            "core={core}: {error}"
        );
        let within_limit = Program::new_with_decision(
            counting_loop(62_499),
            program.metadata,
            counting_loop(62_499),
        );
        executor
            .execute(&within_limit, HashMap::new())
            .await
            .unwrap();
    }
}
