# corint-sdk

**High-Level SDK for CORINT Decision Engine**

## Overview

`corint-sdk` provides a high-level, user-friendly API for building and executing decision engines. It abstracts the complexity of parsing, compilation, and runtime execution into a simple builder pattern interface. This is the primary entry point for applications integrating CORINT decision logic.

## Key Responsibilities

### 1. Unified API
- Simple builder pattern for engine configuration
- Fluent interface for adding rules, rulesets, and pipelines
- Automatic compilation and execution flow
- Minimal boilerplate for common use cases

### 2. Configuration Management
- Engine configuration (metrics, tracing, optimizations)
- Storage configuration (database, cache)
- LLM integration configuration
- Service configuration (API endpoints, timeouts)

### 3. Decision Execution
- Request/Response abstraction
- Automatic request ID generation
- Pipeline routing (registry-based or legacy)
- Result aggregation and formatting

### 4. Integration Layer
- Combines parser, compiler, and runtime
- Manages feature executors and datasources
- Handles decision result persistence
- Provides metrics collection

## Architecture

```
┌─────────────────────────────────────────┐
│   Application Code                      │
│   (uses DecisionEngineBuilder)         │
└─────────────┬───────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│   DecisionEngine                        │
│   - Load & compile rules                │
│   - Route events to pipelines           │
│   - Execute decisions                   │
└─────────────┬───────────────────────────┘
              │
              ├──────────────────┬──────────────────┐
              ▼                  ▼                  ▼
┌──────────────────────┐  ┌──────────────┐  ┌──────────────┐
│  corint-parser       │  │  corint-     │  │  corint-     │
│  (YAML → AST)       │  │  compiler    │  │  runtime     │
│                     │  │  (AST → IR)  │  │  (Execute IR)│
└──────────────────────┘  └──────────────┘  └──────────────┘
```

## Core Components

### DecisionEngineBuilder
Fluent API for constructing decision engines:
```rust
pub struct DecisionEngineBuilder {
    config: EngineConfig,
    feature_executor: Option<Arc<FeatureExecutor>>,
    result_writer: Option<Arc<DecisionResultWriter>>,
}

impl DecisionEngineBuilder {
    /// Add rule files from filesystem
    pub fn add_rule_file(mut self, path: impl Into<PathBuf>) -> Self;

    /// Add rule content directly (from repository/API)
    pub fn add_rule_content(mut self, id: impl Into<String>, content: impl Into<String>) -> Self;

    /// Set pipeline registry for event routing
    pub fn with_registry_file(mut self, path: impl Into<PathBuf>) -> Self;
    pub fn with_registry_content(mut self, content: impl Into<String>) -> Self;

    /// Configure feature executor for lazy feature calculation
    pub fn with_feature_executor(mut self, executor: Arc<FeatureExecutor>) -> Self;

    /// Enable decision result persistence (requires sqlx feature)
    #[cfg(feature = "sqlx")]
    pub fn with_result_writer(mut self, pool: sqlx::PgPool) -> Self;

    /// Build the engine
    pub async fn build(self) -> Result<DecisionEngine>;
}
```

### DecisionEngine
Main engine for executing decisions:
```rust
pub struct DecisionEngine {
    programs: Vec<Program>,           // All compiled programs
    ruleset_map: HashMap<String, Program>,  // Ruleset ID → Program
    rule_map: HashMap<String, Program>,     // Rule ID → Program
    pipeline_map: HashMap<String, Program>, // Pipeline ID → Program
    registry: Option<PipelineRegistry>,     // Optional routing registry
    executor: Arc<PipelineExecutor>,        // Runtime executor
    metrics: Arc<MetricsCollector>,         // Metrics tracking
    config: EngineConfig,                   // Engine configuration
    result_writer: Option<Arc<DecisionResultWriter>>, // Optional persistence
}

impl DecisionEngine {
    /// Create from configuration
    pub async fn new(config: EngineConfig) -> Result<Self>;

    /// Execute a decision request
    pub async fn decide(&self, request: DecisionRequest) -> Result<DecisionResponse>;

    /// Get metrics collector
    pub fn metrics(&self) -> Arc<MetricsCollector>;
}
```

### DecisionRequest
Request abstraction:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRequest {
    /// Event data (user_id, amount, etc.)
    pub event_data: HashMap<String, Value>,

    /// Request metadata (request_id, event_id, etc.)
    pub metadata: HashMap<String, String>,
}

impl DecisionRequest {
    /// Create a new request
    pub fn new(event_data: HashMap<String, Value>) -> Self;

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self;
}
```

### DecisionResponse
Response abstraction:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResponse {
    /// Unique request ID (auto-generated if not provided)
    pub request_id: String,

    /// Pipeline ID that processed this request
    pub pipeline_id: Option<String>,

    /// Decision result (action, score, triggered rules)
    pub result: DecisionResult,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Request metadata (echoed back)
    pub metadata: HashMap<String, String>,
}
```

## Usage

### Basic Usage (File-based)

```rust
use corint_sdk::{DecisionEngineBuilder, DecisionRequest, Value};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build engine from YAML files
    let engine = DecisionEngineBuilder::new()
        .add_rule_file("rules/fraud_detection.yaml")
        .add_rule_file("rules/payment_validation.yaml")
        .with_registry_file("registry.yaml")
        .enable_metrics(true)
        .build()
        .await?;

    // Create decision request
    let mut event_data = HashMap::new();
    event_data.insert("type".to_string(), Value::String("transaction".to_string()));
    event_data.insert("user_id".to_string(), Value::String("user_123".to_string()));
    event_data.insert("amount".to_string(), Value::Number(5000.0));

    let request = DecisionRequest::new(event_data);

    // Execute decision
    let response = engine.decide(request).await?;

    println!("Request ID: {}", response.request_id);
    println!("Action: {:?}", response.result.action);
    println!("Score: {}", response.result.score);
    println!("Triggered Rules: {:?}", response.result.triggered_rules);
    println!("Processing Time: {}ms", response.processing_time_ms);

    Ok(())
}
```

### Advanced Usage (Repository-based with Features)

```rust
use corint_sdk::{DecisionEngineBuilder, DecisionRequest};
use corint_runtime::feature::{FeatureExecutor, FeatureRegistry, DatasourceRegistry};
use corint_runtime::datasource::PostgresClient;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up datasources for feature calculation
    let postgres = PostgresClient::new("postgresql://localhost/fraud_db").await?;

    let mut datasources = DatasourceRegistry::new();
    datasources.register("postgres_events", Arc::new(postgres));

    // Load feature definitions
    let feature_registry = FeatureRegistry::from_yaml("features/user_features.yaml")?;

    // Create feature executor for lazy feature calculation
    let feature_executor = Arc::new(
        FeatureExecutor::new(feature_registry, datasources)
    );

    // Build engine with repository content
    let engine = DecisionEngineBuilder::new()
        .add_rule_content("fraud_pipeline", load_from_repository("fraud_pipeline")?)
        .add_rule_content("payment_pipeline", load_from_repository("payment_pipeline")?)
        .with_registry_content(load_from_repository("registry")?)
        .with_feature_executor(feature_executor)
        .enable_semantic_analysis(true)
        .enable_constant_folding(true)
        .build()
        .await?;

    // Execute decision (features calculated on-demand)
    let mut event_data = HashMap::new();
    event_data.insert("user_id".to_string(), Value::String("user_123".to_string()));

    let response = engine.decide(DecisionRequest::new(event_data)).await?;

    println!("Decision: {:?}", response.result.action);

    Ok(())
}

fn load_from_repository(id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Load from database, API, or filesystem
    todo!("Implement repository loading")
}
```

### With Decision Result Persistence

```rust
use corint_sdk::DecisionEngineBuilder;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create database pool
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect("postgresql://localhost/decisions_db")
        .await?;

    // Build engine with result persistence
    let engine = DecisionEngineBuilder::new()
        .add_rule_file("rules/fraud_detection.yaml")
        .with_registry_file("registry.yaml")
        .with_result_writer(pool)  // Enable automatic persistence
        .build()
        .await?;

    // Decisions are automatically saved to database
    let response = engine.decide(request).await?;

    // Results are persisted asynchronously with:
    // - request_id, event_id, pipeline_id
    // - decision action, score, triggered rules
    // - processing time
    // - individual rule execution details

    Ok(())
}
```

## Configuration

### EngineConfig
```rust
pub struct EngineConfig {
    /// Rule files to load (filesystem paths)
    pub rule_files: Vec<PathBuf>,

    /// Rule contents (from repository/API)
    pub rule_contents: Vec<(String, String)>,

    /// Registry file for pipeline routing
    pub registry_file: Option<PathBuf>,

    /// Registry content (alternative to file)
    pub registry_content: Option<String>,

    /// Storage configuration
    pub storage: Option<StorageConfig>,

    /// LLM configuration
    pub llm: Option<LLMConfig>,

    /// Service configuration
    pub service: Option<ServiceConfig>,

    /// Enable metrics collection
    pub enable_metrics: bool,

    /// Enable tracing
    pub enable_tracing: bool,

    /// Compiler options
    pub compiler_options: CompilerOptions,
}
```

### Compiler Options
```rust
pub struct CompilerOptions {
    /// Enable semantic analysis
    pub enable_semantic_analysis: bool,

    /// Enable constant folding optimization
    pub enable_constant_folding: bool,

    /// Enable dead code elimination
    pub enable_dead_code_elimination: bool,
}
```

## Pipeline Routing

### Registry-based Routing (Recommended)

Registry provides declarative event-to-pipeline routing:

```yaml
# registry.yaml
version: "0.1"
registry:
  - pipeline: fraud_detection
    when:
      event.type: transaction
      conditions:
        - event.amount > 100

  - pipeline: login_security
    when:
      event.type: login

  - pipeline: default_pipeline
    when:
      default: true
```

First match wins (top-to-bottom evaluation).

### Legacy Pipeline Routing

Without registry, pipelines are matched by `event_type` metadata in pipeline definition.

## Request ID Generation

Request IDs are automatically generated using the format:
```
req_YYYYMMDDHHmmss_xxxxxx
```

Example: `req_20231209143052_a3f2e1`

Components:
- Timestamp: UTC time in `YYYYMMDDHHmmss` format
- Random suffix: 6 hex digits for uniqueness

You can also provide custom request IDs via metadata:
```rust
let request = DecisionRequest::new(event_data)
    .with_metadata("request_id".to_string(), "custom_req_123".to_string());
```

## Error Handling

```rust
pub enum SdkError {
    /// Rule file not found or invalid
    InvalidRuleFile(String),

    /// Compilation error
    CompileError(CompilerError),

    /// Runtime error
    RuntimeError(RuntimeError),

    /// I/O error
    Io(#[from] std::io::Error),

    /// Parser error
    ParseError(#[from] ParseError),
}

pub type Result<T> = std::result::Result<T, SdkError>;
```

## Features

- **default**: Basic SDK without database support
- **sqlx**: Enable PostgreSQL integration for:
  - Feature calculation from database
  - Decision result persistence
  - Rule execution audit logs

## Performance Optimizations

### 1. Lazy Feature Calculation
Features are only calculated when referenced in rules:
```rust
// Rule: features.max_transaction_7d > 400
// → Feature calculated on first access
// → Result cached for request duration
// → Subsequent accesses use cached value
```

### 2. Compilation Cache
Compiled programs are cached in memory:
```rust
// Compile once at engine startup
let engine = DecisionEngineBuilder::new()
    .add_rule_file("rules.yaml")
    .build()
    .await?;

// Execute many times (no re-compilation)
for event in events {
    let response = engine.decide(event).await?;
}
```

### 3. Async I/O
All I/O operations are non-blocking:
```rust
// Multiple datasource queries execute concurrently
// Database connections use connection pooling
// API calls are fully async
```

## Testing

```rust
#[tokio::test]
async fn test_simple_decision() {
    let engine = DecisionEngineBuilder::new()
        .add_rule_file("test_rules.yaml")
        .build()
        .await
        .unwrap();

    let mut event_data = HashMap::new();
    event_data.insert("amount".to_string(), Value::Number(1500.0));

    let request = DecisionRequest::new(event_data);
    let response = engine.decide(request).await.unwrap();

    assert_eq!(response.result.action, Some(Action::Review));
}

#[tokio::test]
async fn test_with_metadata() {
    let request = DecisionRequest::new(event_data)
        .with_metadata("event_id".to_string(), "evt_123".to_string())
        .with_metadata("source".to_string(), "mobile_app".to_string());

    let response = engine.decide(request).await.unwrap();

    assert_eq!(response.metadata.get("event_id"), Some(&"evt_123".to_string()));
    assert!(response.request_id.starts_with("req_"));
}
```

## Dependencies

- `corint-core`: Core types (AST, IR, Value)
- `corint-parser`: YAML to AST parsing
- `corint-compiler`: AST to IR compilation
- `corint-runtime`: IR execution engine
- `tokio`: Async runtime
- `serde`: Serialization
- `sqlx` (optional): PostgreSQL support

## Related Documentation

- [Getting Started Guide](../../README.md#quick-start)
- [DSL Overview](../../docs/dsl/overall.md)
- [Pipeline Registry](../../docs/dsl/overall.md#pipeline-registry)
- [Feature Engineering](../../docs/dsl/feature.md)
- [Example Applications](../../examples/README.md)
