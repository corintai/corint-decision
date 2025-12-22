# CORINT Feature DSL Reference (v0.2)

Quick reference for writing feature definitions in CORINT. For detailed implementation details and use cases, see `FEATURE_ENGINEERING.md`.

---

## 1. Overview

### 1.1 Feature Types

| Type | Purpose |
|------|---------|
| **Aggregation** | Count and aggregate events/values (count, sum, avg, max, min, distinct) |
| **State** | Statistical comparisons (z-score, deviation, percentile) |
| **Sequence** | Pattern and trend analysis (consecutive, streak, percent_change) |
| **Graph** | Network and relationship analysis (centrality, community_size, shared_entity) |
| **Expression** | Compute from other features (rate, ratio, ML models) |
| **Lookup** | Retrieve pre-computed values (Redis cache) |

### 1.2 Basic Structure

```yaml
- name: feature_name              # Feature identifier
  type: feature_type              # aggregation|state|sequence|graph|expression|lookup
  method: operator_method         # Specific operator (count, sum, z_score, etc.)
  datasource: datasource_name     # References repository/configs/datasources/
  entity: entity_name             # Table/entity name (for SQL/NoSQL)
  dimension: dimension_field      # Grouping dimension (e.g., user_id)
  dimension_value: "{event.user_id}"  # Template for dimension value
  field: field_name               # Field to aggregate (optional for count)
  window: time_window             # Time window (1h, 24h, 7d, 30d, 90d)
  when:                           # Optional filter conditions
    all:
      - condition1
      - condition2
```

### 1.3 Feature Access

**Direct feature access:**
```yaml
rule:
  when:
    all:
      - features.transaction_sum_7d > 5000      # Registered features
      - event.amount > 1000                      # Event fields
```
---

## 2. Aggregation

**Status:** ✅ Basic operators implemented, 📋 Advanced planned

### 2.1 Field Semantics

| Field | Purpose | SQL Equivalent |
|------|---------|----------------|
| `dimension` | Grouping dimension | `GROUP BY` |
| `field` | Field to compute on | `SUM(field)`, `AVG(field)` |
| `when` | Filter condition | `WHERE` |

**Field requirement:**
- `count` - ❌ No field needed
- `sum`, `avg`, `max`, `min`, `distinct`, `stddev` - ✅ Field required

### 2.2 Implemented Operators

**✅ count** - Count events
```yaml
- name: cnt_userid_login_1h_failed
  type: aggregation
  method: count
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 1h
  when:
    all:
      - event.type == "login"
      - event.status == "failed"
```

**✅ sum** - Sum values
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
  when: event.type == "transaction"
```

**✅ avg** - Average values
```yaml
- name: avg_userid_order_amt_30d
  type: aggregation
  method: avg
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  window: 30d
  when: event.type == "order"
```

**✅ max / min** - Maximum / Minimum
```yaml
- name: max_userid_txn_amt_90d
  type: aggregation
  method: max
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  window: 90d
  when: event.type == "transaction"
```

**✅ distinct** - Count unique values
```yaml
- name: distinct_userid_device_24h
  type: aggregation
  method: distinct
  datasource: postgresql_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: device_id
  window: 24h
```

### 2.3 Planned Operators

**📋 stddev / variance** - Standard deviation / Variance
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
  when: event.type == "transaction"
```

**📋 percentile** - Nth percentile
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
  when: event.type == "transaction"
```

**📋 median / mode / entropy**


---

## 3. State

**Status:** 📋 All operators planned

**📋 z_score** - Statistical z-score
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
  when: event.type == "transaction"
```

**📋 deviation_from_baseline** - Compare to historical average

**📋 percentile_rank** - Rank compared to history

**📋 is_outlier** - Statistical outlier detection

**📋 timezone_consistency** - Timezone pattern check
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

> **Note:** Simple time checks (off-hours) should use Expression operators.

---

## 4. Sequence

**Status:** 📋 All operators planned

**📋 consecutive_count** - Count consecutive occurrences
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
      - event.type == "login"
      - event.status == "failed"
  reset_when: event.status == "success"
  order_by: timestamp
```

**📋 sequence_match** - Match event sequences
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
    - event.type == "password_reset"
    - event.type == "email_change"
    - event.type == "transaction" AND event.amount > 10000
  order_by: timestamp
```

**📋 percent_change** - Percentage change between windows
```yaml
- name: pctchg_userid_txn_cnt_week
  type: sequence
  method: percent_change
  datasource: clickhouse_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 7d
  when: event.type == "transaction"
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
  dimension_value: "{event.user_id}"
  field: session_id
  window: 24h

# Events per session
- name: events_per_session_7d
  type: expression
  method: expression
  expression: "total_events_7d / distinct_sessions_7d"
  depends_on:
    - total_events_7d
    - distinct_sessions_7d
```

---

## 5. Graph

**Status:** 📋 All operators planned

> **Note:** Simple entity linking (devices per IP) should use `distinct` aggregation, not Graph operators.

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

**📋 graph_centrality** - Network centrality score
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

**📋 community_size** - Size of connected component

**📋 shared_entity_count** - Count shared connections
```yaml
- name: shared_devices_between_users
  type: graph
  method: shared_entity_count
  datasource: neo4j_graph
  dimension: user_id
  dimension_value: "{event.user_id}"
  dimension_value2: "{event.target_user_id}"
  dimension2: device_id
  window: 30d
```

**📋 network_distance** - Distance between entities
```yaml
- name: network_dist_to_fraud_account
  type: graph
  method: network_distance
  datasource: neo4j_graph
  dimension: user_id
  dimension_value: "{event.user_id}"
  dimension_value2: "{known_fraud_user_id}"
  dimension2: device_id
  window: 90d
```

---

## 6. Expression

**Status:** ✅ Basic implemented, 📋 ML planned

> **⚠️ Architecture Constraint:** Expression operators **only consume results from other features**. They do not access raw data sources or define time windows.

**✅ expression** - Custom expressions
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

## 7. Lookup

**Status:** ✅ Implemented

> **⚠️ Architecture Principle:** Lookup features only retrieve pre-computed values; they do not perform computation.

```yaml
- name: user_risk_score_90d
  type: lookup
  datasource: redis_features
  key: "user_risk_score:{event.user_id}"
  fallback: 50

- name: device_reputation_score
  type: lookup
  datasource: redis_features
  key: "device_reputation:{event.device_id}"
  fallback: 0
```

---

## 8. Data Source Configuration

### 8.1 Data Source Types

| Type | Purpose | Used By |
|------|---------|---------|
| `postgresql` | Transactional/event data | Aggregation |
| `clickhouse` | High-volume event storage | State, Sequence |
| `neo4j` | Graph/relationship data | Graph |
| `redis` | Pre-computed features | Lookup |

### 8.2 Configuration Files

**`repository/configs/datasources/postgresql_events.yaml`:**
```yaml
name: postgresql_events
type: postgresql
config:
  host: ${POSTGRES_HOST}
  port: 5432
  database: ${POSTGRES_DATABASE}
  user: ${POSTGRES_USER}
  password: ${POSTGRES_PASSWORD}
  sslmode: ${POSTGRES_SSLMODE}
  max_connections: 20
  connection_timeout: 30
```

**`repository/configs/datasources/redis_features.yaml`:**
```yaml
name: redis_features
type: redis
config:
  host: ${REDIS_HOST}
  port: 6379
  password: ${REDIS_PASSWORD}
  db: 0
  key_prefix: "features:"
  ttl: 86400
```

**`repository/configs/datasources/neo4j_graph.yaml`:**
```yaml
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
| Expression | ❌ No | ✅ Yes |
| Lookup | ✅ Yes | ❌ No |

---

## 9. Naming Convention

### 9.1 Pattern

**Computed features:**
```
<operator>_<dimension>_<event>[_field]_<window>[_modifier]
```

**Lookup features:**
```
<descriptive_name>
```

### 9.2 Operator Abbreviations

| Category | Abbreviations |
|----------|---------------|
| Aggregation | `cnt`, `sum`, `avg`, `max`, `min`, `distinct`, `stddev`, `percentile`, `median`, `mode`, `entropy` |
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
version: "0.2"

features:
  # Aggregation
  - name: cnt_userid_login_24h
    type: aggregation
    method: count
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "{event.user_id}"
    window: 24h
    when: event.type == "login"

  - name: cnt_userid_login_1h_failed
    type: aggregation
    method: count
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "{event.user_id}"
    window: 1h
    when:
      all:
        - event.type == "login"
        - event.status == "failed"

  - name: distinct_userid_device_24h
    type: aggregation
    method: distinct
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "{event.user_id}"
    field: device_id
    window: 24h

  # Expression
  - name: rate_userid_login_failure
    type: expression
    method: expression
    expression: "cnt_userid_login_1h_failed / max(cnt_userid_login_24h, 1)"
    depends_on:
      - cnt_userid_login_1h_failed
      - cnt_userid_login_24h

  # Lookup
  - name: user_risk_score_90d
    type: lookup
    datasource: redis_features
    key: "user_risk_score:{event.user_id}"
    fallback: 50

# Usage in rules
rule:
  id: high_risk_pattern
  when:
    event.type: login
    all:
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
| `type` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `method` | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| `datasource` | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| `entity` | ✅ | ✅ | ✅ | ⚠️ | ❌ | ❌ |
| `dimension` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `dimension_value` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `field` | ⚠️ | ✅ | ⚠️ | ❌ | ❌ | ❌ |
| `window` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| `when` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `expression` | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| `depends_on` | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| `key` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| `fallback` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |

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
