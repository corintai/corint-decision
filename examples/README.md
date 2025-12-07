# CORINT Examples

This directory contains example implementations demonstrating how to use the CORINT Decision Engine SDK.

## Examples Overview

### 1. Simple Rule (`simple_rule.rs`)

A basic example showing how to:
- Create a DecisionEngine with a simple rule file
- Execute a decision request
- Handle the decision response

**Run:**
```bash
cargo run --example simple_rule
```

**What it does:** Checks if a transaction amount exceeds $100 and flags it for review.

### 2. Fraud Detection (`fraud_detection.rs`) ⭐ RECOMMENDED

**A comprehensive fraud detection system covering all core concepts:**

This example demonstrates a complete multi-layer fraud detection system with:
- 6-layer decision logic (critical → very high → high → medium-high → medium → low risk)
- Score-based thresholds and progressive risk assessment
- Feature-driven evaluation framework
- Action assignment (approve/review/deny)
- Terminate control for early exit
- Comprehensive documentation with DSL mapping

**Run:**
```bash
cargo run --example fraud_detection
```

**What it demonstrates:**
- **Multi-layer Decision Logic**: 6 risk levels with progressive thresholds
- **Feature Framework**: Structure for association, statistical, temporal, behavioral features
- **Action Assignment**: Approve, review, deny based on risk conditions
- **Terminate Control**: Early exit on high-risk decisions
- **Explainability**: Reason tracking for transparency
- **DSL Mapping**: Complete mapping to documentation concepts

**Concepts Covered** (from DSL documentation):
- Association Features: IP/Device/User relationship analysis (`devices_per_ip`, `users_per_device`)
- Statistical Features: Z-scores, percentiles, outlier detection (`amount_zscore`, `is_amount_outlier`)
- Velocity Features: Rate of change detection (`velocity_ratio`, `transaction_count_24h`)
- Temporal Features: Time patterns (`is_off_hours`, `days_since_last`)
- Behavioral Features: Activity patterns (`is_new_device`, `failed_login_count`)

**Test Scenarios:**
1. $50 → Approve (low risk)
2. $750 → Review (medium risk)
3. $2,500 → Review (medium-high risk)
4. $7,500 → Deny (high risk)
5. $15,000 → Deny (very high risk)
6. $25,000 → Deny (critical risk)

### 3. Feature Data Source (`feature_datasource_example.rs`)

Demonstrates how to configure and use different data sources for feature engineering.

**Run:**
```bash
cargo run --example feature_datasource_example
```

**What it demonstrates:**
- Parsing Redis Feature Store configuration
- Parsing ClickHouse OLAP database configuration
- Building count and count_distinct queries
- Defining time windows and filters
- Serializing queries to JSON

### 4. PostgreSQL Data Source (`postgres_datasource_example.rs`) ⭐ DATABASE EXAMPLE

**A comprehensive PostgreSQL integration example showing how to query databases for feature calculation:**

This example demonstrates how to configure and query PostgreSQL database for real-time feature calculation in risk control scenarios.

**Prerequisites:**

Start PostgreSQL using Docker:
```bash
docker run -d \
  --name postgres-corint \
  -e POSTGRES_DB=corint_risk \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=password \
  -p 5432:5432 \
  postgres:16

# Create schema
psql -h localhost -U postgres -d corint_risk -f docs/schema/postgres-schema.sql

# Insert sample data
psql -h localhost -U postgres -d corint_risk -f docs/schema/postgres-examples.sql
```

**Run:**
```bash
cargo run --example postgres_datasource_example
```

**What it demonstrates:**
- **Data Source Configuration**: Parse PostgreSQL YAML config with connection pooling
- **Query Types**:
  - Count queries for event frequency analysis
  - Count distinct queries for unique entity counting
  - Aggregation queries (SUM, AVG, COUNT) for statistical features
  - GROUP BY queries for segmented analysis
- **Time Windows**: Relative time filtering (last 24h, 7d, 30d, etc.)
- **PostgreSQL Features**:
  - INET type for IP address storage and queries
  - JSONB fields for flexible attribute storage
  - JSONB operators (`->>`, `->`) for nested field access
  - Time-based filtering with INTERVAL
  - Aggregation functions
- **Real-world Use Cases**:
  - Count user login events (velocity features)
  - Count distinct devices per user (association features)
  - Sum transaction amounts (statistical features)
  - Average amounts by payment method (behavioral features)
  - Count failed login attempts from IP (security features)
  - Query risk decisions with grouping

**Query Examples Covered:**
1. Count login events in last 24 hours
2. Count distinct devices in last 7 days
3. Sum transaction amounts in last 30 days
4. Average transaction by payment method in last 90 days
5. Count failed logins from IP in last 1 hour
6. Analyze risk decisions by pipeline and outcome

### 5. Complete Pipeline (`complete_pipeline.rs`)

A comprehensive example showing the full SDK capabilities:
- LLM integration (with mock provider)
- External service integration (with mock clients)
- Complete pipeline execution with multiple stages
- Metrics collection and observability
- All compiler optimizations enabled

**Run:**
```bash
cargo run --example complete_pipeline
```

**What it demonstrates:**
- Full DecisionEngineBuilder API
- LLM and Service configuration
- Complex event data processing
- Multi-stage pipeline execution
- Comprehensive metrics reporting

## Rule Files

All example rule files are located in `examples/rules/`:

- `simple_rule.yaml` - Basic amount threshold check
- `fraud_detection.yaml` - **⭐ Comprehensive example** covering all core concepts with 6-layer decision logic and complete DSL mapping
- `complete_pipeline.yaml` - Full pipeline with context, features, and decision logic (LLM and service integrations)

## Configuration Files

All configuration files are located in the `configs/` directory:

### Data Sources (`configs/datasources/`)

- **[clickhouse_events.yaml](configs/datasources/clickhouse_events.yaml)** - ClickHouse OLAP database configuration
- **[postgres_events.yaml](configs/datasources/postgres_events.yaml)** - PostgreSQL database configuration
- **[redis_features.yaml](configs/datasources/redis_features.yaml)** - Redis feature store configuration

### Features (`configs/features/`)

- **[device_features.yaml](configs/features/device_features.yaml)** - Device-related feature definitions
- **[ip_features.yaml](configs/features/ip_features.yaml)** - IP address-related feature definitions
- **[user_features.yaml](configs/features/user_features.yaml)** - User behavior feature definitions
- **[example_with_datasources.yaml](configs/features/example_with_datasources.yaml)** - Complete example with data source integration

## Running All Examples

To build all examples:
```bash
cargo build --examples
```

To run all examples sequentially:
```bash
cargo run --example simple_rule
cargo run --example fraud_detection
cargo run --example feature_datasource_example
cargo run --example postgres_datasource_example
cargo run --example complete_pipeline
```

**Recommended Learning Path:**
1. **simple_rule** - Learn basic concepts and rule execution
2. **fraud_detection** ⭐ - Understand all core features (multi-layer decisions, feature framework, DSL mapping)
3. **feature_datasource_example** - Learn data source configuration basics
4. **postgres_datasource_example** ⭐ - Learn database integration for feature calculation
5. **complete_pipeline** - Advanced integrations (LLM, external services)

## Key Concepts Demonstrated

### DecisionEngineBuilder

The builder pattern for configuring the decision engine:

```rust
let engine = DecisionEngineBuilder::new()
    .add_rule_file("path/to/rule.yaml")
    .with_llm(llm_config)
    .with_service(service_config)
    .enable_metrics(true)
    .enable_tracing(true)
    .build()
    .await?;
```

### DecisionRequest

Creating a request with event data and metadata:

```rust
let mut event_data = HashMap::new();
event_data.insert("amount".to_string(), Value::Number(150.0));

let request = DecisionRequest::new(event_data)
    .with_metadata("request_id".to_string(), "req-001".to_string());
```

### DecisionResponse

The response includes:
- `result.action` - The recommended action (Approve, Review, Block)
- `result.score` - Aggregate risk score
- `result.triggered_rules` - List of rules that matched
- `result.explanation` - Human-readable explanation
- `processing_time_ms` - Execution time

### Metrics

Accessing engine metrics:

```rust
let metrics = engine.metrics();
println!("Executions: {}", metrics.counter("executions_total").value());
```

## Next Steps

1. **Customize Rules**: Modify the YAML files in `examples/rules/` to match your use case
2. **Add Real Integrations**: Replace mock LLM and service providers with real implementations
3. **Extend Examples**: Add more complex scenarios based on your requirements
4. **Performance Testing**: Use these examples as a baseline for performance benchmarking

## Testing

All examples are compiled as part of the test suite:

```bash
cargo test
```

This ensures the examples stay up-to-date with the SDK API.
