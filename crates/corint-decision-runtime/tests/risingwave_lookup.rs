//! Live acceptance is opt-in and uses only a uniquely named disposable schema.
#![cfg(feature = "sqlx")]

use corint_decision_model::Value;
use corint_decision_runtime::{
    datasource::{
        DataSourceClient, DataSourceConfig, DataSourceType, FeatureLookupKeyType,
        FeatureLookupMapping,
    },
    feature::{FeatureDefinition, FeatureExecutor},
    ContextInput, ExecutionContext,
};
use std::collections::HashMap;

fn config(url: &str, schema: &str) -> DataSourceConfig {
    let mut config: DataSourceConfig = serde_yaml::from_str(
        r#"
name: rw
type: feature_store
provider: risingwave
connection_string: postgresql://root@localhost:4566/dev
feature_mappings:
  count:
    view: counts
    key_column: user_id
    value_column: count
"#,
    )
    .unwrap();
    let DataSourceType::FeatureStore(store) = &mut config.source_type else {
        unreachable!()
    };
    store.connection_string = url.into();
    store.feature_mappings.get_mut("count").unwrap().schema = schema.into();
    config
}

fn feature(name: &str, fallback: &str) -> FeatureDefinition {
    serde_yaml::from_str(&format!(
        "name: {name}\ntype: lookup\ndatasource: rw\nkey: '${{event.user.id}}'\n{fallback}"
    ))
    .unwrap()
}

fn context(key: &str) -> ExecutionContext {
    ExecutionContext::new(ContextInput::new(HashMap::from([(
        "user".into(),
        Value::Object(HashMap::from([("id".into(), Value::String(key.into()))])),
    )])))
    .unwrap()
}

#[tokio::test]
async fn invalid_bindings_and_keys_never_use_fallback() {
    let mut cfg = config("postgresql://root@127.0.0.1:1/dev", "public");
    let DataSourceType::FeatureStore(store) = &mut cfg.source_type else {
        unreachable!()
    };
    store.feature_mappings.get_mut("count").unwrap().key_type = FeatureLookupKeyType::Int64;
    let mut executor = FeatureExecutor::new();
    executor
        .add_datasource("rw", DataSourceClient::new(cfg.clone()).await.unwrap())
        .unwrap();
    assert!(executor
        .register_feature(feature("unmapped", "fallback: 0\n"))
        .is_err());
    executor
        .register_feature(feature("count", "fallback: 0\n"))
        .unwrap();
    let state: FeatureDefinition = serde_yaml::from_str("name: state\ntype: state\nmethod: time_since\ndatasource: rw\nentity: events\ndimension: user_id\ndimension_value: u1\nunit: hours\nfallback: 0\n").unwrap();
    assert!(executor
        .register_feature(state)
        .unwrap_err()
        .to_string()
        .contains("only support Lookup"));
    assert!(DataSourceClient::new(cfg.clone())
        .await
        .unwrap()
        .validate_aggregation("count")
        .is_err());
    assert!(executor
        .execute_feature("count", &context("not-an-integer"))
        .await
        .unwrap_err()
        .to_string()
        .contains("int64"));
    let mut late = FeatureExecutor::new();
    late.register_feature(feature("unmapped", "fallback: 0\n"))
        .unwrap();
    assert!(late
        .add_datasource("rw", DataSourceClient::new(cfg.clone()).await.unwrap())
        .is_err());
    let DataSourceType::FeatureStore(store) = &mut cfg.source_type else {
        unreachable!()
    };
    store.feature_mappings.get_mut("count").unwrap().view = "".into();
    assert!(DataSourceClient::new(cfg).await.is_err());
}

#[tokio::test]
async fn lookup_deadline_and_outage_follow_fallback_contract() {
    // Accept connections without responding to the PostgreSQL handshake.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "postgresql://root@{}/dev?sslmode=disable",
        listener.local_addr().unwrap()
    );
    let server = tokio::spawn(async move {
        let mut connections = Vec::new();
        loop {
            connections.push(listener.accept().await.unwrap());
        }
    });
    let mut cfg = config(&url, "public");
    cfg.timeout_ms = 100;
    let client = DataSourceClient::new(cfg.clone()).await.unwrap();
    assert!(tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client.get_feature("count", "u1")
    )
    .await
    .unwrap()
    .is_err());
    let mut executor = FeatureExecutor::new();
    executor
        .add_datasource("rw", DataSourceClient::new(cfg).await.unwrap())
        .unwrap();
    executor
        .register_feature(feature("count", "fallback: {unavailable: true}\n"))
        .unwrap();
    assert_eq!(
        executor
            .execute_feature("count", &context("u1"))
            .await
            .unwrap(),
        Value::Object(HashMap::from([("unavailable".into(), Value::Bool(true))]))
    );
    server.abort();
}

#[tokio::test]
#[ignore = "requires CORINT_TEST_RISINGWAVE_URL pointing at a disposable/test RisingWave database"]
async fn risingwave_materialized_view_lookup_acceptance() {
    use futures::FutureExt;
    use sqlx::Row;
    let url = std::env::var("CORINT_TEST_RISINGWAVE_URL")
        .expect("CORINT_TEST_RISINGWAVE_URL is required");
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let version: String = sqlx::query("SELECT version()")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    assert!(
        version.to_lowercase().contains("risingwave"),
        "must use real RisingWave: {version}"
    );
    let schema = format!("corint_lookup_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&pool)
        .await
        .unwrap();
    let result = std::panic::AssertUnwindSafe(async {
        sqlx::query(&format!("CREATE TABLE {schema}.events (id BIGINT PRIMARY KEY, user_id VARCHAR, amount DOUBLE PRECISION)"))
            .execute(&pool).await?;
        sqlx::query(&format!("CREATE MATERIALIZED VIEW {schema}.counts AS SELECT user_id, COUNT(*) AS count, SUM(amount) AS total FROM {schema}.events GROUP BY user_id"))
            .execute(&pool).await?;
        sqlx::query(&format!("INSERT INTO {schema}.events VALUES (1, 'u1', 12.5), (2, 'u1', 7.5), (3, 'u2', 99)"))
            .execute(&pool).await?;
        sqlx::query("FLUSH").execute(&pool).await?;
        let cfg = config(&url, &schema);
        let client = DataSourceClient::new(cfg.clone()).await?;
        assert_eq!(client.get_feature("count", "u1").await?, Some(Value::Number(2.0)));
        assert_eq!(client.get_feature("count", "u2").await?, Some(Value::Number(1.0)));
        assert_eq!(client.get_feature("count", "missing").await?, None);
        assert_eq!(client.get_feature("count", "u1' OR TRUE --").await?, None);

        let mut cached_cfg = cfg.clone();
        cached_cfg.query_cache_ttl_secs = 60;
        let cached = DataSourceClient::new(cached_cfg).await?;
        assert_eq!(cached.get_feature("count", "u1").await?, Some(Value::Number(2.0)));
        sqlx::query(&format!("INSERT INTO {schema}.events VALUES (4, 'u1', 5)" )).execute(&pool).await?;
        sqlx::query("FLUSH").execute(&pool).await?;
        assert_eq!(client.get_feature("count", "u1").await?, Some(Value::Number(3.0)));
        assert_eq!(cached.get_feature("count", "u1").await?, Some(Value::Number(2.0)));

        let mut executor = FeatureExecutor::new();
        executor.add_datasource("rw", DataSourceClient::new(cfg.clone()).await?)?;
        executor.register_feature(feature("count", "fallback: 0\n"))?;
        assert_eq!(executor.execute_feature("count", &context("u1")).await?, Value::Number(3.0));
        assert_eq!(executor.execute_feature("count", &context("missing")).await?, Value::Number(0.0));
        sqlx::query(&format!("DELETE FROM {schema}.events WHERE user_id = 'u1'" )).execute(&pool).await?;
        sqlx::query("FLUSH").execute(&pool).await?;
        assert_eq!(client.get_feature("count", "u1").await?, None);

        // Execute the published rolling-window SQL and CDL example together.
        let docs = include_str!("../../../docs/feature-configuration.md");
        let ddl = docs.split("## RisingWave direct Lookup").nth(1).unwrap()
            .split("```sql\n").nth(1).unwrap().split("```").next().unwrap();
        sqlx::query(&format!("CREATE TABLE {schema}.transactions (user_id VARCHAR, event_timestamp TIMESTAMPTZ)"))
            .execute(&pool).await?;
        sqlx::query(&ddl.replace("public.", &format!("{schema}."))).execute(&pool).await?;
        sqlx::query(&format!("INSERT INTO {schema}.transactions VALUES ('u1', NOW() - INTERVAL '1 minute'), ('u1', NOW() - INTERVAL '2 hours'), ('u1', NOW() + INTERVAL '1 hour'), ('u2', NOW() - INTERVAL '1 minute')"))
            .execute(&pool).await?;
        sqlx::query("FLUSH").execute(&pool).await?;
        let cdl = include_str!("../../../CDL/feature.md").split("### RisingWave materialized views").nth(1).unwrap()
            .split("```yaml\n").nth(1).unwrap().split("```").next().unwrap();
        let cdl: serde_yaml::Value = serde_yaml::from_str(cdl)?;
        let definition: FeatureDefinition = serde_yaml::from_value(cdl["features"][0].clone())?;
        let mut documented = cfg.clone();
        let DataSourceType::FeatureStore(store) = &mut documented.source_type else { unreachable!() };
        store.feature_mappings.insert(definition.name.clone(), FeatureLookupMapping {
            schema: schema.clone(), view: "user_features_mv".into(), key_column: "user_id".into(),
            value_column: "txn_count_1h".into(), key_type: FeatureLookupKeyType::Text,
        });
        let mut documented_executor = FeatureExecutor::new();
        documented_executor.add_datasource("rw_features", DataSourceClient::new(documented).await?)?;
        documented_executor.register_feature(definition)?;
        assert_eq!(documented_executor.execute_feature("user_txn_count_1h", &context("u1")).await?, Value::Number(1.0));
        assert_eq!(documented_executor.execute_feature("user_txn_count_1h", &context("missing")).await?, Value::Null);

        // Native result types: numeric-looking strings remain strings, SQL NULL is
        // a found value, and structured JSON is retained without dropping children.
        sqlx::query(&format!("CREATE TABLE {schema}.profiles (user_id BIGINT PRIMARY KEY, label VARCHAR, active BOOLEAN, n SMALLINT, i INTEGER, f REAL, d DECIMAL, payload JSONB, empty INTEGER, ts TIMESTAMPTZ, local_ts TIMESTAMP, day DATE, unsupported BYTEA)"))
            .execute(&pool).await?;
        sqlx::query(&format!("INSERT INTO {schema}.profiles VALUES (42, '00123', TRUE, 7, 8, 1.5, 12.25, '{{\"a\":[1,true,null,\"x\"]}}', NULL, '2026-01-01T00:00:00Z', '2026-01-01 00:00:00', '2026-01-01', NULL)"))
            .execute(&pool).await?;
        sqlx::query("FLUSH").execute(&pool).await?;
        let mut types_cfg = cfg.clone();
        let DataSourceType::FeatureStore(store) = &mut types_cfg.source_type else { unreachable!() };
        for column in ["label", "active", "n", "i", "f", "d", "payload", "empty", "ts", "local_ts", "day"] {
            store.feature_mappings.insert(column.into(), FeatureLookupMapping {
                schema: schema.clone(), view: "profiles".into(), key_column: "user_id".into(),
                value_column: column.into(), key_type: FeatureLookupKeyType::Int64,
            });
        }
        let types = DataSourceClient::new(types_cfg.clone()).await?;
        for (name, expected) in [
            ("label", Value::String("00123".into())), ("active", Value::Bool(true)),
            ("n", Value::Number(7.0)), ("i", Value::Number(8.0)),
            ("f", Value::Number(1.5)), ("d", Value::Number(12.25)), ("empty", Value::Null),
            ("payload", serde_json::from_value(serde_json::json!({"a": [1, true, null, "x"]}))?),
            ("ts", Value::String("2026-01-01T00:00:00+00:00".into())),
            ("local_ts", Value::String("2026-01-01T00:00:00".into())),
            ("day", Value::String("2026-01-01".into())),
        ] { assert_eq!(types.get_feature(name, "42").await?, Some(expected), "{name}"); }
        executor.add_datasource("rw", types).unwrap();
        executor.register_feature(feature("empty", "fallback: 99\n"))?;
        assert_eq!(executor.execute_feature("empty", &context("42")).await?, Value::Null);

        let mut broken = cfg.clone();
        let DataSourceType::FeatureStore(store) = &mut broken.source_type else { unreachable!() };
        store.feature_mappings.get_mut("count").unwrap().view = "absent_view".into();
        let bad = DataSourceClient::new(broken.clone()).await?;
        assert!(bad.get_feature("count", "u2").await.is_err());
        let mut fallback_executor = FeatureExecutor::new();
        fallback_executor.add_datasource("rw", DataSourceClient::new(broken).await?)?;
        fallback_executor.register_feature(feature("count", "fallback: 99\n"))?;
        assert_eq!(fallback_executor.execute_feature("count", &context("u2")).await?, Value::Number(99.0));

        // A relation without unique keys must not silently yield an arbitrary row.
        sqlx::query(&format!("INSERT INTO {schema}.events VALUES (5, 'u2', 2)" )).execute(&pool).await?;
        sqlx::query("FLUSH").execute(&pool).await?;
        let mut ambiguous = cfg;
        let DataSourceType::FeatureStore(store) = &mut ambiguous.source_type else { unreachable!() };
        let mapping = store.feature_mappings.get_mut("count").unwrap();
        mapping.view = "events".into(); mapping.value_column = "amount".into();
        assert!(DataSourceClient::new(ambiguous).await?.get_feature("count", "u2").await.unwrap_err().to_string().contains("multiple rows"));
        // A recreated view has a new internal relation ID. Reusing a connection
        // must not keep a prepared plan pointing at the dropped relation.
        sqlx::query(&format!("DROP MATERIALIZED VIEW {schema}.counts")).execute(&pool).await?;
        assert!(client.get_feature("count", "u2").await.is_err());
        sqlx::query(&format!("CREATE MATERIALIZED VIEW {schema}.counts AS SELECT user_id, COUNT(*) AS count FROM {schema}.events GROUP BY user_id"))
            .execute(&pool).await?;
        sqlx::query("FLUSH").execute(&pool).await?;
        assert_eq!(client.get_feature("count", "u2").await?, Some(Value::Number(2.0)));
        Ok::<(), anyhow::Error>(())
    }).catch_unwind().await;
    let cleanup = sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&pool)
        .await;
    cleanup.expect("remove disposable RisingWave schema");
    match result {
        Ok(result) => result.unwrap(),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
