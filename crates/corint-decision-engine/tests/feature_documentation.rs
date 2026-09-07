#![cfg(feature = "sqlx")]
use corint_decision_compiler::{
    codegen::RuleCompiler,
    core::{validate_core_document, CoreSource},
};
use corint_decision_dsl_parser::RuleParser;
use corint_decision_model::Value;
use corint_decision_runtime::{
    feature::{definition::FeatureCollection, FeatureDefinition, FeatureExecutor, FeatureRegistry},
    DataSourceClient, ExecutionContext, PipelineExecutor,
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};

fn fixture(id: &str) -> &'static str {
    match id {
        "feature-state" => include_str!("../../../tests/conformance/features/state.yaml"),
        "feature-lookup" => include_str!("../../../tests/conformance/features/lookup.yaml"),
        "feature-definitions" => include_str!("../../../tests/conformance/features/payments.yaml"),
        "feature-rule" => include_str!("../../../tests/conformance/features/rule.yaml"),
        _ => panic!("unknown test fixture: {id}"),
    }
}

fn definitions(id: &str) -> Vec<FeatureDefinition> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("features.yaml");
    std::fs::write(&path, fixture(id)).unwrap();
    let mut registry = FeatureRegistry::new();
    registry.load_from_file(&path).unwrap();
    registry.all_features().into_iter().cloned().collect()
}

fn event(id: &str) -> HashMap<String, Value> {
    serde_json::from_value(json!({
        "type": "payment", "user": {"id": id}, "id": "wrong-entity",
        "min_amount": 100, "country": "CN"
    }))
    .unwrap()
}

async fn database() -> (tempfile::TempDir, sqlx::SqlitePool, DataSourceClient) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("events.sqlite");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE events(user_id TEXT, kind TEXT, amount REAL, country TEXT, occurred_at TEXT)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let client = DataSourceClient::new(
        serde_json::from_value(json!({
            "name":"events", "type":"sql", "provider":"sqlite",
            "connection_string":path, "database":"events"
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    (directory, pool, client)
}

#[tokio::test]
async fn feature_and_rule_examples_execute_against_filtered_sql_rows() {
    let (_directory, pool, datasource) = database().await;
    sqlx::query(
        "INSERT INTO events VALUES
        ('u1','payment',120,'CN',datetime('now','-10 minutes')),
        ('u1','payment',180,'CN',datetime('now','-5 minutes')),
        ('u1','payment',80,'CN',datetime('now','-5 minutes')),
        ('u1','refund',999,'CN',datetime('now','-5 minutes')),
        ('u1','payment',999,'US',datetime('now','-5 minutes')),
        ('u1','payment',999,'CN',datetime('now','-2 hours')),
        ('u1','payment',999,'CN',datetime('now','+1 hour')),
        ('wrong-entity','payment',999,'CN',datetime('now','-5 minutes'))",
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut features = FeatureExecutor::new();
    features.add_datasource("events", datasource).unwrap();
    features
        .register_features(definitions("feature-definitions"))
        .unwrap();
    let context = ExecutionContext::from_event(event("u1")).unwrap();
    for (name, expected) in [
        ("payment_count_1h", 2.0),
        ("payment_amount_1h", 300.0),
        ("average_payment_1h", 150.0),
    ] {
        assert_eq!(
            features.execute_feature(name, &context).await.unwrap(),
            Value::Number(expected),
            "{name}"
        );
    }
    let empty = ExecutionContext::from_event(event("absent")).unwrap();
    assert_eq!(
        features
            .execute_feature("payment_count_1h", &empty)
            .await
            .unwrap(),
        Value::Number(0.0)
    );
    assert_eq!(
        features
            .execute_feature("payment_amount_1h", &empty)
            .await
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        features
            .execute_feature("average_payment_1h", &empty)
            .await
            .unwrap(),
        Value::Null
    );
    let mut missing = event("u1");
    missing.remove("min_amount");
    assert!(features
        .execute_feature(
            "payment_count_1h",
            &ExecutionContext::from_event(missing).unwrap()
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("Unresolved"));

    let program =
        RuleCompiler::compile(&RuleParser::parse(fixture("feature-rule")).unwrap()).unwrap();
    let vm = PipelineExecutor::new_offline().with_feature_executor(Arc::new(features));
    assert_eq!(vm.execute(&program, event("u1")).await.unwrap().score, 80);
    assert_eq!(
        vm.execute(&program, event("absent")).await.unwrap().score,
        0
    );
    for id in ["feature-definitions", "feature-state", "feature-lookup"] {
        assert!(validate_core_document(&CoreSource {
            path: format!("{id}.yaml"),
            yaml: fixture(id).into()
        })
        .is_err());
    }
}

#[tokio::test]
async fn state_example_uses_first_timestamp_and_explicit_error_boundaries() {
    let (_directory, pool, datasource) = database().await;
    sqlx::query(
        "INSERT INTO events VALUES
        ('u1','register',0,'CN',datetime('now','-12 days','-1 hour')),
        ('u1','register',0,'CN',datetime('now','-2 days')),
        ('wrong-entity','register',0,'CN',datetime('now','-50 days')),
        ('u1','payment',0,'CN',datetime('now','-50 days'))",
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut features = FeatureExecutor::new();
    features.add_datasource("events", datasource).unwrap();
    let definition = definitions("feature-state").pop().unwrap();
    features.register_feature(definition.clone()).unwrap();
    let context = ExecutionContext::from_event(event("u1")).unwrap();
    assert_eq!(
        features
            .execute_feature("account_age_days", &context)
            .await
            .unwrap(),
        Value::Number(12.0)
    );
    assert_eq!(
        features
            .execute_feature(
                "account_age_days",
                &ExecutionContext::from_event(event("absent")).unwrap()
            )
            .await
            .unwrap(),
        Value::Number(0.0)
    );
    assert!(features
        .execute_feature(
            "account_age_days",
            &ExecutionContext::from_event(HashMap::new()).unwrap()
        )
        .await
        .is_err());
    let mut invalid = definition.clone();
    invalid.state.as_mut().unwrap().window = Some("1h".into());
    assert!(features
        .register_feature(invalid)
        .unwrap_err()
        .to_string()
        .contains("does not support window"));
    let mut invalid = definition.clone();
    invalid.state.as_mut().unwrap().unit = Some("seconds".into());
    assert!(features.register_feature(invalid).is_err());
    let mut unsupported = definition.clone();
    unsupported.method = Some("last_seen".into());
    features.register_feature(unsupported).unwrap();
    assert!(
        features
            .execute_feature("account_age_days", &context)
            .await
            .is_err(),
        "fallback cannot implement an unsupported method"
    );
    let mut unsupported = definition.clone();
    unsupported.state.as_mut().unwrap().when =
        Some(serde_yaml::from_str("'kind contains \"register\"'").unwrap());
    features.register_feature(unsupported).unwrap();
    assert!(features
        .execute_feature("account_age_days", &context)
        .await
        .unwrap_err()
        .to_string()
        .contains("Unsupported time_since filter"));
    features.register_feature(definition.clone()).unwrap();
    sqlx::query("DROP TABLE events")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        features
            .execute_feature("account_age_days", &context)
            .await
            .unwrap(),
        Value::Number(0.0)
    );
    let mut strict = definition;
    strict.state.as_mut().unwrap().fallback = None;
    features.register_feature(strict).unwrap();
    assert!(features
        .execute_feature("account_age_days", &context)
        .await
        .is_err());
}

#[test]
fn lookup_deserialization_preserves_configuration_and_json_fallbacks() {
    let collection: FeatureCollection = serde_yaml::from_str(fixture("feature-lookup")).unwrap();
    assert!(collection.features[0].lookup.is_some());
    for fallback in [
        json!({"available":false,"codes":[1,2]}),
        json!([false, 5, null]),
    ] {
        let feature: FeatureDefinition = serde_json::from_value(json!({
            "name":"lookup", "type":"lookup", "datasource":"profiles",
            "key":"${event.user.id}", "fallback":fallback
        }))
        .unwrap();
        assert_eq!(
            feature.lookup.unwrap().fallback.unwrap(),
            serde_json::from_value::<Value>(fallback).unwrap()
        );
    }
    for invalid in [
        json!({"name":"lookup","type":"lookup","key":"u1"}),
        json!({"name":"lookup","type":"lookup","datasource":"profiles"}),
        json!({"name":"lookup","type":"lookup","datasource":"profiles","key":12}),
    ] {
        assert!(serde_json::from_value::<FeatureDefinition>(invalid).is_err());
    }
}

#[cfg(feature = "redis")]
#[tokio::test]
async fn lookup_example_sends_entity_keys_and_distinguishes_misses_from_errors() {
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    // A deterministic RESP fixture verifies the actual connector request without
    // depending on a separately installed Redis server.
    let server = tokio::spawn(async move {
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
                "GET" => match args[1].as_str() {
                    "tenant:user_risk_score:u1" => "$2\r\n87\r\n",
                    "tenant:user_risk_score:missing" => "$-1\r\n",
                    "tenant:user_risk_score:broken" => "-ERR store unavailable\r\n",
                    key => panic!("Unexpected physical key: {key}"),
                },
                command => panic!("Unexpected Redis command: {command}"),
            };
            reader.get_mut().write_all(reply.as_bytes()).await.unwrap();
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let datasource = DataSourceClient::new(
            serde_json::from_value(json!({
                "name":"profiles", "type":"feature_store", "provider":"redis",
                "connection_string":format!("redis://{address}"),
                "namespace":"tenant", "default_ttl":0
            }))
            .unwrap(),
        )
        .await
        .unwrap();
        let mut executor = FeatureExecutor::new();
        executor.add_datasource("profiles", datasource).unwrap();
        let definition = definitions("feature-lookup").pop().unwrap();
        executor.register_feature(definition.clone()).unwrap();
        for (id, expected) in [("u1", 87.0), ("missing", 50.0), ("broken", 50.0)] {
            assert_eq!(
                executor
                    .execute_feature(
                        "user_risk_score",
                        &ExecutionContext::from_event(event(id)).unwrap()
                    )
                    .await
                    .unwrap(),
                Value::Number(expected),
                "{id}"
            );
        }
        assert!(executor
            .execute_feature(
                "user_risk_score",
                &ExecutionContext::from_event(HashMap::new()).unwrap()
            )
            .await
            .is_err());
        let mut strict = definition;
        strict.lookup.as_mut().unwrap().fallback = None;
        executor.register_feature(strict).unwrap();
        assert_eq!(
            executor
                .execute_feature(
                    "user_risk_score",
                    &ExecutionContext::from_event(event("missing")).unwrap()
                )
                .await
                .unwrap(),
            Value::Null
        );
        assert!(executor
            .execute_feature(
                "user_risk_score",
                &ExecutionContext::from_event(event("broken")).unwrap()
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("store unavailable"));
    })
    .await
    .expect("bounded local lookup execution");
    server.abort();
    let _ = server.await;
}
