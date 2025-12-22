# Feature Engineering for Risk Management

This document outlines the feature types supported and planned for Corint's risk management platform.

## Overview

Feature engineering in risk management follows a structured approach based on **what you want to measure**:

1. **Aggregation (数东西)** - Counting and aggregating events/values
2. **State (看最近状态)** - Checking current or recent state
3. **Sequence (看过程)** - Analyzing patterns and trends over time
4. **Graph (看关系图)** - Analyzing connections and networks between entities
5. **Expression (算分数)** - Computing scores and evaluations

> **Note:** List/Set operations (blacklist/whitelist checking, etc.) are implemented separately in Corint's list management system and are not covered in this feature engineering document.

---

## Operators by Category

### 1. Aggregation Operators
> **Rust Implementation:** `AggregationExecutor::execute(op: AggregationType, config: AggregationConfig)`
>
> **Design Pattern:** Unified executor with enum-based operator selection

**Implemented:**
- `count` - Count events matching conditions within time window
  - *Example: 用户过去24小时登录了5次*
- `sum` - Sum numeric field values
  - *Example: 用户过去30天交易总额为 ¥15,000*
- `avg` - Average of field values
  - *Example: 用户过去7天平均每笔交易金额 ¥500*
- `max` - Maximum value
  - *Example: 用户过去24小时单笔最大交易 ¥2,000*
- `min` - Minimum value
  - *Example: 用户过去7天单笔最小交易 ¥10*
- `distinct` - Count unique values
  - *Example: 用户过去7天使用了3个不同设备*

**Planned:**
- `stddev` - Standard deviation
  - *Example: 用户交易金额标准差 ¥350，波动较大*
- `variance` - Variance
  - *Example: 用户交易金额方差 122,500*
- `percentile` - Nth percentile value
  - *Example: 用户交易金额P95为 ¥1,800*
- `median` - Median value (50th percentile)
  - *Example: 用户交易金额中位数 ¥450*
- `mode` - Most frequent value
  - *Example: 用户最常见的交易金额 ¥100*
- `entropy` - Shannon entropy (diversity measure)
  - *Example: 用户交易类型熵值2.3，行为多样化*

> **Note:** Ratio- and rate-type metrics (e.g. success rate, failure rate, conversion rate) are **not** Aggregation operators. They are derived from aggregation results and must be implemented via **Expression operators**.

**Principle:** Aggregation = Single data source + Single window + Single grouping + One-pass scan

```rust
enum AggregationType {
    Count, Sum, Avg, Max, Min, Distinct,
    StdDev, Variance, Percentile(u8), Median, Mode, Entropy,
}

// Unified executor - all operators share common logic:
// - Time window filtering
// - Dimension grouping
// - Condition matching
// - One-pass aggregation
impl AggregationExecutor {
    fn execute(&self, op: AggregationType, config: &AggregationConfig) -> Result<Value>
}
```

### 2. State Operators
> **Rust Implementation:** `StateExecutor::execute(op: StateQueryType, config: StateConfig)`
>
> **Design Pattern:** Unified executor with specialized query strategies

**Implemented:**
- `first_seen` - First occurrence timestamp
  - *Example: 用户首次登录时间为 2024-01-15 10:23:00*
- `last_seen` - Last occurrence timestamp
  - *Example: 用户最近一次交易时间为 2025-12-20 15:30:00*
- `time_since` - Time elapsed since event
  - *Example: 账户注册至今已365天（账户年龄1年）*
- `velocity` - Check if count exceeds threshold in time window
  - *Example: 用户1小时内登录12次，超过阈值10次*
- `feature_store_lookup` - Lookup from Redis/cache
  - *Example: 用户风险评分为75分（从特征库读取）*
- `profile_lookup` - Lookup from database profile
  - *Example: 用户KYC状态为"已认证"（从用户档案读取）*

**Planned:**
- `z_score` - Statistical z-score compared to baseline
  - *Example: 当前交易金额Z-score为2.8，异常偏高*
- `deviation_from_baseline` - Compare to historical average
  - *Example: 当前登录频率比历史平均高150%*
- `percentile_rank` - Rank compared to history
  - *Example: 当前交易金额处于历史第92百分位*
- `is_outlier` - Statistical outlier detection
  - *Example: 当前行为判定为统计异常值（true）*
- `off_hours_activity` - Activity outside normal hours
  - *Example: 用户在凌晨3点登录（非正常时段）*

```rust
enum StateQueryType {
    FirstSeen, LastSeen, TimeSince,
    Velocity { threshold: usize },
    FeatureStoreLookup { key: String },
    ProfileLookup { field: String },
    ZScore, DeviationFromBaseline, PercentileRank, IsOutlier,
    OffHoursActivity,
}

// Unified executor - operators share query optimization:
// - Caching strategies
// - Lookup patterns
// - Baseline computation
impl StateExecutor {
    fn execute(&self, op: StateQueryType, config: &StateConfig) -> Result<Value>
}
```

### 3. Sequence Operators
> **Rust Implementation:** `SequenceAnalyzer::analyze(op: SequenceAnalysisType, config: SequenceConfig)`
>
> **Design Pattern:** Pipeline-based analyzer with composable stages

**Planned:**
- `consecutive_count` - Count consecutive occurrences
  - *Example: 用户连续失败登录3次*
- `streak` - Longest streak of condition
  - *Example: 用户连续7天每天都有交易（活跃度高）*
- `sequence_match` - Match event sequences
  - *Example: 检测到"修改密码→登录→大额转账"可疑序列*
- `pattern_frequency` - Frequency of specific patterns
  - *Example: "登录→浏览→加购→支付"完整路径出现5次*
- `trend` - Calculate trend (increasing/decreasing/stable)
  - *Example: 用户交易金额呈上升趋势（+15%/周）*
- `percent_change` - Percentage change between windows
  - *Example: 本周交易次数比上周增加120%*
- `rate_of_change` - Rate of change over time
  - *Example: 用户登录频率增长率为+5次/天*
- `anomaly_score` - Statistical anomaly detection
  - *Example: 序列异常评分8.5/10，高度可疑*
- `session_count` - Count sessions in window
  - *Example: 用户过去24小时共8个会话*
- `session_duration` - Average/total session duration
  - *Example: 用户平均会话时长12分钟*
- `events_per_session` - Average events per session
  - *Example: 用户每个会话平均操作25次*
- `moving_average` - Moving average over window
  - *Example: 用户7天移动平均交易额 ¥800/天*
- `exponential_moving_average` - EMA calculation
  - *Example: 用户交易金额EMA为 ¥750（α=0.3）*

```rust
enum SequenceAnalysisType {
    ConsecutiveCount, Streak, SequenceMatch { pattern: Vec<Pattern> },
    PatternFrequency, Trend, PercentChange, RateOfChange, AnomalyScore,
    SessionCount, SessionDuration, EventsPerSession,
    MovingAverage { window_size: usize },
    ExponentialMovingAverage { alpha: f64 },
}

// Pipeline-based analyzer - operators share:
// - Event ordering
// - Windowing logic
// - Pattern matching engine
impl SequenceAnalyzer {
    fn analyze(&self, op: SequenceAnalysisType, config: &SequenceConfig) -> Result<Value>
}
```

### 4. Graph Operators
> **Rust Implementation:** `GraphAnalyzer::analyze(op: GraphAnalysisType, config: GraphConfig)`
>
> **Design Pattern:** Graph-based analyzer with lazy graph construction
>
> **Note:** Graph operators are grouped by risk-domain semantics. Some operators (e.g. spatial/geographic features) do not require graph traversal and may be implemented using Sequence or State executors with spatial logic.

**Implemented:**
- `link_count` - Count linked entities across dimensions
  - *Example: IP地址 1.2.3.4 过去24小时关联了15个不同设备*

**Planned:**
- `graph_degree` - Number of connections in network
  - *Example: 用户在设备网络中的度为8（连接8个设备）*
- `graph_centrality` - Network centrality score
  - *Example: 设备在用户网络中心度0.65，可能是共享设备*
- `community_size` - Size of connected component
  - *Example: 该用户所在欺诈团伙社区规模23人*
- `shared_entity_count` - Count shared connections
  - *Example: 两个用户共享5个相同设备*
- `network_distance` - Distance between entities in graph
  - *Example: 两个账户的网络距离为3跳（间接关联）*
- `distance` - Geographic distance between points (spatiotemporal, not graph traversal)
  - *Example: 两次登录地理距离相距850公里*
- `impossible_travel` - Detect impossible travel patterns (spatiotemporal, not graph traversal)
  - *Example: 1小时内从北京到上海，物理上不可能*
- `location_change_count` - Count location changes (spatiotemporal, not graph traversal)
  - *Example: 用户过去7天更换了5个城市*
- `location_entropy` - Geographic diversity (spatiotemporal, not graph traversal)
  - *Example: 用户地理位置熵值1.8，活动范围较分散*
- `similarity_score` - Similarity to another entity
  - *Example: 两个用户行为相似度0.82（高度相似）*
- `compare_to_peer_group` - Compare to similar users
  - *Example: 用户交易额比同类群体高2.3倍*
- `cohort_average` - Average within cohort
  - *Example: 同类用户平均月交易额 ¥8,500*

```rust
enum GraphAnalysisType {
    LinkCount,
    Degree, Centrality, CommunitySize, SharedEntityCount, NetworkDistance,
    GeoDistance, ImpossibleTravel { max_speed_kmh: f64 },
    LocationChangeCount, LocationEntropy,
    Similarity, PeerComparison, CohortAverage,
}

// Graph analyzer - operators share:
// - Graph construction (for network-based operators)
// - Node/edge indexing
// - Graph algorithms library
//
// Note: Spatial operators (GeoDistance, ImpossibleTravel, LocationEntropy)
// may delegate to Sequence/State executors with spatial logic rather than
// performing graph traversal.
impl GraphAnalyzer {
    fn analyze(&self, op: GraphAnalysisType, config: &GraphConfig) -> Result<Value>
}
```

### 5. Expression Operators
> **Rust Implementation:** `ExpressionEngine::evaluate(expr: ExpressionType, context: &FeatureContext)`
>
> **Design Pattern:** Expression engine with pluggable evaluators
>
> **⚠️ Architecture Constraint:** Expression operators do NOT access raw data sources or define time windows; they ONLY consume results from other features.

**Implemented:**
- `expression` - Evaluate custom expressions using other features

**Planned:**
- `ml_model_score` - Call ML model for prediction
- `embedding_similarity` - Similarity using embeddings
- `clustering_label` - Assign to cluster
- `anomaly_detection_score` - ML-based anomaly score

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

---

## Architecture Benefits

This design provides:

1. **Code Reuse**: Common logic (time windows, filtering, caching) shared across operators
2. **Maintainability**: Adding new operators only requires extending enums, not creating new functions
3. **Performance**: Unified executors can optimize query plans and batch operations
4. **Type Safety**: Enum-based dispatch ensures compile-time operator validation
5. **Testability**: Each executor can be tested independently with all operator variants

---

## Table of Contents

- [1. Aggregation (数东西)](#1-aggregation-数东西)
- [2. State (看最近状态)](#2-state-看最近状态)
- [3. Sequence (看过程)](#3-sequence-看过程)
- [4. Graph (看关系图)](#4-graph-看关系图)
- [5. Expression (算分数)](#5-expression-算分数)
- [Implementation Roadmap](#implementation-roadmap)
- [By Risk Domain](#by-risk-domain)

---

## 1. Aggregation (数东西)

**Purpose:** Count events, aggregate values, and compute statistical measures.

### ✅ 1.1 Basic Counting
**Status:** Implemented

Count events and unique values within time windows.

**Operators:**
- `count` - Count events matching conditions
- `distinct` - Count unique values of a field
- `link_count` - Count linked entities across dimensions

**Use Cases:**
- Login attempts in time window
- Transaction count
- Failed payment attempts
- Unique IP addresses per user
- Devices associated with an IP

**Example:**
```yaml
- name: agg_cnt_userid_login_24h
  type: aggregation
  operator: count
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 24h
  when: event.type == "login"
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
- name: agg_sum_userid_txn_amt_24h
  type: aggregation
  operator: sum
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  window: 24h
  when: event.type == "transaction"
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
- name: agg_stddev_userid_txn_amt_30d
  type: aggregation
  operator: stddev
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  window: 30d
  when: event.type == "transaction"
```

---

## 2. State (看最近状态)

**Purpose:** Check current state, recent activity, and lookup stored values.

### ✅ 2.1 Temporal State
**Status:** Implemented

Track first/last occurrence and time elapsed.

**Operators:**
- `first_seen` - First occurrence timestamp
- `last_seen` - Last occurrence timestamp
- `time_since` - Time elapsed since event

**Use Cases:**
- Account age
- Time since last login
- Time since first transaction
- New vs returning user detection

**Example:**
```yaml
- name: state_timesince_userid_reg_d
  type: state
  operator: time_since
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  unit: d
  when: event.type == "register"
```

---

### ✅ 2.2 Velocity State
**Status:** Implemented

Check if activity rate exceeds threshold.

**Operators:**
- `velocity` - Check if count exceeds threshold

**Use Cases:**
- Login velocity abuse
- Transaction velocity monitoring
- API rate limiting
- Burst detection

**Example:**
```yaml
- name: state_velocity_userid_login_1h
  type: state
  operator: velocity
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 1h
  threshold: 10
  when: event.type == "login"
```

---

### ✅ 2.3 Lookup State
**Status:** Implemented

Retrieve pre-computed or profile data.

**Operators:**
- `feature_store_lookup` - Lookup from Redis/cache
- `profile_lookup` - Lookup from database profile table

**Use Cases:**
- User risk score
- KYC status
- Credit rating
- Account tier
- User profile attributes

**Example:**
```yaml
- name: expr_score_userid_risk
  type: state
  operator: feature_store_lookup
  datasource: redis_features
  key: "user_features:{event.user_id}:risk_score"
  fallback: 0.0
```

---

### 📋 2.4 Time-of-Day/Week State
**Status:** Planned - Medium Priority

Temporal pattern features based on time of day/week.

**Proposed Operators:**
- `off_hours_activity` - Activity outside normal hours (returns boolean or count)
- `timezone_consistency` - Timezone pattern consistency check

> **Note:** Distribution-style features (e.g. hour-of-day or day-of-week histograms) are **not** State operators. They should be implemented as **Aggregation operators** over derived time dimensions (e.g. `hour_of_day`, `day_of_week` fields).

**Use Cases:**
- Off-hours fraud detection
- Bot activity patterns
- Account takeover detection
- Behavioral consistency
- Working hours validation

**Proposed Syntax:**
```yaml
- name: state_offhours_userid_login_7d
  type: state
  operator: off_hours_activity
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 7d
  when: event.type == "login"
  normal_hours:
    start: "08:00"
    end: "22:00"
    timezone: "{user.timezone}"
```

---

### 📋 2.5 Historical Baseline State
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
- name: state_zscore_userid_txn_amt
  type: state
  operator: z_score
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  current_value: "{event.amount}"
  baseline_window: 90d
  when: event.type == "transaction"
```

---

## 3. Sequence (看过程)

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
- name: seq_consec_userid_login_1h_failed
  type: sequence
  operator: consecutive_count
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 1h
  when:
    all:
      - event.type == "login"
      - event.status == "failed"
  order_by: timestamp
  reset_when: event.status == "success"
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
- name: seq_pctchg_userid_txn_amt
  type: sequence
  operator: percent_change
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  current_window: 1h
  baseline_window: 24h
  aggregation: sum
  when: event.type == "transaction"
```

---

### 📋 3.3 Session Sequences
**Status:** Planned - Medium Priority

Analyze user sessions and session patterns.

**Proposed Operators:**
- `session_count` - Count sessions in window
- `session_duration` - Average/total session duration
- `events_per_session` - Average events per session
- `session_gap` - Time between sessions
- `session_pattern` - Session timing patterns

**Use Cases:**
- Bot detection
- Automated script detection
- Human behavior validation
- Session hijacking detection
- Abnormal session patterns

**Proposed Syntax:**
```yaml
- name: seq_duration_userid_sess_7d_avg
  type: sequence
  operator: session_duration
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 7d
  session_config:
    timeout: 30  # minutes of inactivity
    aggregation: avg
```

---

### 📋 3.4 Time-Series Analysis
**Status:** Planned - Future

Advanced time-series analysis and forecasting.

**Proposed Operators:**
- `moving_average` - Moving average over window
- `exponential_moving_average` - EMA calculation
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
- name: seq_movavg_userid_txn_amt_7d
  type: sequence
  operator: moving_average
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  window: 7d
  aggregation: sum
  smoothing: simple  # simple, exponential, weighted
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
- name: seq_pattern_userid_acct_takeover_1h
  type: sequence
  operator: event_pattern_match
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 1h
  pattern:
    - event_type: password_reset
      min_count: 1
    - event_type: login
      min_count: 3
      when: event.status == "failed"
    - event_type: login
      min_count: 1
      when: event.status == "success"
    - event_type: transaction
      min_count: 1
  sequence: ordered  # ordered, unordered, partial
```

---

## 4. Graph (看关系图)

**Purpose:** Analyze connections, networks, and relationship patterns between entities using graph theory.

### ✅ 4.1 Entity Links
**Status:** Implemented (Partial)

Count linked entities across dimensions.

**Operators:**
- `link_count` - Count linked entities across dimensions

**Use Cases:**
- Devices per IP
- Users per device
- Accounts per email domain

**Example:**
```yaml
- name: graph_linkcnt_ip_device_24h
  type: graph
  operator: link_count
  entity: events
  primary_dimension: ip_address
  primary_value: "{event.ip_address}"
  secondary_dimension: device_id
  window: 24h
```

---

### 📋 4.2 Network Analysis
**Status:** Planned - Low Priority (Complex)

Analyze entity relationships and network patterns using graph algorithms.

**Proposed Operators:**
- `graph_degree` - Number of connections
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
- name: graph_degree_userid_device_30d
  type: graph
  operator: graph_degree
  entity: events
  primary_dimension: user_id
  primary_value: "{event.user_id}"
  link_dimension: device_id
  window: 30d
```

---

### 📋 4.3 Spatial Graph
**Status:** Planned - High Priority

Geographic and location-based graph features.

> **Note:** Operators such as geographic distance, impossible travel, and location entropy are **spatiotemporal calculations**. They do **not** construct or traverse a relationship graph and may be implemented internally using **Sequence or State executors with spatial logic**, even though they are grouped here for risk-domain clarity.

**Proposed Operators:**
- `distance` - Geographic distance between points
- `impossible_travel` - Detect impossible travel
- `location_change_count` - Count location changes
- `location_entropy` - Geographic diversity
- `country_count` - Distinct countries

**Use Cases:**
- Impossible travel detection
- VPN/proxy detection
- Location velocity
- Geographic anomaly
- Cross-border transactions

**Proposed Syntax:**
```yaml
- name: graph_distance_userid_location_24h
  type: graph
  operator: impossible_travel
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 24h
  location_fields:
    latitude: lat
    longitude: lon
  max_speed_kmh: 800  # Max reasonable travel speed
```

---

### 📋 4.4 Similarity Graph
**Status:** Planned - Low Priority

Compare and measure similarity between entities and cohorts.

**Proposed Operators:**
- `compare_to_peer_group` - Compare to similar users
- `global_percentile` - Global ranking
- `cohort_average` - Average within cohort
- `similarity_score` - Similarity to another entity

**Use Cases:**
- Peer group analysis
- Cohort comparisons
- Industry benchmarking
- User segmentation
- Anomaly detection

**Proposed Syntax:**
```yaml
- name: graph_similarity_userid_cohort_30d
  type: graph
  operator: compare_to_peer_group
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  aggregation: sum
  window: 30d
  cohort:
    - field: account_type
      value: "{user.account_type}"
    - field: country
      value: "{user.country}"
```

---

## 5. Expression (算分数)

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
- name: expr_rate_userid_login_failure
  type: expression
  operator: expression
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
- name: expr_mlscore_userid_fraud
  type: expression
  operator: ml_model_score
  model: fraud_detection_v2
  inputs:
    - transaction_count_24h
    - transaction_sum_24h
    - unique_devices_7d
    - account_age_days
  output: fraud_probability
```

---

## Implementation Roadmap

### Phase 1: Core Enhancements (Q1 2025)
**Focus:** Complete basic operators and add high-priority features

- ✅ Complete Expression operator implementation
- 📋 Implement Pattern sequences (3.1)
- 📋 Implement Spatial graph features (4.3)
- 📋 Add more time window units (weeks, months)

### Phase 2: Advanced Analytics (Q2 2025)
**Focus:** Statistical and behavioral analysis

- 📋 Statistical aggregations (1.3)
- 📋 Trend detection (3.2)
- 📋 Session sequences (3.3)
- 📋 Time-of-day/week state (2.4)

### Phase 3: Complex Features (Q3 2025)
**Focus:** Advanced graph analysis and baselines

- 📋 Historical baseline state (2.5)
- 📋 Similarity graph features (4.4)
- 📋 Network analysis (4.2) - basic
- 📋 Time-series analysis (3.4) - basic

### Phase 4: Advanced/ML Integration (Q4 2025+)
**Focus:** AI and complex event processing

- 📋 Machine learning integration (5.2)
- 📋 Advanced network analysis algorithms (4.2)
- 📋 Complex event processing (3.5)
- 📋 Real-time streaming features

---

## By Risk Domain

| Risk Domain | Primary Categories | Key Operators |
|-------------|-------------------|---------------|
| **Account Security** | State, Sequence | velocity, consecutive_count, impossible_travel, off_hours_activity |
| **Transaction Fraud** | Aggregation, Sequence, Expression | sum, avg, stddev, trend, z_score, anomaly_score |
| **Bot Detection** | Sequence, State | session_duration, session_pattern, time_of_day, velocity |
| **Account Takeover** | State, Sequence, Graph | location_change, last_seen, consecutive_count, deviation_from_baseline |
| **Payment Fraud** | Aggregation, Sequence, Graph | count, velocity, pattern_frequency, distance |
| **Synthetic Identity** | Graph, Aggregation | graph_degree, shared_entity_count, distinct |
| **Credit Risk** | Aggregation, State, Sequence | sum, avg, stddev, trend, baseline_deviation |
| **AML/Compliance** | Aggregation, Graph, Sequence | sum, distinct, graph_centrality, pattern_match |

---

## Feature Category Summary

### By Implementation Status

| Category | Sub-Category | Status | Priority | Complexity |
|----------|--------------|--------|----------|------------|
| **Aggregation** | Basic Counting (1.1) | ✅ Implemented | - | Low |
| | Basic Aggregations (1.2) | ✅ Implemented | - | Low |
| | Statistical Aggregations (1.3) | 📋 Planned | Medium | Medium |
| **State** | Temporal State (2.1) | ✅ Implemented | - | Low |
| | Velocity State (2.2) | ✅ Implemented | - | Low |
| | Lookup State (2.3) | ✅ Implemented | - | Low |
| | Time-of-Day/Week (2.4) | 📋 Planned | Medium | Low |
| | Historical Baseline (2.5) | 📋 Planned | Low | Medium |
| **Sequence** | Pattern Sequences (3.1) | 📋 Planned | High | Medium |
| | Trend Detection (3.2) | 📋 Planned | Medium | Medium |
| | Session Sequences (3.3) | 📋 Planned | Medium | Medium |
| | Time-Series Analysis (3.4) | 📋 Planned | Future | High |
| | Complex Event Processing (3.5) | 📋 Planned | Future | Very High |
| **Graph** | Entity Links (4.1) | ✅ Partial | - | Low |
| | Network Analysis (4.2) | 📋 Planned | Low | High |
| | Spatial Graph (4.3) | 📋 Planned | High | Medium |
| | Similarity Graph (4.4) | 📋 Planned | Low | Medium |
| **Expression** | Custom Expressions (5.1) | ✅ Partial | - | Medium |
| | Machine Learning (5.2) | 📋 Planned | Future | High |

---

## Feature Naming Convention

Feature names should follow a structured pattern for clarity and machine parseability.

### Naming Pattern

```
<type>_<operator>_<dimension>_<event>[_field]_<window>[_modifier]
```

**Components (in order):**

1. **Type** (required) - Feature category
   - `agg` - Aggregation (数东西)
   - `state` - State (看最近状态)
   - `seq` - Sequence (看过程)
   - `graph` - Graph (看关系图)
   - `expr` - Expression (算分数)

2. **Operator** (required) - Operation type
   - Aggregation: `cnt`, `sum`, `avg`, `max`, `min`, `distinct`, `stddev`, `variance`, `percentile`, `median`, `mode`, `entropy`
   - State: `firstseen`, `lastseen`, `timesince`, `velocity`, `zscore`
   - Sequence: `consec`, `trend`, `pctchg`, `streak`
   - Graph: `linkcnt`, `degree`, `distance`, `similarity`
   - Expression: `expr`, `mlscore`

3. **Dimension** (required) - Aggregation dimension
   - `userid`, `deviceid`, `ip`, `acctid`, `sessid`, `email`

4. **Event** (required) - Event or entity type
   - `login`, `txn`, `pay`, `reg`, `checkout`, `pwd_reset`

5. **Field** (optional) - Field name for aggregations
   - `amt`, `val`, `score`, `dur`, `cnt`

6. **Window** (required for time-based) - Time window
   - `1h`, `24h`, `7d`, `30d`, `90d`

7. **Modifier** (optional) - Additional qualifier
   - `failed`, `success`, `new`, `change`, `rate`

### Standard Abbreviations

To keep feature names concise, use these standard abbreviations:

| Full Word | Abbreviation | Usage |
|-----------|--------------|-------|
| **Events** |
| transaction | `txn` | `agg_cnt_userid_txn_24h` |
| payment | `pay` | `agg_sum_userid_pay_amt_7d` |
| register | `reg` | `state_timesince_userid_reg_d` |
| session | `sess` | `seq_dur_userid_sess_7d_avg` |
| password | `pwd` | `seq_pattern_userid_pwd_reset_1h` |
| checkout | `checkout` | `agg_cnt_userid_checkout_24h` |
| **Dimensions** |
| account | `acct` | `agg_cnt_acctid_login_24h` |
| device | `dev` | Optional: `deviceid` or `devid` |
| session | `sess` | `agg_cnt_sessid_event_1h` |
| **Fields** |
| amount | `amt` | `agg_sum_userid_txn_amt_30d` |
| value | `val` | `agg_max_userid_pay_val_1h` |
| count | `cnt` | Used in operator position |
| duration | `dur` | `seq_avg_userid_sess_dur_7d` |
| distance | `dist` | `graph_dist_userid_location_24h` |
| **Operators** (already short) |
| count | `cnt` | `agg_cnt_*` |
| average | `avg` | `agg_avg_*` |
| distinct | `distinct` | `agg_distinct_*` |
| consecutive | `consec` | `seq_consec_*` |
| percent | `pct` | `seq_pctchg_*` |
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
- name: agg_sum_userid_txn_amt_24h     # name 使用缩写
  field: amount                         # field 使用完整词
  when: event.type == "transaction"     # when 使用完整词

# ❌ Wrong - don't use abbreviations in config
- name: agg_sum_userid_txn_amt_24h
  field: amt                            # ❌ 错误
  when: event.type == "txn"             # ❌ 错误
```

### Examples

**Aggregation Features:**

```yaml
# Basic counting
agg_cnt_userid_login_24h               # 用户24小时登录次数
agg_cnt_userid_txn_7d          # 用户7天交易次数
agg_cnt_deviceid_login_1h              # 设备1小时登录次数

# Sum/Avg with field
agg_sum_userid_txn_amt_30d  # 用户30天交易金额总和
agg_avg_userid_pay_amt_7d       # 用户7天支付平均金额
agg_max_userid_txn_amt_24h  # 用户24小时最大交易金额

# Distinct counting
agg_distinct_userid_device_7d          # 用户7天内不同设备数
agg_distinct_userid_ip_24h             # 用户24小时内不同IP数
agg_distinct_ip_userid_1h              # IP 1小时内不同用户数

# With modifier for conditions
agg_cnt_userid_login_1h_failed         # 用户1小时失败登录次数
agg_cnt_userid_pay_24h_success     # 用户24小时成功支付次数
```

**State Features:**

```yaml
# Temporal state
state_firstseen_userid_login           # 用户首次登录时间
state_lastseen_userid_txn      # 用户最近交易时间
state_timesince_userid_reg_d      # 用户注册天数 (账户年龄)

# Velocity
state_velocity_userid_login_1h         # 用户1小时登录速率检查
state_velocity_userid_txn_24h  # 用户24小时交易速率检查

# Baseline comparison
state_zscore_userid_txn_amt # 用户交易金额Z-score
```

**Sequence Features:**

```yaml
# Pattern sequences
seq_consec_userid_login_1h_failed      # 用户1小时连续失败登录次数
seq_streak_userid_txn_7d       # 用户7天交易连续性

# Trend detection
seq_pctchg_userid_txn_amt   # 用户交易金额变化百分比
seq_trend_userid_login_7d              # 用户7天登录趋势

# Session analysis
seq_duration_userid_sess_7d_avg     # 用户7天平均会话时长
```

**Graph Features:**

```yaml
# Entity links
graph_linkcnt_ip_device_24h            # IP 24小时关联设备数
graph_linkcnt_deviceid_userid_7d       # 设备7天关联用户数

# Network analysis
graph_degree_userid_device_30d         # 用户30天设备网络度
graph_distance_userid_location_24h     # 用户24小时地理距离

# Similarity
graph_similarity_userid_cohort_30d     # 用户30天同类群体相似度
```

**Expression Features:**

```yaml
# Computed scores
expr_score_userid_fraud                # 用户欺诈评分
expr_mlscore_userid_risk               # 用户ML风险评分

# Ratio/Rate (complex expressions)
expr_rate_userid_login_1h_failure      # 用户1小时登录失败率
expr_ratio_userid_txn_7d_change # 用户7天交易比率变化
```

**Avoid:**

```yaml
# ❌ Missing type prefix
cnt_userid_login_24h                   # Missing 'agg_'
velocity_userid_login_1h               # Missing 'state_'

# ❌ Wrong order
userid_cnt_login_24h_agg               # Type should be first
24h_login_cnt_userid_agg               # Wrong order

# ❌ Inconsistent abbreviations
agg_count_userid_login_24h             # Use 'cnt' not 'count'
agg_cnt_user_id_login_24h              # Use 'userid' not 'user_id'

# ❌ Too vague
agg_cnt_24h                            # Missing dimension and event
state_velocity_1h                      # Missing dimension and event
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
