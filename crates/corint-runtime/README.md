# corint-runtime

**Runtime Execution Engine for CORINT Decision Engine**

## Overview

`corint-runtime` executes compiled decision logic (IR) in production environments. It provides the core execution engine, feature calculation, datasource integration, and runtime context management.

## Key Responsibilities

### 1. IR Execution
- Execute compiled intermediate representation (IR) instructions
- Evaluate conditions and expressions
- Manage execution flow and short-circuit optimization

### 2. Feature Calculation
- On-demand feature computation from datasources
- Built-in statistical operators (count, sum, max, count_distinct, etc.)
- Time-window aggregations (last_7d, last_24h, etc.)
- Feature value caching

### 3. Datasource Integration
- PostgreSQL/Supabase: SQL query generation and execution
- Redis: Feature store access
- ClickHouse: OLAP queries
- HTTP APIs: External service integration

### 4. Context Management
- Runtime context (event_data, features, temporary variables)
- Variable scoping and lifecycle
- Result tracking and audit trails

## Architecture

```
┌─────────────────────────────────────────┐
│   DecisionRequest                       │
│   { event_data, context }              │
└─────────────┬───────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│   PipelineExecutor                      │
│   - Execute IR instructions            │
│   - Manage execution flow              │
└─────────────┬───────────────────────────┘
              │
              ├──────────────────┐
              ▼                  ▼
┌──────────────────────┐  ┌─────────────────────┐
│  ExpressionEvaluator │  │  FeatureExecutor    │
│  - Evaluate          │  │  - Calculate        │
│    conditions        │  │    features         │
│  - Binary ops        │  │  - Query datasources│
│  - Function calls    │  │  - Cache results    │
└──────────────────────┘  └─────────┬───────────┘
                                    │
                                    ▼
                          ┌─────────────────────┐
                          │  DatasourceClient   │
                          │  - PostgreSQL       │
                          │  - Redis            │
                          │  - ClickHouse       │
                          │  - HTTP APIs        │
                          └─────────────────────┘
```

## Core Components

### PipelineExecutor
Executes compiled pipeline IR:
```rust
pub struct PipelineExecutor {
    feature_executor: Arc<FeatureExecutor>,
}

impl PipelineExecutor {
    /// Execute a pipeline with event data
    pub async fn execute(
        &self,
        pipeline: &CompiledPipeline,
        event_data: HashMap<String, Value>
    ) -> Result<DecisionResult>;

    /// Execute a single ruleset
    pub async fn execute_ruleset(
        &self,
        ruleset: &CompiledRuleset,
        context: &mut ExecutionContext
    ) -> Result<RulesetResult>;
}
```

### FeatureExecutor
Calculates features on-demand:
```rust
pub struct FeatureExecutor {
    registry: Arc<FeatureRegistry>,
    datasources: Arc<DatasourceRegistry>,
}

impl FeatureExecutor {
    /// Calculate a feature value
    pub async fn calculate_feature(
        &self,
        feature_name: &str,
        context: &HashMap<String, Value>
    ) -> Result<Value>;

    /// Batch calculate multiple features
    pub async fn calculate_features(
        &self,
        features: &[String],
        context: &HashMap<String, Value>
    ) -> Result<HashMap<String, Value>>;
}
```

### DatasourceClient
Abstracts different datasource types:
```rust
#[async_trait]
pub trait DatasourceClient: Send + Sync {
    /// Execute a query and return results
    async fn execute(&self, query: &Query) -> Result<QueryResult>;

    /// Check connection health
    async fn health_check(&self) -> Result<bool>;
}

pub struct PostgresClient {
    pool: PgPool,
}

pub struct RedisClient {
    client: redis::Client,
}
```

### ExecutionContext
Manages runtime state:
```rust
pub struct ExecutionContext {
    /// Event data (user input + calculated features)
    pub event_data: HashMap<String, Value>,

    /// Temporary variables during execution
    pub variables: HashMap<String, Value>,

    /// Triggered rules and their scores
    pub triggered_rules: Vec<TriggeredRule>,

    /// Total accumulated score
    pub total_score: i32,
}

impl ExecutionContext {
    /// Get field value (event_data -> variables -> features)
    pub fn get_field(&self, path: &[String]) -> Option<&Value>;

    /// Set field value
    pub fn set_field(&mut self, path: Vec<String>, value: Value);

    /// Add triggered rule
    pub fn add_triggered_rule(&mut self, rule_id: String, score: i32);
}
```

## Feature Calculation

### Supported Operators

#### Aggregation Operators
- `count`: Count matching records
- `sum`: Sum numeric values
- `max`/`min`: Maximum/minimum values
- `avg`: Average value
- `count_distinct`: Count unique values

#### Statistical Operators (planned)
- `percentile`: Calculate percentiles
- `stddev`: Standard deviation
- `variance`: Variance

### Time Windows
- `last_Nd`: Last N days (e.g., `last_7d`, `last_30d`)
- `last_Nh`: Last N hours (e.g., `last_1h`, `last_24h`)
- `last_n_seconds`: Custom time window in seconds

### Query Generation

Example feature definition:
```yaml
name: transaction_sum_7d
operator: sum
datasource: supabase_events
entity: events
dimension: user_id
dimension_value: "{event.user_id}"
field: amount
window:
  value: 7
  unit: days
filters:
  - field: event_type
    operator: eq
    value: "transaction"
```

Generated SQL:
```sql
SELECT SUM((attributes->>'amount')::numeric) AS sum
FROM events
WHERE user_id = $1
  AND event_type = 'transaction'
  AND event_timestamp >= NOW() - INTERVAL '604800 seconds'
```

## Usage

### Basic Pipeline Execution

```rust
use corint_runtime::PipelineExecutor;
use std::collections::HashMap;

let executor = PipelineExecutor::new(feature_executor);

let mut event_data = HashMap::new();
event_data.insert("type".to_string(), Value::String("transaction".to_string()));
event_data.insert("user_id".to_string(), Value::String("user_001".to_string()));
event_data.insert("amount".to_string(), Value::Number(1500.0));

let result = executor.execute(&compiled_pipeline, event_data).await?;

println!("Action: {:?}", result.action);
println!("Score: {}", result.score);
println!("Triggered rules: {:?}", result.triggered_rules);
```

### Feature Calculation

```rust
use corint_runtime::FeatureExecutor;

let feature_executor = FeatureExecutor::new(registry, datasources);

let mut context = HashMap::new();
context.insert("user_id".to_string(), Value::String("user_001".to_string()));

// Calculate single feature
let value = feature_executor
    .calculate_feature("transaction_sum_7d", &context)
    .await?;

// Calculate multiple features
let features = feature_executor
    .calculate_features(
        &["transaction_sum_7d", "transaction_count_24h"],
        &context
    )
    .await?;
```

### Datasource Configuration

```rust
use corint_runtime::datasource::{PostgresClient, RedisClient, DatasourceRegistry};

// Create datasource clients
let postgres = PostgresClient::new(&database_url).await?;
let redis = RedisClient::new("redis://localhost")?;

// Register datasources
let mut registry = DatasourceRegistry::new();
registry.register("supabase_events", Arc::new(postgres));
registry.register("redis_features", Arc::new(redis));
```

## Performance Optimizations

### 1. Lazy Feature Calculation
Features are only calculated when referenced:
```rust
// Rule condition: features.max_transaction_7d > 400
// ↓
// LoadField("features.max_transaction_7d")
// ↓ (field not found in event_data)
// ↓
// FeatureExecutor.calculate_feature("max_transaction_7d")
// ↓ (query database)
// ↓ (cache result in event_data)
// ↓
// Continue rule evaluation
```

### 2. Feature Caching
Calculated features are cached in `event_data` for the duration of the request:
```rust
// First access: Query database
let value = context.get_field(&["features", "max_transaction_7d"]);  // DB query

// Subsequent accesses: Use cached value
let value = context.get_field(&["features", "max_transaction_7d"]);  // Cache hit
```

### 3. Connection Pooling
Database connections are pooled for reuse:
```rust
let postgres = PostgresClient::new_with_pool(
    &database_url,
    PoolOptions::new().max_connections(10)
).await?;
```

### 4. Async I/O
All I/O operations are non-blocking:
```rust
// Execute multiple datasource queries concurrently
let (postgres_result, redis_result) = tokio::join!(
    postgres_client.execute(&query1),
    redis_client.execute(&query2)
);
```

## Error Handling

```rust
pub enum RuntimeError {
    /// Field not found in context
    FieldNotFound { path: String },

    /// Feature calculation failed
    FeatureError { feature: String, source: Box<dyn Error> },

    /// Datasource query failed
    DatasourceError { datasource: String, source: Box<dyn Error> },

    /// Type mismatch during evaluation
    TypeError { expected: String, found: String },

    /// Division by zero or other arithmetic error
    ArithmeticError { operation: String },
}
```

## Testing

```rust
#[tokio::test]
async fn test_execute_simple_rule() {
    let executor = PipelineExecutor::new(feature_executor);

    let mut event_data = HashMap::new();
    event_data.insert("amount".to_string(), Value::Number(1500.0));

    let result = executor.execute(&pipeline, event_data).await.unwrap();
    assert_eq!(result.action, Action::Review);
}

#[tokio::test]
async fn test_feature_calculation() {
    let feature_executor = FeatureExecutor::new(registry, datasources);

    let mut context = HashMap::new();
    context.insert("user_id".to_string(), Value::String("user_001".to_string()));

    let value = feature_executor
        .calculate_feature("max_transaction_7d", &context)
        .await
        .unwrap();

    assert!(matches!(value, Value::Number(_)));
}
```

## Features

- **default**: Basic runtime without datasource integrations
- **sqlx**: Enable PostgreSQL/SQL datasource support

## Dependencies

- `corint-core`: Core type definitions
- `tokio`: Async runtime
- `sqlx` (optional): PostgreSQL client
- `async-trait`: Async trait support

## Related Documentation

- [Feature Engineering](../../docs/dsl/feature.md)
- [Datasource Configuration](../../docs/dsl/feature.md#datasources)
- [Expression Evaluation](../../docs/dsl/expression.md)
- [Performance Optimization](../../docs/dsl/performance.md)
