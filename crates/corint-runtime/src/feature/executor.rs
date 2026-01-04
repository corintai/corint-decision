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
use corint_core::condition::ConditionParser;
use corint_core::Value;
use futures::future;
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
}

impl FeatureExecutor {
    /// Create a new feature executor
    pub fn new() -> Self {
        Self {
            cache_manager: CacheManager::new(),
            datasources: HashMap::new(),
            features: HashMap::new(),
        }
    }

    /// Enable cache statistics
    pub fn with_stats(mut self) -> Self {
        self.cache_manager = self.cache_manager.with_stats();
        self
    }

    /// Add a data source client
    pub fn add_datasource(&mut self, name: impl Into<String>, client: DataSourceClient) {
        self.datasources.insert(name.into(), Arc::new(client));
    }

    /// Register a feature definition
    pub fn register_feature(&mut self, feature: FeatureDefinition) -> Result<()> {
        feature
            .validate()
            .map_err(|e| anyhow::anyhow!("Failed to validate feature '{}': {}", feature.name, e))?;

        self.features.insert(feature.name.clone(), feature);
        Ok(())
    }

    /// Register multiple features
    pub fn register_features(&mut self, features: Vec<FeatureDefinition>) -> Result<()> {
        for feature in features {
            self.register_feature(feature)?;
        }
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
        Box::pin(async move { self.execute_feature_impl(feature_name, context).await })
    }

    /// Implementation of execute_feature (internal)
    async fn execute_feature_impl(
        &self,
        feature_name: &str,
        context: &ExecutionContext,
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

        // Check dependencies first - compute them in parallel
        let dep_start = Instant::now();
        let dep_values = if !feature.dependencies.is_empty() {
            debug!(
                "Computing {} dependencies in parallel for feature '{}': {:?}",
                feature.dependencies.len(),
                feature_name,
                feature.dependencies
            );

            // Create futures for all dependencies
            let dep_futures: Vec<_> = feature
                .dependencies
                .iter()
                .map(|dep_name| {
                    let dep_name_clone = dep_name.clone();
                    async move {
                        let value = self.execute_feature_impl(&dep_name_clone, context).await?;
                        Ok::<(String, Value), anyhow::Error>((dep_name_clone, value))
                    }
                })
                .collect();

            // Execute all dependency computations in parallel
            let results = future::try_join_all(dep_futures).await?;
            let dep_elapsed = dep_start.elapsed();
            debug!(
                "Feature '{}' dependencies computed in {:?}",
                feature_name, dep_elapsed
            );

            // Convert results to HashMap
            results.into_iter().collect::<HashMap<String, Value>>()
        } else {
            HashMap::new()
        };

        // Try to get from cache
        if let Some(cache_config) = self.cache_manager.get_cache_config(feature) {
            let cache_key = self.cache_manager.build_cache_key(feature_name, &context_map);

            // L1 cache check
            if let Some(value) = self.cache_manager.get_from_l1_cache(&cache_key).await {
                if self.cache_manager.is_stats_enabled() {
                    self.cache_manager.stats().write().await.l1_hits += 1;
                }
                let elapsed = start_time.elapsed();
                debug!("Feature '{}' L1 cache hit ({}ms)", feature_name, elapsed.as_millis());
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
                    debug!("Feature '{}' L2 cache hit ({}ms)", feature_name, elapsed.as_millis());

                    // Populate L1 cache
                    self.cache_manager.set_to_l1_cache(&cache_key, value.clone(), cache_config.ttl)
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
            self.cache_manager.set_to_cache(&cache_key, value.clone(), cache_config)
                .await;

            Ok(value)
        } else {
            // No caching, compute directly
            if self.cache_manager.is_stats_enabled() {
                self.cache_manager.stats().write().await.compute_count += 1;
            }

            let compute_start = Instant::now();
            let value = self.compute_feature(feature, &context_map, &dep_values).await?;
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
            let value = self.execute_feature_impl(feature_name, context).await?;
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
            if sorted_features.is_empty() { 0 } else { batch_elapsed.as_millis() / sorted_features.len() as u128 }
        );

        Ok(results)
    }

    /// Execute all registered features
    pub async fn execute_all(&self, context: &ExecutionContext) -> Result<HashMap<String, Value>> {
        let feature_names: Vec<String> = self.features.keys().cloned().collect();
        self.execute_features(&feature_names, context).await
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
            let result = self.execute_expression(feature, context, dependencies).await?;
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
            FeatureType::State => {
                self.execute_state(feature, datasource, context).await
            }
            FeatureType::Sequence => {
                self.execute_sequence(feature, datasource, context).await
            }
            FeatureType::Graph => {
                self.execute_graph(feature, datasource, context).await
            }
            FeatureType::Expression => {
                // Expression features are handled directly in compute_feature
                // This case should never be reached
                Err(anyhow::anyhow!(
                    "Expression feature '{}' should be handled in compute_feature, not execute_feature_by_type",
                    feature.name
                ))
            }
            FeatureType::Lookup => {
                self.execute_lookup(feature, datasource, context).await
            }
        }
    }

    /// Execute aggregation feature using datasource-aware Query builder
    async fn execute_aggregation(
        &self,
        feature: &FeatureDefinition,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        use crate::datasource::query::{Query, QueryType, Aggregation, AggregationType, Filter, FilterOperator, TimeWindow, TimeWindowType, RelativeWindow};

        let config = feature.aggregation.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing aggregation config for feature '{}'", feature.name))?;

        let method = feature.method.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing method for aggregation feature '{}'", feature.name))?;

        // Build filters from when conditions
        let filters = self.build_filters(&config.when, context)?;

        // Build time window if specified
        let time_window = config.window.as_ref().and_then(|w| {
            RelativeWindow::from_string(w).map(|relative| TimeWindow {
                window_type: TimeWindowType::Relative(relative),
                time_field: config.timestamp_field.clone()
                    .unwrap_or_else(|| "event_timestamp".to_string()),
            })
        });

        // Substitute dimension_value template with context values
        let dimension_value = ExpressionEvaluator::substitute_template(&config.dimension_value, context)?;

        // Add dimension filter to constrain the query
        let mut all_filters = filters;
        all_filters.push(Filter {
            field: config.dimension.clone(),
            operator: FilterOperator::Eq,
            value: Value::String(dimension_value),
        });

        // Determine query type and build aggregation based on method
        let (query_type, aggregations) = match method.as_str() {
            "count" => {
                // COUNT(*) query
                (QueryType::Count, vec![Aggregation {
                    agg_type: AggregationType::Count,
                    field: None,
                    output_name: "count".to_string(),
                }])
            }
            "sum" => {
                let field = config.field.clone()
                    .ok_or_else(|| anyhow::anyhow!("Field required for sum aggregation"))?;
                (QueryType::Aggregate, vec![Aggregation {
                    agg_type: AggregationType::Sum,
                    field: Some(field),
                    output_name: "sum".to_string(),
                }])
            }
            "avg" => {
                let field = config.field.clone()
                    .ok_or_else(|| anyhow::anyhow!("Field required for avg aggregation"))?;
                (QueryType::Aggregate, vec![Aggregation {
                    agg_type: AggregationType::Avg,
                    field: Some(field),
                    output_name: "avg".to_string(),
                }])
            }
            "max" => {
                let field = config.field.clone()
                    .ok_or_else(|| anyhow::anyhow!("Field required for max aggregation"))?;
                (QueryType::Aggregate, vec![Aggregation {
                    agg_type: AggregationType::Max,
                    field: Some(field),
                    output_name: "max".to_string(),
                }])
            }
            "min" => {
                let field = config.field.clone()
                    .ok_or_else(|| anyhow::anyhow!("Field required for min aggregation"))?;
                (QueryType::Aggregate, vec![Aggregation {
                    agg_type: AggregationType::Min,
                    field: Some(field),
                    output_name: "min".to_string(),
                }])
            }
            "distinct" => {
                let field = config.field.clone()
                    .ok_or_else(|| anyhow::anyhow!("Field required for distinct aggregation"))?;
                (QueryType::CountDistinct, vec![Aggregation {
                    agg_type: AggregationType::CountDistinct,
                    field: Some(field),
                    output_name: "distinct_count".to_string(),
                }])
            }
            "stddev" => {
                let field = config.field.clone()
                    .ok_or_else(|| anyhow::anyhow!("Field required for stddev aggregation"))?;
                (QueryType::Aggregate, vec![Aggregation {
                    agg_type: AggregationType::Stddev,
                    field: Some(field),
                    output_name: "stddev".to_string(),
                }])
            }
            "median" => {
                let field = config.field.clone()
                    .ok_or_else(|| anyhow::anyhow!("Field required for median aggregation"))?;
                (QueryType::Aggregate, vec![Aggregation {
                    agg_type: AggregationType::Median,
                    field: Some(field),
                    output_name: "median".to_string(),
                }])
            }
            "percentile" => {
                let field = config.field.clone()
                    .ok_or_else(|| anyhow::anyhow!("Field required for percentile aggregation"))?;
                let p = config.percentile.unwrap_or(50);
                (QueryType::Aggregate, vec![Aggregation {
                    agg_type: AggregationType::Percentile { p },
                    field: Some(field),
                    output_name: "percentile".to_string(),
                }])
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported aggregation method: {}", method));
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
        let result = datasource.query(query).await
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
        use crate::feature::definition::WhenCondition;

        let Some(when) = when else {
            return Ok(vec![]);
        };

        // Collect all condition strings
        let conditions: Vec<&str> = match when {
            WhenCondition::Simple(expr) => vec![expr.as_str()],
            WhenCondition::Complex { all, any } => {
                // For now, we only support 'all' conditions (AND logic)
                // 'any' conditions (OR logic) would require more complex SQL generation
                if any.is_some() {
                    warn!("'any' conditions in 'when' clause are not yet supported for database filters, ignoring");
                }
                all.as_ref()
                    .map(|v| v.iter().map(|s| s.as_str()).collect())
                    .unwrap_or_default()
            }
        };

        // Use shared ConditionParser
        let parser = ConditionParser::with_context(context.clone());
        let mut filters = Vec::new();

        for condition_str in conditions {
            match parser.parse_condition(condition_str) {
                Ok(parsed) => {
                    // Convert core operator to filter operator
                    let filter_op = Self::convert_operator(&parsed.operator);

                    // Get the resolved value
                    let value = match parsed.value.try_to_value() {
                        Some(v) => v,
                        None => {
                            warn!(
                                "Template variable in condition '{}' was not resolved, skipping",
                                condition_str
                            );
                            continue;
                        }
                    };

                    filters.push(Filter {
                        field: parsed.field,
                        operator: filter_op,
                        value,
                    });
                }
                Err(e) => {
                    warn!("Failed to parse condition '{}': {}", condition_str, e);
                }
            }
        }

        Ok(filters)
    }

    /// Convert corint_core::ast::operator::Operator to FilterOperator
    fn convert_operator(op: &corint_core::ast::operator::Operator) -> crate::datasource::query::FilterOperator {
        use corint_core::ast::operator::Operator as CoreOp;
        use crate::datasource::query::FilterOperator;

        match op {
            CoreOp::Eq => FilterOperator::Eq,
            CoreOp::Ne => FilterOperator::Ne,
            CoreOp::Gt => FilterOperator::Gt,
            CoreOp::Ge => FilterOperator::Ge,
            CoreOp::Lt => FilterOperator::Lt,
            CoreOp::Le => FilterOperator::Le,
            CoreOp::In => FilterOperator::In,
            CoreOp::NotIn => FilterOperator::NotIn,
            CoreOp::Regex => FilterOperator::Regex,
            CoreOp::Contains | CoreOp::StartsWith | CoreOp::EndsWith => FilterOperator::Like,
            // For operators that don't have a direct SQL equivalent, default to Eq
            _ => {
                warn!("Operator {:?} not directly supported in SQL filters, defaulting to Eq", op);
                FilterOperator::Eq
            }
        }
    }

    /// Execute state feature (stub)
    async fn execute_state(
        &self,
        feature: &FeatureDefinition,
        _datasource: &DataSourceClient,
        _context: &HashMap<String, Value>,
    ) -> Result<Value> {
        Err(anyhow::anyhow!("State features not yet implemented: {}", feature.name))
    }

    /// Execute sequence feature (stub)
    async fn execute_sequence(
        &self,
        feature: &FeatureDefinition,
        _datasource: &DataSourceClient,
        _context: &HashMap<String, Value>,
    ) -> Result<Value> {
        Err(anyhow::anyhow!("Sequence features not yet implemented: {}", feature.name))
    }

    /// Execute graph feature (stub)
    async fn execute_graph(
        &self,
        feature: &FeatureDefinition,
        _datasource: &DataSourceClient,
        _context: &HashMap<String, Value>,
    ) -> Result<Value> {
        Err(anyhow::anyhow!("Graph features not yet implemented: {}", feature.name))
    }

    /// Execute expression feature - computes from other features
    async fn execute_expression(
        &self,
        feature: &FeatureDefinition,
        _context: &HashMap<String, Value>,
        dependencies: &HashMap<String, Value>,
    ) -> Result<Value> {
        let config = feature.expression.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing expression config for feature '{}'", feature.name))?;

        // Expression features only consume results from other features
        // They do NOT access datasources directly

        if let Some(expr_str) = &config.expression {
            // Evaluate mathematical expression using dependent features
            // Dependencies have already been computed and passed in

            // Use the pre-computed dependency values directly
            debug!(
                "Evaluating expression '{}' for feature '{}' with dependencies: {:?}",
                expr_str, feature.name, dependencies.keys()
            );

            // Evaluate the expression with the dependency values
            ExpressionEvaluator::evaluate_expression(expr_str, dependencies)
        } else if config.model.is_some() {
            // ML model scoring (not yet implemented)
            Err(anyhow::anyhow!("ML model scoring not yet implemented for feature '{}'", feature.name))
        } else {
            Err(anyhow::anyhow!("Expression feature '{}' must have either expression or model", feature.name))
        }
    }

    /// Execute lookup feature - retrieve pre-computed values from Redis/feature store
    async fn execute_lookup(
        &self,
        feature: &FeatureDefinition,
        datasource: &DataSourceClient,
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let config = feature.lookup.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing lookup config for feature '{}'", feature.name))?;

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
                debug!("Lookup feature '{}' not found, using fallback", feature.name);
                Ok(config.fallback.clone().unwrap_or(Value::Null))
            }
            Err(e) => {
                warn!("Lookup feature '{}' error: {}, using fallback", feature.name, e);
                // On error, return fallback
                Ok(config.fallback.clone().unwrap_or(Value::Null))
            }
        }
    }

    /// Get data source name from feature
    fn get_datasource_name(&self, feature: &FeatureDefinition) -> String {
        use crate::feature::definition::FeatureType;

        match feature.feature_type {
            FeatureType::Aggregation => {
                feature.aggregation.as_ref()
                    .map(|c| c.datasource.clone())
                    .unwrap_or_else(|| "default".to_string())
            }
            FeatureType::State => {
                feature.state.as_ref()
                    .map(|c| c.datasource.clone())
                    .unwrap_or_else(|| "default".to_string())
            }
            FeatureType::Sequence => {
                feature.sequence.as_ref()
                    .map(|c| c.datasource.clone())
                    .unwrap_or_else(|| "default".to_string())
            }
            FeatureType::Graph => {
                feature.graph.as_ref()
                    .map(|c| c.datasource.clone())
                    .unwrap_or_else(|| "default".to_string())
            }
            FeatureType::Lookup => {
                feature.lookup.as_ref()
                    .map(|c| c.datasource.clone())
                    .unwrap_or_else(|| "default".to_string())
            }
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

    /// Sort features by dependency order (topological sort)
    fn sort_by_dependencies(&self, feature_names: &[String]) -> Result<Vec<String>> {
        let mut sorted = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut visiting = std::collections::HashSet::new();

        for name in feature_names {
            self.visit_feature(name, &mut sorted, &mut visited, &mut visiting)?;
        }

        Ok(sorted)
    }

    /// Visit a feature in dependency graph (for topological sort)
    fn visit_feature(
        &self,
        name: &str,
        sorted: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
        visiting: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        if visited.contains(name) {
            return Ok(());
        }

        if visiting.contains(name) {
            return Err(anyhow::anyhow!("Circular dependency detected: {}", name));
        }

        visiting.insert(name.to_string());

        if let Some(feature) = self.features.get(name) {
            for dep in &feature.dependencies {
                self.visit_feature(dep, sorted, visited, visiting)?;
            }
        }

        visiting.remove(name);
        visited.insert(name.to_string());
        sorted.push(name.to_string());

        Ok(())
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

/// Parse window string (e.g., "24h", "7d", "30d") to WindowConfig
#[allow(dead_code)]
fn parse_window(window_str: &str) -> Option<crate::feature::operator::WindowConfig> {
    use crate::feature::operator::{WindowConfig, WindowUnit};

    if window_str.is_empty() {
        return None;
    }

    // Parse patterns like "24h", "7d", "30d", "1h"
    let len = window_str.len();
    if len < 2 {
        return None;
    }

    let unit_char = window_str.chars().last()?;
    let value_str = &window_str[..len-1];
    let value = value_str.parse::<u64>().ok()?;

    let unit = match unit_char {
        'h' => WindowUnit::Hours,
        'd' => WindowUnit::Days,
        'm' => WindowUnit::Minutes,
        _ => return None,
    };

    Some(WindowConfig { value, unit })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::operator::Operator;
    use std::time::Duration;
    use tokio::time::sleep;

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
            &feature_values
        ).unwrap();

        assert_eq!(result, Value::Number(0.3));
    }

    #[test]
    fn test_substitute_template() {
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("user123".to_string()));
        context.insert("device_id".to_string(), Value::String("device456".to_string()));

        // Test user_id substitution
        let result = ExpressionEvaluator::substitute_template("{event.user_id}", &context).unwrap();
        assert_eq!(result, "user123");

        // Test device_id substitution
        let result = ExpressionEvaluator::substitute_template("{event.device_id}", &context).unwrap();
        assert_eq!(result, "device456");

        // Test with numeric value
        context.insert("count".to_string(), Value::Number(42.0));
        let result = ExpressionEvaluator::substitute_template("{event.count}", &context).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_substitute_template_with_prefix() {
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("123".to_string()));

        // Test with key prefix
        let result = ExpressionEvaluator::substitute_template("user_risk:{event.user_id}", &context).unwrap();
        assert_eq!(result, "user_risk:123");
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
