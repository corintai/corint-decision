//! Feature Executor Module
//!
//! This module implements the feature execution engine that:
//! - Executes feature operators against data sources
//! - Manages feature caching (L1 local, L2 Redis)
//! - Handles batch feature execution
//! - Manages feature dependencies

use crate::context::ExecutionContext;
use crate::datasource::DataSourceClient;
use crate::feature::cache::CacheManager;
use crate::feature::definition::FeatureDefinition;
use crate::feature::expression::ExpressionEvaluator;
use crate::feature::operator::{CacheBackend, Operator};
use anyhow::{Context as AnyhowContext, Result};
use corint_decision_model::condition::ConditionParser;
use corint_decision_model::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, warn};

// Re-export CacheStats for public API
pub use crate::feature::cache::CacheStats;

/// Feature executor that handles feature computation and caching
pub struct FeatureExecutor {
    /// Cache manager (handles L1/L2 caching and statistics)
    cache_manager: CacheManager,

    /// Data source clients for feature computation
    datasources: HashMap<String, Arc<DataSourceClient>>,

    /// Feature definitions registry
    features: HashMap<String, FeatureDefinition>,
    /// Request-owned cutoff, never populated from an untrusted context field.
    as_of: Option<i64>,
}

impl FeatureExecutor {
    /// Create a new feature executor
    pub fn new() -> Self {
        Self {
            cache_manager: CacheManager::new(),
            datasources: HashMap::new(),
            features: HashMap::new(),
            as_of: None,
        }
    }

    /// Enable cache statistics
    pub fn with_stats(mut self) -> Self {
        self.cache_manager = self.cache_manager.with_stats();
        self
    }

    /// Add a data source client
    pub fn add_datasource(
        &mut self,
        name: impl Into<String>,
        client: DataSourceClient,
    ) -> Result<()> {
        let name = name.into();
        for feature in self.features.values() {
            if feature
                .aggregation
                .as_ref()
                .is_some_and(|config| config.datasource == name)
            {
                client.validate_aggregation(feature.method.as_deref().unwrap_or_default())?;
            }
        }
        self.datasources.insert(name, Arc::new(client));
        Ok(())
    }

    /// Register one definition, allowing forward references but never a cycle.
    pub fn register_feature(&mut self, feature: FeatureDefinition) -> Result<()> {
        self.register_staged(vec![feature], true)
    }

    /// Register a complete batch atomically, validating references and capabilities.
    pub fn register_features(&mut self, features: Vec<FeatureDefinition>) -> Result<()> {
        self.register_staged(features, false)
    }

    fn register_staged(
        &mut self,
        features: Vec<FeatureDefinition>,
        allow_missing: bool,
    ) -> Result<()> {
        let mut staged = self.features.clone();
        for mut feature in features {
            super::dependency::infer_dependencies(&mut feature)?;
            feature.validate().map_err(anyhow::Error::msg)?;
            if let Some(config) = &feature.aggregation {
                if let Some(datasource) = self.datasources.get(&config.datasource) {
                    datasource
                        .validate_aggregation(feature.method.as_deref().unwrap_or_default())?;
                }
            }
            staged.insert(feature.name.clone(), feature);
        }
        super::dependency::order(
            &staged,
            &staged.keys().cloned().collect::<Vec<_>>(),
            allow_missing,
        )?;
        self.features = staged;
        Ok(())
    }

    /// Check if a feature is registered
    pub fn has_feature(&self, feature_name: &str) -> bool {
        self.features.contains_key(feature_name)
    }

    /// Execute a single feature by name
    pub fn execute_feature<'a>(
        &'a self,
        feature_name: &'a str,
        context: &'a ExecutionContext,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            let mut values = self
                .execute_features(&[feature_name.to_owned()], context)
                .await?;
            values
                .remove(feature_name)
                .with_context(|| format!("Feature '{feature_name}' not found"))
        })
    }

    /// Implementation of execute_feature (internal)
    async fn execute_feature_impl(
        &self,
        feature_name: &str,
        context: &ExecutionContext,
        computed: &HashMap<String, Value>,
    ) -> Result<Value> {
        use std::time::Instant;
        let start_time = Instant::now();

        let feature = self
            .features
            .get(feature_name)
            .with_context(|| format!("Feature '{}' not found", feature_name))?;

        if !feature.is_enabled() {
            return Ok(Value::Null);
        }

        // Build context map from ExecutionContext (use event namespace)
        let context_map = context.event.clone();

        // Dependencies were computed once in topological order for this request.
        let dep_values = feature
            .dependencies
            .iter()
            .map(|name| {
                computed
                    .get(name)
                    .cloned()
                    .map(|value| (name.clone(), value))
                    .with_context(|| format!("Feature dependency '{name}' was not computed"))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        // Try to get from cache
        if let Some(cache_config) = self.cache_manager.get_cache_config(feature) {
            let cache_key = self
                .cache_manager
                .build_cache_key(feature_name, &context_map);

            // L1 cache check
            if let Some(value) = self.cache_manager.get_from_l1_cache(&cache_key).await {
                if self.cache_manager.is_stats_enabled() {
                    self.cache_manager.stats().write().await.l1_hits += 1;
                }
                let elapsed = start_time.elapsed();
                debug!(
                    "Feature '{}' L1 cache hit ({}ms)",
                    feature_name,
                    elapsed.as_millis()
                );
                return Ok(value);
            }

            if self.cache_manager.is_stats_enabled() {
                self.cache_manager.stats().write().await.l1_misses += 1;
            }

            // L2 cache check (Redis)
            if cache_config.backend == CacheBackend::Redis {
                if let Some(value) = self.cache_manager.get_from_l2_cache(&cache_key).await {
                    if self.cache_manager.is_stats_enabled() {
                        self.cache_manager.stats().write().await.l2_hits += 1;
                    }
                    let elapsed = start_time.elapsed();
                    debug!(
                        "Feature '{}' L2 cache hit ({}ms)",
                        feature_name,
                        elapsed.as_millis()
                    );

                    // Populate L1 cache
                    self.cache_manager
                        .set_to_l1_cache(&cache_key, value.clone(), cache_config.ttl)
                        .await;

                    return Ok(value);
                }

                if self.cache_manager.is_stats_enabled() {
                    self.cache_manager.stats().write().await.l2_misses += 1;
                }
            }

            // Compute feature
            let compute_start = Instant::now();
            let value = self
                .compute_feature(feature, &context_map, &dep_values)
                .await?;
            let compute_elapsed = compute_start.elapsed();

            if self.cache_manager.is_stats_enabled() {
                self.cache_manager.stats().write().await.compute_count += 1;
            }

            let total_elapsed = start_time.elapsed();
            debug!(
                "Feature '{}' computed (compute: {}ms, total: {}ms)",
                feature_name,
                compute_elapsed.as_millis(),
                total_elapsed.as_millis()
            );

            // Store in cache
            self.cache_manager
                .set_to_cache(&cache_key, value.clone(), cache_config)
                .await;

            Ok(value)
        } else {
            // No caching, compute directly
            if self.cache_manager.is_stats_enabled() {
                self.cache_manager.stats().write().await.compute_count += 1;
            }

            let compute_start = Instant::now();
            let value = self
                .compute_feature(feature, &context_map, &dep_values)
                .await?;
            let compute_elapsed = compute_start.elapsed();
            let total_elapsed = start_time.elapsed();

            debug!(
                "Feature '{}' computed without cache (compute: {}ms, total: {}ms)",
                feature_name,
                compute_elapsed.as_millis(),
                total_elapsed.as_millis()
            );

            Ok(value)
        }
    }

    /// Execute multiple features in batch
    pub async fn execute_features(
        &self,
        feature_names: &[String],
        context: &ExecutionContext,
    ) -> Result<HashMap<String, Value>> {
        use std::time::Instant;
        let batch_start = Instant::now();

        let mut results = HashMap::new();

        // Sort features by dependency order
        let sorted_features = self.sort_by_dependencies(feature_names)?;

        debug!(
            "Executing {} features sequentially in dependency order",
            sorted_features.len()
        );

        for (idx, feature_name) in sorted_features.iter().enumerate() {
            let feature_start = Instant::now();
            let value = self
                .execute_feature_impl(feature_name, context, &results)
                .await?;
            let feature_elapsed = feature_start.elapsed();

            debug!(
                "[{}/{}] Feature '{}' completed in {}ms",
                idx + 1,
                sorted_features.len(),
                feature_name,
                feature_elapsed.as_millis()
            );

            results.insert(feature_name.clone(), value);
        }

        let batch_elapsed = batch_start.elapsed();
        debug!(
            "Batch execution of {} features completed in {}ms (avg: {}ms/feature)",
            sorted_features.len(),
            batch_elapsed.as_millis(),
            if sorted_features.is_empty() {
                0
            } else {
                batch_elapsed.as_millis() / sorted_features.len() as u128
            }
        );

        Ok(results)
    }

    /// Execute all registered features
    pub async fn execute_all(&self, context: &ExecutionContext) -> Result<HashMap<String, Value>> {
        let feature_names: Vec<String> = self.features.keys().cloned().collect();
        self.execute_features(&feature_names, context).await
    }

    /// Fixed-cutoff aggregation/expression execution. Fresh reads only; callers
    /// retain the resulting values for historical replay (late data can change SQL).
    pub async fn execute_features_at(
        &self,
        names: &[String],
        context: &ExecutionContext,
        as_of: i64,
    ) -> Result<HashMap<String, Value>> {
        use super::definition::FeatureType;
        if chrono::DateTime::from_timestamp(as_of, 0).is_none() {
            anyhow::bail!("E_FEATURE_TIME: invalid cutoff");
        }
        for name in self.sort_by_dependencies(names)? {
            let feature = &self.features[&name];
            if !feature.enabled
                || !matches!(
                    feature.feature_type,
                    FeatureType::Aggregation | FeatureType::Expression
                )
            {
                anyhow::bail!("E_FEATURE_CAPABILITY: fixed-cutoff feature '{name}' must be an enabled aggregation or expression");
            }
            if let Some(config) = &feature.aggregation {
                if config.window.is_none() {
                    anyhow::bail!(
                        "E_FEATURE_TIME: fixed-cutoff aggregation '{name}' needs a window"
                    );
                }
                let datasource = self
                    .datasources
                    .get(&config.datasource)
                    .with_context(|| format!("Data source '{}' not found", config.datasource))?;
                if datasource.query_cache_ttl_secs() != 0 {
                    anyhow::bail!(
                        "E_FEATURE_FRESHNESS: fixed-cutoff execution requires query cache TTL 0"
                    );
                }
            }
        }
        let request = Self {
            cache_manager: CacheManager::new(),
            datasources: self.datasources.clone(),
            features: self.features.clone(),
            as_of: Some(as_of),
        };
        request.execute_features(names, context).await
    }

    /// Compute a feature value (no caching)
    async fn compute_feature(
        &self,
        feature: &FeatureDefinition,
        context: &HashMap<String, Value>,
        dependencies: &HashMap<String, Value>,
    ) -> Result<Value> {
        use std::time::Instant;
        let start = Instant::now();

        debug!(
            "Computing feature '{}' (type: {:?})",
            feature.name, feature.feature_type
        );

        // For expression features, pass dependencies directly
        if feature.feature_type == crate::feature::definition::FeatureType::Expression {
            let result = self
                .execute_expression(feature, context, dependencies)
                .await?;
            let elapsed = start.elapsed();
            debug!(
                "Expression feature '{}' computed in {}μs",
                feature.name,
                elapsed.as_micros()
            );
            return Ok(result);
        }

        // Determine data source for other feature types
        let datasource_name = self.get_datasource_name(feature);
        let datasource = self
            .datasources
            .get(&datasource_name)
            .with_context(|| format!("Data source '{}' not found", datasource_name))?;

        debug!(
            "Feature '{}' using datasource '{}'",
            feature.name, datasource_name
        );

        // Execute feature based on type
        let result = self
            .execute_feature_by_type(feature, datasource, context)
            .await?;

        let elapsed = start.elapsed();
        debug!(
            "Feature '{}' datasource query completed in {}ms",
            feature.name,
            elapsed.as_millis()
        );

        Ok(result)
    }

    /// Execute a feature based on its type
    async fn execute_feature_by_type(
        &self,
        feature: &FeatureDefinition,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        use crate::feature::definition::FeatureType;

        match feature.feature_type {
            FeatureType::Aggregation => {
                self.execute_aggregation(feature, datasource, context).await
            }
            FeatureType::State => self.execute_state(feature, datasource, context).await,
            FeatureType::Sequence => self.execute_sequence(feature, datasource, context).await,
            FeatureType::Graph => self.execute_graph(feature, datasource, context).await,
            FeatureType::Expression => {
                // Expression features are handled directly in compute_feature
                // This case should never be reached
                Err(anyhow::anyhow!(
                    "Expression feature '{}' should be handled in compute_feature, not execute_feature_by_type",
                    feature.name
                ))
            }
            FeatureType::Lookup => self.execute_lookup(feature, datasource, context).await,
        }
    }

    /// Execute aggregation feature using datasource-aware Query builder
    async fn execute_aggregation(
        &self,
        feature: &FeatureDefinition,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        use crate::datasource::query::{
            Aggregation, AggregationType, Filter, FilterOperator, Query, QueryType, RelativeWindow,
            TimeWindow, TimeWindowType,
        };

        let config = feature.aggregation.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Missing aggregation config for feature '{}'", feature.name)
        })?;

        let method = feature.method.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Missing method for aggregation feature '{}'", feature.name)
        })?;

        datasource.validate_aggregation(method)?;

        // Build filters from when conditions
        let filters = self.build_filters(&config.when, context)?;

        // Absence permits an unbounded query; an invalid declared window does not.
        let time_window = config
            .window
            .as_ref()
            .map(|w| {
                let relative = RelativeWindow::from_string(w).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Invalid aggregation window '{w}' for feature '{}'",
                        feature.name
                    )
                })?;
                Ok::<_, anyhow::Error>(TimeWindow {
                    window_type: if let Some(end) = self.as_of {
                        let seconds = i64::try_from(relative.to_seconds())?;
                        let start = end
                            .checked_sub(seconds)
                            .ok_or_else(|| anyhow::anyhow!("E_FEATURE_TIME: cutoff underflow"))?;
                        if chrono::DateTime::from_timestamp(start, 0).is_none() {
                            anyhow::bail!("E_FEATURE_TIME: invalid window start");
                        }
                        TimeWindowType::Absolute { start, end }
                    } else {
                        TimeWindowType::Relative(relative)
                    },
                    time_field: config
                        .timestamp_field
                        .clone()
                        .unwrap_or_else(|| "event_timestamp".to_string()),
                })
            })
            .transpose()?;

        // Substitute dimension_value template with context values
        let dimension_value =
            ExpressionEvaluator::substitute_template(&config.dimension_value, context)?;

        // Add dimension filter to constrain the query
        let mut all_filters = filters;
        all_filters.push(Filter {
            field: config.dimension.clone(),
            operator: FilterOperator::Eq,
            value: Value::String(dimension_value),
        });

        // Determine query type and build aggregation based on method
        let (query_type, aggregations) =
            match method.as_str() {
                "count" => {
                    // COUNT(*) query
                    (
                        QueryType::Count,
                        vec![Aggregation {
                            agg_type: AggregationType::Count,
                            field: None,
                            output_name: "count".to_string(),
                        }],
                    )
                }
                "sum" => {
                    let field = config
                        .field
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("Field required for sum aggregation"))?;
                    (
                        QueryType::Aggregate,
                        vec![Aggregation {
                            agg_type: AggregationType::Sum,
                            field: Some(field),
                            output_name: "sum".to_string(),
                        }],
                    )
                }
                "avg" => {
                    let field = config
                        .field
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("Field required for avg aggregation"))?;
                    (
                        QueryType::Aggregate,
                        vec![Aggregation {
                            agg_type: AggregationType::Avg,
                            field: Some(field),
                            output_name: "avg".to_string(),
                        }],
                    )
                }
                "max" => {
                    let field = config
                        .field
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("Field required for max aggregation"))?;
                    (
                        QueryType::Aggregate,
                        vec![Aggregation {
                            agg_type: AggregationType::Max,
                            field: Some(field),
                            output_name: "max".to_string(),
                        }],
                    )
                }
                "min" => {
                    let field = config
                        .field
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("Field required for min aggregation"))?;
                    (
                        QueryType::Aggregate,
                        vec![Aggregation {
                            agg_type: AggregationType::Min,
                            field: Some(field),
                            output_name: "min".to_string(),
                        }],
                    )
                }
                "distinct" => {
                    let field = config.field.clone().ok_or_else(|| {
                        anyhow::anyhow!("Field required for distinct aggregation")
                    })?;
                    (
                        QueryType::CountDistinct,
                        vec![Aggregation {
                            agg_type: AggregationType::CountDistinct,
                            field: Some(field),
                            output_name: "distinct_count".to_string(),
                        }],
                    )
                }
                "stddev" => {
                    let field = config
                        .field
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("Field required for stddev aggregation"))?;
                    (
                        QueryType::Aggregate,
                        vec![Aggregation {
                            agg_type: AggregationType::Stddev,
                            field: Some(field),
                            output_name: "stddev".to_string(),
                        }],
                    )
                }
                "median" => {
                    let field = config
                        .field
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("Field required for median aggregation"))?;
                    (
                        QueryType::Aggregate,
                        vec![Aggregation {
                            agg_type: AggregationType::Median,
                            field: Some(field),
                            output_name: "median".to_string(),
                        }],
                    )
                }
                "percentile" => {
                    let field = config.field.clone().ok_or_else(|| {
                        anyhow::anyhow!("Field required for percentile aggregation")
                    })?;
                    let p = config.percentile.unwrap_or(50);
                    (
                        QueryType::Aggregate,
                        vec![Aggregation {
                            agg_type: AggregationType::Percentile { p },
                            field: Some(field),
                            output_name: "percentile".to_string(),
                        }],
                    )
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported aggregation method: {}",
                        method
                    ));
                }
            };

        // Build the query - datasource-agnostic
        let query = Query {
            query_type,
            entity: config.entity.clone(),
            filters: all_filters,
            time_window,
            aggregations,
            group_by: vec![],
            limit: None,
        };

        // Execute the query - DataSourceClient handles SQL generation based on provider
        let result = datasource
            .query(query)
            .await
            .map_err(|e| anyhow::anyhow!("Query execution failed: {}", e))?;

        // Extract the result value
        if let Some(row) = result.rows.first() {
            // Get the first aggregation output
            let output_key = match method.as_str() {
                "count" => "count",
                "sum" => "sum",
                "avg" => "avg",
                "max" => "max",
                "min" => "min",
                "distinct" => "distinct_count",
                "stddev" => "stddev",
                "median" => "median",
                "percentile" => "percentile",
                _ => "value",
            };

            Ok(row.get(output_key).cloned().unwrap_or(Value::Null))
        } else {
            // No results - return appropriate default
            Ok(match method.as_str() {
                "count" | "distinct" | "sum" => Value::Number(0.0),
                _ => Value::Null,
            })
        }
    }

    /// Build filters from when conditions using the shared ConditionParser
    fn build_filters(
        &self,
        when: &Option<crate::feature::definition::WhenCondition>,
        context: &HashMap<String, Value>,
    ) -> Result<Vec<crate::datasource::query::Filter>> {
        use crate::datasource::query::Filter;

        let Some(when) = when else {
            return Ok(vec![]);
        };

        let conditions = when.conditions().map_err(anyhow::Error::msg)?;

        // Use shared ConditionParser
        let parser = ConditionParser::with_context(context.clone());
        let mut filters = Vec::new();

        for condition_str in conditions {
            match parser.parse_condition(condition_str) {
                Ok(parsed) => {
                    // Convert core operator to filter operator
                    let filter_op = Self::convert_operator(&parsed.operator)?;

                    // Get the resolved value
                    let value = match parsed.value.try_to_value() {
                        Some(v) => v,
                        None => {
                            anyhow::bail!(
                                "Unresolved template variable in condition '{condition_str}'"
                            );
                        }
                    };

                    filters.push(Filter {
                        field: parsed.field,
                        operator: filter_op,
                        value,
                    });
                }
                Err(e) => {
                    anyhow::bail!("Invalid feature condition '{condition_str}': {e}");
                }
            }
        }

        Ok(filters)
    }

    /// Convert corint_decision_model::ast::operator::Operator to FilterOperator
    fn convert_operator(
        op: &corint_decision_model::ast::operator::Operator,
    ) -> Result<crate::datasource::query::FilterOperator> {
        use crate::datasource::query::FilterOperator;
        use corint_decision_model::ast::operator::Operator as CoreOp;

        Ok(match op {
            CoreOp::Eq => FilterOperator::Eq,
            CoreOp::Ne => FilterOperator::Ne,
            CoreOp::Gt => FilterOperator::Gt,
            CoreOp::Ge => FilterOperator::Ge,
            CoreOp::Lt => FilterOperator::Lt,
            CoreOp::Le => FilterOperator::Le,
            CoreOp::In => FilterOperator::In,
            CoreOp::NotIn => FilterOperator::NotIn,
            CoreOp::Regex => FilterOperator::Regex,
            CoreOp::Contains => FilterOperator::Contains,
            CoreOp::StartsWith => FilterOperator::StartsWith,
            CoreOp::EndsWith => FilterOperator::EndsWith,
            _ => anyhow::bail!("Unsupported feature filter operator: {op:?}"),
        })
    }

    /// Execute state feature
    async fn execute_state(
        &self,
        feature: &FeatureDefinition,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        debug!("execute_state called for feature '{}', type: {:?}, method: {:?}, state config present: {}",
               feature.name, feature.feature_type, feature.method, feature.state.is_some());

        let config = feature.state.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Missing state config for feature '{}'", feature.name)
        })?;

        let method = feature.method.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Missing method for state feature '{}'", feature.name)
        })?;

        debug!("State feature '{}': method='{}', config.entity='{}', config.dimension='{}', config.unit='{:?}'",
               feature.name, method, config.entity, config.dimension, config.unit);

        match method.as_str() {
            "time_since" => {
                // Build TimeSinceOperator
                use crate::feature::operator::{
                    FilterConfig, FilterOp, TimeSinceOperator, WindowUnit,
                };

                // Parse unit
                let unit_str = config.unit.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Missing unit for time_since feature '{}'", feature.name)
                })?;

                let unit = match unit_str.as_str() {
                    "minutes" => WindowUnit::Minutes,
                    "hours" => WindowUnit::Hours,
                    "days" => WindowUnit::Days,
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Invalid unit '{}' for time_since feature",
                            unit_str
                        ))
                    }
                };

                // Build filters from when conditions
                let filters_query = self.build_filters(&config.when, context)?;

                // Convert datasource filters to operator FilterConfig format
                let filters: Vec<FilterConfig> = filters_query
                    .iter()
                    .map(|f| {
                        let operator = match f.operator {
                            crate::datasource::query::FilterOperator::Eq => FilterOp::Eq,
                            crate::datasource::query::FilterOperator::Ne => FilterOp::Ne,
                            crate::datasource::query::FilterOperator::Gt => FilterOp::Gt,
                            crate::datasource::query::FilterOperator::Ge => FilterOp::Gte,
                            crate::datasource::query::FilterOperator::Lt => FilterOp::Lt,
                            crate::datasource::query::FilterOperator::Le => FilterOp::Lte,
                            crate::datasource::query::FilterOperator::In => FilterOp::In,
                            crate::datasource::query::FilterOperator::NotIn => FilterOp::NotIn,
                            _ => FilterOp::Eq, // Default for unsupported operators
                        };

                        FilterConfig {
                            field: f.field.clone(),
                            operator,
                            value: f.value.clone(),
                        }
                    })
                    .collect();

                let timestamp_field = config
                    .timestamp_field
                    .clone()
                    .unwrap_or_else(|| "event_timestamp".to_string());

                let operator = TimeSinceOperator {
                    entity: config.entity.clone(),
                    dimension: config.dimension.clone(),
                    dimension_value: config.dimension_value.clone(),
                    filters,
                    unit,
                    timestamp_field,
                };

                match operator.execute(datasource, context).await {
                    Ok(value) => Ok(value),
                    Err(e) => {
                        // If execution fails and fallback is available, use fallback
                        if let Some(fallback) = &config.fallback {
                            warn!("TimeSinceOperator execution failed for feature '{}': {}. Using fallback value: {:?}", 
                                  feature.name, e, fallback);
                            Ok(fallback.clone())
                        } else {
                            Err(anyhow::anyhow!("TimeSinceOperator execution failed: {}", e))
                        }
                    }
                }
            }
            _ => {
                warn!("State feature '{}' has method '{}' which is not yet implemented. Available methods: time_since. State config: {:?}", 
                      feature.name, method, config);
                // If fallback is available, use it instead of failing
                if let Some(fallback) = &config.fallback {
                    warn!("Using fallback value {:?} for state feature '{}' with unsupported method '{}'", 
                          fallback, feature.name, method);
                    Ok(fallback.clone())
                } else {
                    Err(anyhow::anyhow!("State method '{}' not yet implemented for feature '{}'. Supported methods: time_since", method, feature.name))
                }
            }
        }
    }

    /// Execute sequence feature (stub)
    async fn execute_sequence(
        &self,
        feature: &FeatureDefinition,
        _datasource: &DataSourceClient,
        _context: &HashMap<String, Value>,
    ) -> Result<Value> {
        Err(anyhow::anyhow!(
            "Sequence features not yet implemented: {}",
            feature.name
        ))
    }

    /// Execute graph feature (stub)
    async fn execute_graph(
        &self,
        feature: &FeatureDefinition,
        _datasource: &DataSourceClient,
        _context: &HashMap<String, Value>,
    ) -> Result<Value> {
        Err(anyhow::anyhow!(
            "Graph features not yet implemented: {}",
            feature.name
        ))
    }

    /// Execute numeric expressions from feature dependencies and request fields
    async fn execute_expression(
        &self,
        feature: &FeatureDefinition,
        context: &HashMap<String, Value>,
        dependencies: &HashMap<String, Value>,
    ) -> Result<Value> {
        let config = feature.expression.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Missing expression config for feature '{}'", feature.name)
        })?;

        // Expression features consume computed dependencies and request fields
        // They do NOT access datasources directly

        if let Some(expr_str) = &config.expression {
            // Evaluate mathematical expression using dependent features
            // Dependencies have already been computed and passed in

            // Use the pre-computed dependency values directly
            debug!(
                "Evaluating expression '{}' for feature '{}' with dependencies: {:?}",
                expr_str,
                feature.name,
                dependencies.keys()
            );

            // Evaluate the expression with the dependency values
            ExpressionEvaluator::evaluate_with_context(expr_str, dependencies, context)
        } else if config.model.is_some() {
            // ML model scoring (not yet implemented)
            Err(anyhow::anyhow!(
                "ML model scoring not yet implemented for feature '{}'",
                feature.name
            ))
        } else {
            Err(anyhow::anyhow!(
                "Expression feature '{}' must have either expression or model",
                feature.name
            ))
        }
    }

    /// Execute lookup feature - retrieve pre-computed values from Redis/feature store
    async fn execute_lookup(
        &self,
        feature: &FeatureDefinition,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let config = feature.lookup.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Missing lookup config for feature '{}'", feature.name)
        })?;

        // Substitute template in key (e.g., "user_risk_score:{event.user_id}" -> "user_risk_score:123")
        let key = ExpressionEvaluator::substitute_template(&config.key, context)?;

        debug!("Lookup feature '{}' fetching key: {}", feature.name, key);

        // Lookup features only retrieve pre-computed values, they don't compute
        // Use the feature store datasource to get the value
        match datasource.get_feature(&feature.name, &key).await {
            Ok(Some(value)) => {
                debug!("Lookup feature '{}' found value: {:?}", feature.name, value);
                Ok(value)
            }
            Ok(None) => {
                // Not found, use fallback
                debug!(
                    "Lookup feature '{}' not found, using fallback",
                    feature.name
                );
                Ok(config.fallback.clone().unwrap_or(Value::Null))
            }
            Err(e) => {
                warn!(
                    "Lookup feature '{}' error: {}, using fallback",
                    feature.name, e
                );
                // On error, return fallback
                Ok(config.fallback.clone().unwrap_or(Value::Null))
            }
        }
    }

    /// Get data source name from feature
    fn get_datasource_name(&self, feature: &FeatureDefinition) -> String {
        use crate::feature::definition::FeatureType;

        match feature.feature_type {
            FeatureType::Aggregation => feature
                .aggregation
                .as_ref()
                .map(|c| c.datasource.clone())
                .unwrap_or_else(|| "default".to_string()),
            FeatureType::State => feature
                .state
                .as_ref()
                .map(|c| c.datasource.clone())
                .unwrap_or_else(|| "default".to_string()),
            FeatureType::Sequence => feature
                .sequence
                .as_ref()
                .map(|c| c.datasource.clone())
                .unwrap_or_else(|| "default".to_string()),
            FeatureType::Graph => feature
                .graph
                .as_ref()
                .map(|c| c.datasource.clone())
                .unwrap_or_else(|| "default".to_string()),
            FeatureType::Lookup => feature
                .lookup
                .as_ref()
                .map(|c| c.datasource.clone())
                .unwrap_or_else(|| "default".to_string()),
            FeatureType::Expression => {
                "default".to_string() // Expression features don't need datasource
            }
        }
    }

    /// Get data source name from old Operator enum (deprecated, kept for tests)
    #[allow(dead_code)]
    fn get_datasource_name_from_operator(&self, operator: &Operator) -> String {
        match operator {
            // Operators with explicit datasource field
            Operator::FeatureStoreLookup(op) => op.datasource.clone(),
            Operator::ProfileLookup(op) => op.datasource.clone(),

            // Operators with OperatorParams (check params.datasource)
            Operator::Count(op) => op
                .params
                .datasource
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            Operator::Sum(op) => op
                .params
                .datasource
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            Operator::Avg(op) => op
                .params
                .datasource
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            Operator::Max(op) => op
                .params
                .datasource
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            Operator::Min(op) => op
                .params
                .datasource
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            Operator::CountDistinct(op) => op
                .params
                .datasource
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            Operator::Velocity(op) => op
                .params
                .datasource
                .clone()
                .unwrap_or_else(|| "default".to_string()),

            // Other operators use default
            _ => "default".to_string(),
        }
    }

    fn sort_by_dependencies(&self, feature_names: &[String]) -> Result<Vec<String>> {
        super::dependency::execution_order(&self.features, feature_names)
    }

    /// Clear L1 cache
    pub async fn clear_cache(&self) {
        self.cache_manager.clear_cache().await;
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.cache_manager.get_stats().await
    }

    /// Print cache statistics
    pub async fn print_stats(&self) {
        self.cache_manager.print_stats().await;
    }

    // Test helper methods (exposed for testing)
    #[cfg(test)]
    async fn set_to_l1_cache(&self, key: &str, value: Value, ttl: u64) {
        self.cache_manager.set_to_l1_cache(key, value, ttl).await;
    }

    #[cfg(test)]
    async fn get_from_l1_cache(&self, key: &str) -> Option<Value> {
        self.cache_manager.get_from_l1_cache(key).await
    }

    #[cfg(test)]
    fn build_cache_key(&self, feature_name: &str, context: &HashMap<String, Value>) -> String {
        self.cache_manager.build_cache_key(feature_name, context)
    }

    #[cfg(test)]
    fn is_stats_enabled(&self) -> bool {
        self.cache_manager.is_stats_enabled()
    }
}

impl Default for FeatureExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::operator::Operator;
    use std::time::Duration;
    use tokio::time::sleep;

    #[cfg(feature = "sqlx")]
    #[tokio::test]
    async fn aggregation_windows_filter_sql_rows_and_invalid_windows_fail() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("events.sqlite");
        let pool = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query("CREATE TABLE events (user_id TEXT, observed_at TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO events VALUES ('user-1', datetime('now')), ('user-1', datetime('now', '-2 minutes')), ('other', datetime('now'))")
            .execute(&pool).await.unwrap();
        let config = serde_json::from_value(serde_json::json!({
            "name": "events", "type": "sql", "provider": "sqlite",
            "connection_string": path.to_str().unwrap(), "database": "events"
        }))
        .unwrap();
        let datasource = DataSourceClient::new(config).await.unwrap();
        let mut feature: FeatureDefinition = serde_yaml::from_str(
            "name: window_test\ntype: aggregation\nmethod: count\ndatasource: events\nentity: events\ndimension: user_id\ndimension_value: user-1\ntimestamp_field: observed_at\nwindow: 30s\n"
        ).unwrap();
        let executor = FeatureExecutor::new();
        let context = HashMap::new();
        assert_eq!(
            executor
                .execute_aggregation(&feature, &datasource, &context)
                .await
                .unwrap(),
            Value::Number(1.0)
        );
        feature.aggregation.as_mut().unwrap().window = None;
        assert_eq!(
            executor
                .execute_aggregation(&feature, &datasource, &context)
                .await
                .unwrap(),
            Value::Number(2.0)
        );
        // Bypass registration deliberately: the query path itself must reject
        // invalid windows, even with a cached all-history result available.
        for window in ["1q", "1y", "1秒", "0s", "18446744073709551615d"] {
            feature.aggregation.as_mut().unwrap().window = Some(window.into());
            let error = executor
                .execute_aggregation(&feature, &datasource, &context)
                .await
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("Invalid aggregation window"),
                "{window}: {error}"
            );
        }
        pool.close().await;
    }

    #[test]
    fn test_cache_key_building() {
        let executor = FeatureExecutor::new();
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("user123".to_string()));
        context.insert(
            "device_id".to_string(),
            Value::String("device456".to_string()),
        );

        let key = executor.build_cache_key("login_count_24h", &context);
        assert!(key.contains("login_count_24h"));
        assert!(key.contains("user_id:user123"));
        assert!(key.contains("device_id:device456"));
    }

    #[test]
    fn test_cache_key_different_features() {
        let executor = FeatureExecutor::new();
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("user123".to_string()));

        let key1 = executor.build_cache_key("feature1", &context);
        let key2 = executor.build_cache_key("feature2", &context);

        assert_ne!(key1, key2);
        assert!(key1.contains("feature1"));
        assert!(key2.contains("feature2"));
    }

    #[test]
    fn test_cache_key_different_contexts() {
        let executor = FeatureExecutor::new();

        let mut context1 = HashMap::new();
        context1.insert("user_id".to_string(), Value::String("user123".to_string()));

        let mut context2 = HashMap::new();
        context2.insert("user_id".to_string(), Value::String("user456".to_string()));

        let key1 = executor.build_cache_key("login_count", &context1);
        let key2 = executor.build_cache_key("login_count", &context2);

        assert_ne!(key1, key2);
    }

    #[tokio::test]
    async fn test_l1_cache_set_and_get() {
        let executor = FeatureExecutor::new();
        let key = "test_key";
        let value = Value::Number(123.0);

        executor.set_to_l1_cache(key, value.clone(), 300).await;
        let cached = executor.get_from_l1_cache(key).await;

        assert_eq!(cached, Some(value));
    }

    #[tokio::test]
    async fn test_l1_cache_miss() {
        let executor = FeatureExecutor::new();
        let cached = executor.get_from_l1_cache("nonexistent_key").await;
        assert_eq!(cached, None);
    }

    #[tokio::test]
    async fn test_l1_cache_expiration() {
        let executor = FeatureExecutor::new();
        let key = "expire_test";
        let value = Value::Number(42.0);

        executor.set_to_l1_cache(key, value.clone(), 1).await; // 1 second TTL

        // Should be present immediately
        assert_eq!(executor.get_from_l1_cache(key).await, Some(value.clone()));

        // Wait for expiration
        sleep(Duration::from_secs(2)).await;

        // Should be expired now
        assert_eq!(executor.get_from_l1_cache(key).await, None);
    }

    #[tokio::test]
    async fn test_cache_stats_initialization() {
        let executor = FeatureExecutor::new().with_stats();
        let stats = executor.get_stats().await;

        assert_eq!(stats.l1_hits, 0);
        assert_eq!(stats.l1_misses, 0);
        assert_eq!(stats.l2_hits, 0);
        assert_eq!(stats.l2_misses, 0);
        assert_eq!(stats.compute_count, 0);
    }

    #[test]
    fn test_cache_stats_default() {
        let stats = CacheStats::default();
        assert_eq!(stats.l1_hits, 0);
        assert_eq!(stats.l1_misses, 0);
        assert_eq!(stats.l2_hits, 0);
        assert_eq!(stats.l2_misses, 0);
        assert_eq!(stats.compute_count, 0);
    }

    #[test]
    fn test_feature_executor_new() {
        let executor = FeatureExecutor::new();
        assert!(!executor.is_stats_enabled());
        assert_eq!(executor.datasources.len(), 0);
        assert_eq!(executor.features.len(), 0);
    }

    #[test]
    fn test_feature_executor_with_stats() {
        let executor = FeatureExecutor::new().with_stats();
        assert!(executor.is_stats_enabled());
    }

    #[test]
    fn test_feature_executor_has_feature() {
        use crate::feature::operator::{CountOperator, WindowConfig, WindowUnit};

        let mut executor = FeatureExecutor::new();
        assert!(!executor.has_feature("test_feature"));

        let operator = Operator::Count(CountOperator {
            params: crate::feature::operator::OperatorParams {
                datasource: None,
                entity: "test_entity".to_string(),
                dimension: "test_dim".to_string(),
                dimension_value: "{test}".to_string(),
                window: Some(WindowConfig {
                    value: 24,
                    unit: WindowUnit::Hours,
                }),
                filters: vec![],
                cache: None,
            },
        });

        let feature = FeatureDefinition::new("test_feature", operator);

        executor.register_feature(feature).unwrap();
        assert!(executor.has_feature("test_feature"));
    }

    #[tokio::test]
    async fn test_concurrent_cache_access() {
        use tokio::task::JoinSet;

        let executor = Arc::new(FeatureExecutor::new());
        let mut tasks = JoinSet::new();

        // Spawn multiple concurrent tasks
        for i in 0..10 {
            let exec = executor.clone();
            tasks.spawn(async move {
                let key = format!("concurrent_key_{}", i);
                let value = Value::Number(i as f64);
                exec.set_to_l1_cache(&key, value.clone(), 300).await;
                exec.get_from_l1_cache(&key).await
            });
        }

        // Collect results
        let mut results = Vec::new();
        while let Some(result) = tasks.join_next().await {
            results.push(result.unwrap());
        }

        // Verify all writes were successful
        assert_eq!(results.len(), 10);
        assert!(results.iter().all(|r| r.is_some()));
    }

    #[tokio::test]
    async fn test_concurrent_feature_registration() {
        use crate::feature::operator::{CountOperator, WindowConfig, WindowUnit};
        use tokio::task::JoinSet;

        let executor = Arc::new(tokio::sync::RwLock::new(FeatureExecutor::new()));
        let mut tasks = JoinSet::new();

        // Spawn multiple concurrent registration tasks
        for i in 0..5 {
            let exec = executor.clone();
            tasks.spawn(async move {
                let operator = Operator::Count(CountOperator {
                    params: crate::feature::operator::OperatorParams {
                        datasource: None,
                        entity: "test_entity".to_string(),
                        dimension: "test_dim".to_string(),
                        dimension_value: format!("{{test_{}}}", i),
                        window: Some(WindowConfig {
                            value: 24,
                            unit: WindowUnit::Hours,
                        }),
                        filters: vec![],
                        cache: None,
                    },
                });

                let feature = FeatureDefinition::new(format!("feature_{}", i), operator);
                exec.write().await.register_feature(feature)
            });
        }

        // Collect results
        let mut results = Vec::new();
        while let Some(result) = tasks.join_next().await {
            results.push(result.unwrap());
        }

        // Verify all registrations were successful
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.is_ok()));

        // Verify all features are registered
        let exec = executor.read().await;
        for i in 0..5 {
            assert!(exec.has_feature(&format!("feature_{}", i)));
        }
    }

    #[test]
    fn test_expression_evaluator_simple() {
        // Test simple number
        let result = ExpressionEvaluator::eval_math_expr("42").unwrap();
        assert_eq!(result, Value::Number(42.0));

        // Test addition
        let result = ExpressionEvaluator::eval_math_expr("10+5").unwrap();
        assert_eq!(result, Value::Number(15.0));

        // Test subtraction
        let result = ExpressionEvaluator::eval_math_expr("10-5").unwrap();
        assert_eq!(result, Value::Number(5.0));

        // Test multiplication
        let result = ExpressionEvaluator::eval_math_expr("10*5").unwrap();
        assert_eq!(result, Value::Number(50.0));

        // Test division
        let result = ExpressionEvaluator::eval_math_expr("10/5").unwrap();
        assert_eq!(result, Value::Number(2.0));
    }

    #[test]
    fn test_expression_evaluator_division_by_zero() {
        // Division by zero should return Null
        let result = ExpressionEvaluator::eval_math_expr("10/0").unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_expression_evaluator_complex() {
        // Our evaluator now correctly handles operator precedence
        // It evaluates "5+10*2" as 5+(10*2) = 25 (correct)
        // We search for operators in order +, -, /, * to ensure proper precedence
        let result = ExpressionEvaluator::eval_math_expr("5+10*2").unwrap();
        assert_eq!(result, Value::Number(25.0));

        // For correct precedence, users should use parentheses or separate features
        // e.g., create intermediate features or use a proper expression parser
    }

    #[test]
    fn test_evaluate_expression_with_features() {
        let mut feature_values = HashMap::new();
        feature_values.insert("login_count".to_string(), Value::Number(10.0));
        feature_values.insert("failed_logins".to_string(), Value::Number(3.0));

        // Test division expression
        let result = ExpressionEvaluator::evaluate_expression(
            "failed_logins / login_count",
            &feature_values,
        )
        .unwrap();

        assert_eq!(result, Value::Number(0.3));
    }

    #[test]
    fn test_substitute_template_direct_reference() {
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("user123".to_string()));
        context.insert(
            "device_id".to_string(),
            Value::String("device456".to_string()),
        );

        // Test direct reference: event.user_id -> lookup context["user_id"]
        let result = ExpressionEvaluator::substitute_template("event.user_id", &context).unwrap();
        assert_eq!(result, "user123");

        // Test direct reference: event.device_id
        let result = ExpressionEvaluator::substitute_template("event.device_id", &context).unwrap();
        assert_eq!(result, "device456");

        // Test with numeric value
        context.insert("count".to_string(), Value::Number(42.0));
        let result = ExpressionEvaluator::substitute_template("event.count", &context).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_substitute_template_string_interpolation() {
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("user123".to_string()));
        context.insert(
            "device_id".to_string(),
            Value::String("device456".to_string()),
        );

        // Test string interpolation: ${event.user_id} inside string
        let result =
            ExpressionEvaluator::substitute_template("${event.user_id}", &context).unwrap();
        assert_eq!(result, "user123");

        // Test string interpolation with prefix
        let result =
            ExpressionEvaluator::substitute_template("user_risk:${event.user_id}", &context)
                .unwrap();
        assert_eq!(result, "user_risk:user123");

        // Test string interpolation with prefix and suffix
        let result =
            ExpressionEvaluator::substitute_template("prefix:${event.device_id}:suffix", &context)
                .unwrap();
        assert_eq!(result, "prefix:device456:suffix");

        // Test with numeric value
        context.insert("count".to_string(), Value::Number(42.0));
        let result =
            ExpressionEvaluator::substitute_template("count_${event.count}_value", &context)
                .unwrap();
        assert_eq!(result, "count_42_value");
    }

    #[tokio::test]
    async fn test_cache_overwrite() {
        let executor = FeatureExecutor::new();
        let key = "overwrite_key";

        executor.set_to_l1_cache(key, Value::Number(1.0), 300).await;
        assert_eq!(
            executor.get_from_l1_cache(key).await,
            Some(Value::Number(1.0))
        );

        executor.set_to_l1_cache(key, Value::Number(2.0), 300).await;
        assert_eq!(
            executor.get_from_l1_cache(key).await,
            Some(Value::Number(2.0))
        );
    }

    #[tokio::test]
    async fn test_multiple_cache_keys() {
        let executor = FeatureExecutor::new();

        executor
            .set_to_l1_cache("key1", Value::Number(1.0), 300)
            .await;
        executor
            .set_to_l1_cache("key2", Value::Number(2.0), 300)
            .await;
        executor
            .set_to_l1_cache("key3", Value::Number(3.0), 300)
            .await;

        assert_eq!(
            executor.get_from_l1_cache("key1").await,
            Some(Value::Number(1.0))
        );
        assert_eq!(
            executor.get_from_l1_cache("key2").await,
            Some(Value::Number(2.0))
        );
        assert_eq!(
            executor.get_from_l1_cache("key3").await,
            Some(Value::Number(3.0))
        );
    }
}
