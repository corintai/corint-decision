//! Run the complete authored CDL example with local, disposable host bindings.
#![cfg(all(feature = "sqlx", feature = "redis"))]
use corint_decision_compiler::{
    codegen::{PipelineCompiler, RuleCompiler, RulesetCompiler},
    core::{parse_core_input_schema, validate_core_document, CoreSource},
};
use corint_decision_dsl_parser::{PipelineParser, RuleParser, RulesetParser};
use corint_decision_engine::{DecisionEngine, DecisionRequest, DecisionResponse, EngineError};
use corint_decision_model::{ir::Program, Value};
use corint_decision_runtime::{
    feature::{definition::FeatureCollection, FeatureExecutor},
    ContextInput, DataSourceClient, ExecutionContext, ExecutionResult, HttpServiceClient,
    HttpServiceConfig, ListBackend, ListService, MemoryBackend, PipelineExecutor,
};
use corint_decision_toolchain::resolve;
use serde_json::{json, Value as Json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../CDL/examples/payment-review")
}
fn source(name: &str) -> CoreSource {
    CoreSource {
        path: name.into(),
        yaml: std::fs::read_to_string(root().join(name)).unwrap(),
    }
}
fn request() -> Json {
    serde_json::from_str(&std::fs::read_to_string(root().join("request.json")).unwrap()).unwrap()
}
fn core() -> DecisionEngine {
    let resolved =
        resolve::resolve(&root(), "input-schema.yaml", &["registry.yaml".into()]).unwrap();
    assert_eq!(resolved.receipt().manifest.sources.len(), 10);
    assert!(resolved
        .receipt()
        .manifest
        .sources
        .iter()
        .all(|s| !s.path.starts_with("online/")));
    DecisionEngine::from_core(
        &resolved.bundle().sources,
        parse_core_input_schema(&resolved.bundle().input_schema).unwrap(),
    )
    .unwrap()
}
struct Task(tokio::task::JoinHandle<()>);
impl Drop for Task {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn feature_store() -> (DataSourceClient, Task, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(socket);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.unwrap() == 0 {
                break;
            }
            let count: usize = line.trim().strip_prefix('*').unwrap().parse().unwrap();
            let mut args = Vec::new();
            for _ in 0..count {
                line.clear();
                reader.read_line(&mut line).await.unwrap();
                let length: usize = line.trim().strip_prefix('$').unwrap().parse().unwrap();
                let mut bytes = vec![0; length + 2];
                reader.read_exact(&mut bytes).await.unwrap();
                assert_eq!(&bytes[length..], b"\r\n");
                args.push(String::from_utf8(bytes[..length].to_vec()).unwrap());
            }
            let reply = match args[0].as_str() {
                "CLIENT" | "SELECT" => "+OK\r\n",
                "PING" => "+PONG\r\n",
                "GET" => {
                    captured.lock().unwrap().push(args[1].clone());
                    match args[1].as_str() {
                        "example:customer_trust_score:customer/001"
                        | "example:customer_trust_score:trusted/001" => "$2\r\n90\r\n",
                        "example:customer_trust_score:no-history" => "$-1\r\n",
                        other => panic!("Unexpected lookup key: {other}"),
                    }
                }
                other => panic!("Unexpected Redis command: {other}"),
            };
            reader.get_mut().write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let client = DataSourceClient::new(
        serde_json::from_value(json!({
            "name":"customer_profiles", "type":"feature_store", "provider":"redis",
            "connection_string":format!("redis://{address}"), "namespace":"example", "default_ttl":0
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    (client, Task(task), seen)
}

async fn provider(
    score: i32,
    unavailable: bool,
) -> (HttpServiceClient, Task, Arc<Mutex<Vec<(String, Json)>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let header_end = loop {
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(offset) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                    break offset + 4;
                }
                assert!(bytes.len() < 65536);
            };
            let header = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
            let length: usize = header
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < header_end + length {
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            let first = header.lines().next().unwrap().to_owned();
            let body: Json =
                serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
            captured.lock().unwrap().push((first.clone(), body));
            let is_risk = first.contains("/risk?");
            let status = if is_risk && unavailable {
                "503 Service Unavailable"
            } else {
                "200 OK"
            };
            let response = if is_risk {
                json!({"risk":{"score":score},"available":true,"reference":"assessment/001"})
            } else {
                assert!(first.starts_with("POST /v1/assessments/assessment%2F001/explanation "));
                json!({"explanation":"Synthetic provider explanation"})
            };
            let body = response.to_string();
            let message=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
            socket.write_all(message.as_bytes()).await.unwrap();
        }
    });
    let mut config: HttpServiceConfig =
        serde_yaml::from_str(&source("online/customer-risk.yaml").yaml).unwrap();
    // The host binds the illustrative provider URL to this private loopback server.
    config.base_url = format!("http://{address}/v1");
    let mut client = HttpServiceClient::new();
    client.register_service(config).unwrap();
    (client, Task(task), seen)
}

struct Harness {
    _directory: tempfile::TempDir,
    _tasks: Vec<Task>,
    features: FeatureExecutor,
    online: PipelineExecutor,
    program: Program,
    core: DecisionEngine,
    requests: Arc<Mutex<Vec<(String, Json)>>>,
    lookups: Arc<Mutex<Vec<String>>>,
}
impl Harness {
    async fn new(score: i32, unavailable: bool) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let db = directory.path().join("history.sqlite");
        let pool = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query("CREATE TABLE payments(customer_id TEXT, amount_cents REAL, currency TEXT, status TEXT, occurred_at TEXT)").execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE customers(customer_id TEXT, created_at TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        for id in ["customer/001", "trusted/001"] {
            sqlx::query("INSERT INTO customers VALUES (?, datetime('now','-400 days','-1 hour'))")
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            for amount in [60000, 80000, 100000] {
                sqlx::query("INSERT INTO payments VALUES (?,?,'USD','settled',datetime('now','-10 minutes'))").bind(id).bind(amount).execute(&pool).await.unwrap();
            }
            for (currency, status, time) in [
                ("USD", "pending", "-5 minutes"),
                ("EUR", "settled", "-5 minutes"),
                ("USD", "settled", "-2 hours"),
                ("USD", "settled", "+1 hour"),
            ] {
                sqlx::query("INSERT INTO payments VALUES (?,9000000,?,?,datetime('now',?))")
                    .bind(id)
                    .bind(currency)
                    .bind(status)
                    .bind(time)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        pool.close().await;
        let history=DataSourceClient::new(serde_json::from_value(json!({
            "name":"payment_history", "type":"sql", "provider":"sqlite", "connection_string":db, "database":"history"
        })).unwrap()).await.unwrap();
        let (store, store_task, lookups) = feature_store().await;
        let mut features = FeatureExecutor::new();
        features.add_datasource("payment_history", history).unwrap();
        features.add_datasource("customer_profiles", store).unwrap();
        let collection: FeatureCollection =
            serde_yaml::from_str(&source("online/features.yaml").yaml).unwrap();
        features.register_features(collection.features).unwrap();
        let mut lists: HashMap<String, Box<dyn ListBackend>> = HashMap::new();
        for (id, value) in [
            ("blocked_emails", "blocked@example.test"),
            ("trusted_customers", "trusted/001"),
        ] {
            let mut backend = MemoryBackend::new();
            backend.add(id, Value::String(value.into())).await.unwrap();
            lists.insert(id.into(), Box::new(backend));
        }
        let (client, http_task, requests) = provider(score, unavailable).await;
        let rules: Vec<_> = ["online/blocked-email.yaml", "online/loyal-customer.yaml"]
            .iter()
            .map(|name| {
                RuleCompiler::compile(&RuleParser::parse(&source(name).yaml).unwrap()).unwrap()
            })
            .collect();
        let ruleset = RulesetParser::parse(&source("online/markers.yaml").yaml).unwrap();
        let program = PipelineCompiler::compile(
            &PipelineParser::parse(&source("online/prepare.yaml").yaml).unwrap(),
        )
        .unwrap();
        let online = PipelineExecutor::new_offline()
            .with_http_service_client(Arc::new(client))
            .with_list_service(Arc::new(ListService::new_with_backends(lists)))
            .with_ruleset_programs(HashMap::from([(
                ruleset.id.clone(),
                (rules, RulesetCompiler::compile(&ruleset).unwrap()),
            )]));
        Self {
            _directory: directory,
            _tasks: vec![store_task, http_task],
            features,
            online,
            program,
            core: core(),
            requests,
            lookups,
        }
    }
    async fn prepare(&self, event: Json, provider_required: bool) -> Json {
        let values: HashMap<String, Value> = serde_json::from_value(event.clone()).unwrap();
        let features = self
            .features
            .execute_all(&ExecutionContext::from_event(values.clone()).unwrap())
            .await
            .unwrap();
        let input = ContextInput::new(values)
            .with_features(features.clone())
            .with_vars(HashMap::from([(
                "flow".into(),
                Value::String("payment-review".into()),
            )]))
            .with_llm(HashMap::from([(
                "review_note".into(),
                Value::String("Caller-supplied note; no model is invoked".into()),
            )]));
        let result = self
            .online
            .execute_with_result(&self.program, input, ExecutionResult::new())
            .await
            .unwrap();
        assert_eq!(
            result.score, 0,
            "Online marker rules must contribute no decision score"
        );
        let context = serde_json::to_value(&result.context).unwrap();
        let provider = &context["service"]["lookup"];
        assert!(provider["available"].is_boolean());
        assert!(provider["score"].is_number());
        if provider["available"] == true {
            assert_eq!(
                context["review_note"]["text"],
                "Synthetic provider explanation"
            );
        }
        let triggered = &result.triggered_rules;
        let mut prepared = event;
        prepared["evidence"] = json!({
            "blocked_email":triggered.iter().any(|id|id=="email_on_blocklist"),
            "loyal_customer":triggered.iter().any(|id|id=="loyal_customer_marker"),
            "payment_count_1h":features["payment_count_1h"],
            "provider_required":provider_required,
            "provider_available":provider["available"],
            "provider_score":provider["score"],
        });
        // Core optional means absent, not null. This host mapping is explicit.
        if let Value::Number(value) = features["average_payment_1h"] {
            prepared["evidence"]["average_amount_cents"] = json!(value);
        } else {
            assert_eq!(features["average_payment_1h"], Value::Null);
        }
        if prepared["customer"]["id"] != "no-history" {
            assert_eq!(features["payment_count_1h"], Value::Number(3.0));
            assert_eq!(features["payment_amount_1h"], Value::Number(240000.0));
            assert_eq!(features["average_payment_1h"], Value::Number(80000.0));
            assert_eq!(features["account_age_days"], Value::Number(400.0));
            assert_eq!(features["customer_trust_score"], Value::Number(90.0));
        }
        prepared
    }
    fn check_transport(&self, unavailable: bool) {
        let requests = self.requests.lock().unwrap();
        assert!(!requests.is_empty());
        let (path, body) = &requests[0];
        assert!(path
            .starts_with("POST /v1/customers/customer%2F001/risk?currency=USD&channel=example "));
        assert!(body["request_id"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(body["flow"], "payment-review");
        assert_eq!(body["note"], "Caller-supplied note; no model is invoked");
        assert!(body.get("score_limit").is_some());
        assert_eq!(body["tags"], json!(["payment-review", "synthetic"]));
        if unavailable {
            assert!(requests.iter().all(|(path, _)| path.contains("/risk?")));
        }
        assert!(self
            .lookups
            .lock()
            .unwrap()
            .iter()
            .any(|key| key == "example:customer_trust_score:customer/001"));
    }
}

async fn assert_decision(
    engine: &DecisionEngine,
    event: Json,
    score: i32,
    signal: &str,
    actions: &[&str],
) -> DecisionResponse {
    let plain = engine
        .decide(DecisionRequest::new(
            serde_json::from_value(event.clone()).unwrap(),
        ))
        .await
        .unwrap();
    let traced = engine
        .decide(DecisionRequest::new(serde_json::from_value(event).unwrap()).with_trace())
        .await
        .unwrap();
    assert_eq!(plain.result.score, score);
    assert_eq!(
        serde_json::to_value(&plain.result.signal).unwrap()["type"],
        signal
    );
    assert_eq!(
        plain.result.actions,
        actions.iter().map(|s| s.to_string()).collect::<Vec<_>>()
    );
    assert_eq!(traced.result.score, plain.result.score);
    assert_eq!(traced.result.signal, plain.result.signal);
    assert_eq!(traced.result.actions, plain.result.actions);
    assert_eq!(traced.result.triggered_rules, plain.result.triggered_rules);
    traced
}

#[tokio::test]
async fn complete_project_executes_from_online_facts_through_imported_decisions() {
    let harness = Harness::new(20, false).await;
    let prepared = harness.prepare(request(), true).await;
    let snapshot: Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("prepared-input.json")).unwrap())
            .unwrap();
    assert_eq!(
        serde_json::from_value::<Value>(prepared.clone()).unwrap(),
        snapshot
    );
    assert_eq!(prepared["evidence"]["loyal_customer"], false);
    assert_decision(
        &harness.core,
        prepared.clone(),
        50,
        "review",
        &["OPEN_REVIEW"],
    )
    .await;
    for amount in [99999, 100000] {
        let mut event = request();
        event["amount_cents"] = json!(amount);
        assert_decision(
            &harness.core,
            harness.prepare(event, true).await,
            0,
            "approve",
            &[],
        )
        .await;
    }
    let mut event = request();
    event["customer"]["id"] = json!("trusted/001");
    let trusted = harness.prepare(event, true).await;
    assert_eq!(trusted["evidence"]["loyal_customer"], true);
    assert_decision(&harness.core, trusted, 30, "approve", &[]).await;
    let mut event = request();
    event["customer"]["email"] = json!("blocked@example.test");
    let blocked = harness.prepare(event, true).await;
    let response =
        assert_decision(&harness.core, blocked, 100, "decline", &["BLOCK_PAYMENT"]).await;
    let trace = serde_json::to_value(response.trace.unwrap()).unwrap();
    assert!(trace.to_string().contains("skipped"));
    let mut event = request();
    event["device"]["id"] = json!("emulator-42");
    assert_decision(
        &harness.core,
        harness.prepare(event, true).await,
        75,
        "decline",
        &["BLOCK_PAYMENT"],
    )
    .await;
    let mut no_device = request();
    no_device.as_object_mut().unwrap().remove("device");
    assert_decision(
        &harness.core,
        harness.prepare(no_device, true).await,
        50,
        "review",
        &["OPEN_REVIEW"],
    )
    .await;
    let mut unhandled = prepared;
    unhandled["type"] = json!("refund");
    let response = assert_decision(&harness.core, unhandled, 0, "pass", &[]).await;
    assert_eq!(response.pipeline_id.as_deref(), Some("unhandled_event"));
    harness.check_transport(false);
}

#[tokio::test]
async fn child_scores_actions_and_two_kinds_of_skips_are_explicit() {
    let harness = Harness::new(80, false).await;
    let prepared = harness.prepare(request(), true).await;
    let response =
        assert_decision(&harness.core, prepared, 90, "decline", &["BLOCK_PAYMENT"]).await;
    assert!(response
        .result
        .triggered_rules
        .contains(&"provider_risk".to_string()));
    let mut small = request();
    small["amount_cents"] = json!(50000);
    assert_decision(
        &harness.core,
        harness.prepare(small.clone(), true).await,
        40,
        "review",
        &["OPEN_REVIEW"],
    )
    .await;
    assert_decision(
        &harness.core,
        harness.prepare(small, false).await,
        0,
        "approve",
        &[],
    )
    .await;
    harness.check_transport(false);
    let unavailable = Harness::new(80, true).await;
    let prepared = unavailable.prepare(request(), true).await;
    assert_eq!(prepared["evidence"]["provider_available"], false);
    assert_decision(
        &unavailable.core,
        prepared,
        50,
        "hold",
        &["REQUEST_IDENTITY_CHECK"],
    )
    .await;
    unavailable.check_transport(true);
}

#[tokio::test]
async fn missing_history_null_and_invalid_core_inputs_remain_distinct() {
    let harness = Harness::new(20, false).await;
    let mut empty = request();
    empty["customer"]["id"] = json!("no-history");
    empty.as_object_mut().unwrap().remove("device");
    let prepared = harness.prepare(empty, true).await;
    assert_eq!(prepared["evidence"]["payment_count_1h"].as_f64(), Some(0.0));
    assert!(prepared["evidence"].get("average_amount_cents").is_none());
    assert_decision(&harness.core, prepared.clone(), 0, "approve", &[]).await;
    for (pointer, value) in [
        ("/amount_cents", json!("150000")),
        ("/evidence/average_amount_cents", Json::Null),
    ] {
        let mut invalid = prepared.clone();
        if pointer.ends_with("average_amount_cents") {
            invalid["evidence"]["average_amount_cents"] = value;
        } else {
            *invalid.pointer_mut(pointer).unwrap() = value;
        }
        let error = harness
            .core
            .decide(DecisionRequest::new(
                serde_json::from_value(invalid).unwrap(),
            ))
            .await
            .unwrap_err();
        let EngineError::Core(error) = error else {
            panic!("Expected Core diagnostic")
        };
        assert_eq!(error.diagnostic.code, "E_INPUT_SCHEMA");
    }
    let mut zero = prepared;
    zero["amount_cents"] = json!(0);
    let EngineError::Core(error) = harness
        .core
        .decide(DecisionRequest::new(serde_json::from_value(zero).unwrap()))
        .await
        .unwrap_err()
    else {
        panic!("Expected Core diagnostic")
    };
    assert_eq!(error.diagnostic.code, "E_PIPELINE_SKIPPED");
}

#[test]
fn source_forms_metadata_and_extension_boundaries_follow_the_definitions() {
    let resolved =
        resolve::resolve(&root(), "input-schema.yaml", &["registry.yaml".into()]).unwrap();
    assert_eq!(resolved.originals().len(), 11);
    for name in ["registry.yaml", "rulesets/payment-risk.yaml"] {
        let document: Json = serde_yaml::from_str(&source(name).yaml).unwrap();
        assert_eq!(document["version"], "0.1");
        assert!(document["import"].is_object());
    }
    for normalized in &resolved.bundle().sources {
        validate_core_document(normalized).unwrap();
    }
    let mut rule: Json = serde_yaml::from_str(&source("rules/blocked-email.yaml").yaml).unwrap();
    rule["rule"]["metadata"] = json!({"author":"example-author"});
    let compatibility = rule.to_string();
    RuleParser::parse(&compatibility).unwrap();
    assert_eq!(
        validate_core_document(&CoreSource {
            path: "metadata.yaml".into(),
            yaml: compatibility
        })
        .unwrap_err()
        .diagnostic
        .code,
        "E_UNKNOWN_FIELD"
    );
    for name in [
        "online/features.yaml",
        "online/customer-risk.yaml",
        "online/prepare.yaml",
        "online/blocked-email.yaml",
    ] {
        assert!(
            validate_core_document(&source(name)).is_err(),
            "{name} is not a strict Core resource"
        );
    }
    let pipeline = PipelineParser::parse(&source("online/prepare.yaml").yaml).unwrap();
    let ruleset = RulesetParser::parse(&source("online/markers.yaml").yaml).unwrap();
    PipelineCompiler::compile(&pipeline).unwrap();
    RulesetCompiler::compile(&ruleset).unwrap();
}
