//! Compatibility List examples run through the parsers, compilers and real VM.
use corint_decision_compiler::{
    codegen::{PipelineCompiler, RuleCompiler, RulesetCompiler},
    core::{compile_core, parse_core_input_schema, CoreSource},
};
use corint_decision_dsl_parser::{PipelineParser, RuleParser, RulesetParser};
use corint_decision_model::{ir::Program, Value};
use corint_decision_runtime::{
    lists::{backend::ListBackend, FileBackend, ListService, MemoryBackend},
    PipelineExecutor, RuntimeError,
};
use serde_json::{json, Value as Json};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

const RULES: [&str; 3] = [
    "blocked_email.yaml",
    "vip_score_reduction.yaml",
    "untrusted_large_transaction.yaml",
];

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/conformance/cdl_lists")
            .join(name),
    )
    .unwrap()
}

fn rule(yaml: &str) -> Program {
    RuleCompiler::compile(&RuleParser::parse(yaml).unwrap()).unwrap()
}

async fn memory_service() -> Arc<ListService> {
    let data: HashMap<String, Vec<Value>> = serde_json::from_str(&fixture("lists.json")).unwrap();
    let mut backends = HashMap::new();
    for (id, values) in data {
        let mut backend = MemoryBackend::new();
        for value in values {
            backend.add(&id, value).await.unwrap();
        }
        backends.insert(id, Box::new(backend) as Box<dyn ListBackend>);
    }
    Arc::new(ListService::new_with_backends(backends))
}

#[tokio::test]
async fn documented_list_policy_executes_with_bound_lists() {
    let ruleset = RulesetParser::parse(&fixture("ruleset.yaml")).unwrap();
    let rules: Vec<_> = RULES.iter().map(|name| rule(&fixture(name))).collect();
    assert_eq!(
        ruleset.rules,
        rules
            .iter()
            .map(|r| r.metadata.source_id.clone())
            .collect::<Vec<_>>()
    );
    let program =
        PipelineCompiler::compile(&PipelineParser::parse(&fixture("pipeline.yaml")).unwrap())
            .unwrap();
    let executor = PipelineExecutor::new_offline()
        .with_ruleset_programs(HashMap::from([(
            ruleset.id.clone(),
            (rules, RulesetCompiler::compile(&ruleset).unwrap()),
        )]))
        .with_list_service(memory_service().await);
    let cases: Vec<Json> = serde_json::from_str(&fixture("cases.json")).unwrap();
    for case in cases {
        let result = executor
            .execute(
                &program,
                serde_json::from_value(case["event"].clone()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(json!(result.score), case["score"], "{}", case["id"]);
        assert_eq!(
            serde_json::to_value(result.signal).unwrap()["type"],
            case["signal"],
            "{}",
            case["id"]
        );
        assert_eq!(
            json!(result.triggered_rules),
            case["triggered_rules"],
            "{}",
            case["id"]
        );
    }
}

#[test]
fn strict_core_rejects_list_capability_in_otherwise_complete_resources() {
    let mut sources: Vec<_> = RULES
        .into_iter()
        .chain(["ruleset.yaml", "pipeline.yaml", "registry.yaml"])
        .map(|name| CoreSource {
            path: name.into(),
            yaml: fixture(name),
        })
        .collect();
    let input = parse_core_input_schema(&CoreSource {
        path: "input-schema.yaml".into(),
        yaml: fixture("input-schema.yaml"),
    })
    .unwrap();
    // Admit the same complete graph with only the List conditions replaced.
    for source in &mut sources[..RULES.len()] {
        let mut document: Json = serde_yaml::from_str(&source.yaml).unwrap();
        document["rule"]["when"] = json!("true");
        source.yaml = document.to_string();
    }
    compile_core(&sources, input.clone()).unwrap();
    for (index, name) in RULES.into_iter().enumerate() {
        let control = sources[index].yaml.clone();
        sources[index].yaml = fixture(name);
        let error = compile_core(&sources, input.clone()).unwrap_err();
        assert_eq!(
            error.diagnostic.code, "E_UNSUPPORTED_CAPABILITY",
            "{name}: {error}"
        );
        assert_eq!(error.diagnostic.stage.as_deref(), Some("type"));
        assert_eq!(error.diagnostic.source.as_deref(), Some(name));
        sources[index].yaml = control;
    }
}

struct UnavailableBackend;

#[async_trait::async_trait]
impl ListBackend for UnavailableBackend {
    async fn contains(&self, _: &str, _: &Value) -> Result<bool, RuntimeError> {
        Err(RuntimeError::InvalidOperation(
            "test backend unavailable".into(),
        ))
    }
    async fn add(&mut self, _: &str, _: Value) -> Result<(), RuntimeError> {
        unreachable!()
    }
    async fn remove(&mut self, _: &str, _: &Value) -> Result<(), RuntimeError> {
        unreachable!()
    }
    async fn get_all(&self, _: &str) -> Result<Vec<Value>, RuntimeError> {
        unreachable!()
    }
}

fn service_with(backend: Box<dyn ListBackend>) -> Arc<ListService> {
    Arc::new(ListService::new_with_backends(HashMap::from([(
        "email_blocklist".into(),
        backend,
    )])))
}

#[tokio::test]
async fn membership_distinguishes_empty_lists_from_unavailable_lists_and_backends() {
    let event = HashMap::from([("email".into(), Value::String("fraud@example.com".into()))]);
    for operator in ["in", "not in"] {
        let mut document: Json = serde_yaml::from_str(&fixture("blocked_email.yaml")).unwrap();
        document["rule"]["when"] = json!(format!("event.email {operator} list.email_blocklist"));
        let program = rule(&document.to_string());
        for (service, expected) in [
            (None, "E_LIST_UNAVAILABLE"),
            (
                Some(Arc::new(ListService::new_with_memory())),
                "E_LIST_UNAVAILABLE",
            ),
            (
                Some(service_with(Box::new(UnavailableBackend))),
                "test backend unavailable",
            ),
        ] {
            let mut executor = PipelineExecutor::new_offline();
            if let Some(service) = service {
                executor = executor.with_list_service(service);
            }
            let error = executor
                .execute(&program, event.clone())
                .await
                .unwrap_err()
                .to_string();
            assert!(error.contains(expected), "{operator}: {error}");
        }
        for (service, member) in [
            (service_with(Box::new(MemoryBackend::new())), false),
            (memory_service().await, true),
        ] {
            let result = PipelineExecutor::new_offline()
                .with_list_service(service)
                .execute(&program, event.clone())
                .await
                .unwrap();
            let matched = if operator == "not in" {
                !member
            } else {
                member
            };
            assert_eq!(result.score, if matched { 500 } else { 0 });
            assert_eq!(!result.triggered_rules.is_empty(), matched);
        }
    }
}

#[tokio::test]
async fn memory_and_file_matching_boundaries_are_explicit() {
    let mut memory = MemoryBackend::new();
    let entries = [
        "Alice",
        "42",
        "true",
        "null",
        "[1.0]",
        "{\"key\":\"value\"}",
    ];
    for entry in entries {
        memory
            .add("values", Value::String(entry.into()))
            .await
            .unwrap();
    }
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("values.txt");
    std::fs::write(&path, format!("# ignored\n\n {} \n", entries.join("\n"))).unwrap();
    let file = FileBackend::new(path);
    file.load().await.unwrap();
    for (value, expected) in [
        (Value::String("Alice".into()), true),
        (Value::String("alice".into()), false),
        (Value::String(" Alice ".into()), false),
        (Value::String("42".into()), true),
        (Value::Number(42.0), true),
        (Value::Bool(true), true),
    ] {
        assert_eq!(memory.contains("values", &value).await.unwrap(), expected);
        assert_eq!(file.contains("values", &value).await.unwrap(), expected);
    }
    assert!(memory.contains("values", &Value::Null).await.unwrap());
    assert!(!file.contains("values", &Value::Null).await.unwrap());
    for value in [
        Value::Array(vec![Value::Number(1.0)]),
        Value::Object(HashMap::from([(
            "key".into(),
            Value::String("value".into()),
        )])),
    ] {
        assert!(memory.contains("values", &value).await.unwrap());
        assert!(file.contains("values", &value).await.is_err());
    }
}
