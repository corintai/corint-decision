#![cfg(feature = "sqlx")]
use corint_decision_compiler::core::parse_core_input_schema;
use corint_decision_engine::{
    decision_host::{DecisionHost, FeatureDatasource, FeatureHostConfig},
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
        outputs:vec![FeatureInput { available_at_field: None, freshness: None, field:"amount".into(), definition:serde_yaml::from_str(&format!("name: volume\ntype: aggregation\nmethod: sum\ndatasource: events\nentity: {entity}\ndimension: user_id\ndimension_value: '${{event.user_id}}'\nfield: amount\nwindow: 60s\ntimestamp_field: occurred_at\n")).unwrap() }] }
}
fn source_config(provider: &str, connection: &str, ttl: u64) -> DataSourceConfig {
    serde_json::from_value(serde_json::json!({"name":"events","type":"sql","provider":provider,"connection_string":connection,"database":"test","timeout_ms":100,"query_cache_ttl_secs":ttl})).unwrap()
}
async fn client(provider: &str, connection: &str, ttl: u64) -> DataSourceClient {
    DataSourceClient::new(source_config(provider, connection, ttl))
        .await
        .unwrap()
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
    // The public host used by Core HTTP must match the existing SDK path on
    // both real SQLite and disposable PostgreSQL, without adapter-specific logic.
    let host = DecisionHost::new(
        &sources,
        schema.clone(),
        Some(FeatureHostConfig {
            activation_cases: vec![],
            plan: plan.clone(),
            datasources: BTreeMap::from([(
                "events".into(),
                FeatureDatasource {
                    revision: "test-v1".into(),
                    config: source_config(provider, connection, 0),
                },
            )]),
        }),
        true,
    )
    .await
    .unwrap();
    let hosted = host.decide(event("u1"), 120, true).await;
    assert_eq!(
        hosted.result.unwrap().result.score,
        decision.response.result.score
    );
    assert_eq!(
        hosted.feature_evidence.unwrap().values,
        decision.evidence.values
    );
    assert_eq!(hosted.input_evidence["event"]["amount"], 1100.0);
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
    sqlx::query("ALTER TABLE events ADD COLUMN status TEXT")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(&filter_rows("events"))
        .execute(&pool)
        .await
        .unwrap();
    verify_filters("sqlite", path.to_str().unwrap(), "events").await;
    sqlx::query("ALTER TABLE events ADD COLUMN available_at BIGINT DEFAULT 0")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE events_watermark(source TEXT, watermark BIGINT)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO events_watermark VALUES('events',119)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO events VALUES('filter',1,'1970-01-01 00:01:30','good',121)")
        .execute(&pool)
        .await
        .unwrap();
    verify_freshness("sqlite", path.to_str().unwrap(), "events", true).await;
    sqlx::query("UPDATE events_watermark SET watermark=50")
        .execute(&pool)
        .await
        .unwrap();
    verify_freshness("sqlite", path.to_str().unwrap(), "events", false).await;
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
    sqlx::query(&format!("ALTER TABLE {table} ADD COLUMN status TEXT"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(&filter_rows(&table))
        .execute(&pool)
        .await
        .unwrap();
    verify_filters("postgresql", &url, &table).await;
    sqlx::query(&format!(
        "ALTER TABLE {table} ADD COLUMN available_at BIGINT DEFAULT 0"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "CREATE TABLE {table}_watermark(source TEXT, watermark BIGINT)"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {table}_watermark VALUES('events',119)"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {table} VALUES('filter',1,'1970-01-01 00:01:30+00','good',121)"
    ))
    .execute(&pool)
    .await
    .unwrap();
    verify_freshness("postgresql", &url, &table, true).await;
    sqlx::query(&format!("UPDATE {table}_watermark SET watermark=50"))
        .execute(&pool)
        .await
        .unwrap();
    verify_freshness("postgresql", &url, &table, false).await;
    sqlx::query(&format!("DROP TABLE {table}_watermark"))
        .execute(&pool)
        .await
        .unwrap();
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

fn filter_rows(table: &str) -> String {
    format!("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1102) INSERT INTO {table}(user_id,amount,occurred_at,status) SELECT 'filter',1,'1970-01-01 00:01:30',CASE WHEN x=1102 THEN NULL ELSE 'good' END FROM n")
}
async fn verify_filters(provider: &str, connection: &str, entity: &str) {
    use corint_decision_runtime::feature::definition::WhenCondition;
    let (sources, schema) = fixtures();
    for (predicate, count) in [
        ("status != null", 1101.0),
        ("status == null", 1.0),
        ("status != null and amount >= 1", 1101.0),
        ("status in ['good', 'other']", 1101.0),
        ("status in ['good', null]", 1102.0),
        ("status not in ['good']", 1.0),
        ("status not in ['good', null]", 0.0),
        ("status in []", 0.0),
        ("status not in []", 1102.0),
        ("status == 'good and amount > 0'", 0.0),
    ] {
        let mut plan = plan(entity);
        plan.outputs[0].definition.method = Some("count".into());
        plan.outputs[0]
            .definition
            .aggregation
            .as_mut()
            .unwrap()
            .when = Some(WhenCondition::Simple(predicate.into()));
        let host = DecisionHost::new(
            &sources,
            schema.clone(),
            Some(FeatureHostConfig {
                plan,
                activation_cases: vec![],
                datasources: BTreeMap::from([(
                    "events".into(),
                    FeatureDatasource {
                        revision: "test-v1".into(),
                        config: source_config(provider, connection, 0),
                    },
                )]),
            }),
            false,
        )
        .await
        .unwrap();
        let result = host.decide(event("filter"), 120, false).await;
        assert_eq!(
            result.feature_evidence.unwrap().values["amount"],
            Value::Number(count),
            "{provider}: {predicate}"
        );
        assert_eq!(
            result.result.unwrap().result.score,
            if count > 1000.0 { 60 } else { 0 }
        );
    }
    for invalid in [
        "status == 'good' or amount > 0",
        "status == good",
        "status > null",
        "status in 'good'",
        "status matches '.*'",
        "status == 1e999",
        "status in '${event.user_id}'",
        "status in ['${event.user_id}']",
        "status == 'good' garbage",
    ] {
        let mut plan = plan(entity);
        plan.outputs[0]
            .definition
            .aggregation
            .as_mut()
            .unwrap()
            .when = Some(WhenCondition::Simple(invalid.into()));
        assert!(
            DecisionHost::new(
                &sources,
                schema.clone(),
                Some(FeatureHostConfig {
                    plan,
                    activation_cases: vec![],
                    datasources: BTreeMap::from([(
                        "events".into(),
                        FeatureDatasource {
                            revision: "test-v1".into(),
                            config: source_config(provider, connection, 0)
                        }
                    )]),
                }),
                false
            )
            .await
            .is_err(),
            "{invalid}"
        );
    }
}

async fn verify_freshness(provider: &str, connection: &str, entity: &str, fresh: bool) {
    use corint_decision_engine::feature_pipeline::FeatureFreshness;
    let (sources, schema) = fixtures();
    let mut plan = plan(entity);
    plan.outputs[0].definition.method = Some("count".into());
    plan.outputs[0].available_at_field = Some("available_at".into());
    plan.outputs[0].freshness = Some(FeatureFreshness {
        entity: format!("{entity}_watermark"),
        key_field: "source".into(),
        key: "events".into(),
        watermark_field: "watermark".into(),
        max_lag_seconds: 5,
    });
    let host = DecisionHost::new(
        &sources,
        schema,
        Some(FeatureHostConfig {
            plan,
            activation_cases: vec![],
            datasources: BTreeMap::from([(
                "events".into(),
                FeatureDatasource {
                    revision: "test-v1".into(),
                    config: source_config(provider, connection, 0),
                },
            )]),
        }),
        false,
    )
    .await
    .unwrap();
    let execution = host.decide(event("filter"), 120, false).await;
    if fresh {
        assert_eq!(execution.result.unwrap().result.score, 60);
        let evidence = execution.feature_evidence.unwrap();
        assert_eq!(evidence.values["amount"], Value::Number(1102.0));
        assert_eq!(evidence.freshness["amount"]["watermark_unix_seconds"], 119);
        assert_eq!(evidence.freshness["amount"]["availability_filtered"], true);
    } else {
        assert!(execution
            .result
            .unwrap_err()
            .to_string()
            .contains("E_FEATURE_FRESHNESS"));
        assert!(execution.feature_evidence.is_none());
    }
}
