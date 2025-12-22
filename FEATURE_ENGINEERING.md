# Feature Engineering for Risk Management

This document outlines the feature types supported and planned for Corint's risk management platform.

## Overview

Feature engineering in risk management follows a structured approach based on **what you want to measure**:

1. **Aggregation (数东西)** - Counting and aggregating events/values
2. **State (看最近状态)** - Checking current state and statistical comparisons
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
  - **实际应用场景**:
    - 暴力破解检测：统计1小时内失败登录次数，超过10次触发账户锁定
    - 交易频率监控：统计用户24小时内交易次数，异常高频可能是盗号
    - API限流：统计IP地址1分钟内请求次数，超过100次拒绝服务
  - **YAML示例**:
    ```yaml
    - name: agg_cnt_userid_login_1h_failed
      type: aggregation
      operator: count
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      window: 1h
      when:
        all:
          - event.type == "login"
          - event.status == "failed"
    ```

- `sum` - Sum numeric field values
  - *Example: 用户过去30天交易总额为 ¥15,000*
  - **实际应用场景**:
    - 洗钱检测：统计账户24小时内转账总金额，超过¥50万需人工审核
    - 信用额度管理：统计用户30天消费总额，判断是否超过信用额度
    - 积分欺诈：统计用户1小时内获取积分总数，异常高额可能是刷积分
  - **YAML示例**:
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

- `avg` - Average of field values
  - *Example: 用户过去7天平均每笔交易金额 ¥500*
  - **实际应用场景**:
    - 异常交易金额检测：用户平均交易¥500，突然出现¥50,000交易需验证
    - 用户画像：计算用户平均订单金额，用于用户分层（高/中/低消费）
    - 会话时长分析：统计用户平均会话时长，异常短可能是机器人
  - **YAML示例**:
    ```yaml
    - name: agg_avg_userid_order_amt_30d
      type: aggregation
      operator: avg
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      window: 30d
      when: event.type == "order"
    ```

- `max` - Maximum value
  - *Example: 用户过去24小时单笔最大交易 ¥2,000*
  - **实际应用场景**:
    - 大额交易监控：检测用户历史最大交易金额，当前交易超过3倍需验证
    - 单笔限额检查：新注册用户24小时内最大交易不超过¥5,000
    - 异常行为识别：IP地址关联的最大用户数超过50，可能是代理或公共WiFi
  - **YAML示例**:
    ```yaml
    - name: agg_max_userid_txn_amt_90d
      type: aggregation
      operator: max
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      window: 90d
      when: event.type == "transaction"
    ```

- `min` - Minimum value
  - *Example: 用户过去7天单笔最小交易 ¥10*
  - **实际应用场景**:
    - 测试交易检测：大量¥0.01小额交易可能是盗卡测试
    - 刷单识别：最小订单金额异常低（如¥0.1）配合高频次，疑似刷单
    - 异常折扣监控：订单最小金额为¥1，可能存在优惠券漏洞
  - **YAML示例**:
    ```yaml
    - name: agg_min_userid_order_amt_7d
      type: aggregation
      operator: min
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: amount
      window: 7d
      when: event.type == "order"
    ```

- `distinct` - Count unique values
  - *Example: 用户过去7天使用了3个不同设备*
  - **实际应用场景**:
    - 账号共享检测：用户24小时内使用超过5个不同设备，可能是账号被盗或共享
    - IP跳跃检测：用户1小时内使用超过10个不同IP，可能使用代理池
    - 多账户关联：同一设备24小时内登录超过20个不同账户，可能是批量操作
  - **YAML示例**:
    ```yaml
    - name: agg_distinct_userid_device_24h
      type: aggregation
      operator: distinct
      entity: events
      dimension: user_id
      dimension_value: "{event.user_id}"
      field: device_id
      window: 24h
    ```

**Planned:**
- `stddev` - Standard deviation
  - *Example: 用户交易金额标准差 ¥350，波动较大*
  - **实际应用场景**:
    - 行为稳定性分析：交易金额标准差过大，行为不稳定，可能被盗号
    - 异常波动检测：用户历史标准差¥50，近期标准差¥500，行为剧变
    - 用户分群：低标准差用户（固定消费）vs 高标准差用户（消费随机）
  - **YAML示例**:
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

- `variance` - Variance
  - *Example: 用户交易金额方差 122,500*
  - **实际应用场景**:
    - 风险评分：高方差用户风险更高，行为不可预测
    - 机器人检测：机器人交易方差通常很小（固定金额）
    - 信用评估：低方差用户还款行为更稳定，信用更好

- `percentile` - Nth percentile value
  - *Example: 用户交易金额P95为 ¥1,800*
  - **实际应用场景**:
    - 异常阈值设定：超过P95的交易需要额外验证
    - 动态限额：根据用户P90交易金额设置每日限额
    - 信用额度：用户P75消费金额作为信用额度参考

- `median` - Median value (50th percentile)
  - *Example: 用户交易金额中位数 ¥450*
  - **实际应用场景**:
    - 抗异常值统计：中位数不受极端值影响，更准确反映用户典型行为
    - 用户画像：中位数订单金额用于用户价值评估
    - 异常检测：当前交易是中位数的10倍，需要验证

- `mode` - Most frequent value
  - *Example: 用户最常见的交易金额 ¥100*
  - **实际应用场景**:
    - 充值模式识别：用户最常充值¥100，异常充值¥10,000需验证
    - 刷单检测：大量相同金额订单（众数占比>80%）疑似刷单
    - 习惯识别：用户最常在晚上8点登录，凌晨3点登录异常

- `entropy` - Shannon entropy (diversity measure)
  - *Example: 用户交易类型熵值2.3，行为多样化*
  - **实际应用场景**:
    - 机器人检测：熵值过低（<0.5），行为模式单一，可能是机器人
    - 账号活跃度：熵值高的用户行为丰富，更像真实用户
    - 异常检测：用户历史熵值2.5，近期降至0.3，行为异常单一

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
> **Design Pattern:** Statistical comparison and baseline analysis
>


**Planned:**
- `z_score` - Statistical z-score compared to baseline
  - *Example: 当前交易金额Z-score为2.8，异常偏高*
  - **实际应用场景**:
    - 异常交易检测：用户交易金额Z-score > 3，可能被盗刷
    - 登录频率异常：登录频率Z-score > 2.5，可能是暴力破解
    - 动态阈值：根据Z-score自动调整风控策略，而非固定阈值
  - **YAML示例**:
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

- `deviation_from_baseline` - Compare to historical average
  - *Example: 当前登录频率比历史平均高150%*
  - **实际应用场景**:
    - 行为突变检测：用户日均登录2次，今天登录20次，偏离900%
    - 消费习惯变化：历史日均消费¥200，今天消费¥5000，偏离2400%
    - 账号接管：行为模式突然偏离基线，可能被他人控制

- `percentile_rank` - Rank compared to history
  - *Example: 当前交易金额处于历史第92百分位*
  - **实际应用场景**:
    - 大额交易验证：当前交易金额超过历史P95，需要二次验证
    - 异常活跃度：当前登录频率超过历史P99，可能异常
    - 风险分级：P0-P50低风险，P50-P90中风险，P90+高风险

- `is_outlier` - Statistical outlier detection
  - *Example: 当前行为判定为统计异常值（true）*
  - **实际应用场景**:
    - 自动异常标记：统计学判断为异常值，直接触发人工审核
    - 欺诈检测：交易金额/频率/地点等多维度异常值检测
    - 机器学习特征：异常值标记作为ML模型输入特征

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

### 3. Sequence Operators
> **Rust Implementation:** `SequenceAnalyzer::analyze(op: SequenceAnalysisType, config: SequenceConfig)`
>
> **Design Pattern:** Pipeline-based analyzer with composable stages

**Planned:**
- `consecutive_count` - Count consecutive occurrences
  - *Example: 用户连续失败登录3次*
  - **实际应用场景**:
    - 暴力破解：连续失败登录≥5次，锁定账户15分钟
    - 支付失败：连续3次支付失败，可能卡被冻结或余额不足
    - 异常操作：连续10次快速点击，可能是脚本攻击
  - **YAML示例**:
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
      reset_when: event.status == "success"
    ```

- `streak` - Longest streak of condition
  - *Example: 用户连续7天每天都有交易（活跃度高）*
  - **实际应用场景**:
    - 用户活跃度：连续活跃7天的用户，流失风险低
    - 刷单检测：连续30天每天都有订单，且金额相似，疑似刷单
    - 习惯养成：连续3天使用某功能，推荐相关服务

- `sequence_match` - Match event sequences
  - *Example: 检测到"修改密码→登录→大额转账"可疑序列*
  - **实际应用场景**:
    - 账户接管：密码重置→修改邮箱→大额转账（15分钟内），高风险
    - 欺诈模式：注册→实名认证→申请贷款→提现（1小时内），疑似欺诈
    - 正常流程：浏览商品→加入购物车→结算→支付，转化漏斗分析

- `pattern_frequency` - Frequency of specific patterns
  - *Example: "登录→浏览→加购→支付"完整路径出现5次*
  - **实际应用场景**:
    - 刷单检测：相同操作序列重复出现>10次，疑似刷单
    - 用户行为分析：高价值用户的典型路径频率
    - 异常模式：异常操作序列频繁出现，可能是攻击

- `trend` - Calculate trend (increasing/decreasing/stable)
  - *Example: 用户交易金额呈上升趋势（+15%/周）*
  - **实际应用场景**:
    - 消费趋势：交易金额持续上升，用户价值增长
    - 风险趋势：失败交易比例上升趋势，可能卡出问题
    - 异常检测：登录频率突然上升趋势（斜率陡增），可能被盗号

- `percent_change` - Percentage change between windows
  - *Example: 本周交易次数比上周增加120%*
  - **实际应用场景**:
    - 行为突变：本周交易比上周增加500%，异常活跃
    - 促销效果：活动期间交易量增加200%，效果显著
    - 休眠唤醒：本周交易比上周增长从0到10，账户被重新激活

- `rate_of_change` - Rate of change over time
  - *Example: 用户登录频率增长率为+5次/天*
  - **实际应用场景**:
    - 加速度检测：交易频率增长率从1次/天加速到10次/天，异常
    - 渐进式攻击：失败登录率每小时增加2次，逐步升级攻击
    - 趋势预警：订单量下降率-3单/天，可能流失

- `anomaly_score` - Statistical anomaly detection
  - *Example: 序列异常评分8.5/10，高度可疑*
  - **实际应用场景**:
    - 综合异常检测：基于时序模型计算异常分数，>7分触发审核
    - 账户行为画像：行为序列与历史模式差异度评分
    - 欺诈概率：序列异常分数作为欺诈模型输入特征

- `moving_average` - Moving average over window
  - *Example: 用户7天移动平均交易额 ¥800/天*
  - **实际应用场景**:
    - 平滑趋势分析：7日移动平均消除日常波动，观察真实趋势
    - 异常检测：当前交易额超过7日移动平均3倍，异常
    - 动态基线：使用移动平均作为动态基线，自适应用户行为变化

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

### 4. Graph Operators
> **Rust Implementation:** `GraphAnalyzer::analyze(op: GraphAnalysisType, config: GraphConfig)`
>
> **Design Pattern:** Graph-based analyzer with lazy graph construction
>

**Planned:**
- `graph_centrality` - Network centrality score
  - *Example: 设备在用户网络中心度0.65，可能是共享设备*
  - **实际应用场景**:
    - 核心节点识别：中心度>0.8的设备，可能是欺诈团伙核心设备
    - 风险源定位：高中心度账户被标记欺诈，关联账户需审查
    - 黑产识别：中心度异常高的IP，可能是黑产操作节点

- `community_size` - Size of connected component
  - *Example: 该用户所在欺诈团伙社区规模23人*
  - **实际应用场景**:
    - 团伙欺诈：社区规模>20人且交易模式相似，疑似欺诈团伙
    - 洗钱网络：资金在大社区内循环流转，可能洗钱
    - 正常社交：小社区(<5人)且行为正常，可能是家庭/朋友

- `shared_entity_count` - Count shared connections
  - *Example: 两个用户共享5个相同设备*
  - **实际应用场景**:
    - 虚假账户：两个账户共享>3个设备，可能是同一人多账户
    - 关联欺诈：多个高风险账户共享设备/IP，协同欺诈
    - 家庭识别：共享2个设备(手机+电脑)，可能是家庭成员

- `network_distance` - Distance between entities in graph
  - *Example: 两个账户的网络距离为3跳（间接关联）*
  - **实际应用场景**:
    - 风险传播：距离已知欺诈账户≤2跳，需要审查
    - 关联分析：虽无直接关联，但网络距离≤3跳，间接关联
    - 社交推荐：网络距离2-3跳的用户，可能有共同兴趣

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

### 5. Expression Operators
> **Rust Implementation:** `ExpressionEngine::evaluate(expr: ExpressionType, context: &FeatureContext)`
>
> **Design Pattern:** Expression engine with pluggable evaluators
>
> **⚠️ Architecture Constraint:** Expression operators do NOT access raw data sources or define time windows; they ONLY consume results from other features.

**Implemented:**
- `expression` - Evaluate custom expressions using other features
  - *Example: 计算登录失败率 = failed_count / total_count*
  - **实际应用场景**:
    - 失败率计算：login_failure_rate = failed_login_count_1h / login_count_1h
    - 复合评分：risk_score = 0.4 * transaction_anomaly + 0.3 * device_risk + 0.3 * location_risk
    - 比率分析：large_transaction_ratio = transactions_above_1000 / total_transactions
    - 转化率：conversion_rate = purchase_count / view_count
  - **YAML示例**:
    ```yaml
    - name: expr_rate_userid_login_failure
      type: expression
      operator: expression
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

Feature definitions support flexible data source configuration through the `datasource` field. This allows accessing data from different storage systems without changing the operator logic.

### Supported Data Sources

**Event/Transaction Data:**
- `clickhouse` - ClickHouse for high-volume event storage
- `postgresql` - PostgreSQL for transactional data

**Pre-computed Features:**
- `redis` - Redis for cached feature values
- `feature_store` - Dedicated feature store (e.g., Feast, Tecton)

**Profile/Context Data:**
- Should be passed directly in the request payload, not queried via datasource

### Feature Definition with Data Source

```yaml
# Aggregation feature - queries event data from ClickHouse
- name: agg_cnt_userid_login_1h
  type: aggregation
  operator: count
  datasource: clickhouse_events
  entity: events
  dimension: user_id
  dimension_value: "{event.user_id}"
  window: 1h
  when: event.type == "login"

# Pre-computed feature lookup - simple key-value access
- name: user_risk_score_90d
  datasource: redis_features
  key: "user_risk_score_90d:{event.user_id}"
  fallback: 50
  # Note: No type/operator needed for simple lookups

# State feature - computes z-score from historical data
- name: state_zscore_userid_txn_amt
  type: state
  operator: z_score
  datasource: clickhouse_events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: amount
  current_value: "{event.amount}"
  baseline_window: 90d
  when: event.type == "transaction"
```

### Implementation Pattern

```rust
// Data source abstraction
enum DataSource {
    ClickHouse(ClickHouseClient),
    PostgreSQL(PostgresClient),
    Redis(RedisClient),
    FeatureStore(FeatureStoreClient),
}

// Feature execution delegates to appropriate data source
impl FeatureExecutor {
    fn execute(&self, feature: &FeatureConfig) -> Result<Value> {
        match &feature.definition {
            // Simple lookup - no operator, just datasource + key
            FeatureDefinition::Lookup { key, fallback } => {
                self.datasource.get(key).or(fallback)
            }
            // Computed feature - uses operator + datasource
            FeatureDefinition::Computed { operator, config } => {
                match operator.category() {
                    OperatorCategory::Aggregation => {
                        AggregationExecutor::new(&self.datasource)
                            .execute(operator, config)
                    }
                    OperatorCategory::State => {
                        StateExecutor::new(&self.datasource)
                            .execute(operator, config)
                    }
                    // ...
                }
            }
        }
    }
}
```

### Lookup vs Computed Features

**Simple Lookup** (no type/operator):
```yaml
# Just datasource + key - for pre-computed values
- name: user_segment_label
  datasource: redis_features
  key: "user_segment:{event.user_id}"
  fallback: "unknown"
```

**Computed Feature** (with type/operator):
```yaml
# Requires computation - needs operator + datasource
- name: agg_sum_userid_txn_amt_24h
  type: aggregation
  operator: sum
  datasource: clickhouse_events
  # ... aggregation config
```

**Key Differences:**
- **Lookup**: Pre-computed values stored in cache/storage, accessed by key
- **Computed**: Real-time calculation using operators, queries raw data from datasource

---

## Table of Contents

- [Data Source Configuration](#data-source-configuration)
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
- name: state_timezone_consistency_userid_7d
  type: state
  operator: timezone_consistency
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
- name: agg_distinct_userid_session_24h
  type: aggregation
  operator: distinct
  datasource: clickhouse_events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: session_id
  window: 24h

# Average session duration - using avg
- name: agg_avg_userid_session_duration_7d
  type: aggregation
  operator: avg
  datasource: clickhouse_events
  dimension: user_id
  dimension_value: "{event.user_id}"
  field: session_duration  # Business system provides this
  window: 7d

# Events per session - using expression
- name: expr_events_per_session_7d
  type: expression
  operator: expression
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
- name: seq_movavg_userid_txn_amt_7d
  type: sequence
  operator: moving_average
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

> **Note on Entity Linking:**
>
> Simple entity linking (e.g., "devices per IP", "users per device") should use **`distinct` aggregation**, not Graph operators:
>
> ```yaml
> # Count devices per IP - use distinct
> - name: agg_distinct_ip_device_24h
>   type: aggregation
>   operator: distinct
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
- name: graph_centrality_userid_device_30d
  type: graph
  operator: graph_centrality
  entity: events
  primary_dimension: user_id
  primary_value: "{event.user_id}"
  link_dimension: device_id
  window: 30d
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
   - State: `zscore`, `deviation`, `percentile`, `outlier`, `timezone`
   - Sequence: `consec`, `trend`, `pctchg`, `streak`
   - Graph: `centrality`, `community`, `shared`
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
| register | `reg` | N/A (use event.account_age_days) |
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
# Statistical comparison (planned)
state_zscore_userid_txn_amt            # 用户交易金额Z-score
state_deviation_userid_login_freq      # 用户登录频率偏离度
state_percentile_userid_txn_amt        # 用户交易金额百分位
state_timezone_userid_login_7d         # 用户时区一致性检测

# Note: Simple lookups don't need State operators, use datasource directly:
- name: user_risk_score_90d
  datasource: redis_features
  key: "user_risk_score:{event.user_id}"

# Time-based checks should use Expression or Aggregation:
# - Off-hours check: expr_is_off_hours (expression: "event.hour < 8 || event.hour > 22")
# - Off-hours count: agg_cnt_userid_login_7d_offhours (aggregation with when condition)

# Context data should be provided by business system:
# - event.account_age_days, user.last_login_at (temporal)
# - user.kyc_status, user.account_type, user.country (profile)
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
# Entity linking - use distinct (not Graph operators)
agg_distinct_ip_device_24h             # IP 24小时关联设备数（用 distinct）
agg_distinct_deviceid_userid_7d        # 设备7天关联用户数（用 distinct）

# Network analysis (planned)
graph_centrality_userid_device_30d     # 用户30天设备网络中心度
graph_community_userid_network_30d     # 用户30天所在社区大小
graph_shared_userid_device_30d         # 用户间共享设备数
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
