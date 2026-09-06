use corint_decision_compiler::{Compiler, CompilerOptions};
use corint_decision_dsl_parser::{PipelineParser, RuleParser};
use corint_decision_engine::{DecisionEngineBuilder, DecisionRequest};
use corint_decision_model::{ast::Signal, Value};
use corint_decision_runtime::PipelineExecutor;
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn registry_guards_use_the_same_language_and_errors_as_rules() {
    for condition in [
        "!event.flag",
        "event.amount + 1 > 1",
        "event.amount > 0",
        "false || !event.flag",
    ] {
        let mut builder = DecisionEngineBuilder::new();
        for (id, result) in [("risk", "decline"), ("fallback", "approve")] {
            let yaml = serde_yaml::to_string(&json!({"pipeline":{"id":id,"name":id,"entry":"r","steps":[{"step":{"id":"r","name":"r","type":"router","routes":[{"when":"true","next":"end"}],"default":"end"}}],"decision":[{"default":true,"result":result}]}})).unwrap();
            builder = builder.add_rule_content(id, yaml);
        }
        let registry = serde_yaml::to_string(&json!({"registry":[{"pipeline":"risk","when":condition},{"pipeline":"fallback","when":"true"}]})).unwrap();
        let engine = builder
            .with_registry_content(registry)
            .build()
            .await
            .unwrap();
        let output = engine
            .decide(DecisionRequest::new(HashMap::from([
                ("amount".into(), Value::Number(1.0)),
                ("flag".into(), Value::Bool(false)),
            ])))
            .await
            .unwrap();
        assert_eq!(output.pipeline_id.as_deref(), Some("risk"), "{condition}");
        assert_eq!(output.result.signal, Some(Signal::Decline));
    }
    let engine = DecisionEngineBuilder::new()
        .with_registry_content("registry:\n  - pipeline: unused\n    when: '1 / 0 > 0'\n")
        .build()
        .await
        .unwrap();
    assert!(engine
        .decide(DecisionRequest::new(HashMap::new()))
        .await
        .unwrap_err()
        .to_string()
        .contains("zero"));
}

#[tokio::test]
async fn yaml_boolean_groups_short_circuit_in_both_compiler_modes() {
    for (when, matched) in [
        ("{all: ['false', '1 / 0 > 0']}", false),
        ("{any: ['true', '1 / 0 > 0']}", true),
        ("{not: ['true', '1 / 0 > 0']}", false),
        ("{all: [{any: ['true', '1 / 0 > 0']}, 'true']}", true),
    ] {
        let rule = RuleParser::parse(&format!(
            "rule:\n  id: r\n  name: R\n  when: {when}\n  score: 1\n"
        ))
        .unwrap();
        for optimized in [false, true] {
            let program = Compiler::with_options(CompilerOptions {
                enable_dead_code_elimination: optimized,
                ..Default::default()
            })
            .compile_rule(&rule)
            .unwrap();
            let result = PipelineExecutor::new_offline()
                .execute(&program, HashMap::new())
                .await
                .unwrap();
            assert_eq!(result.triggered_rules.len(), usize::from(matched), "{when}");
        }
    }
}

#[tokio::test]
async fn optimized_diamond_preserves_calls_and_context_on_every_route() {
    let mut steps = vec![
        json!({"step":{"id":"router","name":"Router","type":"router","routes":[{"when":"event.amount > 0","next":"yes"}],"default":"nope"}}),
    ];
    for (id, ruleset, next) in [
        ("yes", "accepted", "join"),
        ("nope", "rejected", "b2"),
        ("b2", "b2", "b3"),
        ("b3", "b3", "join"),
        ("join", "join", "end"),
    ] {
        steps.push(
            json!({"step":{"id":id,"name":id,"type":"ruleset","ruleset":ruleset,"next":next}}),
        );
    }
    let yaml = serde_yaml::to_string(
        &json!({"pipeline":{"id":"p","name":"P","entry":"router","steps":steps}}),
    )
    .unwrap();
    let pipeline = PipelineParser::parse(&yaml).unwrap();
    for (amount, calls) in [
        (-1.0, vec!["rejected", "b2", "b3", "join"]),
        (1.0, vec!["accepted", "join"]),
    ] {
        let mut contexts = Vec::new();
        for optimized in [false, true] {
            let program = Compiler::with_options(CompilerOptions {
                enable_dead_code_elimination: optimized,
                ..Default::default()
            })
            .compile_pipeline(&pipeline)
            .unwrap();
            let result = PipelineExecutor::new_offline()
                .execute(
                    &program,
                    HashMap::from([("amount".into(), Value::Number(amount))]),
                )
                .await
                .unwrap();
            assert_eq!(
                result.context.get("__rulesets_to_execute__"),
                Some(&Value::Array(
                    calls.iter().map(|s| Value::String((*s).into())).collect()
                ))
            );
            contexts.push(result.context);
        }
        assert_eq!(contexts[0], contexts[1]);
    }
}

#[test]
fn public_pipeline_parser_paths_have_identical_admission() {
    for field in [
        "when: {typo: ['false']}",
        "routes: false",
        "default: []",
        "next: []",
    ] {
        let yaml = format!("pipeline:\n  id: p\n  name: P\n  entry: s\n  steps:\n    - step:\n        id: s\n        name: S\n        type: router\n        {field}\n");
        assert!(PipelineParser::parse(&yaml).is_err());
        assert!(corint_decision_dsl_parser::pipeline_parser::PipelineParser::parse(&yaml).is_err());
    }
}

#[tokio::test]
async fn invalid_declared_registry_file_cannot_disappear() {
    let path = std::env::temp_dir().join(format!("corint-registry-{}.yaml", uuid::Uuid::new_v4()));
    std::fs::write(&path, "registry: [broken").unwrap();
    let result = DecisionEngineBuilder::new()
        .with_registry_file(&path)
        .build()
        .await;
    std::fs::remove_file(&path).unwrap();
    assert!(result.is_err());
}
