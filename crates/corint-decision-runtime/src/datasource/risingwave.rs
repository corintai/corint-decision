//! Read precomputed Feature values through RisingWave's PostgreSQL wire protocol.

use super::client::{DataSourceImpl, FeatureStoreOps};
use super::config::{FeatureLookupKeyType, FeatureLookupMapping, FeatureStoreConfig};
use super::query::{Query, QueryResult};
use crate::error::{Result, RuntimeError};
use corint_decision_model::Value;
use std::collections::HashMap;

pub(super) struct RisingWaveClient {
    mappings: HashMap<String, FeatureLookupMapping>,
    #[cfg(feature = "sqlx")]
    pool: sqlx::PgPool,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidOperation(message.into())
}

/// Names are separate identifiers, never SQL expressions or dotted paths.
fn identifier(name: &str) -> Result<String> {
    if name.is_empty() || name.contains('\0') {
        return Err(invalid(
            "RisingWave identifiers must be nonempty and contain no NUL",
        ));
    }
    Ok(format!("\"{}\"", name.replace('"', "\"\"")))
}

impl FeatureLookupMapping {
    fn sql(&self) -> Result<String> {
        Ok(format!(
            "SELECT {} FROM {}.{} WHERE {} = $1 LIMIT 2",
            identifier(&self.value_column)?,
            identifier(&self.schema)?,
            identifier(&self.view)?,
            identifier(&self.key_column)?,
        ))
    }

    fn validate_key(&self, key: &str) -> Result<()> {
        if matches!(self.key_type, FeatureLookupKeyType::Int64) {
            key.parse::<i64>()
                .map_err(|_| invalid("RisingWave entity key must be an int64"))?;
        }
        Ok(())
    }
}

impl RisingWaveClient {
    pub(super) fn new(
        config: &FeatureStoreConfig,
        pool_size: u32,
        timeout_ms: u64,
    ) -> Result<Self> {
        if config.feature_mappings.is_empty() {
            return Err(invalid("RisingWave requires nonempty feature_mappings"));
        }
        for (name, mapping) in &config.feature_mappings {
            if name.trim().is_empty() {
                return Err(invalid("RisingWave Feature names must be nonempty"));
            }
            mapping.sql()?;
        }
        if pool_size == 0 || timeout_ms == 0 {
            return Err(invalid("RisingWave pool size and timeout must be positive"));
        }
        #[cfg(feature = "sqlx")]
        {
            // Lazy connection establishment keeps service outages inside the query/fallback
            // boundary. Invalid URLs and mappings remain configuration errors.
            let options: sqlx::postgres::PgConnectOptions = config
                .connection_string
                .parse()
                .map_err(|_| invalid("Invalid RisingWave PostgreSQL connection string"))?;
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(pool_size)
                .acquire_timeout(std::time::Duration::from_millis(timeout_ms))
                .connect_lazy_with(options);
            Ok(Self {
                mappings: config.feature_mappings.clone(),
                pool,
            })
        }
        #[cfg(not(feature = "sqlx"))]
        Err(invalid(
            "RisingWave lookup requires the 'sqlx' build feature",
        ))
    }

    fn mapping(&self, feature: &str) -> Result<&FeatureLookupMapping> {
        self.mappings.get(feature).ok_or_else(|| {
            invalid(format!(
                "Missing RisingWave feature mapping for '{feature}'"
            ))
        })
    }
}

#[async_trait::async_trait]
impl DataSourceImpl for RisingWaveClient {
    async fn execute(&self, _query: Query) -> Result<QueryResult> {
        Err(invalid(
            "RisingWave Feature Store only supports named Feature lookups",
        ))
    }

    fn as_feature_store(&self) -> Option<&dyn FeatureStoreOps> {
        Some(self)
    }
}

#[async_trait::async_trait]
impl FeatureStoreOps for RisingWaveClient {
    fn validate_feature(&self, name: &str) -> Result<()> {
        self.mapping(name).map(|_| ())
    }

    fn validate_key(&self, name: &str, key: &str) -> Result<()> {
        self.mapping(name)?.validate_key(key)
    }

    async fn get_feature(&self, feature: &str, key: &str) -> Result<Option<Value>> {
        let mapping = self.mapping(feature)?;
        mapping.validate_key(key)?;
        #[cfg(feature = "sqlx")]
        {
            let sql = mapping.sql()?;
            // RisingWave 2.8 can retain a prepared plan's old fragment ID after
            // a materialized view is recreated. Keep parameter binding, but
            // prepare afresh so a healthy replacement is visible immediately.
            let query = sqlx::query(&sql).persistent(false);
            let query = match mapping.key_type {
                FeatureLookupKeyType::Text => query.bind(key),
                FeatureLookupKeyType::Int64 => query.bind(
                    key.parse::<i64>()
                        .map_err(|_| invalid("RisingWave entity key must be an int64"))?,
                ),
            };
            let rows = query
                .fetch_all(&self.pool)
                .await
                .map_err(|e| invalid(format!("RisingWave lookup failed: {e}")))?;
            // Never silently pick a row from an ambiguous binding.
            if rows.len() > 1 {
                return Err(invalid(
                    "RisingWave lookup returned multiple rows for one entity key",
                ));
            }
            rows.first().map(decode_value).transpose()
        }
        #[cfg(not(feature = "sqlx"))]
        Err(invalid(
            "RisingWave lookup requires the 'sqlx' build feature",
        ))
    }
}

#[cfg(feature = "sqlx")]
fn decode_value(row: &sqlx::postgres::PgRow) -> Result<Value> {
    use sqlx::{Column, Row, TypeInfo, ValueRef};
    let decode_error = |e| invalid(format!("RisingWave value decoding failed: {e}"));
    if row.try_get_raw(0).map_err(decode_error)?.is_null() {
        return Ok(Value::Null);
    }
    let value = match row.column(0).type_info().name() {
        "BOOL" => Value::Bool(row.try_get(0).map_err(decode_error)?),
        "INT2" => Value::Number(row.try_get::<i16, _>(0).map_err(decode_error)? as f64),
        "INT4" => Value::Number(row.try_get::<i32, _>(0).map_err(decode_error)? as f64),
        "INT8" => Value::Number(row.try_get::<i64, _>(0).map_err(decode_error)? as f64),
        "FLOAT4" => Value::Number(row.try_get::<f32, _>(0).map_err(decode_error)? as f64),
        "FLOAT8" => Value::Number(row.try_get(0).map_err(decode_error)?),
        "NUMERIC" => {
            let decimal: bigdecimal::BigDecimal = row.try_get(0).map_err(decode_error)?;
            Value::Number(
                decimal
                    .to_string()
                    .parse()
                    .map_err(|_| invalid("RisingWave numeric conversion failed"))?,
            )
        }
        "TEXT" | "VARCHAR" | "BPCHAR" => Value::String(row.try_get(0).map_err(decode_error)?),
        "JSON" | "JSONB" => {
            let json: sqlx::types::Json<serde_json::Value> =
                row.try_get(0).map_err(decode_error)?;
            serde_json::from_value(json.0)
                .map_err(|e| invalid(format!("RisingWave JSON conversion failed: {e}")))?
        }
        "TIMESTAMPTZ" => {
            let time: chrono::DateTime<chrono::Utc> = row.try_get(0).map_err(decode_error)?;
            Value::String(time.to_rfc3339())
        }
        "TIMESTAMP" => {
            let time: chrono::NaiveDateTime = row.try_get(0).map_err(decode_error)?;
            Value::String(time.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
        }
        "DATE" => {
            let date: chrono::NaiveDate = row.try_get(0).map_err(decode_error)?;
            Value::String(date.to_string())
        }
        other => {
            return Err(invalid(format!(
                "Unsupported RisingWave value type: {other}; expose JSONB for structured values"
            )))
        }
    };
    if matches!(value, Value::Number(n) if !n.is_finite()) {
        return Err(invalid("RisingWave value must be a finite number"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(feature = "sqlx"))]
    fn missing_sqlx_is_an_explicit_configuration_error() {
        let config: FeatureStoreConfig = serde_yaml::from_str("provider: risingwave\nconnection_string: postgresql://root@localhost:4566/dev\nfeature_mappings:\n  count:\n    view: counts\n    key_column: user_id\n    value_column: count\n").unwrap();
        assert!(RisingWaveClient::new(&config, 1, 100)
            .err()
            .unwrap()
            .to_string()
            .contains("sqlx"));
    }

    #[test]
    fn identifiers_are_quoted_and_values_are_bound() {
        let mapping = FeatureLookupMapping {
            schema: "public".into(),
            view: "v\"; DROP TABLE events;--".into(),
            key_column: "user_id".into(),
            value_column: "count".into(),
            key_type: FeatureLookupKeyType::Text,
        };
        assert_eq!(mapping.sql().unwrap(), "SELECT \"count\" FROM \"public\".\"v\"\"; DROP TABLE events;--\" WHERE \"user_id\" = $1 LIMIT 2");
        assert!(identifier("").is_err());
        assert!(identifier("bad\0name").is_err());
        let numeric = FeatureLookupMapping {
            key_type: FeatureLookupKeyType::Int64,
            ..mapping
        };
        assert!(numeric.validate_key("9223372036854775807").is_ok());
        assert!(numeric.validate_key("9223372036854775808").is_err());
        assert!(numeric.validate_key("1 OR 1=1").is_err());
    }
}
