//! OLAP Database Client Implementation
//!
//! Provides OLAP database connectivity for ClickHouse, Druid, and other analytical databases.

use super::config::{OLAPConfig, OLAPProvider};
use super::query::{
    Aggregation, AggregationType, Filter, FilterOperator, Query, QueryResult, TimeWindowType,
};
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashMap;

// Import the DataSourceImpl trait from client module
use super::client::DataSourceImpl;

/// OLAP Database Client
pub(super) struct OLAPClient {
    config: OLAPConfig,
    #[cfg(feature = "clickhouse")]
    http_client: reqwest::Client,
}

impl OLAPClient {
    pub(super) async fn new(config: OLAPConfig) -> Result<Self> {
        tracing::info!("Initializing OLAP client: {:?}", config.provider);

        #[cfg(feature = "clickhouse")]
        {
            // Initialize HTTP client for ClickHouse with optimized settings
            // - Connection timeout: 5 seconds (faster failure on connection issues)
            // - Request timeout: 30 seconds (already set)
            // - TCP nodelay: enabled (reduce latency)
            // - Connection pool: default (connection reuse)
            let http_client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(30))
                .tcp_nodelay(true)
                .build()
                .map_err(|e| {
                    RuntimeError::RuntimeError(format!("Failed to create HTTP client: {}", e))
                })?;

            Ok(Self {
                config,
                http_client,
            })
        }

        #[cfg(not(feature = "clickhouse"))]
        {
            Ok(Self { config })
        }
    }
}

#[async_trait::async_trait]
impl DataSourceImpl for OLAPClient {
    async fn execute(&self, query: Query) -> Result<QueryResult> {
        tracing::debug!("Executing OLAP query: {:?}", query.query_type);

        // Build SQL query based on query type
        let sql = self.build_sql(&query)?;
        tracing::debug!("Generated SQL: {}", sql);

        // Execute query based on provider
        match self.config.provider {
            OLAPProvider::ClickHouse => self.execute_clickhouse(&sql).await,
            OLAPProvider::Druid => Err(RuntimeError::RuntimeError(
                "Druid not yet implemented".to_string(),
            )),
            OLAPProvider::TimescaleDB => Err(RuntimeError::RuntimeError(
                "TimescaleDB not yet implemented".to_string(),
            )),
            OLAPProvider::InfluxDB => Err(RuntimeError::RuntimeError(
                "InfluxDB not yet implemented".to_string(),
            )),
        }
    }
}

impl OLAPClient {
    /// Build SQL query from Query struct
    fn build_sql(&self, query: &Query) -> Result<String> {
        let mut sql = String::new();

        // Build SELECT clause
        sql.push_str("SELECT ");
        if query.aggregations.is_empty() {
            sql.push('*');
        } else {
            let agg_clauses: Vec<String> = query
                .aggregations
                .iter()
                .map(|agg| self.build_aggregation(agg))
                .collect();
            sql.push_str(&agg_clauses.join(", "));
        }

        // FROM clause
        // Use query.entity if provided, otherwise fall back to configured events_table
        // Format: database.table_name (e.g., risk_events.user_events)
        let table_name = if query.entity.is_empty() {
            // Use configured table name when entity is not specified
            format!("{}.{}", self.config.database, self.config.events_table)
        } else {
            // Use query.entity if specified
            // Prefix with database if it doesn't already contain a dot
            if query.entity.contains('.') {
                query.entity.clone()
            } else {
                format!("{}.{}", self.config.database, query.entity)
            }
        };
        sql.push_str(&format!(" FROM {}", table_name));

        // WHERE clause
        if !query.filters.is_empty() {
            sql.push_str(" WHERE ");
            let filter_clauses: Vec<String> = query
                .filters
                .iter()
                .map(|f| self.build_filter(f))
                .collect::<Result<Vec<_>>>()?;
            sql.push_str(&filter_clauses.join(" AND "));
        }

        // Time window
        if let Some(ref time_window) = query.time_window {
            if !query.filters.is_empty() {
                sql.push_str(" AND ");
            } else {
                sql.push_str(" WHERE ");
            }

            match &time_window.window_type {
                TimeWindowType::Relative(rel) => {
                    let seconds = rel.to_seconds();
                    sql.push_str(&format!(
                        "{} >= now() - INTERVAL {} SECOND",
                        time_window.time_field, seconds
                    ));
                }
                TimeWindowType::Absolute { start, end } => {
                    sql.push_str(&format!(
                        "{} >= toDateTime({}) AND {} < toDateTime({})",
                        time_window.time_field, start, time_window.time_field, end
                    ));
                }
            }
        }

        // GROUP BY clause
        if !query.group_by.is_empty() {
            sql.push_str(&format!(" GROUP BY {}", query.group_by.join(", ")));
        }

        // LIMIT clause
        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        Ok(sql)
    }

    /// Build aggregation clause
    fn build_aggregation(&self, agg: &Aggregation) -> String {
        let field = agg.field.as_deref().unwrap_or("*");

        let expr = match agg.agg_type {
            AggregationType::Count => format!("COUNT({})", field),
            AggregationType::CountDistinct => format!("COUNT(DISTINCT {})", field),
            AggregationType::Sum => format!("SUM({})", field),
            AggregationType::Avg => format!("AVG({})", field),
            AggregationType::Min => format!("MIN({})", field),
            AggregationType::Max => format!("MAX({})", field),
            AggregationType::Median => format!("median({})", field), // ClickHouse function
            AggregationType::Stddev => format!("stddevPop({})", field), // ClickHouse function
            AggregationType::Percentile { p } => {
                format!("quantile({})({})", p as f64 / 100.0, field)
            }
        };

        format!("{} AS {}", expr, agg.output_name)
    }

    /// Build filter clause
    fn build_filter(&self, filter: &Filter) -> Result<String> {
        let value_str = self.format_value(&filter.value)?;

        let expr = match filter.operator {
            FilterOperator::Eq => format!("{} = {}", filter.field, value_str),
            FilterOperator::Ne => format!("{} != {}", filter.field, value_str),
            FilterOperator::Gt => format!("{} > {}", filter.field, value_str),
            FilterOperator::Ge => format!("{} >= {}", filter.field, value_str),
            FilterOperator::Lt => format!("{} < {}", filter.field, value_str),
            FilterOperator::Le => format!("{} <= {}", filter.field, value_str),
            FilterOperator::In => {
                if let Value::Array(ref arr) = filter.value {
                    let values: Vec<String> = arr
                        .iter()
                        .map(|v| self.format_value(v))
                        .collect::<Result<Vec<_>>>()?;
                    format!("{} IN ({})", filter.field, values.join(", "))
                } else {
                    return Err(RuntimeError::RuntimeError(
                        "IN operator requires array value".to_string(),
                    ));
                }
            }
            FilterOperator::NotIn => {
                if let Value::Array(ref arr) = filter.value {
                    let values: Vec<String> = arr
                        .iter()
                        .map(|v| self.format_value(v))
                        .collect::<Result<Vec<_>>>()?;
                    format!("{} NOT IN ({})", filter.field, values.join(", "))
                } else {
                    return Err(RuntimeError::RuntimeError(
                        "NOT IN operator requires array value".to_string(),
                    ));
                }
            }
            FilterOperator::Like => {
                format!("{} LIKE {}", filter.field, value_str)
            }
            FilterOperator::Regex => {
                format!("match({}, {})", filter.field, value_str) // ClickHouse regex function
            }
        };

        Ok(expr)
    }

    /// Format value for SQL
    fn format_value(&self, value: &Value) -> Result<String> {
        match value {
            Value::Null => Ok("NULL".to_string()),
            Value::Bool(b) => Ok(if *b { "1" } else { "0" }.to_string()),
            Value::Number(n) => Ok(n.to_string()),
            Value::String(s) => Ok(format!("'{}'", s.replace('\'', "''"))), // SQL escape
            Value::Array(_) => Err(RuntimeError::RuntimeError(
                "Arrays should be handled by IN/NOT IN operator".to_string(),
            )),
            Value::Object(_) => Err(RuntimeError::RuntimeError(
                "Objects cannot be used in filters".to_string(),
            )),
        }
    }

    /// Execute query on ClickHouse
    #[cfg(feature = "clickhouse")]
    async fn execute_clickhouse(&self, sql: &str) -> Result<QueryResult> {
        use std::time::Instant;

        tracing::debug!("Executing ClickHouse query: {}", sql);
        let start = Instant::now();

        // Build the request URL with query parameters
        let url = format!(
            "{}/?database={}&default_format=JSONEachRow",
            self.config.connection_string.trim_end_matches('/'),
            urlencoding::encode(&self.config.database)
        );

        // Execute the query via HTTP POST
        let http_start = Instant::now();
        let response = self
            .http_client
            .post(&url)
            .body(sql.to_string())
            .send()
            .await
            .map_err(|e| {
                tracing::error!("ClickHouse request failed: {}", e);
                RuntimeError::RuntimeError(format!("ClickHouse request failed: {}", e))
            })?;
        let http_elapsed = http_start.elapsed();

        tracing::debug!(
            "ClickHouse HTTP request completed in {}ms",
            http_elapsed.as_millis()
        );

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(RuntimeError::RuntimeError(format!(
                "ClickHouse query failed with status {}: {}",
                status, error_body
            )));
        }

        // Parse JSONEachRow response (one JSON object per line)
        let parse_start = Instant::now();
        let body = response.text().await.map_err(|e| {
            RuntimeError::RuntimeError(format!("Failed to read ClickHouse response: {}", e))
        })?;

        let rows = self.parse_json_each_row(&body)?;
        let parse_elapsed = parse_start.elapsed();
        let execution_time_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            "ClickHouse query completed: {} rows, {}ms total (http: {}ms, parse: {}ms)",
            rows.len(),
            execution_time_ms,
            http_elapsed.as_millis(),
            parse_elapsed.as_millis()
        );

        Ok(QueryResult {
            rows,
            execution_time_ms,
            source: self.config.database.clone(),
            from_cache: false,
        })
    }

    /// Execute query on ClickHouse (mock implementation when feature is disabled)
    #[cfg(not(feature = "clickhouse"))]
    async fn execute_clickhouse(&self, sql: &str) -> Result<QueryResult> {
        tracing::warn!(
            "ClickHouse feature is not enabled. Using mock implementation for query: {}",
            sql
        );

        // Mock implementation - return sample data
        let mut row = HashMap::new();

        if sql.contains("COUNT(DISTINCT") {
            row.insert("count_distinct".to_string(), Value::Number(15.0));
        } else if sql.contains("COUNT") {
            row.insert("count".to_string(), Value::Number(42.0));
        } else if sql.contains("SUM") {
            row.insert("sum".to_string(), Value::Number(1500.0));
        } else if sql.contains("AVG") {
            row.insert("avg".to_string(), Value::Number(75.5));
        } else if sql.contains("MAX") {
            row.insert("max".to_string(), Value::Number(200.0));
        } else if sql.contains("MIN") {
            row.insert("min".to_string(), Value::Number(10.0));
        }

        Ok(QueryResult {
            rows: vec![row],
            execution_time_ms: 25,
            source: self.config.database.clone(),
            from_cache: false,
        })
    }

    /// Parse JSONEachRow format response from ClickHouse
    #[cfg(feature = "clickhouse")]
    fn parse_json_each_row(&self, body: &str) -> Result<Vec<HashMap<String, Value>>> {
        let mut rows = Vec::new();

        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let json_value: serde_json::Value = serde_json::from_str(line).map_err(|e| {
                RuntimeError::RuntimeError(format!("Failed to parse ClickHouse JSON: {}", e))
            })?;

            if let serde_json::Value::Object(obj) = json_value {
                let mut row = HashMap::new();
                for (key, value) in obj {
                    row.insert(key, Self::json_to_value(value));
                }
                rows.push(row);
            }
        }

        Ok(rows)
    }

    /// Convert serde_json::Value to corint_core::Value
    #[cfg(feature = "clickhouse")]
    fn json_to_value(json: serde_json::Value) -> Value {
        match json {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => {
                // Try to get as f64, fallback to i64 conversion
                if let Some(f) = n.as_f64() {
                    Value::Number(f)
                } else if let Some(i) = n.as_i64() {
                    Value::Number(i as f64)
                } else if let Some(u) = n.as_u64() {
                    Value::Number(u as f64)
                } else {
                    Value::Null
                }
            }
            serde_json::Value::String(s) => {
                // Try to parse numeric strings as numbers
                if let Ok(num) = s.parse::<f64>() {
                    Value::Number(num)
                } else {
                    Value::String(s)
                }
            }
            serde_json::Value::Array(arr) => {
                Value::Array(arr.into_iter().map(Self::json_to_value).collect())
            }
            serde_json::Value::Object(obj) => Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, Self::json_to_value(v)))
                    .collect(),
            ),
        }
    }
}
