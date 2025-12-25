//! Unified Data Source Client
//!
//! Provides a unified interface for accessing different data sources.

use super::cache::FeatureCache;
use super::config::{DataSourceConfig, DataSourceType};
use super::query::{Query, QueryResult};
use crate::error::{Result, RuntimeError};
use corint_core::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(feature = "sqlx")]
use sqlx::{Column, Row};

/// Unified data source client
pub struct DataSourceClient {
    /// Data source configuration
    config: DataSourceConfig,

    /// Feature cache
    cache: Arc<Mutex<FeatureCache>>,

    /// Underlying client implementation
    client: Box<dyn DataSourceImpl>,
}

impl DataSourceClient {
    /// Create a new data source client
    pub async fn new(config: DataSourceConfig) -> Result<Self> {
        let client: Box<dyn DataSourceImpl> = match &config.source_type {
            DataSourceType::FeatureStore(fs_config) => {
                Box::new(FeatureStoreClient::new(fs_config.clone()).await?)
            }
            DataSourceType::OLAP(olap_config) => {
                Box::new(OLAPClient::new(olap_config.clone()).await?)
            }
            DataSourceType::SQL(sql_config) => {
                Box::new(SQLClient::new(sql_config.clone(), config.pool_size).await?)
            }
        };

        Ok(Self {
            config,
            cache: Arc::new(Mutex::new(FeatureCache::new())),
            client,
        })
    }

    /// Execute a query
    pub async fn query(&self, query: Query) -> Result<QueryResult> {
        // Check cache first
        let cache_key = self.generate_cache_key(&query);
        if let Some(cached_value) = self.cache.lock().unwrap().get(&cache_key) {
            tracing::debug!("Cache hit for key: {}", cache_key);
            return Ok(QueryResult {
                rows: vec![cached_value.clone()],
                execution_time_ms: 0,
                source: self.config.name.clone(),
                from_cache: true,
            });
        }

        // Execute query
        let start = Instant::now();
        let result = self.client.execute(query.clone()).await?;
        let execution_time_ms = start.elapsed().as_millis() as u64;

        // Cache result if applicable
        if !result.rows.is_empty() {
            if let Some(row) = result.rows.first() {
                self.cache.lock().unwrap().set(
                    cache_key,
                    row.clone(),
                    std::time::Duration::from_secs(300),
                );
            }
        }

        Ok(QueryResult {
            rows: result.rows,
            execution_time_ms,
            source: self.config.name.clone(),
            from_cache: false,
        })
    }

    /// Get a feature from feature store
    pub async fn get_feature(&self, feature_name: &str, entity_key: &str) -> Result<Option<Value>> {
        let cache_key = format!("feature:{}:{}", feature_name, entity_key);

        // Check cache
        if let Some(cached) = self.cache.lock().unwrap().get(&cache_key) {
            return Ok(cached.get("value").cloned());
        }

        // Get from feature store
        if let Some(fs_client) = self.client.as_feature_store() {
            let value = fs_client.get_feature(feature_name, entity_key).await?;

            // Cache the result
            if let Some(ref val) = value {
                let mut row = HashMap::new();
                row.insert("value".to_string(), val.clone());
                self.cache
                    .lock()
                    .unwrap()
                    .set(cache_key, row, std::time::Duration::from_secs(300));
            }

            Ok(value)
        } else {
            Err(RuntimeError::RuntimeError(
                "Data source is not a feature store".to_string(),
            ))
        }
    }

    /// Generate cache key for a query
    fn generate_cache_key(&self, query: &Query) -> String {
        // Include all query parameters in cache key to avoid collisions
        let mut key_parts = vec![
            self.config.name.clone(),
            query.entity.clone(),
            serde_json::to_string(&query.filters).unwrap_or_default(),
        ];

        // Include time window in cache key
        if let Some(ref time_window) = query.time_window {
            key_parts.push(format!(
                "window:{}:{}",
                time_window.time_field,
                serde_json::to_string(&time_window.window_type).unwrap_or_default()
            ));
        }

        // Include aggregations in cache key (different aggregations should have different cache keys)
        if !query.aggregations.is_empty() {
            key_parts.push(format!(
                "agg:{}",
                serde_json::to_string(&query.aggregations).unwrap_or_default()
            ));
        }

        // Include group_by in cache key
        if !query.group_by.is_empty() {
            key_parts.push(format!("group_by:{}", query.group_by.join(",")));
        }

        // Include limit in cache key
        if let Some(limit) = query.limit {
            key_parts.push(format!("limit:{}", limit));
        }

        format!("query:{}", key_parts.join(":"))
    }

    /// Get data source name
    pub fn name(&self) -> &str {
        &self.config.name
    }
}

/// Trait for data source implementations
#[async_trait::async_trait]
trait DataSourceImpl: Send + Sync {
    /// Execute a query
    async fn execute(&self, query: Query) -> Result<QueryResult>;

    /// Downcast to feature store client
    fn as_feature_store(&self) -> Option<&dyn FeatureStoreOps> {
        None
    }
}

/// Feature store operations
#[async_trait::async_trait]
trait FeatureStoreOps: Send + Sync {
    /// Get a feature value
    async fn get_feature(&self, feature_name: &str, entity_key: &str) -> Result<Option<Value>>;
}

/// Feature Store Client Implementation
struct FeatureStoreClient {
    config: super::config::FeatureStoreConfig,
    // In a real implementation, this would hold a connection pool
}

impl FeatureStoreClient {
    async fn new(config: super::config::FeatureStoreConfig) -> Result<Self> {
        // TODO: Initialize connection to feature store (Redis, Feast, etc.)
        tracing::info!("Initializing feature store client: {:?}", config.provider);
        Ok(Self { config })
    }

    /// Extract entity key from query filters
    fn extract_entity_key(&self, query: &Query) -> Result<String> {
        // Look for dimension filter that contains the entity key
        for filter in &query.filters {
            if filter.operator == super::query::FilterOperator::Eq {
                if let Value::String(ref key) = filter.value {
                    return Ok(key.clone());
                }
            }
        }

        Err(RuntimeError::RuntimeError(
            "Could not extract entity key from query filters".to_string(),
        ))
    }

    /// Get feature from Redis
    async fn get_redis_feature(
        &self,
        feature_name: &str,
        entity_key: &str,
    ) -> Result<Option<Value>> {
        // TODO: Use redis crate to fetch feature
        // For now, return mock data

        let redis_key = if self.config.namespace.is_empty() {
            format!("{}:{}", feature_name, entity_key)
        } else {
            format!("{}:{}:{}", self.config.namespace, feature_name, entity_key)
        };

        tracing::info!("Fetching Redis key: {}", redis_key);

        // Mock implementation - return sample data based on key pattern
        let value = if redis_key.contains("risk_score") {
            Some(Value::Number(75.5))
        } else if redis_key.contains("trust_score") {
            Some(Value::Number(85.0))
        } else if redis_key.contains("avg_transaction") {
            Some(Value::Number(250.0))
        } else if redis_key.contains("lifetime_value") {
            Some(Value::Number(5000.0))
        } else {
            None
        };

        Ok(value)
    }
}

#[async_trait::async_trait]
impl DataSourceImpl for FeatureStoreClient {
    async fn execute(&self, query: Query) -> Result<QueryResult> {
        tracing::debug!("Executing feature store query: {:?}", query.query_type);

        // Feature stores typically use key-value access, not SQL queries
        // For GetFeature queries, we extract the key from filters
        match query.query_type {
            super::query::QueryType::GetFeature => {
                // Extract entity key from filters
                let entity_key = self.extract_entity_key(&query)?;
                let feature_name = query.entity.as_str();

                // Get feature value
                let value = self.get_feature(feature_name, &entity_key).await?;

                // Convert to QueryResult format
                let row = if let Some(val) = value {
                    let mut map = HashMap::new();
                    map.insert("value".to_string(), val);
                    vec![map]
                } else {
                    vec![]
                };

                Ok(QueryResult {
                    rows: row,
                    execution_time_ms: 5, // Feature stores are typically very fast
                    source: self.config.namespace.clone(),
                    from_cache: false,
                })
            }
            _ => Err(RuntimeError::RuntimeError(
                "Feature stores only support GetFeature query type".to_string(),
            )),
        }
    }

    fn as_feature_store(&self) -> Option<&dyn FeatureStoreOps> {
        Some(self)
    }
}

#[async_trait::async_trait]
impl FeatureStoreOps for FeatureStoreClient {
    async fn get_feature(&self, feature_name: &str, entity_key: &str) -> Result<Option<Value>> {
        tracing::debug!("Getting feature {} for entity {}", feature_name, entity_key);

        match self.config.provider {
            super::config::FeatureStoreProvider::Redis => {
                self.get_redis_feature(feature_name, entity_key).await
            }
            super::config::FeatureStoreProvider::Feast => Err(RuntimeError::RuntimeError(
                "Feast not yet implemented".to_string(),
            )),
            super::config::FeatureStoreProvider::Http => Err(RuntimeError::RuntimeError(
                "HTTP feature store not yet implemented".to_string(),
            )),
        }
    }
}

/// OLAP Database Client Implementation (ClickHouse, Druid, etc.)
struct OLAPClient {
    config: super::config::OLAPConfig,
}

impl OLAPClient {
    async fn new(config: super::config::OLAPConfig) -> Result<Self> {
        tracing::info!("Initializing OLAP client: {:?}", config.provider);
        Ok(Self { config })
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
            super::config::OLAPProvider::ClickHouse => self.execute_clickhouse(&sql).await,
            super::config::OLAPProvider::Druid => Err(RuntimeError::RuntimeError(
                "Druid not yet implemented".to_string(),
            )),
            super::config::OLAPProvider::TimescaleDB => Err(RuntimeError::RuntimeError(
                "TimescaleDB not yet implemented".to_string(),
            )),
            super::config::OLAPProvider::InfluxDB => Err(RuntimeError::RuntimeError(
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
                super::query::TimeWindowType::Relative(rel) => {
                    let seconds = rel.to_seconds();
                    sql.push_str(&format!(
                        "{} >= now() - INTERVAL {} SECOND",
                        time_window.time_field, seconds
                    ));
                }
                super::query::TimeWindowType::Absolute { start, end } => {
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
    fn build_aggregation(&self, agg: &super::query::Aggregation) -> String {
        let field = agg.field.as_deref().unwrap_or("*");

        let expr = match agg.agg_type {
            super::query::AggregationType::Count => format!("COUNT({})", field),
            super::query::AggregationType::CountDistinct => format!("COUNT(DISTINCT {})", field),
            super::query::AggregationType::Sum => format!("SUM({})", field),
            super::query::AggregationType::Avg => format!("AVG({})", field),
            super::query::AggregationType::Min => format!("MIN({})", field),
            super::query::AggregationType::Max => format!("MAX({})", field),
            super::query::AggregationType::Median => format!("median({})", field), // ClickHouse function
            super::query::AggregationType::Stddev => format!("stddevPop({})", field), // ClickHouse function
            super::query::AggregationType::Percentile { p } => {
                format!("quantile({})({})", p as f64 / 100.0, field)
            }
        };

        format!("{} AS {}", expr, agg.output_name)
    }

    /// Build filter clause
    fn build_filter(&self, filter: &super::query::Filter) -> Result<String> {
        let value_str = self.format_value(&filter.value)?;

        let expr = match filter.operator {
            super::query::FilterOperator::Eq => format!("{} = {}", filter.field, value_str),
            super::query::FilterOperator::Ne => format!("{} != {}", filter.field, value_str),
            super::query::FilterOperator::Gt => format!("{} > {}", filter.field, value_str),
            super::query::FilterOperator::Ge => format!("{} >= {}", filter.field, value_str),
            super::query::FilterOperator::Lt => format!("{} < {}", filter.field, value_str),
            super::query::FilterOperator::Le => format!("{} <= {}", filter.field, value_str),
            super::query::FilterOperator::In => {
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
            super::query::FilterOperator::NotIn => {
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
            super::query::FilterOperator::Like => {
                format!("{} LIKE {}", filter.field, value_str)
            }
            super::query::FilterOperator::Regex => {
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
    async fn execute_clickhouse(&self, sql: &str) -> Result<QueryResult> {
        // TODO: Use clickhouse crate to execute query
        // For now, return mock data based on query pattern

        tracing::info!("Executing ClickHouse query: {}", sql);

        // Mock implementation - return sample data
        let mut row = HashMap::new();

        if sql.contains("COUNT") {
            row.insert("count".to_string(), Value::Number(42.0));
        } else if sql.contains("SUM") {
            row.insert("sum".to_string(), Value::Number(1500.0));
        } else if sql.contains("AVG") {
            row.insert("avg".to_string(), Value::Number(75.5));
        } else if sql.contains("MAX") {
            row.insert("max".to_string(), Value::Number(200.0));
        } else if sql.contains("MIN") {
            row.insert("min".to_string(), Value::Number(10.0));
        } else if sql.contains("COUNT(DISTINCT") {
            row.insert("count_distinct".to_string(), Value::Number(15.0));
        }

        Ok(QueryResult {
            rows: vec![row],
            execution_time_ms: 25, // Mock execution time
            source: self.config.database.clone(),
            from_cache: false,
        })
    }
}

/// SQL Database Client Implementation
struct SQLClient {
    config: super::config::SQLConfig,
    #[cfg(feature = "sqlx")]
    pg_pool: Option<sqlx::PgPool>,
    #[cfg(feature = "sqlx")]
    sqlite_pool: Option<sqlx::SqlitePool>,
}

impl SQLClient {
    #[cfg_attr(not(feature = "sqlx"), allow(unused_variables))]
    async fn new(config: super::config::SQLConfig, pool_size: u32) -> Result<Self> {
        tracing::info!("Initializing SQL client: {:?}", config.provider);

        #[cfg(feature = "sqlx")]
        {
            let mut pg_pool = None;
            let mut sqlite_pool = None;

            match config.provider {
                super::config::SQLProvider::PostgreSQL => {
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
                            RuntimeError::RuntimeError(format!("Failed to connect to PostgreSQL: {}", e))
                        })?;

                    tracing::info!(
                        "✓ PostgreSQL connection pool created successfully (max_connections: {})",
                        effective_pool_size
                    );
                    pg_pool = Some(pool);
                }
                super::config::SQLProvider::SQLite => {
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
                        SqliteConnectOptions::from_str(&config.connection_string)
                            .map_err(|e| {
                                RuntimeError::RuntimeError(format!("Invalid SQLite connection string: {}", e))
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
                super::config::SQLProvider::MySQL => {
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
            Ok(Self {
                config,
            })
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
            super::config::SQLProvider::PostgreSQL => self.execute_postgresql(&sql).await,
            super::config::SQLProvider::SQLite => self.execute_sqlite(&sql).await,
            super::config::SQLProvider::MySQL => Err(RuntimeError::RuntimeError(
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
                super::query::TimeWindowType::Relative(rel) => {
                    let seconds = rel.to_seconds();
                    match self.config.provider {
                        super::config::SQLProvider::SQLite => {
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
                super::query::TimeWindowType::Absolute { start, end } => {
                    match self.config.provider {
                        super::config::SQLProvider::SQLite => {
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
    fn build_aggregation(&self, agg: &super::query::Aggregation) -> String {
        let field = agg.field.as_deref().unwrap_or("*");

        // For PostgreSQL/SQLite, if field contains JSON access, wrap it with type cast for numeric aggregations
        let needs_numeric_cast =
            matches!(
                agg.agg_type,
                super::query::AggregationType::Sum
                    | super::query::AggregationType::Avg
                    | super::query::AggregationType::Min
                    | super::query::AggregationType::Max
                    | super::query::AggregationType::Stddev
                    | super::query::AggregationType::Percentile { .. }
            ) && (field.contains("->>") || field == "amount" || field.starts_with("attributes"));

        // Build field expression with type cast if needed
        let field_expr = if needs_numeric_cast {
            match self.config.provider {
                super::config::SQLProvider::PostgreSQL => {
                    if field.contains("->>") {
                        // Already has JSONB access, just add cast
                        format!("({})::numeric", field)
                    } else if field == "amount" {
                        // Common case: amount field in attributes JSONB
                        "(attributes->>'amount')::numeric".to_string()
                    } else if field.starts_with("attributes") {
                        // Already has attributes prefix
                        format!("({})::numeric", field)
                    } else {
                        // Assume it's a JSONB field access pattern
                        format!("(attributes->>'{}')::numeric", field)
                    }
                }
                super::config::SQLProvider::SQLite => {
                    // SQLite uses json_extract() function
                    if field.contains("->>") {
                        // Convert PostgreSQL JSONB syntax to SQLite
                        // Example: "attributes->>'amount'" -> "json_extract(attributes, '$.amount')"
                        let parts: Vec<&str> = field.split("->>").collect();
                        if parts.len() == 2 {
                            let json_field = parts[0].trim();
                            let json_key = parts[1].trim_matches('"').trim_matches('\'');
                            format!("CAST(json_extract({}, '$.{}') AS REAL)", json_field, json_key)
                        } else {
                            field.to_string()
                        }
                    } else if field == "amount" {
                        "CAST(json_extract(attributes, '$.amount') AS REAL)".to_string()
                    } else if field.starts_with("attributes") {
                        // Try to extract JSON path
                        format!("CAST(json_extract({}, '$') AS REAL)", field)
                    } else {
                        format!("CAST(json_extract(attributes, '$.{}') AS REAL)", field)
                    }
                }
                _ => field.to_string(),
            }
        } else {
            field.to_string()
        };

        let expr = match agg.agg_type {
            super::query::AggregationType::Count => format!("COUNT({})", field_expr),
            super::query::AggregationType::CountDistinct => {
                format!("COUNT(DISTINCT {})", field_expr)
            }
            super::query::AggregationType::Sum => format!("SUM({})", field_expr),
            super::query::AggregationType::Avg => format!("AVG({})", field_expr),
            super::query::AggregationType::Min => format!("MIN({})", field_expr),
            super::query::AggregationType::Max => format!("MAX({})", field_expr),
            super::query::AggregationType::Median => {
                match self.config.provider {
                    super::config::SQLProvider::SQLite => {
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
            super::query::AggregationType::Stddev => {
                match self.config.provider {
                    super::config::SQLProvider::SQLite => format!("STDEV({})", field_expr),
                    _ => format!("STDDEV_POP({})", field_expr),
                }
            }
            super::query::AggregationType::Percentile { p } => {
                match self.config.provider {
                    super::config::SQLProvider::SQLite => {
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
    fn build_filter(&self, filter: &super::query::Filter) -> Result<String> {
        let value_str = self.format_value(&filter.value)?;

        let expr = match filter.operator {
            super::query::FilterOperator::Eq => format!("{} = {}", filter.field, value_str),
            super::query::FilterOperator::Ne => format!("{} != {}", filter.field, value_str),
            super::query::FilterOperator::Gt => format!("{} > {}", filter.field, value_str),
            super::query::FilterOperator::Ge => format!("{} >= {}", filter.field, value_str),
            super::query::FilterOperator::Lt => format!("{} < {}", filter.field, value_str),
            super::query::FilterOperator::Le => format!("{} <= {}", filter.field, value_str),
            super::query::FilterOperator::In => {
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
            super::query::FilterOperator::NotIn => {
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
            super::query::FilterOperator::Like => {
                format!("{} LIKE {}", filter.field, value_str)
            }
            super::query::FilterOperator::Regex => {
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
                                    tracing::warn!("Failed to extract value for column {} (name: {})", idx, column_name);
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
