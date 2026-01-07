# CORINT Feature DSL Reference (v0.2)

Quick reference for writing feature definitions in CORINT. For detailed implementation details and use cases, see `FEATURE_ENGINEERING.md`.

---

## Implementation Status

| Feature Type | Status | Implemented Methods | Planned Methods |
|--------------|--------|---------------------|-----------------|
| **Aggregation** | 🟢 **Implemented** | count, sum, avg, min, max, distinct, stddev, median, percentile | variance, mode, entropy |
| **State** | 🔴 **Planned** | - | z_score, deviation_from_baseline, percentile_rank, is_outlier, timezone_consistency |
| **Sequence** | 🔴 **Planned** | - | consecutive_count, sequence_match, percent_change, streak, pattern_frequency, trend, rate_of_change, anomaly_score, moving_average |
| **Graph** | 🔴 **Planned** | - | graph_centrality, community_size, shared_entity_count, network_distance |
| **Expression** | 🟢 **Implemented** | expression | - |
| **Lookup** | 🟢 **Implemented** | lookup | - |

**Legend:**
- 🟢 **Implemented**: Ready for production use
- 🟡 **Partial**: Some methods implemented, others in development
- 🔴 **Planned**: Documented but not yet implemented

**Note:** SQL generation support exists for some advanced statistics (percentile, stddev, median), but full feature orchestration is still in development.

---

## 1. Overview

### 1.1 Feature Types

| Type | Status | Purpose |
|------|--------|---------|
| **Aggregation** | 🟢 | Count and aggregate events/values (count, sum, avg, max, min, distinct) |
| **State** | 🔴 | Statistical comparisons (z-score, deviation, percentile) |
| **Sequence** | 🔴 | Pattern and trend analysis (consecutive, streak, percent_change) |
| **Graph** | 🔴 | Network and relationship analysis (centrality, community_size, shared_entity) |
| **Expression** | 🟢 | Compute from other features (rate, ratio, ML models) |
| **Lookup** | 🟢 | Retrieve pre-computed values (Redis cache) |

### 1.2 Basic Structure

```yaml
- name: feature_name              # Feature identifier
  description: "Feature description"  # Human-readable description
  type: feature_type              # aggregation|state|sequence|graph|expression|lookup
  method: method_name             # Specific method (count, sum, z_score, etc.)
  datasource: datasource_name     # Logical datasource name (events_datasource or lookup_datasource)
                                  # Actual datasources are defined in config/server.yaml
  entity: entity_name             # Table/entity name (for SQL/NoSQL)
  dimension: dimension_field      # Grouping dimension (e.g., user_id)
  dimension_value: "${event.user_id}"  # Template for dimension value
  field: field_name               # Field to aggregate (optional for count)
  window: time_window             # Time window (1h, 24h, 7d, 30d, 90d)
  when:                           # Optional filter conditions
    all:
      - condition1
      - condition2
```

### 1.3 Field Reference Syntax in `when` Conditions

> **Important:** The syntax differs between **Feature definitions** and **Rules/Pipelines**

#### In Feature Definitions (for database filtering)

When filtering database rows using the `when` field in feature definitions, you reference:

**1. Database Fields (from the entity table being queried)**
- No prefix needed - directly reference the column name
- Examples: `type`, `status`, `amount`, `country`
- Supports JSON nested fields: `attributes.device.fingerprint`, `metadata.user.tier`

**2. Request Fields (from the incoming API request via context.event)**
- Use template syntax with curly braces: `${event.field_name}`
- Examples: `${event.user_id}`, `${event.min_amount}`, `${event.threshold}`
- Used for dynamic filtering and template substitution

#### In Rules/Pipelines (for runtime conditions)

When defining conditions in rules and pipelines, use:

**1. Event Fields**
- Use `event.` prefix: `event.type`, `event.amount`, `event.user_id`
- Example: `event.type == "transaction"`

**2. Feature Values**
- Use `features.` prefix: `features.txn_count_24h`, `features.risk_score`
- Example: `features.txn_count_24h > 10`

**Examples (Feature Definitions):**
```yaml
# Feature definition - Database field filtering (no prefix)
- name: transaction_count_24h
  type: aggregation
  method: count
  when: type == "transaction"              # Database field

# Feature definition - Database field with JSON nested access
- name: high_risk_events
  type: aggregation
  method: count
  when: attributes.risk_level == "high"    # Database JSON field

# Feature definition - Combining database and request fields
- name: filtered_transactions
  type: aggregation
  method: count
  when:
    all:
      - type == "payment"                      # Database field
      - amount > ${event.threshold}             # Request value (dynamic)
      - metadata.country == "${event.country}"  # Database JSON field matches request

# Feature definition - Complex nested JSON field
- name: verified_users
  type: aggregation
  method: count
  when: user_profile.verification_status == "verified"
```

**SQL Generation Example (Feature Definition):**
```yaml
# Feature definition
- name: mobile_transactions_above_threshold
  type: aggregation
  method: count
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  window: 24h
  when:
    all:
      - type == "transaction"                # Database field
      - amount > ${event.min_amount}          # Request value (template)
      - attributes.device_type == "mobile"   # Database JSON field
```

**Generated SQL:**
```sql
SELECT COUNT(*)
FROM events
WHERE user_id = $1                                   -- From dimension_value template
  AND event_timestamp >= NOW() - INTERVAL '24 hours'
  AND type = 'transaction'                           -- Database field
  AND amount > $2                                     -- Request value substituted
  AND attributes->>'device_type' = 'mobile'          -- JSON field access
```

### 1.4 Feature Access in Rules

**In rules and pipelines, use prefixes for field access:**
```yaml
# Rule definition (NOT feature definition)
rule:
  id: high_value_transaction
  when:
    all:
      - event.type == "transaction"              # Event fields (event. prefix)
      - event.amount > 1000                      # Event fields (event. prefix)
      - features.transaction_sum_7d > 5000       # Feature values (features. prefix)
  score: 80
```
---

## 2. Aggregation 🟢 Implemented

**Implementation Status:** ✅ Core operators and most statistics production-ready | 📋 Some advanced statistics (variance, mode, entropy) in development

### 2.1 Field Semantics

| Field | Purpose | SQL Equivalent |
|------|---------|----------------|
| `dimension` | Grouping dimension | `GROUP BY` |
| `field` | Field to compute on | `SUM(field)`, `AVG(field)` |
| `when` | Filter condition | `WHERE` |

**Field requirement:**
- `count` - ❌ No field needed
- `sum`, `avg`, `max`, `min`, `distinct`, `stddev`, `median`, `percentile` - ✅ Field required
- `variance`, `mode`, `entropy` - 📋 Planned (not yet implemented)

### 2.2 Implemented Methods

**✅ count** - Count events
```yaml
- name: cnt_userid_login_1h_failed
  description: "Number of failed login attempts in last 1 hour"
  type: aggregation
  method: count
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"   # Request field (from context.event)
  window: 1h
  when:
    all:
      - type == "login"                # Database field (no prefix)
      - status == "failed"             # Database field (no prefix)
```

**✅ sum** - Sum values
```yaml
- name: sum_userid_txn_amt_24h
  description: "Total transaction amount in last 24 hours"
  type: aggregation
  method: sum
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"  # Request field (from context.event)
  field: amount
  window: 24h
  when: type == "transaction"         # Database field (no prefix)
```

**✅ avg** - Average values
```yaml
- name: avg_userid_order_amt_30d
  description: "Average order amount in last 30 days"
  type: aggregation
  method: avg
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"  # Request field (from context.event)
  field: amount
  window: 30d
  when: type == "order"               # Database field (no prefix)
```

**✅ max / min** - Maximum / Minimum
```yaml
- name: max_userid_txn_amt_90d
  description: "Maximum transaction amount in last 90 days"
  type: aggregation
  method: max
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"  # Request field (from context.event)
  field: amount
  window: 90d
  when: type == "transaction"         # Database field (no prefix)
```

**✅ distinct** - Count unique values
```yaml
- name: distinct_userid_device_24h
  description: "Number of unique devices used in last 24 hours"
  type: aggregation
  method: distinct
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"  # Request field (from context.event)
  field: device_id
  window: 24h
```

**✅ stddev** - Standard deviation
```yaml
- name: stddev_userid_txn_amt_30d
  description: "Standard deviation of transaction amounts (30 days)"
  type: aggregation
  method: stddev
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  field: amount
  window: 30d
  when: type == "transaction"         # Database field (no prefix)
```

> **Note:** SQL generation varies by database provider:
> - PostgreSQL/MySQL: `STDDEV_POP(field)`
> - SQLite: `STDEV(field)`
> - ClickHouse: `stddevPop(field)`

**✅ median** - Median value
```yaml
- name: median_userid_txn_amt_30d
  description: "Median transaction amount (30 days)"
  type: aggregation
  method: median
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  field: amount
  window: 30d
  when: type == "transaction"         # Database field (no prefix)
```

> **Note:** SQL generation varies by database provider:
> - PostgreSQL/MySQL: `PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY field)`
> - SQLite: Uses subquery workaround
> - ClickHouse: `median(field)`

**✅ percentile** - Nth percentile
```yaml
- name: p95_userid_txn_amt_30d
  description: "95th percentile of transaction amounts (30 days)"
  type: aggregation
  method: percentile
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  field: amount
  percentile: 95
  window: 30d
  when: type == "transaction"         # Database field (no prefix)
```

> **Note:** SQL generation varies by database provider:
> - PostgreSQL/MySQL: `PERCENTILE_CONT(p/100.0) WITHIN GROUP (ORDER BY field)`
> - SQLite: Uses subquery workaround with LIMIT/OFFSET
> - ClickHouse: `quantile(p/100.0)(field)`

### 2.3 Planned Methods

**📋 variance** - Variance (not yet implemented)
```yaml
# ⚠️ Not yet implemented - use stddev as workaround
- name: variance_userid_txn_amt_30d
  type: aggregation
  method: variance
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  field: amount
  window: 30d
  when: type == "transaction"         # Database field (no prefix)
```

**📋 mode / entropy** - Most frequent value / Shannon entropy (not yet implemented)


---

## 3. State 🔴 Planned

**Implementation Status:** 🔴 Not yet implemented - all operators are in development roadmap

> ⚠️ **Note**: The features documented in this section are planned for future releases and are not currently available in production.

**📋 z_score** - Statistical z-score
```yaml
- name: zscore_userid_txn_amt
  type: state
  method: z_score
  datasource: events_datasource
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  field: amount
  current_value: "${event.amount}"
  window: 90d
  when: type == "transaction"         # Database field (no prefix)
```

**📋 deviation_from_baseline** - Compare to historical average

**📋 percentile_rank** - Rank compared to history

**📋 is_outlier** - Statistical outlier detection

**📋 timezone_consistency** - Timezone pattern check
```yaml
- name: timezone_consistency_userid_7d
  type: state
  method: timezone_consistency
  datasource: events_datasource
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  window: 7d
  expected_timezone: "${event.user.timezone}"
```

> **Note:** Simple time checks (off-hours) should use Expression operators.

---

## 4. Sequence 🔴 Planned

**Implementation Status:** 🔴 Not yet implemented - all operators are in development roadmap

> ⚠️ **Note**: The features documented in this section are planned for future releases and are not currently available in production.

**📋 consecutive_count** - Count consecutive occurrences
```yaml
- name: consec_userid_login_1h_failed
  type: sequence
  method: consecutive_count
  datasource: events_datasource
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  window: 1h
  when:
    all:
      - type == "login"                # Database field (no prefix)
      - status == "failed"             # Database field (no prefix)
  reset_when: status == "success"      # Database field (no prefix)
  order_by: timestamp
```

**📋 sequence_match** - Match event sequences
```yaml
- name: seq_userid_account_takeover_pattern
  type: sequence
  method: sequence_match
  datasource: events_datasource
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  window: 1h
  pattern:
    - type == "password_reset"                     # Database field
    - type == "email_change"                       # Database field
    - type == "transaction" AND amount > 10000     # Database fields
  order_by: timestamp
```

**📋 percent_change** - Percentage change between windows
```yaml
- name: pctchg_userid_txn_cnt_week
  type: sequence
  method: percent_change
  datasource: events_datasource
  entity: events
  dimension: user_id
  dimension_value: "${event.user_id}"
  window: 7d
  when: type == "transaction"         # Database field (no prefix)
  aggregation: count
```

**Calculation:** (current_window - baseline_window) / baseline_window × 100%
- Current window: [now - 7d, now]
- Baseline window: [now - 14d, now - 7d]

**📋 Other Sequence Operators:**
- `streak` - Longest streak of condition
- `pattern_frequency` - Frequency of patterns
- `trend` - Trend direction (increasing/decreasing/stable)
- `rate_of_change` - Rate of change over time
- `anomaly_score` - Statistical anomaly detection
- `moving_average` - Moving average

**Session-based analysis:** Use Aggregation with `session_id`
```yaml
# Session count
- name: distinct_userid_session_24h
  type: aggregation
  method: distinct
  dimension: user_id
  dimension_value: "${event.user_id}"
  field: session_id
  window: 24h

# Events per session
- name: events_per_session_7d
  type: expression
  expression: "total_events_7d / distinct_sessions_7d"
```

---

## 5. Graph 🔴 Planned

**Implementation Status:** 🔴 Not yet implemented - all operators are in development roadmap

> ⚠️ **Note**: The features documented in this section are planned for future releases and are not currently available in production.

> **Recommendation:** For simple entity linking (devices per IP), use `distinct` aggregation which is already implemented.

### 5.1 Field Semantics

**Single-node methods** (centrality, community_size):
- `dimension` - Primary entity type (e.g., user_id)
- `dimension_value` - The node to analyze
- `dimension2` - Associated entity type (e.g., device_id)

**Two-node methods** (shared_entity_count, network_distance):
- `dimension` - Primary entity type
- `dimension_value` - Source node
- `dimension_value2` - Target node (same type as source)
- `dimension2` - Connection type (intermediate entity)

### 5.2 Operators

**📋 graph_centrality** - Network centrality score (🔴 Planned)
```yaml
# ⚠️ Not yet implemented - Graph features are in development
- name: centrality_device_in_user_network
  type: graph
  method: graph_centrality
  datasource: neo4j_graph  # Neo4j support planned
  dimension: device_id
  dimension_value: "${event.device_id}"
  dimension2: user_id
  window: 30d
```

**📋 community_size** - Size of connected component

**📋 shared_entity_count** - Count shared connections (🔴 Planned)
```yaml
# ⚠️ Not yet implemented - Graph features are in development
- name: shared_devices_between_users
  type: graph
  method: shared_entity_count
  datasource: neo4j_graph  # Neo4j support planned
  dimension: user_id
  dimension_value: "${event.user_id}"
  dimension_value2: "${event.target_user_id}"
  dimension2: device_id
  window: 30d
```

**📋 network_distance** - Distance between entities (🔴 Planned)
```yaml
# ⚠️ Not yet implemented - Graph features are in development
- name: network_dist_to_fraud_account
  type: graph
  method: network_distance
  datasource: neo4j_graph  # Neo4j support planned
  dimension: user_id
  dimension_value: "${event.user_id}"
  dimension_value2: "${known_fraud_user_id}"
  dimension2: device_id
  window: 90d
```

---

## 6. Expression 🟢 Implemented

**Implementation Status:** ✅ Production-ready | 📋 ML model integration planned

> **⚠️ Architecture Constraint:** Expression methods **only consume results from other features**. They do not access raw data sources or define time windows.

**✅ expression** - Custom expressions

> **Note:** The `method` field is **optional** for expression type since it's always "expression". Omitting it makes the configuration more concise.

> **Note:** The `depends_on` field is **no longer required**. Feature dependencies are automatically extracted from the expression string.

```yaml
- name: rate_userid_login_failure
  description: "Login failure rate (failed/total logins)"
  type: expression
  # method: expression  # Optional - can be omitted for expression type
  expression: "failed_login_count_1h / login_count_1h"
  # Dependencies are automatically extracted from the expression
```

---

## 7. Lookup 🟢 Implemented

**Implementation Status:** ✅ Production-ready

> **⚠️ Architecture Principle:** Lookup features only retrieve pre-computed values; they do not perform computation.

```yaml
- name: user_risk_score_90d
  description: "Pre-computed user risk score (90-day window)"
  type: lookup
  datasource: lookup_datasource
  key: "user_risk_score:${event.user_id}"
  fallback: 50

- name: device_reputation_score
  description: "Device reputation score from feature store"
  type: lookup
  datasource: lookup_datasource
  key: "device_reputation:${event.device_id}"
  fallback: 0
```

---

## 8. Data Source Configuration

### 8.1 Data Source Types

| Type | Status | Purpose | Used By |
|------|--------|---------|---------|
| `postgresql` | 🟢 **Implemented** | Transactional/event data | Aggregation, Expression |
| `clickhouse` | 🟢 **Implemented** | High-volume event storage | Aggregation |
| `redis` | 🟢 **Implemented** | Pre-computed features | Lookup |
| `mysql` | 🟢 **Implemented** | Transactional/event data | Aggregation |
| `sqlite` | 🟢 **Implemented** | Embedded/testing | Aggregation |
| `neo4j` | 🔴 **Planned** | Graph/relationship data | Graph (when implemented) |

### 8.2 Configuration Files

**Datasources are defined in `config/server.yaml`:**

```yaml
datasource:
  # Events datasource (for aggregation features)
  postgres_events:
    type: sql
    provider: postgresql
    connection_string: "postgresql://user:password@localhost:5432/corint_risk"
    database: "corint_risk"
    events_table: "events"
    options:
      max_connections: "20"
      connection_timeout: "30"

  # Lookup datasource (for feature lookups)
  redis_features:
    type: feature_store
    provider: redis
    connection_string: "redis://localhost:6379/0"
    options:
      namespace: "user_features"
      default_ttl: "86400"
```

**Note:** Features use logical datasource names (`events_datasource`, `lookup_datasource`) which are automatically mapped to actual datasources defined in `config/server.yaml`.

**Planned datasource (not yet implemented):** Neo4j graph database support
```yaml
# ⚠️ WARNING: Neo4j support is planned but not yet implemented
# This configuration is for future reference only
name: neo4j_graph
type: neo4j
config:
  uri: ${NEO4J_URI}
  user: ${NEO4J_USER}
  password: ${NEO4J_PASSWORD}
  database: ${NEO4J_DATABASE}
  max_connection_lifetime: 3600
  max_connection_pool_size: 50
  connection_timeout: 30
```

### 8.3 Feature Category Requirements

| Feature Type | Needs `datasource`? | Needs `method`? |
|--------------|---------------------|-----------------|
| Aggregation | ✅ Yes | ✅ Yes |
| State | ✅ Yes | ✅ Yes |
| Sequence | ✅ Yes | ✅ Yes |
| Graph | ✅ Yes | ✅ Yes |
| Expression | ❌ No | ⚠️ Optional* |
| Lookup | ✅ Yes | ❌ No |

\* Expression `method` defaults to "expression" if omitted

---

## 9. Naming Convention

### 9.1 Pattern

**Computed features:**
```
<method>_<dimension>_<event>[_field]_<window>[_modifier]
```

**Lookup features:**
```
<descriptive_name>
```

### 9.2 Method Abbreviations

| Category | Abbreviations |
|----------|---------------|
| Aggregation | `cnt`, `sum`, `avg`, `max`, `min`, `distinct`, `stddev`, `p95` (percentile), `median` |
| State | `zscore`, `deviation`, `percentile`, `outlier`, `timezone` |
| Sequence | `consec`, `trend`, `pctchg`, `streak` |
| Graph | `centrality`, `community`, `shared` |
| Expression | `rate`, `ratio`, `score` |

### 9.3 Examples

```yaml
# Aggregation
cnt_userid_login_24h               # Count
sum_userid_txn_amt_30d             # Sum
avg_userid_pay_amt_7d              # Average
distinct_userid_device_7d          # Distinct
cnt_userid_login_1h_failed         # With modifier

# State
zscore_userid_txn_amt              # Z-score

# Sequence
consec_userid_login_1h_failed      # Consecutive
pctchg_userid_txn_amt              # Percent change

# Graph
centrality_userid_device_30d       # Centrality

# Expression
rate_userid_login_1h_failure       # Rate
score_userid_fraud                 # Score

# Lookup
user_risk_score_90d                # Pre-computed
device_reputation_score            # Pre-computed
```

---

## 10. Time Window Units

| Unit | Meaning | Example |
|------|---------|---------|
| `s` | second | `login_count_30s` |
| `m` | minute | `login_count_5m` |
| `h` | hour | `login_count_1h`, `transaction_sum_24h` |
| `d` | day (24h) | `unique_devices_7d`, `transaction_sum_30d` |
| `mo` | month (calendar) | `avg_txn_3mo` |
| `q` | quarter (calendar) | `revenue_sum_1q` |
| `y` | year (calendar) | `annual_txn_1y` |

---

## 11. Complete Example

```yaml
version: "0.1"

features:
  # Aggregation
  - name: cnt_userid_login_24h
    description: "Total login count in last 24 hours"
    type: aggregation
    method: count
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "${event.user_id}"
    window: 24h
    when: type == "login"

  - name: cnt_userid_login_1h_failed
    description: "Failed login count in last 1 hour"
    type: aggregation
    method: count
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "${event.user_id}"
    window: 1h
    when:
      all:
        - type == "login"
        - status == "failed"

  - name: distinct_userid_device_24h
    description: "Number of distinct devices in last 24 hours"
    type: aggregation
    method: distinct
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "${event.user_id}"
    field: device_id
    window: 24h

  # Expression
  - name: rate_userid_login_failure
    description: "Login failure rate calculation"
    type: expression
    expression: "cnt_userid_login_1h_failed / (cnt_userid_login_24h + 0.0001)"
    # Dependencies are automatically extracted from the expression

  # Lookup
  - name: user_risk_score_90d
    description: "Pre-computed user risk score (90-day)"
    type: lookup
    datasource: lookup_datasource
    key: "user_risk_score:${event.user_id}"
    fallback: 50

# Usage in rules
rule:
  id: high_risk_pattern
  when:
    all:
      - event.type == "login"
      - features.cnt_userid_login_1h_failed > 5
      - features.rate_userid_login_failure > 0.5
      - features.distinct_userid_device_24h > 3
      - features.user_risk_score_90d > 70
  score: 90
```

---

## 12. Quick Reference Tables

### 12.1 Field Usage by Type

| Field | Aggregation | State | Sequence | Graph | Expression | Lookup |
|-------|-------------|-------|----------|-------|------------|--------|
| `name` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `description` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `type` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `method` | ✅ | ✅ | ✅ | ✅ | ⚠️ | ❌ |
| `datasource` | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| `entity` | ✅ | ✅ | ✅ | ⚠️ | ❌ | ❌ |
| `dimension` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `dimension_value` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `field` | ⚠️ | ✅ | ⚠️ | ❌ | ❌ | ❌ |
| `window` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `when` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `expression` | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| `key` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| `fallback` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |

> **Note:** ⚠️ = Optional. For Expression, `method` defaults to "expression" if omitted.

### 12.2 Recommended Time Windows

| Scenario | Window | Reason |
|----------|--------|--------|
| Real-time fraud | 5m, 1h | Fast response |
| Login risk | 5h, 24h | Balance performance/accuracy |
| Transaction risk | 24h, 7d | Short-term patterns |
| User profiling | 30d, 90d | Stable long-term features |
| Device association | 5h, 24h | Quick changes |

---

## 13. Related Documentation

- `FEATURE_ENGINEERING.md` - Detailed implementation guide and use cases
- `expression.md` - Expression language reference
- `context.md` - Context and variable management
- `rule.md` - Rule definitions
- `pipeline.md` - Feature extraction in pipelines
