//! Seeded generated cases: the seed/case/expression is printed on failure.
//! Expected values are constructed independently; execution uses production APIs.
use corint_decision_compiler::{core::parse_core_input_schema, Compiler, CompilerOptions};
use corint_decision_dsl_parser::{ExpressionParser, PipelineParser, RuleParser};
use corint_decision_engine::{CoreSource, DecisionEngine, DecisionRequest, Value};
use corint_decision_runtime::PipelineExecutor;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde_json::json;
use std::{collections::HashMap, path::PathBuf};

const SEED: u64 = 0xCD1_2026_0906;

fn condition(rng: &mut StdRng, depth: usize, amount: i32) -> (String, bool) {
    if depth == 0 {
        let delta: i32 = rng.gen_range(-20..20);
        let threshold: i32 = rng.gen_range(-50..50);
        return (
            format!("event.amount + ({delta}) > ({threshold})"),
            amount + delta > threshold,
        );
    }
    let (a, av) = condition(rng, depth - 1, amount);
    let (b, bv) = condition(rng, depth - 1, amount);
    match rng.gen_range(0..4) {
        0 => (format!("({a}) && ({b})"), av && bv),
        1 => (format!("({a}) || ({b})"), av || bv),
        2 => (format!("!({a})"), !av),
        _ => (format!("({a}) || (false && (1 / 0 > 0))"), av),
    }
}

#[tokio::test]
async fn generated_expressions_preserve_oracle_optimization_and_trace() {
    let mut rng = StdRng::seed_from_u64(SEED);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core");
    let input = CoreSource {
        path: "input-schema.yaml".into(),
        yaml: std::fs::read_to_string(root.join("input-schema.yaml")).unwrap(),
    };
    let base: Vec<_> = [
        "rule.yaml",
        "ruleset.yaml",
        "pipeline.yaml",
        "registry.yaml",
    ]
    .iter()
    .map(|name| CoreSource {
        path: (*name).into(),
        yaml: std::fs::read_to_string(root.join(name)).unwrap(),
    })
    .collect();
    for case in 0..128 {
        let amount = rng.gen_range(-100..100);
        let (expression, expected) = condition(&mut rng, 3, amount);
        let event = HashMap::from([("amount".into(), Value::Number(amount as f64))]);
        let mut sources = base.clone();
        let mut doc: serde_json::Value = serde_yaml::from_str(&sources[0].yaml).unwrap();
        doc["rule"]["when"] = json!(expression);
        sources[0].yaml = serde_yaml::to_string(&doc).unwrap();
        let rule = RuleParser::parse(&sources[0].yaml).unwrap();
        for optimized in [false, true] {
            let program = Compiler::with_options(CompilerOptions {
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
                "seed={SEED} case={case} optimized={optimized} {expression}"
            );
        }
        let engine =
            DecisionEngine::from_core(&sources, parse_core_input_schema(&input).unwrap()).unwrap();
        let plain = engine
            .decide(DecisionRequest::new(event.clone()))
            .await
            .unwrap();
        let traced = engine
            .decide(DecisionRequest::new(event).with_trace())
            .await
            .unwrap();
        assert_eq!(
            plain.result.score,
            if expected { 60 } else { 0 },
            "case={case} {expression}"
        );
        assert_eq!(
            serde_json::to_value(&plain.result).unwrap(),
            serde_json::to_value(&traced.result).unwrap(),
            "case={case} {expression}"
        );
        assert!(traced.trace.unwrap().core_conditions_v1.is_some());
    }
}

#[tokio::test]
async fn generated_dags_preserve_all_branches_under_optimization() {
    let mut rng = StdRng::seed_from_u64(SEED);
    for case in 0..64 {
        let left = rng.gen_range(1..12);
        let right = rng.gen_range(1..12);
        let mut steps = vec![
            json!({"step":{"id":"route","name":"route","type":"router","routes":[{"when":"event.amount > 0","next":"l0"}],"default":"r0"}}),
        ];
        for (prefix, count) in [("l", left), ("r", right)] {
            for n in 0..count {
                let id = format!("{prefix}{n}");
                let next = if n + 1 == count {
                    "end".into()
                } else {
                    format!("{prefix}{}", n + 1)
                };
                steps.push(
                    json!({"step":{"id":id,"name":id,"type":"ruleset","ruleset":id,"next":next}}),
                );
            }
        }
        // Explicit transitions must survive arbitrary source order.
        use rand::seq::SliceRandom;
        steps.shuffle(&mut rng);
        let pipeline = PipelineParser::parse(
            &serde_yaml::to_string(
                &json!({"pipeline":{"id":"p","name":"p","entry":"route","steps":steps}}),
            )
            .unwrap(),
        )
        .unwrap();
        for (amount, prefix, count) in [(1.0, "l", left), (-1.0, "r", right)] {
            let expected = Value::Array(
                (0..count)
                    .map(|n| Value::String(format!("{prefix}{n}")))
                    .collect(),
            );
            let mut results = Vec::new();
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
                    Some(&expected),
                    "case={case}"
                );
                results.push(result.context);
            }
            assert_eq!(results[0], results[1], "case={case}");
        }
    }
}

#[test]
fn seeded_parser_mutations_and_limits_never_panic() {
    let mut rng = StdRng::seed_from_u64(SEED);
    let alphabet: Vec<char> = "abc.0123+-*/%&|!<>=()[]{}'\"\\\n中😀".chars().collect();
    for case in 0..4096 {
        let len = rng.gen_range(0..256);
        let input: String = (0..len)
            .map(|_| alphabet[rng.gen_range(0..alphabet.len())])
            .collect();
        assert!(
            std::panic::catch_unwind(|| ExpressionParser::parse(&input)).is_ok(),
            "seed={SEED} case={case} {input:?}"
        );
    }
    for input in [
        vec!["true"; 5000].join(" && "),
        format!("{}true{}", "(".repeat(1024), ")".repeat(1024)),
        format!("{}true", "!".repeat(1024)),
    ] {
        assert!(ExpressionParser::parse(&input).is_err());
    }
}
