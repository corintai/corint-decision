# Data Source Integration and Feature Calculation Guide

## Table of Contents

1. [Overview](#overview)
2. [Data Source Types](#data-source-types)
3. [Feature Operators](#feature-operators)
4. [Data Source Configuration](#data-source-configuration)
5. [Implementation Architecture](#implementation-architecture)
6. [Data Source Routing](#data-source-routing)
7. [Query Execution Flow](#query-execution-flow)
8. [Performance Optimization](#performance-optimization)
9. [Best Practices](#best-practices)
10. [Troubleshooting](#troubleshooting)

---

## Overview

This document provides a comprehensive guide to understanding how the Corint risk control system reads data from various data sources to perform real-time feature calculation for fraud detection and risk assessment.

### Key Concepts

- **Feature**: A calculated metric used in risk rules (e.g., login count in 24 hours)
- **Operator**: The computation pattern that defines how a feature is calculated
- **Data Source**: The underlying storage system where raw data is stored
- **Query**: An abstract representation of a data retrieval request
- **Executor**: The component that orchestrates feature calculation

---

## Data Source Types

The system supports three primary data source types, each optimized for specific use cases:

### 1. OLAP Databases (ClickHouse)

**Purpose**: Real-time event data and time-series analytics

**Characteristics**:
- Columnar storage optimized for analytical queries
- Fast aggregation over large datasets
- Time-series query optimization
- Best for: event counting, windowed aggregations, behavioral analytics

**Configuration Example**:
```yaml
type: olap
name: clickhouse_events
provider: clickhouse
connection:
  host: localhost
  port: 9000
  database: risk_control
  user: default
  password: ""
```

**Use Cases**:
- Transaction velocity detection
- Login frequency analysis
- Device usage patterns
- Time-based behavioral analysis

### 2. SQL Databases (PostgreSQL)

**Purpose**: Structured relational data and user profiles

**Characteristics**:
- ACID transactions
- Relational data model
- Complex JOIN operations
- Best for: user profiles, static attributes, reference data

**Configuration Example**:
```yaml
type: sql
name: postgresql_profiles
provider: postgresql
connection:
  host: localhost
  port: 5432
  database: user_profiles
  user: postgres
  password: ""
```

**Use Cases**:
- User KYC status lookup
- Account verification status
- User profile attributes
- Device registry information

### 3. Feature Stores (Redis)

**Purpose**: Pre-computed features and high-frequency access data

**Characteristics**:
- In-memory storage with sub-millisecond latency
- Key-value access pattern
- TTL support for cache expiration
- Best for: pre-aggregated metrics, hot data, cached features

**Configuration Example**:
```yaml
type: feature_store
name: redis_features
provider: redis
connection:
  host: localhost
  port: 6379
  db: 0
  password: ""
```

**Use Cases**:
- Pre-computed risk scores
- User historical averages
- Device trust scores
- Cached aggregations

---

## Feature Operators

Operators are the building blocks for feature engineering. Each operator represents a specific computation pattern.

### Operator Classification

#### 1. Aggregation Operators

Compute statistical metrics over a sliding time window.

##### Count Operator

**Purpose**: Count events matching filters within a time window.

**Example**:
```yaml
features:
  - name: login_count_24h
    description: "Number of login events in last 24 hours"
    operator: count
    datasource: clickhouse_events
    entity: user_events
    dimension: user_id
    dimension_value: "{event.user_id}"
    window:
      value: 24
      unit: hours
    filters:
      - field: event_type
        operator: eq
        value: "login"
```

**Generated SQL**:
```sql
SELECT COUNT(*) as count
FROM user_events
WHERE user_id = '12345'
  AND event_type = 'login'
  AND timestamp >= NOW() - INTERVAL 24 HOURS
```

##### Sum Operator

**Purpose**: Sum numeric values within a time window.

**Example**:
```yaml
features:
  - name: transaction_sum_7d
    operator: sum
    datasource: clickhouse_events
    entity: user_events
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

##### Avg, Max, Min, Percentile

Similar patterns for average, maximum, minimum, and percentile calculations.

#### 2. Distinct Count Operators

Count unique values within a dimension.

**Example**:
```yaml
features:
  - name: unique_devices_7d
    operator: count_distinct
    datasource: clickhouse_events
    entity: user_events
    dimension: user_id
    dimension_value: "{event.user_id}"
    distinct_field: device_id
    window:
      value: 7
      unit: days
```

#### 3. Lookup Operators

Retrieve stored values from databases or feature stores.

##### Profile Lookup

**Purpose**: Get value from user/device profile.

**Example**:
```yaml
features:
  - name: user_kyc_status
    operator: profile_lookup
    datasource: postgresql_profiles
    table: user_profiles
    dimension: user_id
    dimension_value: "{event.user_id}"
    field: kyc_status
```

**Generated SQL**:
```sql
SELECT kyc_status
FROM user_profiles
WHERE user_id = '12345'
```

##### Feature Store Lookup

**Purpose**: Get pre-computed feature from Redis.

**Example**:
```yaml
features:
  - name: user_risk_score
    operator: feature_store_lookup
    datasource: redis_features
    key: "user_features:{event.user_id}:risk_score"
    fallback: 0.0
```

**Redis Command**:
```
GET user_features:12345:risk_score
```

#### 4. Temporal Operators

Extract time-based features.

**Examples**:
- `first_seen`: Get first occurrence timestamp
- `last_seen`: Get last occurrence timestamp
- `time_since`: Calculate time elapsed since an event

#### 5. Velocity Operators

Detect velocity anomalies.

**Example**:
```yaml
features:
  - name: exceeds_transaction_velocity
    operator: velocity
    datasource: clickhouse_events
    entity: user_events
    dimension: user_id
    dimension_value: "{event.user_id}"
    window:
      value: 1
      unit: hours
    threshold: 10
    filters:
      - field: event_type
        operator: eq
        value: "transaction"
```

---

## Data Source Configuration

### Specifying Data Sources in Features

#### Method 1: Explicit datasource Field (Recommended)

```yaml
features:
  - name: login_count_24h
    operator: count
    datasource: clickhouse_events  # ← Explicitly specify data source
    entity: user_events
    dimension: user_id
    dimension_value: "{event.user_id}"
    window:
      value: 24
      unit: hours
```

#### Method 2: Using Default Data Source

If `datasource` is not specified, the system uses the data source registered as `"default"`:

```yaml
features:
  - name: login_count_24h
    operator: count
    # datasource: not specified, uses "default"
    entity: user_events
    dimension: user_id
    dimension_value: "{event.user_id}"
```

### Complete Example: Mixed Architecture

```yaml
features:
  # Real-time event statistics from ClickHouse
  - name: transaction_sum_24h
    operator: sum
    datasource: clickhouse_events
    entity: user_events
    dimension: user_id
    dimension_value: "{event.user_id}"
    field: amount
    window:
      value: 24
      unit: hours

  # User profile from PostgreSQL
  - name: user_is_verified
    operator: profile_lookup
    datasource: postgresql_profiles
    table: user_profiles
    dimension: user_id
    dimension_value: "{event.user_id}"
    field: is_verified

  # Pre-computed feature from Redis
  - name: user_avg_transaction_historical
    operator: feature_store_lookup
    datasource: redis_features
    key: "user_stats:{event.user_id}:avg_transaction"
    fallback: 0.0
```

---

## Implementation Architecture

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Feature YAML: login_count_24h                                  │
│    datasource: clickhouse_events                                │
│    entity: user_events                                          │
└────────────────────────┬────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────────┐
│  FeatureExecutor                                                │
│    - Loads feature definition                                   │
│    - Resolves datasource name → "clickhouse_events"            │
│    - Gets DataSourceClient from datasources HashMap             │
└────────────────────────┬────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────────┐
│  CountOperator::execute()                                       │
│    - Builds Query object:                                       │
│      • entity: "user_events"                                    │
│      • filters: [user_id = "12345"]                            │
│      • time_window: last 24 hours                              │
└────────────────────────┬────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────────┐
│  DataSourceClient::query()                                      │
│    - Checks L2 cache (optional)                                 │
│    - Calls self.client.execute(query)                          │
└────────────────────────┬────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────────┐
│  OLAPClient::execute()                   ← ⭐ Actual data read  │
│    - Converts Query → ClickHouse SQL                            │
│    - Executes: SELECT COUNT(*) FROM user_events                 │
│                WHERE user_id = '12345'                          │
│                AND timestamp >= NOW() - INTERVAL 24 HOURS       │
│    - Returns QueryResult                                        │
└────────────────────────┬────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────────┐
│  ClickHouse Database                                            │
│    - Executes SQL query                                         │
│    - Returns result: count = 42                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Complete Call Chain

```
User Code
  ↓
FeatureExecutor::execute_feature()
  ↓ (lookup feature definition)
FeatureExecutor::execute_feature_impl()
  ↓ (check cache)
FeatureExecutor::compute_feature()
  ↓ (get datasource)
FeatureExecutor::execute_operator()
  ↓ (dispatch by operator type)
[Operator]::execute(datasource, context)  ← Build query
  ↓
DataSourceClient::query(query)  ← Unified entry point
  ↓ (check cache)
DataSourceImpl::execute(query)  ← Polymorphic dispatch
  ↓
Concrete implementation (OLAPClient / SQLClient / FeatureStoreClient)
  ↓
Database / Redis / Feature Store
```

### Key Components

#### 1. Operator Layer: Query Construction

**File**: `crates/corint-runtime/src/feature/operator.rs`

**Key Code** (CountOperator example):
```rust
impl CountOperator {
    pub async fn execute(
        &self,
        datasource: &DataSourceClient,  // ← Receives data source client
        context: &HashMap<String, Value>,
    ) -> Result<Value> {
        // 1. Resolve template variables
        let dimension_value = resolve_template(&self.params.dimension_value, context)?;

        // 2. Build Query object
        let query = Query {
            query_type: QueryType::Count,
            entity: self.params.entity.clone(),
            filters: build_filters(&self.params, &dimension_value, context)?,
            time_window: build_time_window(&self.params)?,
            aggregations: vec![],
            group_by: vec![],
            limit: None,
        };

        // 3. Execute query ← KEY: Actual data read
        let result = datasource.query(query).await?;

        // 4. Extract result
        if let Some(row) = result.rows.first() {
            if let Some(value) = row.get("count") {
                return Ok(value.clone());
            }
        }

        Ok(Value::Number(0.0))
    }
}
```

#### 2. DataSourceClient: Unified Query Entry Point

**File**: `crates/corint-runtime/src/datasource/client.rs`

**Key Code**:
```rust
impl DataSourceClient {
    /// Execute a query
    pub async fn query(&self, query: Query) -> Result<QueryResult> {
        // 1. Check cache
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

        // 2. Execute query ← KEY: Polymorphic dispatch
        let start = Instant::now();
        let result = self.client.execute(query.clone()).await?;
        let execution_time_ms = start.elapsed().as_millis() as u64;

        // 3. Cache result
        if !result.rows.is_empty() {
            if let Some(row) = result.rows.first() {
                self.cache
                    .lock()
                    .unwrap()
                    .set(cache_key, row.clone(), std::time::Duration::from_secs(300));
            }
        }

        // 4. Return result
        Ok(QueryResult {
            rows: result.rows,
            execution_time_ms,
            source: self.config.name.clone(),
            from_cache: false,
        })
    }
}
```

#### 3. DataSourceImpl Trait: Polymorphic Interface

```rust
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
```

**Three Implementations**:
1. `OLAPClient` - ClickHouse, Druid, etc.
2. `SQLClient` - PostgreSQL, MySQL, etc.
3. `FeatureStoreClient` - Redis, Feast, etc.

#### 4. OLAPClient: ClickHouse Implementation

**Features**:
- SQL query generator for ClickHouse
- Support for all aggregation types (COUNT, SUM, AVG, MIN, MAX, COUNT DISTINCT)
- ClickHouse-specific functions: `median()`, `stddevPop()`, `quantile()`
- Complete WHERE clause support
- Time window query optimization

**SQL Generation Example**:

**Input Query**:
```rust
Query {
    query_type: Count,
    entity: "user_events",
    filters: [
        Filter { field: "user_id", operator: Eq, value: "12345" },
        Filter { field: "event_type", operator: Eq, value: "login" }
    ],
    time_window: Some(RelativeWindow { value: 24, unit: Hours }),
}
```

**Generated SQL**:
```sql
SELECT COUNT(*) AS count
FROM user_events
WHERE user_id = '12345'
  AND event_type = 'login'
  AND timestamp >= now() - INTERVAL 86400 SECOND
```

**Code Location**: `client.rs:244-430`

#### 5. SQLClient: PostgreSQL Implementation

**Features**:
- Standard SQL generation for PostgreSQL
- Support for all standard aggregation functions
- PostgreSQL-specific functions: `PERCENTILE_CONT`, `STDDEV_POP`
- PostgreSQL regex operator: `~`

**Differences from ClickHouse**:

| Feature | ClickHouse | PostgreSQL |
|---------|-----------|-----------|
| Median | `median(field)` | `PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY field)` |
| Std Dev | `stddevPop(field)` | `STDDEV_POP(field)` |
| Percentile | `quantile(0.95)(field)` | `PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY field)` |
| Regex | `match(field, pattern)` | `field ~ pattern` |
| Boolean | `1`, `0` | `TRUE`, `FALSE` |
| Interval | `INTERVAL 3600 SECOND` | `INTERVAL '3600 seconds'` |

**Code Location**: `client.rs:466-658`

#### 6. FeatureStoreClient: Redis Implementation

**Features**:
- Key-value access pattern (no SQL)
- Namespace support
- Fast lookup for pre-computed features

**Key Format**:
```
Without namespace: {feature_name}:{entity_key}
With namespace: {namespace}:{feature_name}:{entity_key}

Examples:
- user_features:user_12345:risk_score
- device_features:device_67890:trust_score
```

**Query Conversion**:
1. Extract `entity_key` from Query.filters
2. Use Query.entity as `feature_name`
3. Construct Redis key and fetch value

**Code Location**: `client.rs:162-279`

---

## Data Source Routing

### Routing Flow

```
Feature YAML Config
        ↓
┌───────────────────────────────────┐
│  datasource: clickhouse_events?   │
│  → Yes: Use "clickhouse_events"   │
│  → No:  Use "default"             │
└───────────────────────────────────┘
        ↓
┌───────────────────────────────────┐
│  FeatureExecutor.get_datasource() │
│  → Lookup in datasources HashMap  │
└───────────────────────────────────┘
        ↓
┌───────────────────────────────────┐
│  Found?                           │
│  → Yes: Return DataSourceClient   │
│  → No:  Error "not found"         │
└───────────────────────────────────┘
```

### Initialization Examples

#### Method 1: Explicit "default" Registration (Recommended)

```rust
let mut executor = FeatureExecutor::new();

// Register multiple data sources
executor.add_datasource("clickhouse_events", clickhouse_client);
executor.add_datasource("postgresql_profiles", postgres_client);
executor.add_datasource("redis_features", redis_client);

// ⭐ Explicitly register "default" pointing to ClickHouse
executor.add_datasource("default", clickhouse_client.clone());
```

**Advantages**:
- Clear and explicit
- "default" coexists with other data sources
- Recommended approach ✅

#### Method 2: Single Data Source Named "default"

```rust
let mut executor = FeatureExecutor::new();

// Register only one data source named "default"
executor.add_datasource("default", clickhouse_client);
```

**Characteristics**:
- All features without `datasource` field use this data source
- Cannot use multiple data sources
- Suitable for simple scenarios

#### Method 3: No "default" Registration (Will Error)

```rust
let mut executor = FeatureExecutor::new();

// Register only specific named data sources
executor.add_datasource("clickhouse_events", clickhouse_client);
executor.add_datasource("postgresql_profiles", postgres_client);
// ❌ No "default" registered
```

**Result**:
- If feature config doesn't specify `datasource`, execution will error:
  ```
  Error: Data source 'default' not found
  ```

### Configuration Mapping Table

| Feature YAML Config | Actual Data Source Used | Description |
|---------------------|------------------------|-------------|
| `datasource: clickhouse_events` | `clickhouse_events` | Explicitly specified |
| `datasource: postgresql_profiles` | `postgresql_profiles` | Explicitly specified |
| `datasource: default` | `default` | Explicitly use default |
| (datasource not specified) | `default` | Implicitly use default |
| `datasource: non_existent` | ❌ Error | Data source not found |

### Practical Usage Scenarios

#### Scenario 1: Mixed Architecture (Recommended)

```rust
// Initialization code
executor.add_datasource("clickhouse_events", clickhouse_client);
executor.add_datasource("postgresql_profiles", postgres_client);
executor.add_datasource("redis_features", redis_client);
executor.add_datasource("default", clickhouse_client.clone()); // Default to ClickHouse
```

```yaml
# Feature configuration
features:
  # Real-time event statistics - explicitly use ClickHouse
  - name: login_count_24h
    datasource: clickhouse_events
    entity: user_events
    operator: count

  # User profile query - use PostgreSQL
  - name: user_kyc_status
    datasource: postgresql_profiles
    operator: profile_lookup
    table: user_profiles

  # Simple statistics - not specified, use default (ClickHouse)
  - name: event_count_1h
    # datasource not specified -> uses default -> clickhouse_events
    entity: user_events
    operator: count
```

#### Scenario 2: Single Data Source Architecture

```rust
// Only use ClickHouse
executor.add_datasource("default", clickhouse_client);
```

```yaml
features:
  # All features read from same data source
  - name: login_count_24h
    entity: user_events
    operator: count

  - name: transaction_sum_24h
    entity: user_events
    operator: sum
    field: amount
```

#### Scenario 3: Explicitly Specify All Data Sources (Best Practice)

```rust
executor.add_datasource("olap", clickhouse_client);
executor.add_datasource("sql", postgres_client);
executor.add_datasource("cache", redis_client);
executor.add_datasource("default", clickhouse_client.clone());
```

```yaml
features:
  # All features explicitly specify data source
  - name: login_count_24h
    datasource: olap  # Clear and explicit
    entity: user_events
    operator: count

  - name: user_is_verified
    datasource: sql   # Clear and explicit
    operator: profile_lookup
    table: user_profiles

  - name: user_risk_score
    datasource: cache # Clear and explicit
    operator: feature_store_lookup
    key: "user:{user_id}:score"
```

---

## Feature Naming and Access

### Feature Naming Convention

To avoid naming conflicts between features and event fields, CORINT **requires** explicit feature access using the `features.` namespace prefix.

#### Feature Access Syntax (features.xxx)

**All features must be accessed using the `features.` prefix:**

```yaml
rule:
  id: high_transaction_volume
  when:
    conditions:
      - features.transaction_sum_7d > 5000      # ← Feature access (required)
      - features.transaction_count_24h > 10     # ← Feature access (required)
      - event.amount > 1000                      # ← Regular event field
```

**Benefits:**
- ✅ **Clear distinction**: Immediately identifies this as a calculated feature
- ✅ **No conflicts**: Completely avoids naming collisions with event fields
- ✅ **Better debugging**: Easy to trace where values come from
- ✅ **Explicit intent**: Makes it clear that this requires computation
- ✅ **Type safety**: Prevents accidental use of event fields when features are intended

### Feature Lookup Priority

When accessing a field, the system follows this priority order:

1. **Feature Namespace** (`features.xxx`): Direct feature lookup - calculates feature on-demand
2. **Event Data** (`event_data`): Raw event fields
3. **Variables** (`result.variables`): Stored context variables
4. **Special Fields**: Built-in fields like `total_score`, `triggered_rules`

### Examples

#### Example 1: Explicit Feature Access (Recommended)

```yaml
features:
  - name: login_count_24h
    operator: count
    # ... configuration ...

rule:
  id: frequent_login
  when:
    conditions:
      - features.login_count_24h > 10    # ← Explicit feature access
      - event.user_id exists               # ← Event field
```

#### Example 2: Mixed Access Patterns

```yaml
rule:
  id: risk_assessment
  when:
    conditions:
      # Feature access (required)
      - features.transaction_sum_7d > 5000
      - features.unique_devices_7d >= 2
      - features.login_count_24h > 5
      
      # Event fields
      - event.amount > 100
      - event.user_id exists
```

#### Example 3: Clear Separation

```yaml
# Event data
event_data:
  transaction_sum_7d: 1000  # From event (if provided)

features:
  - name: transaction_sum_7d  # Calculated feature
    operator: sum
    # ...

# Rule - always uses calculated feature, never event field
rule:
  when:
    conditions:
      - features.transaction_sum_7d > 5000  # ← Always uses calculated feature
      - event.amount > 100                    # ← Uses event field
```

---

## Query Execution Flow

### Complete Execution Chain

```
User Code
  ↓
FeatureExecutor::execute_feature()
  ↓
Operator::execute()  (e.g., CountOperator)
  ↓
DataSourceClient::query()
  ├─ Check L1 cache (local memory)
  ├─ Cache miss, call client.execute()
  ↓
DataSourceImpl::execute()  (polymorphic call)
  ├─ OLAPClient::execute()
  │   ├─ build_sql()          Generate ClickHouse SQL
  │   └─ execute_clickhouse() Execute query
  │
  ├─ SQLClient::execute()
  │   ├─ build_sql()          Generate PostgreSQL SQL
  │   └─ execute_postgresql() Execute query
  │
  └─ FeatureStoreClient::execute()
      ├─ extract_entity_key() Extract key
      └─ get_redis_feature()  Redis GET
  ↓
QueryResult
  ├─ Store in L1 cache
  ├─ Store in L2 cache (if enabled)
  └─ Return to caller
```

### Performance Optimization

#### Two-Tier Caching

1. **L1 (Local Memory)**: Sub-millisecond latency
2. **L2 (Redis)**: 1-5ms latency
3. **Source Database**: 50-200ms latency

#### Cache Key Generation

```rust
let cache_key = format!(
    "query:{}:{}:{}",
    datasource_name,
    entity,
    serde_json::to_string(&filters)?
);
```

#### TTL Configuration

- Default: 300 seconds (5 minutes)
- Customizable in feature configuration
- High-frequency features use shorter TTL

---

## Performance Optimization

### Data Source Selection Guide

| Scenario | Recommended Data Source | Reason |
|----------|------------------------|--------|
| Real-time event statistics (sliding window) | ClickHouse | Columnar storage, time-series optimization |
| User profile queries | PostgreSQL | ACID transactions, relational data |
| High-frequency access features | Redis | In-memory storage, sub-millisecond latency |
| Pre-aggregated metrics | Redis + ClickHouse materialized views | Hybrid architecture, optimal performance |

### Cache Strategy

```yaml
cache:
  enabled: true
  ttl: 300              # Cache for 5 minutes
  backend: redis        # Use Redis as L2 cache
```

**Cache Hierarchy**:
- L1 (Local Memory): Sub-millisecond
- L2 (Redis): 1-5ms
- L3 (ClickHouse Materialized Views): 5-20ms
- L4 (Real-time Query): 50-200ms

### Performance Metrics

#### Current Performance (Mock Implementation)

| Data Source | Avg Latency | QPS | Notes |
|------------|------------|-----|-------|
| OLAPClient | 25ms | ~400 | Mock returns fixed latency |
| SQLClient | 15ms | ~666 | Mock returns fixed latency |
| FeatureStore | 5ms | ~2000 | Mock returns fixed latency |

#### Expected Performance (Real Implementation)

| Data Source | Avg Latency | QPS | Optimization Tips |
|------------|------------|-----|------------------|
| ClickHouse | 50-200ms | ~100-200 | Materialized views, pre-aggregation |
| PostgreSQL | 10-50ms | ~200-500 | Index optimization, connection pooling |
| Redis | 1-5ms | ~1000-5000 | Connection reuse, pipelining |

### Cache Effectiveness

With two-tier caching enabled:
- Cache hit rate > 80%: Average latency < 1ms
- Cache hit rate > 95%: Average latency < 0.1ms

---

## Best Practices

### ✅ Recommended Practices

1. **Always Register "default" Data Source**
   ```rust
   executor.add_datasource("default", most_common_datasource);
   ```

2. **Explicitly Specify datasource in Configuration**
   ```yaml
   datasource: clickhouse_events  # Clear and explicit
   ```

3. **Use Meaningful Data Source Names**
   ```rust
   "clickhouse_events"    // ✅ Good
   "postgres_profiles"    // ✅ Good
   "redis_cache"          // ✅ Good
   ```

4. **Choose Data Source Based on Data Characteristics**
   - Real-time changing time-series data (event streams) → ClickHouse
   - Relatively static profile data (user information) → PostgreSQL
   - Pre-computed high-frequency access data (risk scores) → Redis

5. **Enable Appropriate Caching**
   ```yaml
   cache:
     enabled: true
     ttl: 300
     backend: redis
   ```

6. **Group Operators with Same Data Source and Window**
   ```yaml
   # Good: Batch query
   - operator: multi_count
     dimension: user_id
     window: {value: 24, unit: hours}
     counts:
       - name: login_count
         filters: [{field: event_type, operator: eq, value: "login"}]
       - name: transaction_count
         filters: [{field: event_type, operator: eq, value: "transaction"}]
   ```

### ❌ Anti-Patterns

1. **Don't Register "default" and Don't Specify datasource**
   ```yaml
   # Config doesn't specify
   entity: user_events
   ```
   ```rust
   // Code doesn't have default
   executor.add_datasource("ch", client);  // ❌ Runtime error
   ```

2. **Don't Use Unclear Data Source Names**
   ```rust
   "ds1", "ds2", "db"  // ❌ Unclear
   ```

3. **Don't Use Redis for Cold Data**
   - Redis is for high-frequency access data
   - Low-frequency data should be stored in PostgreSQL

4. **Don't Disable Cache for Real-time Data**
   ```yaml
   cache:
     enabled: false  # ❌ Queries database every time, poor performance
   ```

5. **Don't Register Same Data Source to Different Names**
   ```rust
   executor.add_datasource("clickhouse", client.clone());
   executor.add_datasource("ch", client.clone());
   executor.add_datasource("events", client.clone());
   // ❌ Wastes memory and causes confusion
   ```

---

## Troubleshooting

### Common Errors

#### Error 1: default Data Source Not Registered

```
Error: Data source 'default' not found
```

**Cause**: Feature config doesn't specify `datasource`, but executor doesn't have a data source registered as "default".

**Solution**:
```rust
// Add default data source
executor.add_datasource("default", your_most_common_datasource);
```

#### Error 2: Specified Data Source Does Not Exist

```
Error: Data source 'my_datasource' not found
```

**Cause**: Feature config specifies `datasource: my_datasource`, but executor doesn't have it registered.

**Solution**:
```rust
// Register the data source
executor.add_datasource("my_datasource", datasource_client);
```

#### Error 3: Data Source Name Typo

```yaml
# In config file
datasource: clickhouse_event  # ❌ Missing 's'
```

```rust
// In code registered as
executor.add_datasource("clickhouse_events", client);  # Correct name
```

**Solution**: Check that data source names are consistent between config and code.

#### Error 4: Generated SQL Syntax Error

```
Error: SQL execution failed: syntax error at or near "INTERVAL"
```

**Cause**: Different databases have different time interval syntax
- ClickHouse: `INTERVAL 3600 SECOND`
- PostgreSQL: `INTERVAL '3600 seconds'`

**Solution**: Ensure correct SQL generator is used (build_sql).

#### Error 5: Feature Value Type Mismatch

```
Error: Expected Number, got String
```

**Cause**: Redis returned value type doesn't match expected type.

**Solution**: Use JSON serialization when storing in Feature Store, or specify type conversion in configuration.

#### Error 6: Cache Key Collision

```
Warning: Cache key collision detected
```

**Cause**: Different queries generated the same cache key.

**Solution**: Improve `generate_cache_key()` logic to include more query parameters.

### Viewing Data Source Resolution Logs

Enable debug logging to see data source resolution process:

```rust
// Enable tracing
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

**Log Output Example**:
```
DEBUG corint_runtime::feature::executor: Computing feature 'login_count_24h'
DEBUG corint_runtime::feature::executor: Using datasource: clickhouse_events
DEBUG corint_runtime::datasource::client: Executing OLAP query: Count
DEBUG corint_runtime::datasource::client: Generated SQL: SELECT COUNT(*) AS count FROM user_events WHERE user_id = '12345' AND timestamp >= now() - INTERVAL 86400 SECOND
INFO  corint_runtime::datasource::client: Executing ClickHouse query: ...
```

---

## Summary

### Key Concepts

| Question | Answer |
|----------|--------|
| **What does "default" data source read?** | Reads the data source registered as `"default"` name |
| **How to set default?** | `executor.add_datasource("default", client)` |
| **Is default required?** | Required if any feature doesn't specify datasource |
| **Can there be multiple defaults?** | No, HashMap can only have one value per key |
| **Which data source can default point to?** | Any registered DataSourceClient |
| **Recommended practice?** | Set most common data source as default, explicitly specify others in config |

### Core Files

| File | Purpose | Key Line |
|------|---------|----------|
| `feature/operator.rs` | Build query | Line 200: `datasource.query()` |
| `datasource/client.rs` | Unified entry point | Line 64: `client.execute()` |
| `datasource/client.rs` | OLAP implementation | Line 221: `OLAPClient::execute()` |
| `datasource/client.rs` | SQL implementation | Line 249: `SQLClient::execute()` |
| `datasource/client.rs` | Redis implementation | Line 186: `get_feature()` |

### Data Read Location

**Simple Answer**:
1. **API Call**: `Operator::execute()` → `datasource.query(query).await`
2. **Unified Entry**: `DataSourceClient::query()`
3. **Actual Execution**: `DataSourceImpl::execute()` (polymorphic dispatch)
4. **Concrete Implementation**: `OLAPClient` / `SQLClient` / `FeatureStoreClient`

Currently, these concrete implementations are **TODO/Placeholder** and need to be implemented according to the guidelines above.

### Implementation Status

#### ✅ Completed

- Complete call chain from FeatureExecutor to DataSourceClient
- Abstract layer design: Query, QueryResult, DataSourceImpl trait
- Polymorphic dispatch mechanism based on data source type
- Two-tier cache architecture
- Complete filter support
- Time window queries
- Aggregation function support
- SQL query generators (ClickHouse & PostgreSQL)
- Redis key-value access

#### 🔲 TODO

- Replace mock implementation with real database connections
- Add connection pool management
- Implement query result streaming
- Add query timeout and retry mechanisms
- Support batch query optimization
- Add query performance monitoring
- Implement query plan caching

### Next Steps

1. Choose databases based on actual deployment environment
2. Add corresponding Rust crate dependencies
3. Implement real database connection logic
4. Perform performance testing and tuning
5. Improve error handling and logging

---

## Appendix

### Required Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# ClickHouse
clickhouse = "0.11"

# PostgreSQL
tokio-postgres = "0.7"
# or
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-rustls"] }

# Redis
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }

# Connection Pooling
deadpool-postgres = "0.11"
bb8-redis = "0.13"
```

### Testing Data Read

Once implementation is complete, test like this:

```rust
#[tokio::test]
async fn test_clickhouse_query() {
    // 1. Create ClickHouse client
    let config = DataSourceConfig::olap(...);
    let client = DataSourceClient::new(config).await.unwrap();

    // 2. Build query
    let query = Query {
        query_type: QueryType::Count,
        entity: "user_events".to_string(),
        filters: vec![...],
        ...
    };

    // 3. Execute query
    let result = client.query(query).await.unwrap();

    // 4. Verify result
    assert!(!result.rows.is_empty());
    assert_eq!(result.source, "clickhouse_events");
}
```

### Extending the System

#### Adding New Data Source Type

1. **Define Configuration**
   ```rust
   // config.rs
   #[derive(Debug, Clone)]
   pub struct MyDataSourceConfig {
       pub connection_string: String,
       // ...
   }
   ```

2. **Create Client**
   ```rust
   // client.rs
   struct MyDataSourceClient {
       config: MyDataSourceConfig,
   }

   #[async_trait::async_trait]
   impl DataSourceImpl for MyDataSourceClient {
       async fn execute(&self, query: Query) -> Result<QueryResult> {
           // Implement query logic
       }
   }
   ```

3. **Register in Type System**
   ```rust
   // config.rs
   pub enum DataSourceType {
       FeatureStore(FeatureStoreConfig),
       OLAP(OLAPConfig),
       SQL(SQLConfig),
       MyDataSource(MyDataSourceConfig),  // New
   }
   ```

#### Adding New Aggregation Function

1. **Define Aggregation Type**
   ```rust
   // query.rs
   pub enum AggregationType {
       Count,
       Sum,
       // ...
       ApproxCountDistinct { precision: u8 },  // New
   }
   ```

2. **Support in SQL Generator**
   ```rust
   // client.rs - OLAPClient
   fn build_aggregation(&self, agg: &Aggregation) -> String {
       match agg.agg_type {
           // ...
           AggregationType::ApproxCountDistinct { precision } => {
               format!("uniqHLL12({})", field)  // ClickHouse
           }
       }
   }
   ```

---

**End of Documentation**
