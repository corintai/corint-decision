use corint_decision_model::{
    ir::{Instruction, Program, ProgramMetadata},
    Value,
};
use corint_decision_runtime::feature::{FeatureDefinition, FeatureExecutor, FeatureRegistry};
use corint_decision_runtime::{ContextInput, ExecutionContext, PipelineExecutor};
use std::collections::HashMap;

fn context() -> ExecutionContext {
    ExecutionContext::new(ContextInput::new(HashMap::new())).unwrap()
}
fn feature(name: &str, dependencies: &[&str]) -> FeatureDefinition {
    serde_yaml::from_str(&format!(
        "name: {name}\ntype: expression\nexpression: '1'\ndependencies: {}\n",
        serde_json::to_string(dependencies).unwrap()
    ))
    .unwrap()
}

#[tokio::test]
async fn dependency_registration_is_atomic_and_execution_is_bounded() {
    let mut executor = FeatureExecutor::new().with_stats();
    executor.register_feature(feature("a", &["b"])).unwrap();
    assert!(executor
        .register_feature(feature("b", &["a"]))
        .unwrap_err()
        .to_string()
        .contains("Circular"));
    assert!(!executor.has_feature("b"));
    assert!(executor
        .execute_feature("a", &context())
        .await
        .unwrap_err()
        .to_string()
        .contains("not found"));
    let mut executor = FeatureExecutor::new().with_stats();
    executor
        .register_features(vec![
            feature("a", &[]),
            feature("b", &["a"]),
            feature("c", &["a"]),
            feature("d", &["b", "c"]),
        ])
        .unwrap();
    assert_eq!(
        executor.execute_feature("d", &context()).await.unwrap(),
        Value::Number(1.0)
    );
    assert_eq!(executor.get_stats().await.compute_count, 4);
    assert!(executor
        .register_features(vec![feature("missing", &["unknown"])])
        .is_err());
    assert!(!executor.has_feature("missing"));
    let mut disabled = feature("disabled", &["unavailable"]);
    disabled.enabled = false;
    executor.register_feature(disabled).unwrap();
    assert_eq!(
        executor
            .execute_feature("disabled", &context())
            .await
            .unwrap(),
        Value::Null
    );
    let chain: Vec<_> = (0..256)
        .map(|n| {
            feature(
                &format!("f{n}"),
                if n == 0 {
                    vec![]
                } else {
                    vec![format!("f{}", n - 1)]
                }
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .as_slice(),
            )
        })
        .collect();
    assert!(FeatureExecutor::new()
        .register_features(chain)
        .unwrap_err()
        .to_string()
        .contains("depth"));
}

#[test]
fn directory_load_rejects_cycles_and_partial_success() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = FeatureRegistry::new();
    std::fs::write(
        dir.path().join("a.yaml"),
        "features:\n- name: a\n  type: expression\n  expression: b\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.yaml"),
        "features:\n- name: b\n  type: expression\n  expression: a\n",
    )
    .unwrap();
    assert!(registry
        .load_from_directory(dir.path())
        .unwrap_err()
        .to_string()
        .contains("Circular"));
    assert_eq!(registry.count(), 0);
    std::fs::write(dir.path().join("b.yaml"), "features: [broken").unwrap();
    assert!(registry.load_from_directory(dir.path()).is_err());
    assert_eq!(registry.count(), 0);
}

#[tokio::test]
async fn vm_rejects_invalid_jumps_and_exhausts_loops() {
    for offset in [0, -1, 2] {
        let program = Program::new(
            vec![Instruction::Jump { offset }],
            ProgramMetadata::default(),
        );
        let error = PipelineExecutor::new_offline()
            .execute(&program, HashMap::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(if offset == 0 { "budget" } else { "jump" }),
            "{error}"
        );
    }
}

#[cfg(feature = "sqlx")]
mod sqlite {
    use super::*;
    use corint_decision_model::ast::Operator;
    use corint_decision_runtime::{
        Aggregation, AggregationType, DataSourceClient, DataSourceConfig, Query, QueryType,
    };
    use std::sync::Arc;

    async fn database() -> (tempfile::TempDir, sqlx::SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let pool = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(dir.path().join("events.sqlite"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query("CREATE TABLE events(user_id TEXT, label TEXT, kind TEXT, amount REAL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO events VALUES ('u1','a','good',10),('u1','ab','bad',20),('u1','b','good',30)").execute(&pool).await.unwrap();
        (dir, pool)
    }
    async fn client(dir: &tempfile::TempDir, ttl: u64) -> DataSourceClient {
        let config: DataSourceConfig = serde_json::from_value(serde_json::json!({"name":"events","type":"sql","provider":"sqlite","connection_string":dir.path().join("events.sqlite"),"database":"events","query_cache_ttl_secs":ttl})).unwrap();
        DataSourceClient::new(config).await.unwrap()
    }
    fn aggregation(method: &str, when: &str) -> FeatureDefinition {
        serde_yaml::from_str(&format!("name: value\ntype: aggregation\nmethod: {method}\ndatasource: events\nentity: events\ndimension: user_id\ndimension_value: u1\nfield: amount\nwhen: {when}\n")).unwrap()
    }
    fn query() -> Query {
        Query {
            query_type: QueryType::RawEvents,
            entity: "events".into(),
            filters: vec![],
            time_window: None,
            aggregations: vec![],
            group_by: vec![],
            limit: None,
        }
    }

    #[tokio::test]
    async fn filters_never_disappear_and_string_predicates_preserve_literals() {
        let (dir, pool) = database().await;
        for invalid in ["{any: ['kind == \"missing\"']}", "'not a predicate'", "{}"] {
            let mut executor = FeatureExecutor::new();
            assert!(
                executor
                    .register_feature(aggregation("count", invalid))
                    .is_err(),
                "{invalid}"
            );
        }
        let mut executor = FeatureExecutor::new();
        executor
            .add_datasource("events", client(&dir, 0).await)
            .unwrap();
        executor
            .register_feature(aggregation("count", "'kind == \"{event.missing}\"'"))
            .unwrap();
        assert!(executor
            .execute_feature("value", &context())
            .await
            .unwrap_err()
            .to_string()
            .contains("Unresolved"));
        for (when, expected) in [
            ("'label contains \"a\"'", 2.0),
            ("'label starts_with \"a\"'", 2.0),
            ("'label ends_with \"b\"'", 2.0),
            ("{all: ['kind == \"good\"']}", 2.0),
        ] {
            executor
                .register_feature(aggregation("count", when))
                .unwrap();
            assert_eq!(
                executor.execute_feature("value", &context()).await.unwrap(),
                Value::Number(expected),
                "{when}"
            );
        }
        sqlx::query("INSERT INTO events VALUES ('u1','a%_!','good',40),('u1','axb','good',50)")
            .execute(&pool)
            .await
            .unwrap();
        executor
            .register_feature(aggregation("count", "'label contains \"%_!\"'"))
            .unwrap();
        assert_eq!(
            executor.execute_feature("value", &context()).await.unwrap(),
            Value::Number(1.0)
        );
    }

    #[tokio::test]
    async fn supported_sqlite_aggregations_have_real_backend_evidence() {
        let (dir, _pool) = database().await;
        let mut executor = FeatureExecutor::new();
        executor
            .add_datasource("events", client(&dir, 0).await)
            .unwrap();
        for (method, expected) in [
            ("count", 3.0),
            ("sum", 60.0),
            ("avg", 20.0),
            ("min", 10.0),
            ("max", 30.0),
            ("distinct", 3.0),
        ] {
            executor
                .register_feature(aggregation(method, "{all: []}"))
                .unwrap();
            assert_eq!(
                executor.execute_feature("value", &context()).await.unwrap(),
                Value::Number(expected),
                "{method}"
            );
        }
    }

    #[tokio::test]
    async fn cache_preserves_all_rows_empty_results_and_freshness_policy() {
        let (dir, pool) = database().await;
        let cached = client(&dir, 3600).await;
        let first = cached.query(query()).await.unwrap();
        let second = cached.query(query()).await.unwrap();
        assert_eq!(first.rows.len(), 3);
        assert_eq!(first.rows, second.rows);
        assert!(second.from_cache);
        let mut empty = query();
        empty.limit = Some(0);
        assert!(cached.query(empty.clone()).await.unwrap().rows.is_empty());
        assert!(cached.query(empty).await.unwrap().from_cache);
        let fresh = client(&dir, 0).await;
        assert_eq!(fresh.query(query()).await.unwrap().rows.len(), 3);
        sqlx::query("INSERT INTO events VALUES ('u1','d','good',40)")
            .execute(&pool)
            .await
            .unwrap();
        let result = fresh.query(query()).await.unwrap();
        assert_eq!(result.rows.len(), 4);
        assert!(!result.from_cache);
        let expiring = client(&dir, 1).await;
        expiring.query(query()).await.unwrap();
        assert!(expiring.query(query()).await.unwrap().from_cache);
        tokio::time::sleep(std::time::Duration::from_millis(1050)).await;
        assert!(!expiring.query(query()).await.unwrap().from_cache);
        cached.clear_query_cache();
        assert_eq!(cached.query(query()).await.unwrap().rows.len(), 4);
    }

    #[tokio::test]
    async fn backend_capabilities_are_checked_in_both_registration_orders_and_raw_queries() {
        let (dir, _pool) = database().await;
        for (method, agg_type) in [
            ("median", AggregationType::Median),
            ("stddev", AggregationType::Stddev),
            ("percentile", AggregationType::Percentile { p: 95 }),
        ] {
            let mut executor = FeatureExecutor::new();
            executor
                .add_datasource("events", client(&dir, 0).await)
                .unwrap();
            assert!(executor
                .register_feature(aggregation(method, "'true == true'"))
                .unwrap_err()
                .to_string()
                .contains("Unsupported aggregation"));
            let mut executor = FeatureExecutor::new();
            executor
                .register_feature(aggregation(method, "'kind == \"good\"'"))
                .unwrap();
            assert!(executor
                .add_datasource("events", client(&dir, 0).await)
                .is_err());
            let mut q = query();
            q.aggregations = vec![Aggregation {
                agg_type,
                field: Some("amount".into()),
                output_name: "value".into(),
            }];
            assert!(client(&dir, 0)
                .await
                .query(q)
                .await
                .unwrap_err()
                .to_string()
                .contains("Unsupported aggregation"));
        }
    }

    #[tokio::test]
    async fn feature_query_failure_propagates_through_rule_execution() {
        let (dir, pool) = database().await;
        sqlx::query("DROP TABLE events")
            .execute(&pool)
            .await
            .unwrap();
        let mut executor = FeatureExecutor::new();
        executor
            .add_datasource("events", client(&dir, 0).await)
            .unwrap();
        executor
            .register_feature(aggregation("count", "'kind == \"good\"'"))
            .unwrap();
        let program = Program::new(
            vec![
                Instruction::LoadField {
                    path: vec!["features".into(), "value".into()],
                },
                Instruction::LoadConst {
                    value: Value::Number(0.0),
                },
                Instruction::Compare { op: Operator::Gt },
                Instruction::JumpIfFalse { offset: 2 },
                Instruction::AddScore { value: 100 },
                Instruction::Return,
            ],
            ProgramMetadata::for_rule("guard".into()),
        );
        let error = PipelineExecutor::new_offline()
            .with_feature_executor(Arc::new(executor))
            .execute(&program, HashMap::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Feature 'value' calculation failed") && error.contains("no such table"),
            "{error}"
        );
    }
}
