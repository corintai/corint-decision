#![cfg(feature = "sqlx")]
use corint_decision_compiler::core::parse_core_input_schema;
use corint_decision_engine::{
    feature_pipeline::{FeatureInput, FeaturePipeline, FeaturePlan},
    CoreSource, FieldType, Schema, SchemaField, Value,
};
use corint_decision_runtime::{DataSourceClient, DataSourceConfig};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

fn fixtures() -> (Vec<CoreSource>, Schema) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core");
    let read = |name: &str| CoreSource {
        path: name.into(),
        yaml: std::fs::read_to_string(root.join(name)).unwrap(),
    };
    let mut schema = parse_core_input_schema(&read("input-schema.yaml")).unwrap();
    schema.fields.insert(
        "user_id".into(),
        SchemaField::new("user_id".into(), FieldType::String).required(),
    );
    (
        [
            "rule.yaml",
            "ruleset.yaml",
            "pipeline.yaml",
            "registry.yaml",
        ]
        .iter()
        .map(|p| read(p))
        .collect(),
        schema,
    )
}
fn plan(entity: &str) -> FeaturePlan {
    FeaturePlan { format_version:"1".into(), revision:"volume-v1".into(), datasource_revisions:BTreeMap::from([("events".into(), "test-v1".into())]), timeout_ms:1000,
        outputs:vec![FeatureInput { field:"amount".into(), definition:serde_yaml::from_str(&format!("name: volume\ntype: aggregation\nmethod: sum\ndatasource: events\nentity: {entity}\ndimension: user_id\ndimension_value: '${{event.user_id}}'\nfield: amount\nwindow: 60s\ntimestamp_field: occurred_at\n")).unwrap() }] }
}
async fn client(provider: &str, connection: &str, ttl: u64) -> DataSourceClient {
    let config: DataSourceConfig = serde_json::from_value(serde_json::json!({"name":"events","type":"sql","provider":provider,"connection_string":connection,"database":"test","timeout_ms":100,"query_cache_ttl_secs":ttl})).unwrap();
    DataSourceClient::new(config).await.unwrap()
}
fn event(user: &str) -> HashMap<String, Value> {
    HashMap::from([("user_id".into(), Value::String(user.into()))])
}
const ROWS: &str = "('u1',400,'1970-01-01 00:01:00'),('u1',700,'1970-01-01 00:01:59'),('u1',99999,'1970-01-01 00:02:00'),('u1',99999,'1970-01-01 00:00:59'),('u2',99999,'1970-01-01 00:01:30')";

async fn verify(provider: &str, connection: &str, entity: &str) {
    let (sources, schema) = fixtures();
    let plan = plan(entity);
    let binding = plan.binding_sha256();
    let engine = FeaturePipeline::new(
        &sources,
        schema.clone(),
        plan.clone(),
        HashMap::from([(
            "events".into(),
            ("test-v1".into(), client(provider, connection, 0).await),
        )]),
        &binding,
    )
    .unwrap();
    let decision = engine.decide(event("u1"), 120, true).await.unwrap();
    // Offline oracle: [60,120), same entity; future and preceding rows excluded.
    assert_eq!(decision.evidence.values["amount"], Value::Number(1100.0));
    assert_eq!(decision.response.result.score, 60);
    assert_eq!(decision.evidence.binding_sha256, binding);
    let replay = corint_decision_engine::DecisionEngine::from_core(&sources, schema.clone())
        .unwrap()
        .decide(corint_decision_engine::DecisionRequest::new(
            decision.replay_event,
        ))
        .await
        .unwrap();
    assert_eq!(replay.result.score, 60);
    assert!(engine
        .decide(event("missing"), 120, false)
        .await
        .unwrap_err()
        .to_string()
        .contains("E_FEATURE_VALUE"));
    let mut injected = event("u1");
    injected.insert("amount".into(), Value::Number(0.0));
    assert!(engine.decide(injected, 120, false).await.is_err());
    assert!(FeaturePipeline::new(
        &sources,
        schema.clone(),
        plan.clone(),
        HashMap::from([(
            "events".into(),
            (
                "other-version".into(),
                client(provider, connection, 0).await
            )
        )]),
        &binding
    )
    .is_err());
    assert!(FeaturePipeline::new(
        &sources,
        schema.clone(),
        plan.clone(),
        HashMap::from([(
            "events".into(),
            ("test-v1".into(), client(provider, connection, 60).await)
        )]),
        &binding
    )
    .is_err());
    let mut changed = plan;
    changed.outputs[0].definition.method = Some("avg".into());
    assert!(FeaturePipeline::new(&sources, schema, changed, HashMap::new(), &binding).is_err());
}

#[tokio::test]
async fn sqlite_feature_to_core_preserves_cutoff_binding_and_replay() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("feature.sqlite");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE events(user_id TEXT, amount REAL, occurred_at TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(&format!("INSERT INTO events VALUES {ROWS}"))
        .execute(&pool)
        .await
        .unwrap();
    verify("sqlite", path.to_str().unwrap(), "events").await;
    // Remove the source: computation must fail instead of producing a zero score.
    let (sources, schema) = fixtures();
    let plan = plan("events");
    let binding = plan.binding_sha256();
    let engine = FeaturePipeline::new(
        &sources,
        schema,
        plan,
        HashMap::from([(
            "events".into(),
            (
                "test-v1".into(),
                client("sqlite", path.to_str().unwrap(), 0).await,
            ),
        )]),
        &binding,
    )
    .unwrap();
    sqlx::query("DROP TABLE events")
        .execute(&pool)
        .await
        .unwrap();
    assert!(engine
        .decide(event("u1"), 120, false)
        .await
        .unwrap_err()
        .to_string()
        .contains("E_FEATURE_EXECUTION"));
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL: tests/scripts/run_feature_postgres_tests.py"]
async fn postgres_feature_to_core_matches_offline_oracle() {
    let url = std::env::var("CORINT_TEST_POSTGRES_URL").expect("Set disposable PostgreSQL URL");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let table = format!("feature_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!(
        "CREATE TABLE {table}(user_id TEXT, amount DOUBLE PRECISION, occurred_at TIMESTAMPTZ)"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let utc_rows = ROWS
        .split("'")
        .map(|part| {
            if part.starts_with("1970-") {
                format!("{part}+00")
            } else {
                part.into()
            }
        })
        .collect::<Vec<_>>()
        .join("'");
    sqlx::query(&format!("INSERT INTO {table} VALUES {utc_rows}"))
        .execute(&pool)
        .await
        .unwrap();
    verify("postgresql", &url, &table).await;
    sqlx::query(&format!("DROP TABLE {table}"))
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn feature_deadline_expires_without_running_a_decision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("locked.sqlite");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE events(user_id TEXT, amount REAL, occurred_at TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    let (sources, schema) = fixtures();
    let mut plan = plan("events");
    plan.timeout_ms = 10;
    let binding = plan.binding_sha256();
    let engine = FeaturePipeline::new(
        &sources,
        schema,
        plan,
        HashMap::from([(
            "events".into(),
            (
                "test-v1".into(),
                client("sqlite", path.to_str().unwrap(), 0).await,
            ),
        )]),
        &binding,
    )
    .unwrap();
    let mut writer = pool.acquire().await.unwrap();
    sqlx::query("BEGIN EXCLUSIVE")
        .execute(&mut *writer)
        .await
        .unwrap();
    let result = engine.decide(event("u1"), 120, false).await;
    sqlx::query("ROLLBACK").execute(&mut *writer).await.unwrap();
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("E_FEATURE_TIMEOUT"));
}
