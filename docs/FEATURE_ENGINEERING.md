# Feature Engineering for Risk Management

This document outlines the feature types supported and planned for Corint's risk management platform.

## 🚦 Implementation Status Overview

| Feature Category | Status | Production Ready | In Development |
|-----------------|--------|------------------|----------------|
| **Aggregation** | 🟢 **Implemented** | count, sum, avg, min, max, distinct | stddev, percentile, median, mode, entropy |
| **State** | 🔴 **Planned** | - | All methods (z_score, outlier detection, etc.) |
| **Sequence** | 🔴 **Planned** | - | All methods (pattern matching, trends, etc.) |
| **Graph** | 🔴 **Planned** | - | All methods (network analysis, centrality, etc.) |
| **Expression** | 🟢 **Implemented** | expression | ML model integration (planned) |
| **Lookup** | 🟢 **Implemented** | lookup | - |

**Legend:**
- 🟢 **Implemented**: Ready for production use
- 🟡 **Partial**: Some methods implemented
- 🔴 **Planned**: Documented but not yet implemented

> ⚠️ **Important**: Sections marked as "Planned" show the intended design and API. The implementation is in development and not yet available in production.

---

## Overview

Feature engineering in risk management follows a structured approach based on **what you want to measure**:

1. **Aggregation (Counting/Aggregating)** 🟢 - Counting and aggregating events/values
2. **State (Checking Current State)** 🔴 - Checking current state and statistical comparisons
3. **Sequence (Analyzing Process)** 🔴 - Analyzing patterns and trends over time
4. **Graph (Analyzing Relationships)** 🔴 - Analyzing connections and networks between entities
5. **Expression (Computing Scores)** 🟢 - Computing scores and evaluations
6. **Lookup (Looking up Pre-computed Values)** 🟢 - Retrieving pre-computed feature values

> **Note:** List/Set operations (blacklist/whitelist checking, etc.) are implemented separately in Corint's list management system and are not covered in this feature engineering document.

---

## Methods by Category

### 1. Aggregation Methods 🟢 Implemented
> **Rust Implementation:** `AggregationExecutor::execute(method: AggregationType, config: AggregationConfig)`
>
> **Design Pattern:** Unified executor with method-based dispatch
>
> **Status:** ✅ Core methods production-ready | 📋 Advanced statistics in development

**✅ Implemented (Production-Ready):**
- `count` - Count events matching conditions within time window
  - *Example: User's past24hours logged in5times*
  - **Real-world Use Cases**:
    - Brute force detection: Count failed logins within 1 hour, trigger account lock after exceeding 10 attempts
    - Transaction frequency monitoring: Count user transactions within 24 hours, abnormally high frequency may indicate account theft
    - API rate limiting: Count IP address requests within 1 minute, reject service after exceeding 100 requests
  - **YAML Example**:
    ```yaml
    - name: cnt_userid_login_1h_failed
      type: aggregation
      method: count
      datasource: postgresql_events
      entity: events
      dimension: user_id              # Group by user (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      window: 1h
      # Note: count operation does not need field field, only counts events matching conditions
      when:
        all:
          - type == "login"               # Database field (no prefix)
          - status == "failed"             # Database field (no prefix)
    ```

- `sum` - Sum numeric field values
  - *Example: User's past30days total transaction amount is ¥15,000*
  - **Real-world Use Cases**:
    - Money laundering detection: Count account24hours total transfer amount, exceeding ¥500k requires manual review
    - Credit limit management: Count user30days total spending, determine if credit limit exceeded
    - Points fraud: Count user1hours total points earned, abnormally high may indicate points farming
  - **YAML Example**:
    ```yaml
    - name: sum_userid_txn_amt_24h
      type: aggregation
      method: sum
      datasource: postgresql_events
      entity: events
      dimension: user_id              # Group by user (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: amount                   # Calculate sum of amount (SUM(amount))
      window: 24h
      when: type == "transaction"         # Database field (no prefix)
    ```

- `avg` - Average of field values
  - *Example: User's past7days average transaction amount ¥500*
  - **Real-world Use Cases**:
    - Abnormal transaction amount detection: User average transaction ¥500, suddenly appears ¥50,000transaction requires verification
    - User profiling: Calculate user average order amount for user segmentation (high/medium/low spending)
    - Session duration analysis: Count user average session duration, abnormally short may indicate bot
  - **YAML Example**:
    ```yaml
    - name: avg_userid_order_amt_30d
      type: aggregation
      method: avg
      datasource: postgresql_events
      entity: events
      dimension: user_id              # Group by user (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: amount                   # Calculate average of amount (AVG(amount))
      window: 30d
      when: type == "order"               # Database field (no prefix)
    ```

- `max` - Maximum value
  - *Example: User's past24hours maximum single transaction ¥2,000*
  - **Real-world Use Cases**:
    - Large transaction monitoring: Detect user historical maximum transaction amount, current transaction exceeds3times requires verification
    - Single transaction limit check: Newly registered user24hours maximum transaction does not exceed ¥5,000
    - Abnormal behavior identification: IP address associated maximum user count exceeds50, may be proxy or public WiFi
  - **YAML Example**:
    ```yaml
    - name: max_userid_txn_amt_90d
      type: aggregation
      method: max
      datasource: postgresql_events
      entity: events
      dimension: user_id              # Group by user (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: amount                   # Calculate maximum of amount (MAX(amount))
      window: 90d
      when: type == "transaction"         # Database field (no prefix)
    ```

- `min` - Minimum value
  - *Example: User's past7days minimum single transaction ¥10*
  - **Real-world Use Cases**:
    - Test transaction detection: Large amount of ¥0.01small transactions may be stolen card testing
    - Order brushing identification: Minimum order amount abnormally low (e.g. ¥0.1) combined with high frequency, suspected order brushing
    - Abnormal discount monitoring: Order minimum amount is ¥1, may have coupon vulnerability
  - **YAML Example**:
    ```yaml
    - name: min_userid_order_amt_7d
      type: aggregation
      method: min
      datasource: postgresql_events
      entity: events
      dimension: user_id              # Group by user (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: amount                   # Calculate minimum of amount (MIN(amount))
      window: 7d
      when: type == "order"               # Database field (no prefix)
    ```

- `distinct` - Count unique values
  - *Example: User's past7days used3different devices*
  - **Real-world Use Cases**:
    - Account sharing detection: User uses more than 5 different devices within 24 hours, may indicate account theft or sharing
    - IP hopping detection: User1hours uses more than10different IPs, may be using proxy pool
    - Multi-account association: Same device24hours logs into more than20different accounts, may be batch operation
  - **YAML Example**:
    ```yaml
    - name: distinct_userid_device_24h
      type: aggregation
      method: distinct
      datasource: postgresql_events
      entity: events
      dimension: user_id              # Group by user (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: device_id                # Count distinct device IDs (COUNT(DISTINCT device_id))
      window: 24h
    ```

**Planned:**
- `stddev` - Standard deviation
  - *Example: User transaction amount standard deviation ¥350, high volatility*
  - **Real-world Use Cases**:
    - Behavior stability analysis: Transaction amount standard deviation too large, unstable behavior, may be account theft
    - Abnormal volatility detection: User historical standard deviation ¥50, recent standard deviation ¥500, behavior drastically changed
    - User segmentation: Low standard deviation users (fixed spending) vs high standard deviation users (random spending)
  - **YAML Example**:
    ```yaml
    - name: stddev_userid_txn_amt_30d
      type: aggregation
      method: stddev
      datasource: postgresql_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      window: 30d
      when: type == "transaction"         # Database field (no prefix)
    ```

- `variance` - Variance
  - *Example: User transaction amount variance 122,500*
  - **Real-world Use Cases**:
    - Risk scoring: High variance users have higher risk, unpredictable behavior
    - Bot detection: Bot transaction variance usually very small (fixed amount)
    - Credit assessment: Low variance users have more stable repayment behavior, better credit
  - **YAML Example**:
    ```yaml
    - name: variance_userid_txn_amt_30d
      type: aggregation
      method: variance
      datasource: postgresql_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      window: 30d
      when: type == "transaction"         # Database field (no prefix)
    ```

- `percentile` - Nth percentile value
  - *Example: User transaction amount P95 is ¥1,800*
  - **Real-world Use Cases**:
    - Abnormal threshold setting: Transactions exceeding P95 require additional verification
    - Dynamic limits: Set daily limits based on user P90 transaction amount
    - Credit limit: User P75 spending amount as credit limit reference
  - **YAML Example**:
    ```yaml
    - name: p95_userid_txn_amt_30d
      type: aggregation
      method: percentile
      datasource: postgresql_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      percentile: 95
      window: 30d
      when: type == "transaction"         # Database field (no prefix)
    ```

- `median` - Median value (50th percentile)
  - *Example: User transaction amount median ¥450*
  - **Real-world Use Cases**:
    - Outlier-resistant statistics: Median is not affected by extreme values, more accurately reflects user typical behavior
    - User profiling: Median order amount for user value assessment
    - Abnormal detection: Current transaction is 10 times the median, requires verification
  - **YAML Example**:
    ```yaml
    - name: median_userid_txn_amt_30d
      type: aggregation
      method: median
      datasource: postgresql_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      window: 30d
      when: type == "transaction"         # Database field (no prefix)
    ```

- `mode` - Most frequent value
  - *Example: User most frequent transaction amount ¥100*
  - **Real-world Use Cases**:
    - Recharge pattern recognition: User most frequently recharges ¥100, abnormal recharge of ¥10,000 requires verification
    - Order brushing detection: Large number of same amount orders (mode ratio >80%) suspected order brushing
    - Habit recognition: User most frequently logs in at night 8 o'clock, early morning 3 o'clock login is abnormal
  - **YAML Example**:
    ```yaml
    - name: mode_userid_txn_amt_30d
      type: aggregation
      method: mode
      datasource: postgresql_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      window: 30d
      when: type == "transaction"         # Database field (no prefix)
    ```

- `entropy` - Shannon entropy (diversity measure)
  - *Example: User transaction type entropy 2.3, diverse behavior*
  - **Real-world Use Cases**:
    - Bot detection: Entropy too low (<0.5), single behavior pattern, may be bot
    - Account activity: High entropy users have rich behavior, more like real users
    - Abnormal detection: User historical entropy 2.5, recently dropped to 0.3, behavior abnormally uniform
  - **YAML Example**:
    ```yaml
    - name: entropy_userid_txn_type_30d
      type: aggregation
      method: entropy
      datasource: postgresql_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: transaction_type
      window: 30d
      when: type == "transaction"         # Database field (no prefix)
    ```

> **Note:** Ratio- and rate-type metrics (e.g. success rate, failure rate, conversion rate) are **not** Aggregation operators. They are derived from aggregation results and must be implemented via **Expression operators**.

**Principle:** Aggregation = Single data source + Single window + Single grouping + One-pass scan

```rust
enum AggregationType {
    Count, Sum, Avg, Max, Min, Distinct,
    StdDev, Variance, Percentile(u8), Median, Mode, Entropy,
}

struct AggregationConfig {
    pub datasource: String,
    pub entity: String,
    pub dimension: String,
    pub dimension_value: String,
    pub field: Option<String>,       // count doesn't need, others need
    pub window: Duration,
    pub when: Option<Condition>,
}

// ✅ All Aggregation operators can be implemented with a unified function!
// Common logic:
// - Time window filtering (window field)
// - Dimension grouping (dimension, dimension_value)
// - Condition matching (when field)
// - One-pass aggregation (different operators)
impl AggregationExecutor {
    fn execute(&self, op: AggregationType, config: &AggregationConfig) -> Result<Value> {
        // 1. Build query
        let sql = self.build_query(op, config)?;

        // Generate different SQL aggregation functions based on operator:
        // COUNT(*), SUM(field), AVG(field), MAX(field), MIN(field),
        // COUNT(DISTINCT field), STDDEV(field), etc.

        // 2. Execute query
        self.datasource.query(&sql)
    }

    fn build_query(&self, op: AggregationType, config: &AggregationConfig) -> String {
        let agg_expr = match op {
            AggregationType::Count => "COUNT(*)".to_string(),
            AggregationType::Sum => format!("SUM({})", config.field.as_ref().unwrap()),
            AggregationType::Avg => format!("AVG({})", config.field.as_ref().unwrap()),
            AggregationType::Max => format!("MAX({})", config.field.as_ref().unwrap()),
            AggregationType::Min => format!("MIN({})", config.field.as_ref().unwrap()),
            AggregationType::Distinct => format!("COUNT(DISTINCT {})", config.field.as_ref().unwrap()),
            AggregationType::StdDev => format!("STDDEV({})", config.field.as_ref().unwrap()),
            // ... other operators
        };

        format!(
            "SELECT {} FROM {} WHERE {} = {} AND timestamp > now() - {} {}",
            agg_expr,
            config.entity,
            config.dimension,
            config.dimension_value,
            config.window,
            config.when.as_ref().map(|w| format!("AND {}", w)).unwrap_or_default()
        )
    }
}
```

---

## DSL Consistency Analysis Across All Feature Types

### Cross-Type Field Comparison Table

| Field | Aggregation | State | Sequence | Graph | Expression | Lookup | Description |
|------|-------------|-------|----------|-------|------------|--------|-------------|
| `type` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | All types need |
| `method` | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | Lookup doesn't need |
| `datasource` | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ | Expression doesn't need |
| `entity` | ✅ | ✅ | ✅ | ⚠️ | ❌ | ❌ | Specifies table/data entity (see below) |
| `dimension` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | grouping dimension |
| `dimension_value` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | dimension value |
| `dimension_value2` | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | Second dimension value (only for dual-node Graph methods) |
| `dimension2` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | Second dimension (Graph relationship dimension) |
| `window` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | time window |
| `when` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | condition filter |
| `field` | ⚠️ | ✅ | ⚠️ | ❌ | ❌ | ❌ | calculation field |

### `entity` Field Description

**Purpose of `entity`: Specifies which table/data entity to read data from**

| Datasource Type | Needs entity? | Meaning of entity | Example |
|-----------------|---------------|-------------------|---------|
| **PostgreSQL** | ✅ Needs | Table name | `entity: events` → queries `events` table |
| **ClickHouse** | ✅ Needs | Table name | `entity: events` → queries `events` table |
| **Neo4j** | ⚠️ Depends on design | Node label or relationship type | `entity: events` or not needed |
| **Redis** | ❌ Not needed | N/A (key-value storage) | Direct access via key |
| **Expression** | ❌ Not needed | N/A (doesn't access data source) | Only uses results from other features |

**SQL Generation Example:**

```yaml
# PostgreSQL/ClickHouse
- name: cnt_userid_login_24h
  datasource: postgresql_events
  entity: events              # ← Specifies table name
  dimension: user_id
  window: 24h
```

Generated SQL:
```sql
SELECT COUNT(*)
FROM events                   -- ← entity maps to FROM clause
WHERE user_id = :current_user
  AND timestamp > now() - interval '24 hours'
```

**Entity Mapping for Different Data Sources:**

```yaml
# 1. PostgreSQL - entity = table name
datasource: postgresql_events
entity: events                # SELECT * FROM events

# 2. ClickHouse - entity = table name
datasource: clickhouse_events
entity: events                # SELECT * FROM events

# 3. Neo4j - entity may represent node label (needs design decision)
datasource: neo4j_graph
entity: events                # MATCH (e:events) or MATCH ()-[r:events]->()
# Or specify directly in query logic without using entity

# 4. Redis - doesn't need entity
datasource: redis_features
# No entity field, direct access via key

# 5. Expression - doesn't need entity
type: expression
# No entity field, doesn't access data source
```

**Design Recommendations:**

For Neo4j graph database, two approaches can be considered:

**Approach 1: Use `entity` to represent node label**
```yaml
datasource: neo4j_graph
entity: User                  # Node label
dimension: user_id
dimension2: device_id
```

**Approach 2: Don't use `entity`, specify in datasource configuration**
```yaml
datasource: neo4j_graph       # Datasource config already specifies node/relationship type to query
dimension: user_id
dimension2: device_id
# Doesn't need entity field
```

### Type-Specific Fields

**State Specific:**
- `current_value` - Current value (for comparison)

**Sequence Specific:**
- `reset_when` - Reset condition
- `order_by` - Sort field
- `baseline_window` - Baseline window (only for methods that need historical comparison like anomaly_score)
- `aggregation` - Aggregation method within window
- `pattern` - Event pattern matching
- `window_size` - Moving average window size

**Graph Specific:**
- `dimension2` - Second dimension (relationship dimension, e.g. device_id related to user_id)
- `dimension_value2` - Second dimension value (only for methods that need two nodes, such as shared_entity_count, network_distance)

**Expression Specific:**
- `expression` - Expression string
- `depends_on` - List of dependent features
- `model` / `inputs` / `output` - ML model configuration

**Lookup Specific:**
- `key` - Redis key template
- `fallback` - Default value

### Unified Implementation Feasibility Analysis

| Type | DSL Consistency | Can be unified? | Recommendation |
|------|-----------------|-----------------|----------------|
| **Aggregation** | ✅ Highly consistent | ✅ Yes | One Executor handles all methods |
| **State** | ✅ Relatively consistent | ✅ Yes | One Executor handles all methods |
| **Sequence** | ⚠️ Moderate | ⚠️ Partially | Simple ones can be unified, complex ones (pattern) need separate handling |
| **Graph** | ⚠️ Field differences | ✅ Yes | One Executor, but field names differ |
| **Expression** | ✅ Simple and consistent | ✅ Yes | Dispatch based on method: expression vs ml_model |
| **Lookup** | ✅ Simplest | ✅ Yes | Direct key-value query |

### Implementation Recommendations

```rust
// 1. Aggregation - Highly unified ✅
impl AggregationExecutor {
    fn execute(&self, method: AggregationType, config: AggregationConfig) -> Result<Value> {
        // All methods share: time window, dimension grouping, condition filtering
        // Only aggregation functions differ: COUNT/SUM/AVG/MAX/MIN/DISTINCT...
    }
}

// 2. State - Relatively unified ✅
impl StateExecutor {
    fn execute(&self, method: StateQueryType, config: StateConfig) -> Result<Value> {
        // Shared: dimension, baseline window
        // Differences: z_score needs current_value, timezone_consistency needs expected_timezone
    }
}

// 3. Sequence - Partially unified ⚠️
impl SequenceExecutor {
    fn execute(&self, method: SequenceAnalysisType, config: SequenceConfig) -> Result<Value> {
        match method {
            ConsecutiveCount => { /* Simple, can be unified */ }
            PercentChange => { /* Needs dual windows, can be unified */ }
            MovingAverage => { /* Needs window_size, can be unified */ }
            EventPatternMatch => { /* Complex, needs pattern matching engine */ }
        }
    }
}

// 4. Graph - Can be unified ✅
impl GraphExecutor {
    fn execute(&self, method: GraphAnalysisType, config: GraphConfig) -> Result<Value> {
        // Uses unified dimension/dimension_value fields
        // dimension2 represents second dimension (relationship dimension)
    }
}

// 5. Expression - Simple and unified ✅
impl ExpressionExecutor {
    fn execute(&self, method: ExpressionType, config: ExpressionConfig) -> Result<Value> {
        match method {
            CustomExpression => { /* Expression engine */ }
            MLModelScore => { /* Model inference */ }
        }
    }
}

// 6. Lookup - Simplest ✅
impl LookupExecutor {
    fn execute(&self, config: LookupConfig) -> Result<Value> {
        // Direct Redis GET operation
        self.redis.get(&config.key).or(config.fallback)
    }
}
```

### Summary

✅ **All types can be implemented with unified Executor**
- Aggregation, State, Expression, Lookup: Highly unified
- Graph: Highly unified (already using consistent field naming)
- Sequence: Simple methods can be unified, complex ones (pattern) need special handling

✅ **DSL Naming Consistency**
- All types uniformly use `dimension` / `dimension_value` as primary dimension
- Graph types use `dimension2` to represent second dimension (relationship dimension)
- Maintain consistent field naming across types

---

### 2. State Operators 🔴 Planned
> **Rust Implementation:** `StateExecutor::execute(op: StateQueryType, config: StateConfig)`
>
> **Status:** 🔴 Not yet implemented - all operators are in development roadmap
>
> ⚠️ **Important**: This section describes planned functionality. The implementation is in development and not currently available.
>
> **Design Pattern:** Statistical comparison and baseline analysis
>


**Planned:**
- `z_score` - Statistical z-score compared to baseline
  - *Example: Current transaction amount Z-score is 2.8, abnormally high*
  - **Real-world Use Cases**:
    - Abnormal transaction detection: User transaction amount Z-score > 3, may be fraudulent
    - Login frequency anomaly: Login frequency Z-score > 2.5, may be brute force
    - Dynamic threshold: Automatically adjust risk control strategy based on Z-score, not fixed threshold
  - **YAML Example**:
    ```yaml
    - name: zscore_userid_txn_amt
      type: state
      method: z_score
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      current_value: "{event.amount}"
      window: 90d
      when: type == "transaction"         # Database field (no prefix)
    ```

- `deviation_from_baseline` - Compare to historical average
  - *Example: Current login frequency higher than historical average by150%*
  - **Real-world Use Cases**:
    - Behavior mutation detection: User daily average login 2 times, today logged in 20 times, deviated 900%
    - Spending habit change: Historical daily average spending ¥200, today spending ¥5000, deviated 2400%
    - Account takeover: Behavior pattern suddenly deviates from baseline, may be controlled by others
  - **YAML Example**:
    ```yaml
    - name: deviation_userid_login_freq
      type: state
      method: deviation_from_baseline
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: login_count
      current_value: "{event.current_login_count}"
      window: 90d
      when: type == "login"               # Database field (no prefix)
    ```

- `percentile_rank` - Rank compared to history
  - *Example: Current transaction amount at historical92percentile*
  - **Real-world Use Cases**:
    - Large transaction verification: Current transaction amount exceeds historical P95, requires two-factor verification
    - Abnormal activity: Current login frequency exceeds historical P99, may be abnormal
    - Risk classification: P0-P50 low risk, P50-P90 medium risk, P90+ high risk
  - **YAML Example**:
    ```yaml
    - name: pctrank_userid_txn_amt
      type: state
      method: percentile_rank
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      current_value: "{event.amount}"
      window: 90d
      when: type == "transaction"         # Database field (no prefix)
    ```

- `is_outlier` - Statistical outlier detection
  - *Example: Current behavior determined as statistical outlier (true)*
  - **Real-world Use Cases**:
    - Automatic outlier tagging: Statistically determined as outlier, directly triggers manual review
    - Fraud detection: Multi-dimensional outlier detection for transaction amount/frequency/location etc.
    - Machine learning features: Outlier tags as ML model input features
  - **YAML Example**:
    ```yaml
    - name: outlier_userid_txn_amt
      type: state
      method: is_outlier
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      current_value: "{event.amount}"
      window: 90d
      when: type == "transaction"         # Database field (no prefix)
    ```

```rust
enum StateQueryType {
    ZScore,
    DeviationFromBaseline,
    PercentileRank,
    IsOutlier,
}

// Unified executor for statistical comparison operators:
// - Baseline computation from historical data
// - Statistical analysis (z-score, percentile, outlier detection)
// - Time-based pattern analysis
impl StateExecutor {
    fn execute(&self, op: StateQueryType, config: &StateConfig) -> Result<Value>
}
```

### 3. Sequence Operators 🔴 Planned
> **Rust Implementation:** `SequenceAnalyzer::analyze(op: SequenceAnalysisType, config: SequenceConfig)`
>
> **Status:** 🔴 Not yet implemented - all operators are in development roadmap
>
> ⚠️ **Important**: This section describes planned functionality. The implementation is in development and not currently available.
>
> **Design Pattern:** Pipeline-based analyzer with composable stages

**Planned:**
- `consecutive_count` - Count consecutive occurrences
  - *Example: User consecutively failed login 3 times*
  - **Real-world Use Cases**:
    - Brute force attack: Consecutive failed logins ≥5 times, lock account for 15 minutes
    - Payment failure: Consecutive 3 payment failures, card may be frozen or insufficient balance
    - Abnormal operation: Consecutive 10 rapid clicks, may be script attack
  - **YAML Example**:
    ```yaml
    - name: consec_userid_login_1h_failed
      type: sequence
      method: consecutive_count
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      window: 1h
      when:
        all:
          - type == "login"               # Database field (no prefix)
          - status == "failed"             # Database field (no prefix)
      reset_when: status == "success"     # Database field (no prefix)
    ```

- `streak` - Longest streak of condition
  - *Example: User consecutive7days with daily transactions (high activity)*
  - **Real-world Use Cases**:
    - User activity: Consecutive active7days users, low churn risk
    - Order brushing detection: Consecutive 30 days with daily orders, similar amounts, suspected order brushing
    - Habit formation: Consecutive3days using a feature, recommend related services
  - **YAML Example**:
    ```yaml
    - name: streak_userid_daily_txn_30d
      type: sequence
      method: streak
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      window: 30d
      when: type == "transaction"         # Database field (no prefix)
      aggregation: count_per_day
      reset_when: count_per_day == 0
    ```

- `sequence_match` - Match event sequences
  - *Example: Detected "password reset → login → large transfer" suspicious sequence*
  - **Real-world Use Cases**:
    - Account takeover: Password reset → email change → large transfer (within 15 minutes), high risk
    - Fraud pattern: Registration → identity verification → loan application → withdrawal (within 1 hour), suspected fraud
    - Normal flow: Browse products → add to cart → checkout → payment, conversion funnel analysis
  - **YAML Example**:
    ```yaml
    - name: seq_userid_account_takeover_pattern
      type: sequence
      method: sequence_match
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      window: 1h
      pattern:
        - type == "password_reset"                     # Database field
        - type == "email_change"                       # Database field
        - type == "transaction" AND amount > 10000     # Database fields
      order_by: timestamp
    ```

- `pattern_frequency` - Frequency of specific patterns
  - *Example: "Login → browse → add to cart → payment" complete path appears5times*
  - **Real-world Use Cases**:
    - Order brushing detection: Same operation sequence repeatedly appears >10 times, suspected order brushing
    - User behavior analysis: Typical path frequency of high-value users
    - Abnormal pattern: Abnormal operation sequences frequently appear, may be attack
  - **YAML Example**:
    ```yaml
    - name: freq_userid_purchase_pattern_7d
      type: sequence
      method: pattern_frequency
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      window: 7d
      pattern:
        - type == "login"                              # Database field
        - type == "browse"                             # Database field
        - type == "add_to_cart"                        # Database field
        - type == "payment"                            # Database field
      order_by: timestamp
    ```

- `trend` - Calculate trend (increasing/decreasing/stable)
  - *Example: User transaction amount shows upward trend (+15%/week)*
  - **Real-world Use Cases**:
    - Spending trend: Transaction amount continues to rise, user value grows
    - Risk trend: Failed transaction ratio shows upward trend, card may have issues
    - Abnormal detection: Login frequency suddenly shows upward trend (steep slope increase), account may be stolen
  - **YAML Example**:
    ```yaml
    - name: trend_userid_txn_amt_30d
      type: sequence
      method: trend
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      window: 30d
      when: type == "transaction"         # Database field (no prefix)
      aggregation: sum_per_week
    ```

- `percent_change` - Percentage change between windows
  - *Example: This week transaction count increased by 120% compared to last week*
  - **Real-world Use Cases**:
    - Behavior mutation: This week transactions increased by 500%, abnormally active
    - Promotion effect: Transaction volume increased by200%, significant effect
    - Dormant awakening: This week transactions increased from 0 to10, account reactivated
  - **YAML Example**:
    ```yaml
    - name: pctchg_userid_txn_cnt_week
      type: sequence
      method: percent_change
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      window: 7d
      when: type == "transaction"         # Database field (no prefix)
      aggregation: count
    ```
  - **Calculation Logic**:
    - Current window: [now - 7d, now]
    - Baseline window: [now - 14d, now - 7d]
    - Percentage change = (current window value - baseline window value) / baseline window value × 100%

- `rate_of_change` - Rate of change over time
  - *Example: User login frequency growth rate is +5times/day*
  - **Real-world Use Cases**:
    - Acceleration detection: Transaction frequency growth rate accelerates from 1 times/day to 10 times/day, abnormal
    - Progressive attack: Failed login rate increases by 2 times per hour, gradually escalating attack
    - Trend warning: Order volume decline rate -3orders/day, may churn
  - **YAML Example**:
    ```yaml
    - name: roc_userid_login_freq_7d
      type: sequence
      method: rate_of_change
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: login_count
      window: 7d
      when: type == "login"               # Database field (no prefix)
      aggregation: count_per_day
    ```

- `anomaly_score` - Statistical anomaly detection
  - *Example: Sequence anomaly score 8.5/10, highly suspicious*
  - **Real-world Use Cases**:
    - Comprehensive anomaly detection: Calculate anomaly score based on time-series model, >7 points triggers review
    - Account behavior profile: Behavior sequence difference score from historical pattern
    - Fraud probability: Sequence anomaly score as fraud model input feature
  - **YAML Example**:
    ```yaml
    - name: anomaly_userid_behavior_score_7d
      type: sequence
      method: anomaly_score
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: event_type
      window: 7d
      baseline_window: 90d
      order_by: timestamp
    ```

- `moving_average` - Moving average over window
  - *Example: User7day moving average transaction amount ¥800/day*
  - **Real-world Use Cases**:
    - Smooth trend analysis: 7day moving average eliminates daily fluctuations, observe real trend
    - Abnormal detection: Current transaction amount exceeds 7-day moving average by 3 times, abnormal
    - Dynamic baseline: Use moving average as dynamic baseline, adapts to user behavior changes
  - **YAML Example**:
    ```yaml
    - name: ma7_userid_txn_amt
      type: sequence
      method: moving_average
      datasource: clickhouse_events
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      window: 7d
      when: type == "transaction"         # Database field (no prefix)
      window_size: 7
    ```

```rust
enum SequenceAnalysisType {
    ConsecutiveCount, Streak, SequenceMatch { pattern: Vec<Pattern> },
    PatternFrequency, Trend, PercentChange, RateOfChange, AnomalyScore,
    MovingAverage { window_size: usize },
}

// Pipeline-based analyzer - operators share:
// - Event ordering
// - Windowing logic
// - Pattern matching engine
impl SequenceAnalyzer {
    fn analyze(&self, op: SequenceAnalysisType, config: &SequenceConfig) -> Result<Value>
}

// Note: Session-based analysis (session_count, session_duration, events_per_session)
// should be implemented using Aggregation operators with session_id provided by
// the business system. Examples:
//   - session_count → distinct(session_id)
//   - session_duration → avg(session_duration) where session_duration is provided
//   - events_per_session → expression: total_events / distinct_sessions
```

### 4. Graph Operators 🔴 Planned
> **Rust Implementation:** `GraphAnalyzer::analyze(op: GraphAnalysisType, config: GraphConfig)`
>
> **Status:** 🔴 Not yet implemented - all operators are in development roadmap
>
> ⚠️ **Important**: This section describes planned functionality. The implementation is in development and not currently available.
>
> **Design Pattern:** Graph-based analyzer with lazy graph construction

**Field Semantics Description:**

Graph type analyzes bipartite graph structure, where:
- `dimension` - Primary entity type (e.g. user_id)
- `dimension2` - Related entity type (e.g. device_id)
- Forms graph structure: User <--> Device <--> User <--> Device

**Single-node methods** (e.g. graph_centrality, community_size):
- `dimension_value` - Node to analyze
- `dimension2` - How this node relates to other nodes

**Dual-node methods** (e.g. shared_entity_count, network_distance):
- `dimension_value` - Start/source node
- `dimension_value2` - End/target node (same type)
- `dimension2` - What connects the two nodes (intermediate node type)

**Planned:**
- `graph_centrality` - Network centrality score
  - *Example: Device centrality in user network 0.65, may be shared device*
  - **Real-world Use Cases**:
    - Core node identification: Centrality >0.8devices, may be core devices of fraud gang
    - Risk source location: High centrality accounts marked as fraud, associated accounts need review
    - Black market identification: IPs with abnormally high centrality may be black market operation nodes
  - **YAML Example**:
    ```yaml
    - name: centrality_device_in_user_network
      type: graph
      method: graph_centrality
      datasource: neo4j_graph
      dimension: device_id
      dimension_value: "{event.device_id}"
      dimension2: user_id
      window: 30d
    ```

- `community_size` - Size of connected component
  - *Example: Fraud gang community size for this user is 23 people*
  - **Real-world Use Cases**:
    - Gang fraud: Community size >20 people with similar transaction patterns, suspected fraud gang
    - Money laundering network: Funds circulate within large community, may be money laundering
    - Normal social: Small community (<5 people) with normal behavior, may be family/friends
  - **YAML Example**:
    ```yaml
    - name: community_size_userid_device_network
      type: graph
      method: community_size
      datasource: neo4j_graph
      dimension: user_id
      dimension_value: "{event.user_id}"
      dimension2: device_id
      window: 90d
    ```

- `shared_entity_count` - Count shared connections
  - *Example: Two users share 5 same devices*
  - **Real-world Use Cases**:
    - Fake accounts: Two accounts share >3 devices, may be multiple accounts by same person
    - Associated fraud: Multiple high-risk accounts share devices/IPs, coordinated fraud
    - Family identification: Share2devices (phone + computer), may be family members
  - **YAML Example**:
    ```yaml
    - name: shared_devices_between_users
      type: graph
      method: shared_entity_count
      datasource: neo4j_graph
      dimension: user_id                      # Node type
      dimension_value: "{event.user_id}"      # Node 1 (source)
      dimension_value2: "{event.target_user_id}"  # Node 2 (target)
      dimension2: device_id                   # Shared entity type
      window: 30d
    ```
  - **Field Description**:
    - `dimension: user_id` - Main entity type (both nodes are User)
    - `dimension_value` - User1
    - `dimension_value2` - User2
    - `dimension2: device_id` - **Shared intermediate node type**
    - Calculation result: How many Devices do User1 and User2 share
  - **Graph Structure Example**:
    ```
    User1 --uses--> Device1 <--uses-- User2
          --uses--> Device2 <--uses--
          --uses--> Device3 <--uses--

    Result = 3 (share 3 devices)
    ```
    - User1 and User2 are both `user_id` type (dimension)
    - Device1/2/3 are `device_id` type (dimension2, shared nodes)
    - Find: How many Devices connect to both User1 and User2

- `network_distance` - Distance between entities in graph
  - *Example: Network distance between two accounts is3hops (indirect association)*
  - **Real-world Use Cases**:
    - Risk propagation: Distance to known fraud account ≤2hops, needs review
    - Association analysis: Although no direct association, network distance ≤3hops, indirect association
    - Social recommendation: Users with network distance 2-3 hops may have common interests
  - **YAML Example**:
    ```yaml
    - name: network_dist_to_fraud_account
      type: graph
      method: network_distance
      datasource: neo4j_graph
      dimension: user_id                      # Node type
      dimension_value: "{event.user_id}"      # Start node
      dimension_value2: "{known_fraud_user_id}"   # End node
      dimension2: device_id                   # Connection path
      window: 90d
    ```
  - **Field Description**:
    - `dimension: user_id` - Primary entity type (Start and End are both User)
    - `dimension_value` - Start User
    - `dimension_value2` - End User
    - `dimension2: device_id` - **Intermediate connection node type** (not End type!)
    - Calculation result: Shortest hop count from start to end
  - **Graph Structure Example**:
    ```
    UserA --uses--> Device1 <--uses-- UserC --uses--> Device2 <--uses-- UserB
    Start                                                              End

    Hop count = 2 hops (UserA -> Device1 -> UserC -> Device2 -> UserB)
    ```
    - UserA and UserB are both `user_id` type (dimension)
    - Device1 and Device2 are `device_id` type (dimension2, intermediate connections)
    - Graph traversal: User -> Device -> User -> Device -> User

```rust
enum GraphAnalysisType {
    // Network analysis (require graph traversal)
    Centrality, CommunitySize, SharedEntityCount, NetworkDistance,
}

// Graph analyzer focuses on true graph algorithms:
// - Graph construction and indexing (for network-based operators)
// - Graph traversal (BFS, DFS)
// - Graph algorithms (PageRank, community detection, shortest path)
//
// Note:
// - Entity linking (e.g., devices per IP) should use distinct() aggregation
impl GraphAnalyzer {
    fn analyze(&self, op: GraphAnalysisType, config: &GraphConfig) -> Result<Value>
}
```

### 5. Expression Operators 🟢 Implemented
> **Rust Implementation:** `ExpressionEngine::evaluate(expr: ExpressionType, context: &FeatureContext)`
>
> **Status:** ✅ Production-ready | 📋 ML model integration planned
>
> **Design Pattern:** Expression engine with pluggable evaluators
>
> **⚠️ Architecture Constraint:** Expression operators do NOT access raw data sources or define time windows; they ONLY consume results from other features.

**Implemented:**
- `expression` - Evaluate custom expressions using other features
  - *Example: Calculate login failure rate = failed_count / total_count*
  - **Real-world Use Cases**:
    - Failure rate calculation: login_failure_rate = failed_login_count_1h / login_count_1h
    - Composite score: risk_score = 0.4 * transaction_anomaly + 0.3 * device_risk + 0.3 * location_risk
    - Ratio analysis: large_transaction_ratio = transactions_above_1000 / total_transactions
    - Conversion rate: conversion_rate = purchase_count / view_count
  - **YAML Example**:
    ```yaml
    - name: rate_userid_login_failure
      type: expression
      method: expression
      expression: "failed_login_count_1h / login_count_1h"
      depends_on:
        - failed_login_count_1h
        - login_count_1h
    ```

```rust
enum ExpressionType {
    CustomExpression { expr: String },
    MlModelScore { model_id: String, inputs: Vec<String> },
    EmbeddingSimilarity { embedding_a: String, embedding_b: String },
    ClusteringLabel { algorithm: String },
    MlAnomalyScore { model_id: String },
}

// Expression engine - provides:
// - Expression parser
// - Feature dependency resolution
// - ML model integration
//
// IMPORTANT: ExpressionEngine only operates on pre-computed feature values,
// never directly accessing data sources or executing queries.
impl ExpressionEngine {
    fn evaluate(&self, expr: ExpressionType, context: &FeatureContext) -> Result<Value>
}
```

### 6. Lookup Operators 🟢 Implemented
> **Rust Implementation:** `DataSource::get(key: &str) -> Result<Value>`
>
> **Status:** ✅ Production-ready
>
> **Design Pattern:** Simple key-value retrieval from Redis cache
>
> **⚠️ Architecture Constraint:** Lookup features do NOT perform any computation; they only retrieve pre-computed values.

**Implemented:**
- Direct key-value lookup from datasource
  - *Example: Query user 90-day risk score from Redis*
  - **Real-world Use Cases**:
    - Batch-computed risk scores: Batch compute user risk scores every morning, stored in Redis
    - User segmentation labels: User clustering labels generated by data analysis team, cached in Redis
    - Device fingerprint: Device reputation database maintained by security team, stored in Redis
    - Cached aggregation features: Aggregation metrics pre-calculated by scheduled tasks, accelerate real-time queries
  - **YAML Example**:
    ```yaml
    # Redis lookup
    - name: user_risk_score_90d
      type: lookup
      datasource: redis_features
      key: "user_risk_score:{event.user_id}"
      fallback: 50
    ```

```rust
// Lookup features use simple datasource interface
// No complex executor needed - just key-value retrieval

impl FeatureExecutor {
    fn execute_lookup(&self, config: &LookupConfig) -> Result<Value> {
        let datasource = self.datasources.get(&config.datasource)?;

        match datasource.get(&config.key) {
            Ok(value) => Ok(value),
            Err(_) => Ok(config.fallback.clone().unwrap_or(Value::Null)),
        }
    }
}
```

---

## Architecture Benefits

This design provides:

1. **Code Reuse**: Common logic (time windows, filtering, caching) shared across operators
2. **Maintainability**: Adding new operators only requires extending enums, not creating new functions
3. **Performance**: Unified executors can optimize query plans and batch operations
4. **Type Safety**: Enum-based dispatch ensures compile-time operator validation
5. **Testability**: Each executor can be tested independently with all operator variants

---

## Data Source Configuration

Feature definitions reference data sources through the `datasource` field, which points to data source configurations defined in `repository/configs/datasources/`. This design provides:

- **Configuration Separation**: Data source credentials and connection details are managed independently
- **Environment Isolation**: Different environments (dev/staging/prod) can use different data source configurations
- **Security**: Sensitive information (passwords, API keys) managed via environment variables
- **Reusability**: Multiple features can reference the same data source

### Data Source Types

**Event/Transaction Data** (for real-time computation):
- `postgresql` - PostgreSQL for transactional and event data (used by Aggregation features)
- `clickhouse` - ClickHouse for high-volume event storage (used by State/Sequence features)

**Graph Data** (for network analysis):
- `neo4j` - Neo4j graph database for relationship and network analysis

**Pre-computed Features** (for lookups):
- `redis` - Redis for cached feature values

**Profile/Context Data:**
- Should be passed directly in the request payload, not queried via datasource

### Data Source Configuration Files

Data sources are defined in `repository/configs/datasources/` as YAML files:

**`repository/configs/datasources/postgresql_events.yaml`**:
```yaml
name: postgresql_events
type: postgresql
config:
  host: ${POSTGRES_HOST}
  port: 5432
  database: ${POSTGRES_DATABASE}
  user: ${POSTGRES_USER}
  password: ${POSTGRES_PASSWORD}
  sslmode: ${POSTGRES_SSLMODE}  # disable, require, verify-ca, verify-full
  max_connections: 20
  connection_timeout: 30
```

**`repository/configs/datasources/redis_features.yaml`**:
```yaml
name: redis_features
type: redis
config:
  host: ${REDIS_HOST}
  port: 6379
  password: ${REDIS_PASSWORD}
  db: 0
  key_prefix: "features:"
  ttl: 86400  # Default TTL in seconds (24 hours)
```

**`repository/configs/datasources/neo4j_graph.yaml`**:
```yaml
name: neo4j_graph
type: neo4j
config:
  uri: ${NEO4J_URI}  # e.g., bolt://localhost:7687
  user: ${NEO4J_USER}
  password: ${NEO4J_PASSWORD}
  database: ${NEO4J_DATABASE}  # e.g., neo4j
  max_connection_lifetime: 3600
  max_connection_pool_size: 50
  connection_timeout: 30
```

### Feature Definitions Referencing Data Sources

Features reference data sources by name through the `datasource` field:

```yaml
# Aggregation feature - queries event data from PostgreSQL
- name: cnt_userid_login_1h
  type: aggregation
  method: count
  datasource: postgresql_events  # References postgresql_events.yaml
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 1h
  when: type == "login"               # Database field (no prefix)

# Lookup feature - retrieves pre-computed value from Redis
- name: user_risk_score_90d
  type: lookup
  datasource: redis_features      # References redis_features.yaml
  key: "user_risk_score_90d:{event.user_id}"
  fallback: 50

# State feature - computes z-score from historical data
- name: zscore_userid_txn_amt
  type: state
  method: z_score
  datasource: clickhouse_events    # References clickhouse_events.yaml
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  current_value: "{event.amount}"
  window: 90d
  when: type == "transaction"         # Database field (no prefix)

# Graph feature - analyzes network relationships from Neo4j
- name: centrality_userid_device_30d
  type: graph
  method: graph_centrality
  datasource: neo4j_graph           # References neo4j_graph.yaml
  dimension: user_id
  dimension_value: "{event.user_id}"
  dimension2: device_id
  window: 30d

# Expression feature - no datasource needed
- name: rate_userid_login_failure
  type: expression
  method: expression
  expression: "failed_login_count_1h / login_count_1h"
  depends_on:
    - failed_login_count_1h
    - login_count_1h
  # Note: Expression features don't need datasource
```

### Implementation Pattern

```rust
// Data source configuration loaded from repository/configs/datasources/
pub struct DataSourceConfig {
    pub name: String,
    pub source_type: DataSourceType,
    pub config: serde_json::Value,
}

pub enum DataSourceType {
    ClickHouse,
    PostgreSQL,
    Redis,
    Neo4j,
}

// Feature type enum with Lookup as independent category
pub enum FeatureType {
    Aggregation { method: AggregationType, config: AggregationConfig },
    State { method: StateQueryType, config: StateConfig },
    Sequence { method: SequenceAnalysisType, config: SequenceConfig },
    Graph { method: GraphAnalysisType, config: GraphConfig },
    Expression { expr: String, depends_on: Vec<String> },
    Lookup { key: String, fallback: Option<Value> },  // New category
}

// Feature definition references datasource by name
pub struct FeatureDefinition {
    pub name: String,
    pub feature_type: FeatureType,
    pub datasource: Option<String>,  // References datasource name
    // ...
}

// Feature executor with datasource registry
pub struct FeatureExecutor {
    datasources: HashMap<String, Box<dyn DataSource>>,
}

impl FeatureExecutor {
    // Load datasources from repository/configs/datasources/
    pub fn new(datasource_configs: Vec<DataSourceConfig>) -> Result<Self> {
        let mut datasources = HashMap::new();
        for config in datasource_configs {
            let datasource = create_datasource(&config)?;
            datasources.insert(config.name.clone(), datasource);
        }
        Ok(Self { datasources })
    }

    pub fn execute(&self, feature: &FeatureDefinition) -> Result<Value> {
        match &feature.feature_type {
            // Lookup - simple key-value retrieval
            FeatureType::Lookup { key, fallback } => {
                let datasource = self.datasources.get(
                    feature.datasource.as_ref().ok_or("datasource required")?
                )?;
                datasource.get(key).or(fallback.clone())
            }

            // Aggregation - compute from events
            FeatureType::Aggregation { operator, config } => {
                let datasource = self.datasources.get(
                    feature.datasource.as_ref().ok_or("datasource required")?
                )?;
                AggregationExecutor::new(datasource).execute(operator, config)
            }

            // Expression - no datasource needed
            FeatureType::Expression { expr, depends_on } => {
                ExpressionEngine::evaluate(expr, depends_on, context)
            }

            // ... other types
        }
    }
}
```

### Feature Category Requirements

| Feature Type | Needs `datasource`? | Needs `operator`? | Description |
|--------------|---------------------|-------------------|-------------|
| **Aggregation** | ✅ Yes | ✅ Yes | Queries event data for real-time computation |
| **State** | ✅ Yes | ✅ Yes | Queries historical data for baseline analysis |
| **Sequence** | ✅ Yes | ✅ Yes | Queries time-series data for pattern analysis |
| **Graph** | ✅ Yes | ✅ Yes | Queries relationship data for graph algorithms |
| **Expression** | ❌ No | ✅ Yes | Only consumes other feature results |
| **Lookup** | ✅ Yes | ❌ No | Directly retrieves pre-computed values |

---

## Table of Contents

- [Data Source Configuration](#data-source-configuration)
- [1. Aggregation (Counting/Aggregating)](#1-aggregation-Counting/Aggregating)
- [2. State (Checking Current State)](#2-state-Checking Current State)
- [3. Sequence (Analyzing Process)](#3-sequence-Analyzing Process)
- [4. Graph (Analyzing Relationships)](#4-graph-Analyzing Relationships)
- [5. Expression (Computing Scores)](#5-expression-Computing Scores)
- [6. Lookup (Looking up Pre-computed Values)](#6-lookup-Looking up Pre-computed Values)
- [Implementation Roadmap](#implementation-roadmap)
- [By Risk Domain](#by-risk-domain)

---

## 1. Aggregation (Counting/Aggregating)

**Purpose:** Count events, aggregate values, and compute statistical measures.

### Key Concepts

**Understanding `dimension` vs `field`:**

- **`dimension`** - **grouping dimension**（GROUP BY）
  - Represents "group by what"
  - For example: `dimension: user_id` means Group by user
  - Equivalent to SQL `GROUP BY user_id`

- **`field`** - **calculation field** (field on which aggregation function operates)
  - Represents "which field to calculate"
  - For example: `field: amount` means aggregate on amount field
  - Equivalent to SQL `AVG(amount)`, `SUM(amount)` etc.

**Example Understanding:**
```yaml
- name: avg_userid_order_amt_30d
  dimension: user_id    # Group by user
  field: amount         # Calculate average of amount
```
Equivalent to SQL:
```sql
SELECT user_id, AVG(amount)
FROM events
WHERE type='order' AND timestamp > now() - 30d
GROUP BY user_id
```

**When is `field` needed:**
- `count` - ❌ Not needed (only counts, doesn't care about specific field values)
- `sum`, `avg`, `max`, `min`, `stddev` - ✅ Needed (must specify which field to calculate)
- `distinct` - ✅ Needed (counts distinct values of a field)

### Field Reference Syntax in `when` Conditions

When filtering database rows in `when` conditions, you can reference two types of fields:

**1. Database fields (from the data table specified by entity)**
- No prefix needed, directly reference column names
- Examples: `type`, `status`, `amount`, `country`
- Supports JSON nested fields: `attributes.device.fingerprint`, `metadata.user.tier`

**2. Request fields (from API request's context.event)**
- Use template syntax with curly braces: `{event.field_name}`
- Examples: `{event.user_id}`, `{event.min_amount}`, `{event.threshold}`
- Used for dynamic filtering and template substitution

**Examples:**

```yaml
# Database field filtering (no prefix needed)
when: type == "transaction"

# Database JSON nested field access
when: attributes.risk_level == "high"

# Combining database fields and request fields
when:
  all:
    - type == "payment"                      # Database field
    - amount > {event.threshold}             # Request field (dynamic value)
    - metadata.country == "{event.country}"  # Database JSON field matches request value

# Complex JSON nested fields
when: user.profile.verification_status == "verified"
```

**SQL Generation Example:**

Configuration:
```yaml
when:
  all:
    - type == "transaction"
    - amount > {event.min_amount}
    - attributes.device_type == "mobile"
```

Generated SQL:
```sql
SELECT COUNT(*)
FROM events
WHERE user_id = $1
  AND event_timestamp >= NOW() - INTERVAL '24 hours'
  AND type = 'transaction'                           -- Database field
  AND amount > $2                                     -- Request value substitution
  AND attributes->>'device_type' = 'mobile'          -- JSON field access
```

### `dimension` vs `when` - Cannot replace each other

**Important conceptual distinction:**

| Field | Role | SQL Equivalent | Purpose |
|-------|------|----------------|---------|
| `dimension` | grouping dimension | `GROUP BY` | Determines "**for whom** to calculate" (grouping) |
| `when` | filter condition | `WHERE` | Determines "**what** to calculate" (filtering events) |
| `field` | calculation field | `SUM(field)` | Determines "**which field** to calculate" |

**Example comparison:**

```yaml
# Correct: Use dimension for grouping
- name: cnt_userid_login_24h
  dimension: user_id              # Calculate separately for each user
  when: type == "login"           # Only count login events (database field, no prefix needed)
```

SQL equivalent:
```sql
SELECT user_id, COUNT(*)
FROM events
WHERE type = 'login' AND timestamp > now() - 24h
GROUP BY user_id               -- role of dimension
-- Result: Each user has their own login count
```

**What happens if dimension is removed?**

```yaml
# Wrong: Missing dimension, no grouping
- name: cnt_all_login_24h
  when: type == "login"           # Database field, no prefix needed
  # No dimension = no grouping
```

SQL equivalent:
```sql
SELECT COUNT(*)
FROM events
WHERE type = 'login' AND timestamp > now() - 24h
-- No GROUP BY
-- Result: All users' login counts added together, only one total count!
```

**Using dimension and when together:**

```yaml
- name: cnt_userid_txn_24h_large
  dimension: user_id              # Group by user
  when:
    all:
      - type == "transaction"        # Filter: only transaction events (database field, no prefix needed)
      - amount > 1000                # Filter: amount greater than 1000 (database field, no prefix needed)
```

SQL equivalent:
```sql
SELECT user_id, COUNT(*)
FROM events
WHERE type = 'transaction'          -- when condition 1 (database field)
  AND amount > 1000                 -- when condition 2 (database field)
  AND timestamp > now() - 24h
GROUP BY user_id                    -- dimension
-- Result: Each user's large transactions (>1000) count
```

**Summary:**
- ✅ `dimension` creates groups, one result value per group
- ✅ `when` filters events, only events matching conditions participate in calculation
- ❌ **Cannot** use `when` to replace `dimension`, they have completely different roles!

### Why can't `dimension_value` be replaced with `when`?

**Common misunderstanding:** "Since `dimension_value: "{event.user_id}"` also extracts values from request events, why can't we use `when: user_id == ...` to replace it?"

**Key differences:**

| Approach | Nature | Number of Results | Use Case |
|----------|--------|-------------------|----------|
| `dimension + dimension_value` | **Dynamic grouping** | Calculate one result for **each different value** in data | Calculate for all users |
| `when` condition | **Static filtering** | Can only calculate for **one hardcoded value** | Calculate only for specific user |

**Example comparison:**

```yaml
# Approach 1: Using dimension (correct) - dynamic grouping
- name: cnt_userid_login_24h
  dimension: user_id
  dimension_value: "{event.user_id}"    # Dynamically extract user_id from each event
  when: type == "login"               # Database field (no prefix)
```

**Execution logic:**
```
Event stream:
  event1: {user_id: "user_A", type: "login"}  → Grouped into user_A group
  event2: {user_id: "user_B", type: "login"}  → Grouped into user_B group
  event3: {user_id: "user_A", type: "login"}  → Grouped into user_A group
  event4: {user_id: "user_C", type: "login"}  → Grouped into user_C group

Results (calculated for each user):
  user_A: 2 times
  user_B: 1 times
  user_C: 1 times
  ... (all users with logins in the system)
```

```yaml
# Approach 2: Using when condition (wrong) - static filtering
- name: cnt_login_24h_for_userA
  when:
    all:
      - type == "login"              # Database field
      - user_id == "user_A"          # Hardcoded to only look at user_A!
```

**Execution logic:**
```
Event stream:
  event1: {user_id: "user_A", type: "login"}  → ✅ Included
  event2: {user_id: "user_B", type: "login"}  → ❌ Filtered out
  event3: {user_id: "user_A", type: "login"}  → ✅ Included
  event4: {user_id: "user_C", type: "login"}  → ❌ Filtered out

Results (only one value):
  Total: 2 times  (Only user_A's login count, all other users are lost!)
```

**Difference at runtime:**

When the risk engine evaluates a user's (e.g., user_B) transaction:

```yaml
# Using dimension (correct)
dimension: user_id
dimension_value: "{event.user_id}"  # Automatically replaced at runtime with "user_B"

→ Query: SELECT COUNT(*) WHERE user_id = 'user_B' AND ...
→ Returns: user_B's login count
```

```yaml
# Using when (wrong)
when: user_id == "user_A"           # Hardcoded!

→ Query: SELECT COUNT(*) WHERE user_id = 'user_A' AND ...
→ Returns: user_A's login count (wrong! What we want is user_B's data)
```

**True role of `dimension_value`:**

`dimension_value` is a **template expression** that will be replaced at runtime:
- When configuring, write: `dimension_value: "{event.user_id}"`
- At runtime, evaluate current event, automatically becomes: `dimension_value: "user_123"` (current user)
- Then query this user's historical data, group and aggregate by user_id

**Key Understanding:**
- ✅ `dimension_value` is the **grouping basis**, tells the system "which value to extract from current event as grouping key"
- ❌ `when` is a **fixed filter condition**, can only hardcode one specific value, cannot dynamically adapt to different users
- 💡 If you need to calculate independent statistics for **each user**, you must use `dimension`, cannot use `when`!

### True role of `dimension` in real-time risk control scenarios

**Your question might be:** "In real-time decision making, only calculating for current user, why can't we directly use `when` condition to filter `user_id`?"

**Answer: Technically possible, but should not be done in design.** Reasons are as follows:

#### 1. **Actual query during real-time calculation**

When evaluating user_B's transaction, the actual SQL query:

```sql
-- Using dimension approach
SELECT COUNT(*)
FROM events
WHERE user_id = 'user_B'      -- From dimension_value
  AND type = 'login'          -- From when condition
  AND timestamp > now() - 24h
```

You're right! In this scenario, `user_id = 'user_B'` is indeed a WHERE condition, **can theoretically be placed in `when`**.

#### 2. **Why still use `dimension` instead of `when`?**

**Reason 1: Clear semantics**

```yaml
# Approach 1: Using dimension (recommended) - clear semantics
dimension: user_id              # Clear: This is the dimension for "whom" to calculate
dimension_value: "{event.user_id}"
when: type == "login"           # Clear: This is filter condition (database field, no prefix needed)

# Approach 2: All placed in when (not recommended) - semantic confusion
when:
  all:
    - user_id == "{event.user_id}"     # This is not filtering, this is specifying query subject
    - type == "login"                  # This is the filtering (database field)
```

**Reason 2: Support multiple calculation modes**

System needs to support two modes:

| Mode | Scenario | SQL | Needs GROUP BY? |
|------|----------|-----|-----------------|
| **Online mode** | Real-time decision for single event | `WHERE user_id = :current_user` | ❌ Not needed |
| **Offline mode** | Batch pre-compute features | `GROUP BY user_id` | ✅ Needed |

Benefit of using `dimension`: **Same configuration can be used for both modes**

```yaml
# Same configuration
dimension: user_id
dimension_value: "{event.user_id}"
when: type == "login"              # Database field, no prefix needed

# When executing in online mode:
SELECT COUNT(*) WHERE user_id = 'user_B' AND type = 'login'

# When batch calculating offline:
SELECT user_id, COUNT(*) WHERE type = 'login' GROUP BY user_id
```

**Reason 3: Query optimization**

System can recognize `dimension` as grouping dimension and do targeted optimization:
- Automatically select appropriate index
- Identify partition key (if database is partitioned by user_id)
- Parallel computation optimization

If all placed in `when`, system cannot distinguish which condition is "grouping dimension" and which is "filter condition".

**Reason 4: Configuration reuse**

```yaml
# Using dimension - can easily change dimension
dimension: user_id       # By user
# dimension: device_id   # Change to by device, only change one line
# dimension: ip_address  # Change to by IP, only change one line

# Using when - need to change multiple places
when:
  all:
    - user_id == "{event.user_id}"        # Need to change here
    - type == "login"                     # Database field
```

#### Summary: Design Principles

| Field | Semantics | Role |
|-------|-----------|------|
| `dimension` | "**For whom** to calculate" (query subject) | Specify aggregation dimension, support both online/offline modes |
| `when` | "**What kind of** events participate in calculation" (business filtering) | Filter events matching business conditions |

**Although in pure online scenarios, technically can put dimension_value into when, but for:**
- ✅ Clear semantics (clearly distinguish "for whom" and "what")
- ✅ Support offline batch calculation
- ✅ Facilitate system optimization
- ✅ Configuration easier to maintain and reuse

**We still recommend using `dimension` field.**

### DSL Consistency Analysis ✅

**Conclusion: All Aggregation operators' DSL structure is highly consistent, can be implemented with unified Rust function!**

| Field | count | sum | avg | max | min | distinct | stddev | Consistency |
|------|-------|-----|-----|-----|-----|----------|--------|--------|
| `type` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `operator` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `datasource` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `entity` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `dimension` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `dimension_value` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `window` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `when` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `field` | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ Only count doesn't need |

**Implementation recommendations:**

```rust
// ✅ Unified configuration structure
struct AggregationConfig {
    datasource: String,
    entity: String,
    dimension: String,
    dimension_value: String,
    field: Option<String>,      // None for count, Some(field_name) for others
    window: Duration,
    when: Option<Condition>,
}

// ✅ Unified executor
impl AggregationExecutor {
    // One function handles all operators!
    fn execute(&self, op: AggregationType, config: &AggregationConfig) -> Result<Value> {
        // Only difference is the SQL aggregation function generated:
        // COUNT(*) vs SUM(field) vs AVG(field) vs MAX(field) ...
        let sql = self.build_query(op, config)?;
        self.datasource.query(&sql)
    }
}
```

**Advantages:**
- ✅ High Code Reuse rate (share time window, dimension grouping, condition filtering logic)
- ✅ Easy to extend (adding new operator only requires adding variant in enum)
- ✅ Unified testing (one test framework covers all operators)
- ✅ Consistent configuration (users learn once and can use all operators)

### ✅ 1.1 Basic Counting
**Status:** Implemented

Count events and unique values within time windows.

**Operators:**
- `count` - Count events matching conditions
- `distinct` - Count unique values of a field (also used for entity linking)

**Use Cases:**
- Login attempts in time window
- Transaction count
- Failed payment attempts
- Unique IP addresses per user
- **Entity linking**: Devices associated with an IP (use `distinct`)
- **Entity linking**: Users per device (use `distinct`)

**Example:**
```yaml
- name: cnt_userid_login_24h
  type: aggregation
  method: count
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 24h
  when: type == "login"               # Database field (no prefix)
```

---

### ✅ 1.2 Basic Aggregations
**Status:** Implemented

Statistical aggregations over numeric fields.

**Operators:**
- `sum` - Sum of field values
- `avg` - Average of field values
- `max` - Maximum value
- `min` - Minimum value

**Use Cases:**
- Total transaction amount
- Average order value
- Maximum single transaction
- Minimum deposit amount

**Example:**
```yaml
- name: sum_userid_txn_amt_24h
  type: aggregation
  method: sum
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  window: 24h
  when: type == "transaction"         # Database field (no prefix)
```

---

### 📋 1.3 Statistical Aggregations
**Status:** Planned - Medium Priority

Advanced statistical measures for distribution analysis.

**Proposed Operators:**
- `stddev` - Standard deviation
- `variance` - Variance
- `percentile` - Nth percentile value
- `median` - Median value
- `mode` - Most frequent value
- `entropy` - Shannon entropy (diversity measure)
- `coefficient_of_variation` - Stddev / mean

**Use Cases:**
- Transaction amount variability
- Behavior consistency scoring
- Distribution analysis
- User behavior diversity

**Proposed Syntax:**
```yaml
- name: stddev_userid_txn_amt_30d
  type: aggregation
  method: stddev
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  window: 30d
  when: type == "transaction"         # Database field (no prefix)
```

---

## 2. State (Checking Current State)

**Purpose:** Statistical comparison and baseline analysis for anomaly detection.

> **Note:** State operators focus on **statistical comparisons** (z-score, baseline deviation, etc.). For simple lookups, use `datasource` configuration without operators. See "Data Source Configuration" section.

### 📋 2.1 Time-of-Day/Week State
**Status:** Planned - Medium Priority

Temporal pattern features based on time of day/week.

**Proposed Operators:**
- `timezone_consistency` - Timezone pattern consistency check

> **Note:** Simple time-based checks (e.g., off-hours activity) should use **Expression** or **Aggregation** operators, not State operators:
> - Off-hours check: Use expression like `event.hour < 8 || event.hour > 22`
> - Off-hours count: Use aggregation with when condition (see State operators section for examples)
>
> **Note:** Distribution-style features (e.g. hour-of-day or day-of-week histograms) are **not** State operators. They should be implemented as **Aggregation operators** over derived time dimensions (e.g. `hour_of_day`, `day_of_week` fields).

**Use Cases:**
- Timezone anomaly detection
- Behavioral consistency across timezones
- VPN/proxy detection

**Proposed Syntax:**
```yaml
- name: timezone_consistency_userid_7d
  type: state
  method: timezone_consistency
  datasource: clickhouse_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 7d
  expected_timezone: "{user.timezone}"
```

---

### 📋 2.2 Historical Baseline State
**Status:** Planned - Low Priority

Compare current behavior to historical baselines.

**Proposed Operators:**
- `deviation_from_baseline` - Compare to historical average
- `percentile_rank` - Rank compared to history
- `z_score` - Statistical z-score
- `is_outlier` - Statistical outlier detection

**Use Cases:**
- Anomaly detection
- Behavior change detection
- Risk scoring
- Account takeover indicators

**Proposed Syntax:**
```yaml
- name: zscore_userid_txn_amt
  type: state
  method: z_score
  datasource: clickhouse_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  current_value: "{event.amount}"
  window: 90d
  when: type == "transaction"         # Database field (no prefix)
```

---

## 3. Sequence (Analyzing Process)

**Purpose:** Analyze patterns, trends, and sequences of events over time.

### 📋 3.1 Pattern Sequences
**Status:** Planned - High Priority

Detect sequential patterns and consecutive events.

**Proposed Operators:**
- `consecutive_count` - Count consecutive occurrences
- `streak` - Longest streak of condition
- `sequence_match` - Match event sequences
- `pattern_frequency` - Frequency of specific patterns

**Use Cases:**
- Consecutive failed logins
- Consecutive successful transactions
- Login → Transaction → Withdrawal pattern
- Dormant account reactivation
- Unusual event sequences

**Proposed Syntax:**
```yaml
- name: consec_userid_login_1h_failed
  type: sequence
  method: consecutive_count
  datasource: clickhouse_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 1h
  when:
    all:
      - type == "login"              # Database field (no prefix)
      - status == "failed"           # Database field (no prefix)
  order_by: timestamp
  reset_when: status == "success"    # Database field (no prefix)
```

---

### 📋 3.2 Trend Detection
**Status:** Planned - Medium Priority

Detect changes and trends over time.

**Proposed Operators:**
- `trend` - Calculate trend (increasing/decreasing/stable)
- `percent_change` - Percentage change between windows
- `rate_of_change` - Rate of change over time
- `anomaly_score` - Statistical anomaly detection
- `seasonal_deviation` - Deviation from seasonal baseline

**Use Cases:**
- Sudden transaction volume spike
- Spending pattern shift
- Behavior change detection
- Account takeover indicators

**Proposed Syntax:**
```yaml
- name: pctchg_userid_txn_amt_1h
  type: sequence
  method: percent_change
  datasource: clickhouse_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  window: 1h
  aggregation: sum
  when: type == "transaction"         # Database field (no prefix)
```

**Calculation:**
- Current window: [now - 1h, now]
- Baseline window: [now - 2h, now - 1h]
- Percent change: (current - baseline) / baseline × 100%

---

### 💡 3.3 Session-Based Analysis
**Status:** Use Aggregation Operators

Session-based features should be implemented using **Aggregation operators** with `session_id` provided by the business system.

**Implementation Approach:**

| Session Metric | Implementation |
|----------------|----------------|
| Session count | `distinct(session_id)` |
| Average session duration | `avg(session_duration)` where business system provides duration |
| Events per session | `expression: total_events / distinct(session_id)` |
| Session gap | Compute via business system or use `time_since` on session_start_time |

**Example:**
```yaml
# Session count - using distinct
- name: distinct_userid_session_24h
  type: aggregation
  method: distinct
  datasource: postgresql_events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: session_id
  window: 24h

# Average session duration - using avg
- name: avg_userid_session_duration_7d
  type: aggregation
  method: avg
  datasource: postgresql_events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: session_duration  # Business system provides this
  window: 7d

# Events per session - using expression
- name: events_per_session_7d
  type: expression
  method: expression
  expression: "total_events_7d / distinct_sessions_7d"
  depends_on:
    - total_events_7d
    - distinct_sessions_7d
```

**Use Cases:**
- Bot detection (high session count, short duration)
- Automated script detection (abnormal events per session)
- Human behavior validation (session patterns)
- Session hijacking detection (sudden session characteristic changes)

> **Important**: Business system is responsible for computing `session_id` based on timeout rules (e.g., 30 minutes of inactivity = new session). This ensures consistent session definition across the platform.

---

### 📋 3.4 Time-Series Analysis
**Status:** Planned - Future

Advanced time-series analysis and forecasting.

**Proposed Operators:**
- `moving_average` - Moving average over window
- `lag` - Previous value at offset
- `forecast` - Time-series forecast
- `seasonality_score` - Seasonal pattern strength

**Use Cases:**
- Trend analysis
- Forecasting
- Smoothing
- Pattern detection
- Seasonal adjustments

**Proposed Syntax:**
```yaml
- name: movavg_userid_txn_amt_7d
  type: sequence
  method: moving_average
  datasource: clickhouse_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  window: 7d
  window_size: 7  # Number of periods for moving average
  aggregation: sum
```

---

### 📋 3.5 Complex Event Processing (CEP)
**Status:** Planned - Future

Stateful event pattern matching and correlation.

**Proposed Operators:**
- `event_pattern_match` - Match complex event patterns
- `state_machine` - Track state transitions
- `funnel_analysis` - Conversion funnel tracking
- `event_correlation` - Correlate related events

**Use Cases:**
- Fraud pattern detection
- User journey tracking
- Attack pattern recognition
- Complex rule evaluation
- Multi-stage fraud detection

**Proposed Syntax:**
```yaml
- name: pattern_userid_acct_takeover_1h
  type: sequence
  method: event_pattern_match
  datasource: clickhouse_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 1h
  pattern:
    - event_type: password_reset
      min_count: 1
    - event_type: login
      min_count: 3
      when: status == "failed"            # Database field (no prefix)
    - event_type: login
      min_count: 1
      when: status == "success"           # Database field (no prefix)
    - event_type: transaction
      min_count: 1
  sequence: ordered  # ordered, unordered, partial
```

---

## 4. Graph (Analyzing Relationships)

**Purpose:** Analyze connections, networks, and relationship patterns between entities using graph theory.

> **Note on Entity Linking:**
>
> Simple entity linking (e.g., "devices per IP", "users per device") should use **`distinct` aggregation**, not Graph operators:
>
> ```yaml
> # Count devices per IP - use distinct
> - name: distinct_ip_device_24h
>   type: aggregation
>   method: distinct
>   datasource: postgresql_events
>   entity: events
>   dimension: ip_address
>   dimension_value: "{event.ip_address}"
>   field: device_id
>   window: 24h
> ```
>
> Graph operators should focus on operations that **require graph algorithms** (network analysis, community detection, etc.).

### 📋 4.1 Network Analysis
**Status:** Planned - Low Priority (Complex)

Analyze entity relationships and network patterns using graph algorithms.

**Proposed Operators:**
- `graph_centrality` - Network centrality score
- `community_size` - Size of connected component
- `shared_entity_count` - Count shared connections
- `network_distance` - Distance between entities

**Use Cases:**
- Fraud ring detection
- Device sharing networks
- IP address clustering
- Account linking
- Synthetic identity detection
- Money mule networks

**Proposed Syntax:**
```yaml
- name: centrality_userid_device_30d
  type: graph
  method: graph_centrality
  datasource: neo4j_graph
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  dimension2: device_id
  window: 30d
```

---

## 5. Expression (Computing Scores)

**Purpose:** Compute custom scores, evaluate expressions, and integrate models.

> **⚠️ Architecture Constraint (Red Line):** Expression operators **must not** access raw data sources or define time windows; they **only consume results from other features**. This ensures clear separation of concerns and prevents architecture degradation.

### ✅ 5.1 Custom Expressions
**Status:** Implemented (Partial)

Evaluate custom expressions using other features.

**Operators:**
- `expression` - Evaluate custom expressions

**Use Cases:**
- Ratio calculations
- Composite scores
- Derived features
- Custom business logic

**Example:**
```yaml
- name: rate_userid_login_failure
  type: expression
  method: expression
  expression: "failed_login_count_1h / login_count_1h"
  depends_on:
    - failed_login_count_1h
    - login_count_1h
```

---

### 📋 5.2 Machine Learning Integration
**Status:** Planned - Future

Integration with ML models and embeddings.

**Proposed Operators:**
- `ml_model_score` - Call ML model for prediction
- `embedding_similarity` - Similarity using embeddings
- `clustering_label` - Assign to cluster
- `anomaly_detection_score` - ML-based anomaly score

**Use Cases:**
- ML model integration
- Advanced fraud detection
- User behavior modeling
- Risk scoring
- Recommendation features

**Proposed Syntax:**
```yaml
- name: mlscore_userid_fraud
  type: expression
  method: ml_model_score
  model: fraud_detection_v2
  inputs:
    - transaction_count_24h
    - transaction_sum_24h
    - unique_devices_7d
    - account_age_days
  output: fraud_probability
```

---

## 6. Lookup (Looking up Pre-computed Values)

**Purpose:** Retrieve pre-computed feature values from Redis cache.

> **⚠️ Architecture Principle:** Lookup features only retrieve pre-computed values; they do not perform any computation. All computation should happen in other feature categories or external batch jobs.

### ✅ 6.1 Direct Lookups
**Status:** Implemented

Retrieve pre-computed values from Redis by key.

**Configuration Fields:**
- `type: lookup` - Feature category
- `datasource` - Data source name (references `repository/configs/datasources/`)
- `key` - Key template for retrieval (supports variable interpolation)
- `fallback` - Optional default value if key not found

**Use Cases:**
- Pre-computed risk scores (from batch jobs)
- User segments (from analytics pipeline)
- Cached aggregations (from scheduled computations)
- Device fingerprints and reputation scores

**Example:**
```yaml
- name: user_risk_score_90d
  type: lookup
  datasource: redis_features
  key: "user_risk_score:{event.user_id}"
  fallback: 50

- name: user_segment_label
  type: lookup
  datasource: redis_features
  key: "user_segment:{event.user_id}"
  fallback: "unknown"

- name: device_reputation_score
  type: lookup
  datasource: redis_features
  key: "device_reputation:{event.device_id}"
  fallback: 0
```

### Implementation Details

**Redis Configuration:**
- Fast in-memory cache (sub-millisecond latency)
- Key-based retrieval with pattern matching
- TTL support for automatic expiration
- Fallback values for missing keys
- Connection pooling for high concurrency

**Why Lookup is a Separate Category:**

✅ **Semantic Clarity**: Explicitly indicates "no computation, just retrieval"
✅ **Configuration Simplicity**: No operator needed, simpler YAML
✅ **Performance Optimization**: Can use different caching strategies
✅ **Schema Validation**: Clear requirements (needs datasource, no operator)

---

## Implementation Roadmap

### Phase 1: Core Enhancements (Q1 2025)
**Focus:** Complete basic operators and add high-priority features

- ✅ Complete Expression operator implementation
- 📋 Implement Pattern sequences (3.1)
- 📋 Add more time window units (weeks, months)

### Phase 2: Advanced Analytics (Q2 2025)
**Focus:** Statistical and behavioral analysis

- 📋 Statistical aggregations (1.3)
- 📋 Trend detection (3.2)
- 📋 Time-of-day/week state (2.1)
- 💡 Session-based analysis (3.3) - Use Aggregation operators

### Phase 3: Complex Features (Q3 2025)
**Focus:** Advanced graph analysis and baselines

- 📋 Historical baseline state (2.2)
- 📋 Network analysis (4.1) - basic
- 📋 Time-series analysis (3.4) - basic

### Phase 4: Advanced/ML Integration (Q4 2025+)
**Focus:** AI and complex event processing

- 📋 Machine learning integration (5.2)
- 📋 Advanced network analysis algorithms (4.1)
- 📋 Complex event processing (3.5)
- 📋 Real-time streaming features

---

## By Risk Domain

| Risk Domain | Primary Categories | Key Operators |
|-------------|-------------------|---------------|
| **Account Security** | Aggregation, Sequence, Expression | count, consecutive_count, distinct(city), expression |
| **Transaction Fraud** | Aggregation, Sequence, Expression | sum, avg, stddev, trend, z_score, expression |
| **Bot Detection** | Aggregation, Sequence, Expression | distinct(session_id), avg(session_duration), expression, count |
| **Account Takeover** | State, Sequence, Aggregation | consecutive_count, deviation_from_baseline, z_score, distinct(city) |
| **Payment Fraud** | Aggregation, Sequence, Expression | count, sum, pattern_frequency, expression |
| **Synthetic Identity** | Graph, Aggregation | centrality, shared_entity_count, distinct |
| **Credit Risk** | Aggregation, State, Sequence | sum, avg, stddev, trend, baseline_deviation |
| **AML/Compliance** | Aggregation, Graph, Sequence | sum, distinct, centrality, pattern_match |

---

## Feature Category Summary

### By Implementation Status

| Category | Sub-Category | Status | Priority | Complexity |
|----------|--------------|--------|----------|------------|
| **Aggregation** | Basic Counting (1.1) | ✅ Implemented | - | Low |
| | Basic Aggregations (1.2) | ✅ Implemented | - | Low |
| | Statistical Aggregations (1.3) | 📋 Planned | Medium | Medium |
| **State** | Time-of-Day/Week (2.1) | 📋 Planned | Medium | Low |
| | Historical Baseline (2.2) | 📋 Planned | Low | Medium |
| **Sequence** | Pattern Sequences (3.1) | 📋 Planned | High | Medium |
| | Trend Detection (3.2) | 📋 Planned | Medium | Medium |
| | Session-Based Analysis (3.3) | 💡 Use Aggregation | - | - |
| | Time-Series Analysis (3.4) | 📋 Planned | Future | High |
| | Complex Event Processing (3.5) | 📋 Planned | Future | Very High |
| **Graph** | Network Analysis (4.1) | 📋 Planned | Low | High |
| **Expression** | Custom Expressions (5.1) | ✅ Partial | - | Medium |
| | Machine Learning (5.2) | 📋 Planned | Future | High |
| **Lookup** | Direct Lookups (6.1) | ✅ Implemented | - | Low |

---

## Feature Naming Convention

Feature names should follow a structured pattern for clarity and machine parseability.

### Naming Pattern

**For features with operators** (Aggregation, State, Sequence, Graph, Expression):
```
<operator>_<dimension>_<event>[_field]_<window>[_modifier]
```

**For Lookup features** (no operator):
```
<descriptive_name>
```

**Components (in order):**

1. **Operator** (required for computed features) - Operation type
   - Aggregation: `cnt`, `sum`, `avg`, `max`, `min`, `distinct`, `stddev`, `variance`, `percentile`, `median`, `mode`, `entropy`
   - State: `zscore`, `deviation`, `percentile`, `outlier`, `timezone`
   - Sequence: `consec`, `trend`, `pctchg`, `streak`
   - Graph: `centrality`, `community`, `shared`
   - Expression: `rate`, `ratio`, `score`

2. **Dimension** (required) - Aggregation dimension
   - `userid`, `deviceid`, `ip`, `acctid`, `sessid`, `email`

3. **Event** (required) - Event or entity type
   - `login`, `txn`, `pay`, `reg`, `checkout`, `pwd_reset`

4. **Field** (optional) - Field name for aggregations
   - `amt`, `val`, `score`, `dur`, `cnt`

5. **Window** (required for time-based) - Time window
   - `1h`, `24h`, `7d`, `30d`, `90d`

6. **Modifier** (optional) - Additional qualifier
   - `failed`, `success`, `new`, `change`

### Standard Abbreviations

To keep feature names concise, use these standard abbreviations:

| Full Word | Abbreviation | Usage |
|-----------|--------------|-------|
| **Events** |
| transaction | `txn` | `cnt_userid_txn_24h` |
| payment | `pay` | `sum_userid_pay_amt_7d` |
| register | `reg` | N/A (use event.account_age_days) |
| session | `sess` | `avg_userid_sess_dur_7d` |
| password | `pwd` | `consec_userid_pwd_reset_1h` |
| checkout | `checkout` | `cnt_userid_checkout_24h` |
| **Dimensions** |
| account | `acct` | `cnt_acctid_login_24h` |
| device | `dev` | Optional: `deviceid` or `devid` |
| session | `sess` | `cnt_sessid_event_1h` |
| **Fields** |
| amount | `amt` | `sum_userid_txn_amt_30d` |
| value | `val` | `max_userid_pay_val_1h` |
| count | `cnt` | Used in operator position |
| duration | `dur` | `avg_userid_sess_dur_7d` |
| distance | `dist` | `centrality_userid_device_30d` |
| **Operators** (already short) |
| count | `cnt` | `cnt_*` |
| average | `avg` | `avg_*` |
| distinct | `distinct` | `distinct_*` |
| consecutive | `consec` | `consec_*` |
| percent | `pct` | `pctchg_*` |
| **Modifiers** |
| failed | `failed` | `*_failed` |
| success | `success` | `*_success` |

**Guidelines:**
- **Abbreviations are ONLY used in the `name` field** - all other fields use full words
- Use abbreviations for words longer than 6 characters
- Keep common short words unchanged: `login`, `ip`, `sum`, `max`, `min`
- Be consistent: always use the same abbreviation for the same word

**Important:** Configuration fields use full words, not abbreviations:
```yaml
# ✅ Correct - abbreviation only in name
- name: sum_userid_txn_amt_24h     # name uses abbreviation
  field: amount                    # field uses full word
  when: type == "transaction"      # when uses full word (database field, no prefix needed)

# ❌ Wrong - don't use abbreviations in config
- name: sum_userid_txn_amt_24h
  field: amt                       # ❌ Wrong (don't use abbreviation)
  when: type == "txn"              # ❌ Wrong (don't use abbreviation)
```

### Examples

**Aggregation Features:**

```yaml
# Basic counting
cnt_userid_login_24h               # User login count in 24 hours
cnt_userid_txn_7d                  # User transaction count in 7 days
cnt_deviceid_login_1h              # Device login count in 1 hour

# Sum/Avg with field
sum_userid_txn_amt_30d             # User total transaction amount in 30 days
avg_userid_pay_amt_7d              # User average payment amount in 7 days
max_userid_txn_amt_24h             # User maximum transaction amount in 24 hours

# Distinct counting
distinct_userid_device_7d          # User distinct device count in 7 days
distinct_userid_ip_24h             # User distinct IP count in 24 hours
distinct_ip_userid_1h              # IP distinct user count in 1 hour

# With modifier for conditions
cnt_userid_login_1h_failed         # User failed login count in 1 hour
cnt_userid_pay_24h_success         # User successful payment count in 24 hours
```

**State Features:**

```yaml
# Statistical comparison (planned)
zscore_userid_txn_amt              # User transaction amount Z-score
deviation_userid_login_freq        # User login frequency deviation
percentile_userid_txn_amt          # User transaction amount percentile
timezone_userid_login_7d           # User timezone consistency check

# Note: Use Lookup features for pre-computed values:
- name: user_risk_score_90d
  type: lookup
  datasource: redis_features
  key: "user_risk_score:{event.user_id}"
  fallback: 50

# Time-based checks should use Expression or Aggregation:
# - Off-hours check: is_off_hours (expression: "event.hour < 8 || event.hour > 22")
# - Off-hours count: cnt_userid_login_7d_offhours (aggregation with when condition)

# Context data should be provided by business system:
# - event.account_age_days, user.last_login_at (temporal)
# - user.kyc_status, user.account_type, user.country (profile)
```

**Sequence Features:**

```yaml
# Pattern sequences
consec_userid_login_1h_failed      # User consecutive failed login count in 1 hour
streak_userid_txn_7d               # User transaction streak in 7 days

# Trend detection
pctchg_userid_txn_amt              # User transaction amount percentage change
trend_userid_login_7d              # User login trend in 7 days

# Session analysis
avg_userid_sess_dur_7d             # User average session duration in 7 days
```

**Graph Features:**

```yaml
# Entity linking - use distinct (not Graph operators)
distinct_ip_device_24h             # IP associated device count in 24 hours (use distinct)
distinct_deviceid_userid_7d        # Device associated user count in 7 days (use distinct)

# Network analysis (planned)
centrality_userid_device_30d       # User device network centrality in 30 days
community_userid_network_30d       # User community size in network in 30 days
shared_userid_device_30d           # Shared device count between users
```

**Expression Features:**

```yaml
# Computed scores
score_userid_fraud                     # User fraud score
score_userid_risk                      # User risk score

# Ratio/Rate (complex expressions)
rate_userid_login_1h_failure           # User login failure rate in 1 hour
ratio_userid_txn_7d_change             # User transaction ratio change in 7 days
```

**Lookup Features:**

```yaml
# Pre-computed values (no operator, descriptive names)
user_risk_score_90d                    # User 90-day risk score (pre-computed)
user_segment                           # User segmentation label
device_reputation_score                # Device reputation score
ip_risk_level                          # IP risk level

# Note: Lookup features don't follow the operator pattern
# Use descriptive names that indicate what is being looked up
```

**Avoid:**

```yaml
# ❌ Wrong order
userid_cnt_login_24h                   # Operator should be first
24h_login_cnt_userid                   # Wrong order

# ❌ Inconsistent abbreviations
count_userid_login_24h                 # Use 'cnt' not 'count'
cnt_user_id_login_24h                  # Use 'userid' not 'user_id'

# ❌ Too vague
cnt_24h                                # Missing dimension and event
zscore_1h                              # Missing dimension and event

# ❌ Adding type prefix (not needed)
agg_cnt_userid_login_24h               # Don't add 'agg_' prefix
zscore_userid_txn_amt            # Don't add 'state_' prefix
```

---

## Time Window Units

Corint uses concise time unit notation:

| Unit | Meaning | Type | Example |
|------|---------|------|---------|
| `s` | second | Physical | `login_count_30s` |
| `m` | minute | Physical | `login_count_5m` |
| `h` | hour | Physical | `login_count_1h`, `transaction_sum_24h` |
| `d` | day (24h) | Physical | `unique_devices_7d`, `transaction_sum_30d` |
| `mo` | month (calendar) | Business | `avg_txn_3mo` |
| `q` | quarter (calendar) | Business | `revenue_sum_1q` |
| `y` | year (calendar) | Business | `annual_txn_1y` |

---

## Design Principles

### 1. Category-Based Design
Each feature belongs to one primary category based on its purpose:
- **Aggregation** for counting and statistical calculations
- **State** for current/recent status checks
- **Sequence** for temporal patterns and trends
- **Graph** for entity connections and network analysis
- **Expression** for computed scores

### 2. Composability
Features from different categories can be combined:
- Aggregation results feed into Expression
- State checks can trigger Sequence analysis
- Graph features enhance Aggregation context

### 3. Performance
All operators should support:
- Time-windowed queries
- Efficient caching
- Incremental computation where possible
- Batch processing for bulk evaluation

### 4. Flexibility
- Parameterized dimensions
- Dynamic conditions (when clauses)
- Multiple data sources
- Template-based values

### 5. Observability
- Feature computation metrics
- Cache hit rates
- Query performance tracking
- Data quality monitoring

---

## Contributing

When adding new operators:

1. **Identify the category** - Which of the 5 categories does it belong to?
2. **Define the use case** - What risk scenarios does it address?
3. **Design the operator** - What parameters are needed?
4. **Implement efficiently** - Consider performance and scale
5. **Add tests** - Unit tests and integration tests
6. **Document examples** - Provide real-world examples
7. **Update this document** - Keep the feature catalog current

---

## References

- [Operator Implementation](crates/corint-runtime/src/feature/operator.rs)
- [Feature Definitions](repository/configs/features/)
- [Data Source Integration](crates/corint-runtime/src/datasource/)
- [Risk Rule Examples](repository/library/rules/)
