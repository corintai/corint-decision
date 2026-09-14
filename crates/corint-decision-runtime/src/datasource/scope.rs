//! Host-owned row and relation authorization. Never deserialize this from an event.
use super::query::{Filter, FilterOperator, Query};
use crate::error::{Result, RuntimeError};
use corint_decision_model::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataAccessScope {
    /// Includes tenant/environment and the authorized resource revision.
    pub namespace: String,
    pub entity: String,
    pub equalities: BTreeMap<String, String>,
}

pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit())
}
fn field(value: &str) -> bool {
    value.len() <= 256 && value.split('.').all(identifier)
}
fn denied() -> RuntimeError {
    RuntimeError::InvalidOperation("E_RESOURCE_SCOPE: query outside authorized resource".into())
}
impl DataAccessScope {
    pub fn validate(&self) -> Result<()> {
        if self.namespace.is_empty()
            || self.namespace.len() > 1024
            || !field(&self.entity)
            || self.equalities.is_empty()
            || self.equalities.len() > 8
            || self
                .equalities
                .iter()
                .any(|(k, v)| !identifier(k) || v.is_empty() || v.len() > 128)
        {
            return Err(denied());
        }
        Ok(())
    }
    pub fn apply(&self, query: &mut Query) -> Result<()> {
        self.validate()?;
        // Do not permit raw SQL expressions to escape an appended AND predicate.
        // Scoped queries use identifiers/dotted field paths and typed operators only.
        if query.entity != self.entity
            || query.filters.iter().any(|f| !field(&f.field))
            || query.group_by.iter().any(|f| !field(f))
            || query
                .time_window
                .as_ref()
                .is_some_and(|w| !field(&w.time_field))
            || query
                .aggregations
                .iter()
                .any(|a| !identifier(&a.output_name) || a.field.as_ref().is_some_and(|f| !field(f)))
        {
            return Err(denied());
        }
        for (name, value) in &self.equalities {
            if query.filters.iter().any(|f| {
                f.field == *name
                    && f.operator == FilterOperator::Eq
                    && f.value == Value::String(value.clone())
            }) {
                continue;
            }
            query.filters.push(Filter {
                field: name.clone(),
                operator: FilterOperator::Eq,
                value: Value::String(value.clone()),
            });
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "sqlx"))]
mod tests {
    use super::*;
    use crate::datasource::{
        query::{Aggregation, AggregationType, QueryType},
        DataSourceClient,
    };
    #[tokio::test]
    async fn scoped_sql_and_cache_cannot_escape_rows_relations_or_identifiers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.sqlite");
        let pool = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query("CREATE TABLE events(tenant TEXT, environment TEXT, user_id TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO events VALUES('a','prod','123'),('b','prod','123'),('b','prod','123'),('a','dev','123')").execute(&pool).await.unwrap();
        let cfg = serde_json::json!({"name":"shared","type":"sql","provider":"sqlite","database":"test","connection_string":path,"pool_size":1,"query_cache_ttl_secs":60});
        let mut clients = Vec::new();
        for tenant in ["a", "b"] {
            clients.push(
                DataSourceClient::new_scoped(
                    serde_json::from_value(cfg.clone()).unwrap(),
                    Some(DataAccessScope {
                        namespace: tenant.into(),
                        entity: "events".into(),
                        equalities: BTreeMap::from([
                            ("tenant".into(), tenant.into()),
                            ("environment".into(), "prod".into()),
                        ]),
                    }),
                )
                .await
                .unwrap(),
            );
        }
        let query = Query {
            query_type: QueryType::Count,
            entity: "events".into(),
            filters: vec![],
            time_window: None,
            aggregations: vec![Aggregation {
                agg_type: AggregationType::Count,
                field: None,
                output_name: "count".into(),
            }],
            group_by: vec![],
            limit: None,
        };
        for (i, client) in clients.iter().enumerate() {
            assert_eq!(
                client.query(query.clone()).await.unwrap().rows[0]["count"],
                Value::Number((i + 1) as f64)
            );
            assert!(client.query(query.clone()).await.unwrap().from_cache);
        }
        // An internal caller going directly to SQLClient must still be scoped.
        use crate::datasource::{
            client::DataSourceImpl,
            config::{DataSourceConfig, DataSourceType},
            sql::SQLClient,
        };
        let direct_config: DataSourceConfig = serde_json::from_value(cfg).unwrap();
        let DataSourceType::SQL(sql) = direct_config.source_type else {
            panic!()
        };
        let direct = SQLClient::new_with_scope(
            sql,
            1,
            Some(DataAccessScope {
                namespace: "direct-a".into(),
                entity: "events".into(),
                equalities: BTreeMap::from([
                    ("tenant".into(), "a".into()),
                    ("environment".into(), "prod".into()),
                ]),
            }),
        )
        .await
        .unwrap();
        assert_eq!(
            direct.execute(query.clone()).await.unwrap().rows[0]["count"],
            Value::Number(1.0)
        );
        let mut contradictory = query.clone();
        contradictory.filters.push(Filter {
            field: "tenant".into(),
            operator: FilterOperator::Eq,
            value: Value::String("b".into()),
        });
        assert_eq!(
            clients[0].query(contradictory).await.unwrap().rows[0]["count"],
            Value::Number(0.0)
        );
        for location in ["relation", "filter", "aggregate", "alias", "group", "time"] {
            let mut injected = query.clone();
            match location {
                "relation" => injected.entity = "events WHERE 1=1 --".into(),
                "filter" => injected.filters.push(Filter {
                    field: "user_id='123' OR 1=1 --".into(),
                    operator: FilterOperator::Eq,
                    value: Value::String("x".into()),
                }),
                "aggregate" => {
                    injected.aggregations[0].field = Some("(SELECT COUNT(*) FROM events)".into())
                }
                "alias" => injected.aggregations[0].output_name = "count FROM events --".into(),
                "group" => {
                    injected.group_by = vec!["user_id UNION SELECT tenant FROM events --".into()]
                }
                _ => {
                    injected.time_window = Some(crate::datasource::query::TimeWindow {
                        time_field: "occurred_at OR 1=1 --".into(),
                        window_type: crate::datasource::query::TimeWindowType::Absolute {
                            start: 0,
                            end: 1,
                        },
                    })
                }
            }
            assert!(
                clients[0]
                    .query(injected)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("E_RESOURCE_SCOPE"),
                "{location}"
            );
        }
        pool.close().await;
    }
}
