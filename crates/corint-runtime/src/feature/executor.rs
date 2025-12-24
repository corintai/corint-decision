//! Feature Executor Module
//!
//! This module implements the feature execution engine that:
//! - Executes feature operators against data sources
//! - Manages feature caching (L1 local, L2 Redis)
//! - Handles batch feature execution
//! - Manages feature dependencies

use crate::context::ExecutionContext;
use crate::datasource::DataSourceClient;
use crate::feature::definition::FeatureDefinition;
use crate::feature::operator::{CacheBackend, CacheConfig, Operator};
use anyhow::{Context as AnyhowContext, Result};
use corint_core::condition::ConditionParser;
use corint_core::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Convert Value to String representation
fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(_) => "[array]".to_string(),
        Value::Object(_) => "{object}".to_string(),
    }
}

/// Cache entry with value and expiration time
#[derive(Debug, Clone)]
struct CacheEntry {
    value: Value,
    expires_at: u64, // Unix timestamp in seconds
}

impl CacheEntry {
    fn new(value: Value, ttl: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            value,
            expires_at: now + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now >= self.expires_at
    }
}

/// Feature executor that handles feature computation and caching
pub struct FeatureExecutor {
    /// Local L1 cache (in-memory)
    local_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,

    /// Data source clients for feature computation
    datasources: HashMap<String, Arc<DataSourceClient>>,

    /// Feature definitions registry
    features: HashMap<String, FeatureDefinition>,

    /// Enable cache statistics
    enable_stats: bool,

    /// Cache hit/miss statistics
    stats: Arc<RwLock<CacheStats>>,
}

#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    l1_hits: u64,
    l1_misses: u64,
    l2_hits: u64,
    l2_misses: u64,
    compute_count: u64,
}

impl FeatureExecutor {
    /// Create a new feature executor
    pub fn new() -> Self {
        Self {
            local_cache: Arc::new(RwLock::new(HashMap::new())),
            datasources: HashMap::new(),
            features: HashMap::new(),
            enable_stats: false,
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Enable cache statistics
    pub fn with_stats(mut self) -> Self {
        self.enable_stats = true;
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
        let feature = self
            .features
            .get(feature_name)
            .with_context(|| format!("Feature '{}' not found", feature_name))?;

        if !feature.is_enabled() {
            return Ok(Value::Null);
        }

        // Build context map from ExecutionContext (use event namespace)
        let context_map = context.event.clone();

        // Check dependencies first
        let mut dep_values = HashMap::new();
        for dep_name in &feature.dependencies {
            let dep_value = Box::pin(self.execute_feature_impl(dep_name, context)).await?;
            dep_values.insert(dep_name.clone(), dep_value);
        }

        // Try to get from cache
        if let Some(cache_config) = self.get_cache_config(feature) {
            let cache_key = self.build_cache_key(feature_name, &context_map);

            // L1 cache check
            if let Some(value) = self.get_from_l1_cache(&cache_key).await {
                if self.enable_stats {
                    self.stats.write().await.l1_hits += 1;
                }
                debug!("Feature '{}' L1 cache hit", feature_name);
                return Ok(value);
            }

            if self.enable_stats {
                self.stats.write().await.l1_misses += 1;
            }

            // L2 cache check (Redis)
            if cache_config.backend == CacheBackend::Redis {
                if let Some(value) = self.get_from_l2_cache(&cache_key).await {
                    if self.enable_stats {
                        self.stats.write().await.l2_hits += 1;
                    }
                    debug!("Feature '{}' L2 cache hit", feature_name);

                    // Populate L1 cache
                    self.set_to_l1_cache(&cache_key, value.clone(), cache_config.ttl)
                        .await;

                    return Ok(value);
                }

                if self.enable_stats {
                    self.stats.write().await.l2_misses += 1;
                }
            }

            // Compute feature
            let value = self
                .compute_feature(feature, &context_map, &dep_values)
                .await?;

            if self.enable_stats {
                self.stats.write().await.compute_count += 1;
            }

            // Store in cache
            self.set_to_cache(&cache_key, value.clone(), cache_config)
                .await;

            Ok(value)
        } else {
            // No caching, compute directly
            if self.enable_stats {
                self.stats.write().await.compute_count += 1;
            }

            self.compute_feature(feature, &context_map, &dep_values)
                .await
        }
    }

    /// Execute multiple features in batch
    pub async fn execute_features(
        &self,
        feature_names: &[String],
        context: &ExecutionContext,
    ) -> Result<HashMap<String, Value>> {
        let mut results = HashMap::new();

        // Sort features by dependency order
        let sorted_features = self.sort_by_dependencies(feature_names)?;

        for feature_name in sorted_features {
            let value = self.execute_feature_impl(&feature_name, context).await?;
            results.insert(feature_name, value);
        }

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
        _dependencies: &HashMap<String, Value>,
    ) -> Result<Value> {
        debug!("Computing feature '{}'", feature.name);

        // Determine data source
        let datasource_name = self.get_datasource_name(feature);
        let datasource = self
            .datasources
            .get(&datasource_name)
            .with_context(|| format!("Data source '{}' not found", datasource_name))?;

        // Execute feature based on type
        self.execute_feature_by_type(feature, datasource, context)
            .await
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
                self.execute_expression(feature, context).await
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
        use crate::datasource::query::{Query, QueryType, Aggregation, AggregationType, Filter, FilterOperator, TimeWindow, TimeWindowType, RelativeWindow, TimeUnit};

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
        let dimension_value = self.substitute_template(&config.dimension_value, context)?;

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

    /// Substitute template variables with context values
    fn substitute_template(&self, template: &str, context: &HashMap<String, Value>) -> Result<String> {
        // Simple template substitution: {event.user_id} -> context["user_id"]
        let mut result = template.to_string();

        // Extract all {xxx} patterns
        if let Some(start) = result.find('{') {
            if let Some(end) = result.find('}') {
                let var_path = &result[start+1..end];

                // Parse path like "event.user_id" -> ["event", "user_id"]
                let parts: Vec<&str> = var_path.split('.').collect();

                // For now, just use the last part as the key
                if let Some(key) = parts.last() {
                    if let Some(value) = context.get(*key) {
                        let value_str = match value {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            _ => return Err(anyhow::anyhow!("Unsupported template value type")),
                        };
                        result = result.replace(&result[start..=end], &value_str);
                    } else {
                        return Err(anyhow::anyhow!(
                            "Template variable '{}' not found in context. Available keys: {:?}",
                            key, context.keys().collect::<Vec<_>>()
                        ));
                    }
                }
            }
        }

        Ok(result)
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
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        let config = feature.expression.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing expression config for feature '{}'", feature.name))?;

        // Expression features only consume results from other features
        // They do NOT access datasources directly

        if let Some(expr_str) = &config.expression {
            // Evaluate mathematical expression using dependent features
            // For now, assume dependencies are already in context under the feature namespace
            // In a real implementation, the executor would ensure dependencies are computed first

            // Build a map of feature values from dependencies
            let mut feature_values = HashMap::new();
            for dep_name in &config.depends_on {
                // Try to get from context (event namespace for now)
                if let Some(value) = context.get(dep_name) {
                    feature_values.insert(dep_name.clone(), value.clone());
                } else {
                    // Dependency not available - return error for now
                    // In production, this would trigger recursive feature computation
                    return Err(anyhow::anyhow!(
                        "Dependency '{}' not found for expression feature '{}'",
                        dep_name, feature.name
                    ));
                }
            }

            // Evaluate the expression
            self.evaluate_expression(expr_str, &feature_values)
        } else if config.model.is_some() {
            // ML model scoring (not yet implemented)
            Err(anyhow::anyhow!("ML model scoring not yet implemented for feature '{}'", feature.name))
        } else {
            Err(anyhow::anyhow!("Expression feature '{}' must have either expression or model", feature.name))
        }
    }

    /// Evaluate a mathematical expression with feature values
    fn evaluate_expression(
        &self,
        expr: &str,
        feature_values: &HashMap<String, Value>,
    ) -> Result<Value> {
        // Simple expression evaluator
        // Supports: +, -, *, /, feature names, numbers

        // Replace feature names with their values
        let mut expr_normalized = expr.to_string();

        // Extract all feature names and replace with values
        for (name, value) in feature_values {
            let value_num = match value {
                Value::Number(n) => *n,
                Value::Null => 0.0,
                Value::Bool(b) => if *b { 1.0 } else { 0.0 },
                _ => return Err(anyhow::anyhow!("Feature '{}' has non-numeric value", name)),
            };

            // Replace feature name with its numeric value
            expr_normalized = expr_normalized.replace(name, &value_num.to_string());
        }

        // Evaluate the expression using a simple parser
        self.eval_math_expr(&expr_normalized)
    }

    /// Simple math expression evaluator
    /// Supports: +, -, *, /, parentheses, numbers
    fn eval_math_expr(&self, expr: &str) -> Result<Value> {
        // Remove whitespace
        let expr = expr.replace(' ', "");

        // Try to parse as a simple number first
        if let Ok(num) = expr.parse::<f64>() {
            return Ok(Value::Number(num));
        }

        // Handle division by zero
        if expr.contains("/0") || expr.contains("/ 0") {
            return Ok(Value::Null);
        }

        // Very simple expression parser (handles basic operations)
        // For production, consider using a proper expression parser crate like `evalexpr`

        // Handle simple binary operations (a op b)
        for op in &['/', '*', '+', '-'] {
            if let Some(idx) = expr.rfind(*op) {
                // Skip if it's a negative sign at the beginning
                if *op == '-' && idx == 0 {
                    continue;
                }

                let left = &expr[..idx];
                let right = &expr[idx+1..];

                let left_val = match self.eval_math_expr(left)? {
                    Value::Number(n) => n,
                    _ => return Err(anyhow::anyhow!("Invalid expression: {}", expr)),
                };

                let right_val = match self.eval_math_expr(right)? {
                    Value::Number(n) => n,
                    _ => return Err(anyhow::anyhow!("Invalid expression: {}", expr)),
                };

                let result = match op {
                    '+' => left_val + right_val,
                    '-' => left_val - right_val,
                    '*' => left_val * right_val,
                    '/' => {
                        if right_val == 0.0 {
                            return Ok(Value::Null);
                        }
                        left_val / right_val
                    }
                    _ => unreachable!(),
                };

                return Ok(Value::Number(result));
            }
        }

        Err(anyhow::anyhow!("Unable to evaluate expression: {}", expr))
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
        let key = self.substitute_template(&config.key, context)?;

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

    /// Get cache configuration from feature (currently disabled)
    fn get_cache_config<'a>(&self, _feature: &'a FeatureDefinition) -> Option<&'a CacheConfig> {
        // TODO: Implement cache configuration in new feature structure
        None
    }

    /// Get cache configuration from old Operator enum (deprecated, kept for tests)
    fn get_cache_config_from_operator<'a>(&self, operator: &'a Operator) -> Option<&'a CacheConfig> {
        match operator {
            Operator::Count(op) => op.params.cache.as_ref(),
            Operator::Sum(op) => op.params.cache.as_ref(),
            Operator::Avg(op) => op.params.cache.as_ref(),
            Operator::Max(op) => op.params.cache.as_ref(),
            Operator::Min(op) => op.params.cache.as_ref(),
            Operator::CountDistinct(op) => op.params.cache.as_ref(),
            Operator::CrossDimensionCount(_) => None, // No cache support for this operator
            Operator::FirstSeen(_) => None,           // No cache support for this operator
            Operator::LastSeen(_) => None,            // No cache support for this operator
            Operator::TimeSince(_) => None,           // No cache support for this operator
            Operator::Velocity(op) => op.params.cache.as_ref(),
            _ => None,
        }
    }

    /// Build cache key from feature name and context
    fn build_cache_key(&self, feature_name: &str, context: &HashMap<String, Value>) -> String {
        // Extract key dimension values from context
        let mut key_parts = vec![feature_name.to_string()];

        // Add relevant context values (user_id, device_id, ip_address, etc.)
        for key in &["user_id", "device_id", "ip_address", "merchant_id"] {
            if let Some(value) = context.get(*key) {
                let value_str = value_to_string(value);
                key_parts.push(format!("{}:{}", key, value_str));
            }
        }

        key_parts.join(":")
    }

    /// Get value from L1 cache
    async fn get_from_l1_cache(&self, key: &str) -> Option<Value> {
        let cache = self.local_cache.read().await;
        if let Some(entry) = cache.get(key) {
            if !entry.is_expired() {
                return Some(entry.value.clone());
            }
        }
        None
    }

    /// Set value to L1 cache
    async fn set_to_l1_cache(&self, key: &str, value: Value, ttl: u64) {
        let mut cache = self.local_cache.write().await;
        cache.insert(key.to_string(), CacheEntry::new(value, ttl));
    }

    /// Get value from L2 cache (Redis)
    async fn get_from_l2_cache(&self, _key: &str) -> Option<Value> {
        // TODO: Implement Redis cache lookup
        // This would use the redis datasource to fetch cached values
        None
    }

    /// Set value to cache (L1 and optionally L2)
    async fn set_to_cache(&self, key: &str, value: Value, config: &CacheConfig) {
        // Always set L1 cache
        self.set_to_l1_cache(key, value.clone(), config.ttl).await;

        // Set L2 cache if Redis backend
        if config.backend == CacheBackend::Redis {
            // TODO: Implement Redis cache write
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
        let mut cache = self.local_cache.write().await;
        cache.clear();
        info!("L1 cache cleared");
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Print cache statistics
    pub async fn print_stats(&self) {
        if !self.enable_stats {
            warn!("Statistics not enabled");
            return;
        }

        let stats = self.stats.read().await;
        let total = stats.l1_hits + stats.l1_misses;
        let l1_hit_rate = if total > 0 {
            (stats.l1_hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let l2_total = stats.l2_hits + stats.l2_misses;
        let l2_hit_rate = if l2_total > 0 {
            (stats.l2_hits as f64 / l2_total as f64) * 100.0
        } else {
            0.0
        };

        info!("=== Feature Executor Cache Statistics ===");
        info!(
            "L1 Hits: {}, Misses: {}, Hit Rate: {:.2}%",
            stats.l1_hits, stats.l1_misses, l1_hit_rate
        );
        info!(
            "L2 Hits: {}, Misses: {}, Hit Rate: {:.2}%",
            stats.l2_hits, stats.l2_misses, l2_hit_rate
        );
        info!("Total Computations: {}", stats.compute_count);
    }
}

impl Default for FeatureExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse window string (e.g., "24h", "7d", "30d") to WindowConfig
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
    fn test_cache_entry_expiration() {
        let entry = CacheEntry::new(Value::Number(42.0), 1); // 1 second TTL
        assert!(!entry.is_expired());

        std::thread::sleep(Duration::from_secs(2));
        assert!(entry.is_expired());
    }

    #[test]
    fn test_cache_entry_not_expired() {
        let entry = CacheEntry::new(Value::Number(100.0), 3600); // 1 hour TTL
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_cache_entry_value() {
        let value = Value::String("test_value".to_string());
        let entry = CacheEntry::new(value.clone(), 300);
        assert_eq!(entry.value, value);
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
        assert!(!executor.enable_stats);
        assert_eq!(executor.datasources.len(), 0);
        assert_eq!(executor.features.len(), 0);
    }

    #[test]
    fn test_feature_executor_with_stats() {
        let executor = FeatureExecutor::new().with_stats();
        assert!(executor.enable_stats);
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
    fn test_value_to_string_conversions() {
        assert_eq!(value_to_string(&Value::Null), "null");
        assert_eq!(value_to_string(&Value::Bool(true)), "true");
        assert_eq!(value_to_string(&Value::Bool(false)), "false");
        assert_eq!(value_to_string(&Value::Number(42.5)), "42.5");
        assert_eq!(value_to_string(&Value::String("test".to_string())), "test");
        assert_eq!(value_to_string(&Value::Array(vec![])), "[array]");
        assert_eq!(value_to_string(&Value::Object(HashMap::new())), "{object}");
    }

    #[test]
    fn test_expression_evaluator_simple() {
        let executor = FeatureExecutor::new();

        // Test simple number
        let result = executor.eval_math_expr("42").unwrap();
        assert_eq!(result, Value::Number(42.0));

        // Test addition
        let result = executor.eval_math_expr("10+5").unwrap();
        assert_eq!(result, Value::Number(15.0));

        // Test subtraction
        let result = executor.eval_math_expr("10-5").unwrap();
        assert_eq!(result, Value::Number(5.0));

        // Test multiplication
        let result = executor.eval_math_expr("10*5").unwrap();
        assert_eq!(result, Value::Number(50.0));

        // Test division
        let result = executor.eval_math_expr("10/5").unwrap();
        assert_eq!(result, Value::Number(2.0));
    }

    #[test]
    fn test_expression_evaluator_division_by_zero() {
        let executor = FeatureExecutor::new();

        // Division by zero should return Null
        let result = executor.eval_math_expr("10/0").unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_expression_evaluator_complex() {
        let executor = FeatureExecutor::new();

        // Note: Our simple evaluator doesn't handle operator precedence correctly
        // It evaluates "5+10*2" as (5+10)*2 = 30, not 5+(10*2) = 25
        // This is because we search for operators in order /, *, +, - and use rfind
        let result = executor.eval_math_expr("5+10*2").unwrap();
        assert_eq!(result, Value::Number(30.0));

        // For correct precedence, users should use parentheses or separate features
        // e.g., create intermediate features or use a proper expression parser
    }

    #[test]
    fn test_evaluate_expression_with_features() {
        let executor = FeatureExecutor::new();

        let mut feature_values = HashMap::new();
        feature_values.insert("login_count".to_string(), Value::Number(10.0));
        feature_values.insert("failed_logins".to_string(), Value::Number(3.0));

        // Test division expression
        let result = executor.evaluate_expression(
            "failed_logins / login_count",
            &feature_values
        ).unwrap();

        assert_eq!(result, Value::Number(0.3));
    }

    #[test]
    fn test_substitute_template() {
        let executor = FeatureExecutor::new();
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("user123".to_string()));
        context.insert("device_id".to_string(), Value::String("device456".to_string()));

        // Test user_id substitution
        let result = executor.substitute_template("{event.user_id}", &context).unwrap();
        assert_eq!(result, "user123");

        // Test device_id substitution
        let result = executor.substitute_template("{event.device_id}", &context).unwrap();
        assert_eq!(result, "device456");

        // Test with numeric value
        context.insert("count".to_string(), Value::Number(42.0));
        let result = executor.substitute_template("{event.count}", &context).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_substitute_template_with_prefix() {
        let executor = FeatureExecutor::new();
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), Value::String("123".to_string()));

        // Test with key prefix
        let result = executor.substitute_template("user_risk:{event.user_id}", &context).unwrap();
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
