//! SQL Database Client Implementation
//!
//! Provides SQL database connectivity for PostgreSQL, SQLite, and MySQL.

use super::config::{SQLConfig, SQLProvider};
use super::query::{
    Aggregation, AggregationType, Filter, FilterOperator, Query, QueryResult, TimeWindowType,
};
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashMap;
use std::time::Instant;

#[cfg(feature = "sqlx")]
use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
#[cfg(feature = "sqlx")]
use sqlx::{Column, Row};

// Import the DataSourceImpl trait from client module
use super::client::DataSourceImpl;

/// SQL Database Client
pub(super) struct SQLClient {
    config: SQLConfig,
    #[cfg(feature = "sqlx")]
    pg_pool: Option<sqlx::PgPool>,
    #[cfg(feature = "sqlx")]
    sqlite_pool: Option<sqlx::SqlitePool>,
}

impl SQLClient {
    #[cfg_attr(not(feature = "sqlx"), allow(unused_variables))]
    pub(super) async fn new(config: SQLConfig, pool_size: u32) -> Result<Self> {
        tracing::info!("Initializing SQL client: {:?}", config.provider);

        #[cfg(feature = "sqlx")]
        {
            let mut pg_pool = None;
            let mut sqlite_pool = None;

            match config.provider {
                SQLProvider::PostgreSQL => {
                    use sqlx::postgres::PgPoolOptions;

                    tracing::info!("Creating PostgreSQL connection pool");
                    // Use provided pool_size or get from config options, default to 10
                    let effective_pool_size = config
                        .options
                        .get("max_connections")
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or_else(|| pool_size.max(1));

                    let pool = PgPoolOptions::new()
                        .max_connections(effective_pool_size)
                        .connect(&config.connection_string)
                        .await
                        .map_err(|e| {
                            RuntimeError::RuntimeError(format!(
                                "Failed to connect to PostgreSQL: {}",
                                e
                            ))
                        })?;

                    tracing::info!(
                        "✓ PostgreSQL connection pool created successfully (max_connections: {})",
                        effective_pool_size
                    );
                    pg_pool = Some(pool);
                }
                SQLProvider::SQLite => {
                    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
                    use std::str::FromStr;

                    tracing::info!("Creating SQLite connection pool");
                    // Use provided pool_size or get from config options, default to 1 (SQLite doesn't need large pools)
                    let effective_pool_size = config
                        .options
                        .get("max_connections")
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or_else(|| pool_size.max(1).min(10)); // SQLite typically uses smaller pools

                    // Parse connection string (SQLite uses file path or sqlite:// URI)
                    let connect_options = if config.connection_string.starts_with("sqlite://") {
                        SqliteConnectOptions::from_str(&config.connection_string).map_err(|e| {
                            RuntimeError::RuntimeError(format!(
                                "Invalid SQLite connection string: {}",
                                e
                            ))
                        })?
                    } else {
                        // Treat as file path
                        SqliteConnectOptions::new()
                            .filename(&config.connection_string)
                            .create_if_missing(true) // Create database file if it doesn't exist
                    };

                    let pool = SqlitePoolOptions::new()
                        .max_connections(effective_pool_size)
                        .connect_with(connect_options)
                        .await
                        .map_err(|e| {
                            RuntimeError::RuntimeError(format!("Failed to connect to SQLite: {}", e))
                        })?;

                    tracing::info!(
                        "✓ SQLite connection pool created successfully (max_connections: {})",
                        effective_pool_size
                    );
                    sqlite_pool = Some(pool);
                }
                SQLProvider::MySQL => {
                    // MySQL not yet implemented
                    tracing::warn!("MySQL provider not yet implemented");
                }
            }

            Ok(Self {
                config,
                pg_pool,
                sqlite_pool,
            })
        }

        #[cfg(not(feature = "sqlx"))]
        {
            Ok(Self { config })
        }
    }
}

#[async_trait::async_trait]
impl DataSourceImpl for SQLClient {
    async fn execute(&self, query: Query) -> Result<QueryResult> {
        tracing::debug!("Executing SQL query: {:?}", query.query_type);

        // Build SQL query
        let sql = self.build_sql(&query)?;
        tracing::debug!("Generated SQL: {}", sql);

        // Execute query based on provider
        match self.config.provider {
            SQLProvider::PostgreSQL => self.execute_postgresql(&sql).await,
            SQLProvider::SQLite => self.execute_sqlite(&sql).await,
            SQLProvider::MySQL => Err(RuntimeError::RuntimeError(
                "MySQL not yet implemented".to_string(),
            )),
        }
    }
}

impl SQLClient {
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
        sql.push_str(&format!(" FROM {}", query.entity));

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
                    match self.config.provider {
                        SQLProvider::SQLite => {
                            // SQLite uses datetime() function
                            sql.push_str(&format!(
                                "{} >= datetime('now', '-{} seconds')",
                                time_window.time_field, seconds
                            ));
                        }
                        _ => {
                            // PostgreSQL and others use INTERVAL
                            sql.push_str(&format!(
                                "{} >= NOW() - INTERVAL '{} seconds'",
                                time_window.time_field, seconds
                            ));
                        }
                    }
                }
                TimeWindowType::Absolute { start, end } => {
                    match self.config.provider {
                        SQLProvider::SQLite => {
                            // SQLite uses datetime() function with unix timestamp
                            sql.push_str(&format!(
                                "{} >= datetime({}, 'unixepoch') AND {} < datetime({}, 'unixepoch')",
                                time_window.time_field, start, time_window.time_field, end
                            ));
                        }
                        _ => {
                            // PostgreSQL uses TO_TIMESTAMP
                            sql.push_str(&format!(
                                "{} >= TO_TIMESTAMP({}) AND {} < TO_TIMESTAMP({})",
                                time_window.time_field, start, time_window.time_field, end
                            ));
                        }
                    }
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

        // For PostgreSQL/SQLite, if field contains JSON access, wrap it with type cast for numeric aggregations
        let needs_numeric_cast = matches!(
            agg.agg_type,
            AggregationType::Sum
                | AggregationType::Avg
                | AggregationType::Min
                | AggregationType::Max
                | AggregationType::Stddev
                | AggregationType::Percentile { .. }
        ) && (field.contains("->>")
            || field == "amount"
            || field.starts_with("attributes"));

        // Build field expression with type cast if needed
        // Field path syntax:
        // - Direct column: "amount" -> "amount"
        // - JSON field with dot notation: "attributes.amount" -> "attributes->>'amount'" (PostgreSQL) or "json_extract(attributes, '$.amount')" (SQLite)
        // - JSON field with explicit syntax: "attributes->>'amount'" -> keep as is (PostgreSQL) or convert to SQLite
        // - Nested JSON: "attributes.device.fingerprint" -> "attributes->'device'->>'fingerprint'" (PostgreSQL) or "json_extract(attributes, '$.device.fingerprint')" (SQLite)
        let field_expr = if needs_numeric_cast {
            match self.config.provider {
                SQLProvider::PostgreSQL => {
                    if field.contains("->>") || field.contains("->") {
                        // Already has JSONB access syntax, just add cast
                        format!("({})::numeric", field)
                    } else if field.contains('.') && !field.starts_with("${") {
                        // Dot notation for JSON fields: "attributes.amount" -> "attributes->>'amount'"
                        // Nested: "attributes.device.fingerprint" -> "attributes->'device'->>'fingerprint'"
                        let parts: Vec<&str> = field.split('.').collect();
                        if parts.len() == 2 {
                            // Simple: attributes.amount
                            format!("(attributes->>'{}')::numeric", parts[1])
                        } else if parts.len() > 2 {
                            // Nested: attributes.device.fingerprint
                            let mut expr = parts[0].to_string();
                            for (idx, part) in parts.iter().enumerate().skip(1) {
                                if idx == parts.len() - 1 {
                                    // Last part uses ->> for text extraction
                                    expr = format!("{}->>'{}'", expr, part);
                                } else {
                                    // Intermediate parts use -> for JSON object access
                                    expr = format!("{}->'{}'", expr, part);
                                }
                            }
                            format!("({})::numeric", expr)
                        } else {
                            field.to_string()
                        }
                    } else {
                        // Plain field name - assume direct column (no JSON access)
                        format!("CAST({} AS numeric)", field)
                    }
                }
                SQLProvider::SQLite => {
                    if field.contains("->>") || field.contains("->") {
                        // Convert PostgreSQL JSONB syntax to SQLite
                        // Example: "attributes->>'amount'" -> "json_extract(attributes, '$.amount')"
                        let parts: Vec<&str> = field.split("->>").collect();
                        if parts.len() == 2 {
                            let json_field = parts[0].trim();
                            let json_key = parts[1].trim_matches('"').trim_matches('\'');
                            format!(
                                "CAST(json_extract({}, '$.{}') AS REAL)",
                                json_field, json_key
                            )
                        } else {
                            // Handle -> syntax for nested JSON
                            let parts: Vec<&str> = field.split("->").collect();
                            if parts.len() > 1 {
                                // Build JSON path
                                let json_field = parts[0].trim();
                                let mut json_path = String::new();
                                for (idx, part) in parts.iter().enumerate().skip(1) {
                                    if idx > 1 {
                                        json_path.push('.');
                                    }
                                    json_path.push_str(part.trim_matches('"').trim_matches('\'').trim());
                                }
                                format!(
                                    "CAST(json_extract({}, '$.{}') AS REAL)",
                                    json_field, json_path
                                )
                            } else {
                                field.to_string()
                            }
                        }
                    } else if field.contains('.') && !field.starts_with("${") {
                        // Dot notation for JSON fields: "attributes.amount" -> "json_extract(attributes, '$.amount')"
                        // Nested: "attributes.device.fingerprint" -> "json_extract(attributes, '$.device.fingerprint')"
                        let parts: Vec<&str> = field.split('.').collect();
                        if parts.len() >= 2 {
                            let json_field = parts[0];
                            let json_path = parts[1..].join(".");
                            format!("CAST(json_extract({}, '$.{}') AS REAL)", json_field, json_path)
                        } else {
                            field.to_string()
                        }
                    } else {
                        // Plain field name - assume direct column (no JSON access)
                        format!("CAST({} AS REAL)", field)
                    }
                }
                _ => field.to_string(),
            }
        } else {
            // No numeric cast needed, but still handle field path syntax
            match self.config.provider {
                SQLProvider::PostgreSQL => {
                    if field.contains("->>") || field.contains("->") {
                        // Already has JSONB access syntax
                        field.to_string()
                    } else if field.contains('.') && !field.starts_with("${") {
                        // Dot notation for JSON fields
                        let parts: Vec<&str> = field.split('.').collect();
                        if parts.len() == 2 {
                            format!("attributes->>'{}'", parts[1])
                        } else if parts.len() > 2 {
                            let mut expr = parts[0].to_string();
                            for (idx, part) in parts.iter().enumerate().skip(1) {
                                if idx == parts.len() - 1 {
                                    expr = format!("{}->>'{}'", expr, part);
                                } else {
                                    expr = format!("{}->'{}'", expr, part);
                                }
                            }
                            expr
                        } else {
                            field.to_string()
                        }
                    } else {
                        // Direct column
                        field.to_string()
                    }
                }
                SQLProvider::SQLite => {
                    if field.contains("->>") || field.contains("->") {
                        // Convert PostgreSQL syntax to SQLite
                        let parts: Vec<&str> = field.split("->>").collect();
                        if parts.len() == 2 {
                            let json_field = parts[0].trim();
                            let json_key = parts[1].trim_matches('"').trim_matches('\'');
                            format!("json_extract({}, '$.{}')", json_field, json_key)
                        } else {
                            field.to_string()
                        }
                    } else if field.contains('.') && !field.starts_with("${") {
                        // Dot notation: convert to json_extract
                        let parts: Vec<&str> = field.split('.').collect();
                        if parts.len() >= 2 {
                            let json_field = parts[0];
                            let json_path = parts[1..].join(".");
                            format!("json_extract({}, '$.{}')", json_field, json_path)
                        } else {
                            field.to_string()
                        }
                    } else {
                        // Direct column
                        field.to_string()
                    }
                }
                _ => field.to_string(),
            }
        };

        let expr = match agg.agg_type {
            AggregationType::Count => format!("COUNT({})", field_expr),
            AggregationType::CountDistinct => {
                format!("COUNT(DISTINCT {})", field_expr)
            }
            AggregationType::Sum => format!("SUM({})", field_expr),
            AggregationType::Avg => format!("AVG({})", field_expr),
            AggregationType::Min => format!("MIN({})", field_expr),
            AggregationType::Max => format!("MAX({})", field_expr),
            AggregationType::Median => {
                match self.config.provider {
                    SQLProvider::SQLite => {
                        // SQLite doesn't have PERCENTILE_CONT, use a workaround
                        format!(
                            "(SELECT {} FROM (SELECT {} FROM (SELECT {} ORDER BY {}) LIMIT 1 OFFSET (SELECT COUNT(*) / 2 FROM (SELECT {})))",
                            field_expr, field_expr, field_expr, field_expr, field_expr
                        )
                    }
                    _ => {
                        format!(
                            "PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY {})",
                            field_expr
                        )
                    }
                }
            }
            AggregationType::Stddev => match self.config.provider {
                SQLProvider::SQLite => format!("STDEV({})", field_expr),
                _ => format!("STDDEV_POP({})", field_expr),
            },
            AggregationType::Percentile { p } => {
                match self.config.provider {
                    SQLProvider::SQLite => {
                        // SQLite doesn't have PERCENTILE_CONT, use a workaround
                        let percentile = p as f64 / 100.0;
                        format!(
                            "(SELECT {} FROM (SELECT {} FROM (SELECT {} ORDER BY {}) LIMIT 1 OFFSET (SELECT CAST(COUNT(*) * {} AS INTEGER) FROM (SELECT {})))",
                            field_expr, field_expr, field_expr, field_expr, percentile, field_expr
                        )
                    }
                    _ => {
                        format!(
                            "PERCENTILE_CONT({}) WITHIN GROUP (ORDER BY {})",
                            p as f64 / 100.0,
                            field_expr
                        )
                    }
                }
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
                format!("{} ~ {}", filter.field, value_str) // PostgreSQL regex operator
            }
        };

        Ok(expr)
    }

    /// Format value for SQL
    fn format_value(&self, value: &Value) -> Result<String> {
        match value {
            Value::Null => Ok("NULL".to_string()),
            Value::Bool(b) => Ok(b.to_string().to_uppercase()),
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

    /// Execute query on PostgreSQL
    async fn execute_postgresql(&self, sql: &str) -> Result<QueryResult> {
        tracing::info!("Executing PostgreSQL query: {}", sql);

        #[cfg(feature = "sqlx")]
        {
            let pool = self.pg_pool.as_ref().ok_or_else(|| {
                RuntimeError::RuntimeError(
                    "PostgreSQL connection pool not available. Enable 'sqlx' feature.".to_string(),
                )
            })?;

            let start = Instant::now();

            // Execute query
            let rows = sqlx::query(sql).fetch_all(pool).await.map_err(|e| {
                RuntimeError::RuntimeError(format!("Failed to execute PostgreSQL query: {}", e))
            })?;

            let execution_time_ms = start.elapsed().as_millis() as u64;

            // Convert rows to QueryResult format
            let mut result_rows = Vec::new();

            for row in rows {
                let mut map = HashMap::new();

                // Get column names and values
                for (idx, column) in row.columns().iter().enumerate() {
                    let column_name = column.name().to_string();

                    tracing::debug!("Column {}: name={}", idx, column_name);

                    // Try to get value based on type
                    // PostgreSQL aggregate functions (SUM, AVG, MAX, MIN) return NULL when no rows match
                    // PostgreSQL numeric type needs special handling - use BigDecimal for numeric type
                    let value = {
                        #[cfg(feature = "sqlx")]
                        {
                            // Try BigDecimal first for PostgreSQL numeric type
                            if let Ok(v) = row.try_get::<Option<bigdecimal::BigDecimal>, _>(idx) {
                                if let Some(bd) = v {
                                    // Convert BigDecimal to f64
                                    if let Ok(num) = bd.to_string().parse::<f64>() {
                                        Value::Number(num)
                                    } else {
                                        Value::Null
                                    }
                                } else {
                                    Value::Null
                                }
                            } else if let Ok(v) = row.try_get::<bigdecimal::BigDecimal, _>(idx) {
                                // Try non-nullable BigDecimal
                                if let Ok(num) = v.to_string().parse::<f64>() {
                                    Value::Number(num)
                                } else {
                                    Value::Null
                                }
                            } else if let Ok(v) = row.try_get::<Option<DateTime<Utc>>, _>(idx) {
                                v.map(|dt| Value::String(dt.to_rfc3339())).unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<DateTime<Utc>, _>(idx) {
                                Value::String(v.to_rfc3339())
                            } else if let Ok(v) = row.try_get::<Option<DateTime<FixedOffset>>, _>(idx) {
                                v.map(|dt| Value::String(dt.with_timezone(&Utc).to_rfc3339()))
                                    .unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<DateTime<FixedOffset>, _>(idx) {
                                Value::String(v.with_timezone(&Utc).to_rfc3339())
                            } else if let Ok(v) = row.try_get::<Option<NaiveDateTime>, _>(idx) {
                                v.map(|dt| {
                                    Value::String(
                                        DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)
                                            .to_rfc3339(),
                                    )
                                })
                                .unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<NaiveDateTime, _>(idx) {
                                Value::String(
                                    DateTime::<Utc>::from_naive_utc_and_offset(v, Utc)
                                        .to_rfc3339(),
                                )
                            } else if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
                                // Try String for numeric type (PostgreSQL numeric can be read as String)
                                if let Some(s) = v {
                                    if let Ok(num) = s.parse::<f64>() {
                                        Value::Number(num)
                                    } else {
                                        Value::String(s)
                                    }
                                } else {
                                    Value::Null
                                }
                            } else if let Ok(v) = row.try_get::<String, _>(idx) {
                                // Try non-nullable String
                                if let Ok(num) = v.parse::<f64>() {
                                    Value::Number(num)
                                } else {
                                    Value::String(v)
                                }
                            } else if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
                                // Try Option<f64> for numeric aggregates
                                v.map(Value::Number).unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<f64, _>(idx) {
                                Value::Number(v)
                            } else if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
                                v.map(|n| Value::Number(n as f64)).unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<i64, _>(idx) {
                                Value::Number(v as f64)
                            } else if let Ok(v) = row.try_get::<Option<i32>, _>(idx) {
                                v.map(|n| Value::Number(n as f64)).unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<i32, _>(idx) {
                                Value::Number(v as f64)
                            } else if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
                                v.map(Value::Bool).unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<bool, _>(idx) {
                                Value::Bool(v)
                            } else {
                                // Last resort: try to get as text and parse
                                if let Ok(v) = row.try_get::<String, _>(idx) {
                                    if let Ok(num) = v.parse::<f64>() {
                                        Value::Number(num)
                                    } else {
                                        Value::String(v)
                                    }
                                } else {
                                    tracing::warn!("Failed to extract value for column {} (name: {}). Tried: BigDecimal, Option<BigDecimal>, String, Option<String>, f64, Option<f64>, i64, Option<i64>, i32, Option<i32>, bool, Option<bool>", idx, column_name);
                                    Value::Null
                                }
                            }
                        }
                        #[cfg(not(feature = "sqlx"))]
                        {
                            Value::Null
                        }
                    };

                    tracing::debug!("Extracted value for {}: {:?}", column_name, value);
                    map.insert(column_name, value);
                }

                result_rows.push(map);
            }

            Ok(QueryResult {
                rows: result_rows,
                execution_time_ms,
                source: self.config.database.clone(),
                from_cache: false,
            })
        }

        #[cfg(not(feature = "sqlx"))]
        {
            Err(RuntimeError::RuntimeError(
                "PostgreSQL queries require 'sqlx' feature to be enabled".to_string(),
            ))
        }
    }

    /// Execute query on SQLite
    async fn execute_sqlite(&self, sql: &str) -> Result<QueryResult> {
        tracing::info!("Executing SQLite query: {}", sql);

        #[cfg(feature = "sqlx")]
        {
            let pool = self.sqlite_pool.as_ref().ok_or_else(|| {
                RuntimeError::RuntimeError(
                    "SQLite connection pool not available. Enable 'sqlx' feature.".to_string(),
                )
            })?;

            let start = Instant::now();

            // Execute query
            let rows = sqlx::query(sql).fetch_all(pool).await.map_err(|e| {
                RuntimeError::RuntimeError(format!("Failed to execute SQLite query: {}", e))
            })?;

            let execution_time_ms = start.elapsed().as_millis() as u64;

            // Convert rows to QueryResult format
            let mut result_rows = Vec::new();

            for row in rows {
                let mut map = HashMap::new();

                // Get column names and values
                for (idx, column) in row.columns().iter().enumerate() {
                    let column_name = column.name().to_string();

                    tracing::debug!("Column {}: name={}", idx, column_name);

                    // Try to get value based on type
                    // SQLite is more flexible with types, but we'll try common types
                    let value = {
                        #[cfg(feature = "sqlx")]
                        {
                            // Try different types in order of likelihood
                            if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
                                v.map(Value::Number).unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<f64, _>(idx) {
                                Value::Number(v)
                            } else if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
                                v.map(|n| Value::Number(n as f64)).unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<i64, _>(idx) {
                                Value::Number(v as f64)
                            } else if let Ok(v) = row.try_get::<Option<i32>, _>(idx) {
                                v.map(|n| Value::Number(n as f64)).unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<i32, _>(idx) {
                                Value::Number(v as f64)
                            } else if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
                                if let Some(s) = v {
                                    // Try to parse as number if possible
                                    if let Ok(num) = s.parse::<f64>() {
                                        Value::Number(num)
                                    } else {
                                        Value::String(s)
                                    }
                                } else {
                                    Value::Null
                                }
                            } else if let Ok(v) = row.try_get::<String, _>(idx) {
                                // Try to parse as number if possible
                                if let Ok(num) = v.parse::<f64>() {
                                    Value::Number(num)
                                } else {
                                    Value::String(v)
                                }
                            } else if let Ok(v) = row.try_get::<Option<bool>, _>(idx) {
                                v.map(Value::Bool).unwrap_or(Value::Null)
                            } else if let Ok(v) = row.try_get::<bool, _>(idx) {
                                Value::Bool(v)
                            } else {
                                // Last resort: try to get as text
                                if let Ok(v) = row.try_get::<String, _>(idx) {
                                    if let Ok(num) = v.parse::<f64>() {
                                        Value::Number(num)
                                    } else {
                                        Value::String(v)
                                    }
                                } else {
                                    tracing::warn!(
                                        "Failed to extract value for column {} (name: {})",
                                        idx,
                                        column_name
                                    );
                                    Value::Null
                                }
                            }
                        }
                        #[cfg(not(feature = "sqlx"))]
                        {
                            Value::Null
                        }
                    };

                    tracing::debug!("Extracted value for {}: {:?}", column_name, value);
                    map.insert(column_name, value);
                }

                result_rows.push(map);
            }

            Ok(QueryResult {
                rows: result_rows,
                execution_time_ms,
                source: self.config.database.clone(),
                from_cache: false,
            })
        }

        #[cfg(not(feature = "sqlx"))]
        {
            Err(RuntimeError::RuntimeError(
                "SQLite queries require 'sqlx' feature to be enabled".to_string(),
            ))
        }
    }
}
