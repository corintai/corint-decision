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

1. **Aggregation (数东西)** 🟢 - Counting and aggregating events/values
2. **State (看最近状态)** 🔴 - Checking current state and statistical comparisons
3. **Sequence (看过程)** 🔴 - Analyzing patterns and trends over time
4. **Graph (看关系图)** 🔴 - Analyzing connections and networks between entities
5. **Expression (算分数)** 🟢 - Computing scores and evaluations
6. **Lookup (查预算值)** 🟢 - Retrieving pre-computed feature values

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
  - *Example: 用户过去24小时登录了5次*
  - **实际应用场景**:
    - 暴力破解检测：统计1小时内失败登录次数，超过10次触发账户锁定
    - 交易频率监控：统计用户24小时内交易次数，异常高频可能是盗号
    - API限流：统计IP地址1分钟内请求次数，超过100次拒绝服务
  - **YAML示例**:
    ```yaml
    - name: cnt_userid_login_1h_failed
      type: aggregation
      method: count
      datasource: postgresql_events
      entity: events
      dimension: user_id              # 按用户分组 (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      window: 1h
      # 注意：count操作不需要field字段，只计算符合条件的事件数量
      when:
        all:
          - type == "login"               # Database field (no prefix)
          - status == "failed"             # Database field (no prefix)
    ```

- `sum` - Sum numeric field values
  - *Example: 用户过去30天交易总额为 ¥15,000*
  - **实际应用场景**:
    - 洗钱检测：统计账户24小时内转账总金额，超过¥50万需人工审核
    - 信用额度管理：统计用户30天消费总额，判断是否超过信用额度
    - 积分欺诈：统计用户1小时内获取积分总数，异常高额可能是刷积分
  - **YAML示例**:
    ```yaml
    - name: sum_userid_txn_amt_24h
      type: aggregation
      method: sum
      datasource: postgresql_events
      entity: events
      dimension: user_id              # 按用户分组 (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: amount                   # 计算金额的总和 (SUM(amount))
      window: 24h
      when: type == "transaction"         # Database field (no prefix)
    ```

- `avg` - Average of field values
  - *Example: 用户过去7天平均每笔交易金额 ¥500*
  - **实际应用场景**:
    - 异常交易金额检测：用户平均交易¥500，突然出现¥50,000交易需验证
    - 用户画像：计算用户平均订单金额，用于用户分层（高/中/低消费）
    - 会话时长分析：统计用户平均会话时长，异常短可能是机器人
  - **YAML示例**:
    ```yaml
    - name: avg_userid_order_amt_30d
      type: aggregation
      method: avg
      datasource: postgresql_events
      entity: events
      dimension: user_id              # 按用户分组 (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: amount                   # 计算金额的平均值 (AVG(amount))
      window: 30d
      when: type == "order"               # Database field (no prefix)
    ```

- `max` - Maximum value
  - *Example: 用户过去24小时单笔最大交易 ¥2,000*
  - **实际应用场景**:
    - 大额交易监控：检测用户历史最大交易金额，当前交易超过3倍需验证
    - 单笔限额检查：新注册用户24小时内最大交易不超过¥5,000
    - 异常行为识别：IP地址关联的最大用户数超过50，可能是代理或公共WiFi
  - **YAML示例**:
    ```yaml
    - name: max_userid_txn_amt_90d
      type: aggregation
      method: max
      datasource: postgresql_events
      entity: events
      dimension: user_id              # 按用户分组 (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: amount                   # 计算金额的最大值 (MAX(amount))
      window: 90d
      when: type == "transaction"         # Database field (no prefix)
    ```

- `min` - Minimum value
  - *Example: 用户过去7天单笔最小交易 ¥10*
  - **实际应用场景**:
    - 测试交易检测：大量¥0.01小额交易可能是盗卡测试
    - 刷单识别：最小订单金额异常低（如¥0.1）配合高频次，疑似刷单
    - 异常折扣监控：订单最小金额为¥1，可能存在优惠券漏洞
  - **YAML示例**:
    ```yaml
    - name: min_userid_order_amt_7d
      type: aggregation
      method: min
      datasource: postgresql_events
      entity: events
      dimension: user_id              # 按用户分组 (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: amount                   # 计算金额的最小值 (MIN(amount))
      window: 7d
      when: type == "order"               # Database field (no prefix)
    ```

- `distinct` - Count unique values
  - *Example: 用户过去7天使用了3个不同设备*
  - **实际应用场景**:
    - 账号共享检测：用户24小时内使用超过5个不同设备，可能是账号被盗或共享
    - IP跳跃检测：用户1小时内使用超过10个不同IP，可能使用代理池
    - 多账户关联：同一设备24小时内登录超过20个不同账户，可能是批量操作
  - **YAML示例**:
    ```yaml
    - name: distinct_userid_device_24h
      type: aggregation
      method: distinct
      datasource: postgresql_events
      entity: events
      dimension: user_id              # 按用户分组 (GROUP BY user_id)
      dimension_value: "{event.user_id}"
      field: device_id                # 统计不同设备ID的数量 (COUNT(DISTINCT device_id))
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
  - *Example: 用户交易金额方差 122,500*
  - **实际应用场景**:
    - 风险评分：高方差用户风险更高，行为不可预测
    - 机器人检测：机器人交易方差通常很小（固定金额）
    - 信用评估：低方差用户还款行为更稳定，信用更好
  - **YAML示例**:
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
  - *Example: 用户交易金额P95为 ¥1,800*
  - **实际应用场景**:
    - 异常阈值设定：超过P95的交易需要额外验证
    - 动态限额：根据用户P90交易金额设置每日限额
    - 信用额度：用户P75消费金额作为信用额度参考
  - **YAML示例**:
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
  - *Example: 用户交易金额中位数 ¥450*
  - **实际应用场景**:
    - 抗异常值统计：中位数不受极端值影响，更准确反映用户典型行为
    - 用户画像：中位数订单金额用于用户价值评估
    - 异常检测：当前交易是中位数的10倍，需要验证
  - **YAML示例**:
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
  - *Example: 用户最常见的交易金额 ¥100*
  - **实际应用场景**:
    - 充值模式识别：用户最常充值¥100，异常充值¥10,000需验证
    - 刷单检测：大量相同金额订单（众数占比>80%）疑似刷单
    - 习惯识别：用户最常在晚上8点登录，凌晨3点登录异常
  - **YAML示例**:
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
  - *Example: 用户交易类型熵值2.3，行为多样化*
  - **实际应用场景**:
    - 机器人检测：熵值过低（<0.5），行为模式单一，可能是机器人
    - 账号活跃度：熵值高的用户行为丰富，更像真实用户
    - 异常检测：用户历史熵值2.5，近期降至0.3，行为异常单一
  - **YAML示例**:
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
    pub field: Option<String>,       // count不需要，其他需要
    pub window: Duration,
    pub when: Option<Condition>,
}

// ✅ 所有Aggregation操作符可以用统一的函数实现！
// 共同逻辑:
// - Time window filtering (window字段)
// - Dimension grouping (dimension, dimension_value)
// - Condition matching (when字段)
// - One-pass aggregation (不同的operator)
impl AggregationExecutor {
    fn execute(&self, op: AggregationType, config: &AggregationConfig) -> Result<Value> {
        // 1. 构建查询
        let sql = self.build_query(op, config)?;

        // 根据operator生成不同的SQL聚合函数:
        // COUNT(*), SUM(field), AVG(field), MAX(field), MIN(field),
        // COUNT(DISTINCT field), STDDEV(field), etc.

        // 2. 执行查询
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

## 所有Feature类型的DSL一致性分析

### 跨类型字段对比表

| 字段 | Aggregation | State | Sequence | Graph | Expression | Lookup | 说明 |
|------|-------------|-------|----------|-------|------------|--------|------|
| `type` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | 所有类型都需要 |
| `method` | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | Lookup不需要 |
| `datasource` | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ | Expression不需要 |
| `entity` | ✅ | ✅ | ✅ | ⚠️ | ❌ | ❌ | 指定表名/数据实体（见下方说明） |
| `dimension` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | 分组维度 |
| `dimension_value` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | 维度值 |
| `dimension_value2` | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | 第二个维度值（仅双节点Graph方法） |
| `dimension2` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | 第二维度（Graph关联维度） |
| `window` | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | 时间窗口 |
| `when` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | 条件过滤 |
| `field` | ⚠️ | ✅ | ⚠️ | ❌ | ❌ | ❌ | 计算字段 |

### `entity` 字段说明

**`entity` 的作用：指定从哪个表/数据实体中读取数据**

| Datasource类型 | 需要entity? | entity的含义 | 示例 |
|---------------|-----------|------------|------|
| **PostgreSQL** | ✅ 需要 | 表名 | `entity: events` → 查询 `events` 表 |
| **ClickHouse** | ✅ 需要 | 表名 | `entity: events` → 查询 `events` 表 |
| **Neo4j** | ⚠️ 取决于设计 | 节点标签或关系类型 | `entity: events` 或不需要 |
| **Redis** | ❌ 不需要 | N/A (key-value存储) | 直接通过key访问 |
| **Expression** | ❌ 不需要 | N/A (不访问数据源) | 只使用其他特征的结果 |

**SQL生成示例：**

```yaml
# PostgreSQL/ClickHouse
- name: cnt_userid_login_24h
  datasource: postgresql_events
  entity: events              # ← 指定表名
  dimension: user_id
  window: 24h
```

生成的SQL：
```sql
SELECT COUNT(*)
FROM events                   -- ← entity 映射到 FROM 子句
WHERE user_id = :current_user
  AND timestamp > now() - interval '24 hours'
```

**不同数据源的entity映射：**

```yaml
# 1. PostgreSQL - entity = 表名
datasource: postgresql_events
entity: events                # SELECT * FROM events

# 2. ClickHouse - entity = 表名
datasource: clickhouse_events
entity: events                # SELECT * FROM events

# 3. Neo4j - entity可能表示节点标签（需要设计决定）
datasource: neo4j_graph
entity: events                # MATCH (e:events) 或 MATCH ()-[r:events]->()
# 或者不使用entity，直接在查询逻辑中指定

# 4. Redis - 不需要entity
datasource: redis_features
# 没有entity字段，直接通过key访问

# 5. Expression - 不需要entity
type: expression
# 没有entity字段，不访问数据源
```

**设计建议：**

对于Neo4j图数据库，可以考虑两种方案：

**方案1：使用 `entity` 表示节点标签**
```yaml
datasource: neo4j_graph
entity: User                  # 节点标签
dimension: user_id
dimension2: device_id
```

**方案2：不使用 `entity`，在datasource配置中指定**
```yaml
datasource: neo4j_graph       # datasource配置中已指定要查询的节点/关系类型
dimension: user_id
dimension2: device_id
# 不需要entity字段
```

### 各类型特有字段

**State 特有:**
- `current_value` - 当前值（用于对比）

**Sequence 特有:**
- `reset_when` - 重置条件
- `order_by` - 排序字段
- `baseline_window` - 基线窗口（仅用于 anomaly_score 等需要历史对比的方法）
- `aggregation` - 窗口内的聚合方式
- `pattern` - 事件模式匹配
- `window_size` - 移动平均窗口大小

**Graph 特有:**
- `dimension2` - 第二维度（关联维度，如device_id关联到user_id）
- `dimension_value2` - 第二个维度值（仅用于需要两个节点的方法，如 shared_entity_count, network_distance）

**Expression 特有:**
- `expression` - 表达式字符串
- `depends_on` - 依赖的特征列表
- `model` / `inputs` / `output` - ML模型配置

**Lookup 特有:**
- `key` - Redis key模板
- `fallback` - 默认值

### 统一实现可行性分析

| 类型 | DSL一致性 | 可统一实现？ | 建议 |
|------|-----------|-------------|------|
| **Aggregation** | ✅ 高度一致 | ✅ 是 | 一个Executor处理所有method |
| **State** | ✅ 较一致 | ✅ 是 | 一个Executor处理所有method |
| **Sequence** | ⚠️ 中等 | ⚠️ 部分 | 简单的可统一，复杂的(pattern)需单独处理 |
| **Graph** | ⚠️ 字段差异 | ✅ 是 | 一个Executor，但字段名不同 |
| **Expression** | ✅ 简单一致 | ✅ 是 | 根据method分发：expression vs ml_model |
| **Lookup** | ✅ 最简单 | ✅ 是 | 直接key-value查询 |

### 实现建议

```rust
// 1. Aggregation - 高度统一 ✅
impl AggregationExecutor {
    fn execute(&self, method: AggregationType, config: AggregationConfig) -> Result<Value> {
        // 所有method共享：时间窗口、维度分组、条件过滤
        // 只有聚合函数不同：COUNT/SUM/AVG/MAX/MIN/DISTINCT...
    }
}

// 2. State - 较统一 ✅
impl StateExecutor {
    fn execute(&self, method: StateQueryType, config: StateConfig) -> Result<Value> {
        // 共享：维度、基线窗口
        // 差异：z_score需要current_value，timezone_consistency需要expected_timezone
    }
}

// 3. Sequence - 部分统一 ⚠️
impl SequenceExecutor {
    fn execute(&self, method: SequenceAnalysisType, config: SequenceConfig) -> Result<Value> {
        match method {
            ConsecutiveCount => { /* 简单，可统一 */ }
            PercentChange => { /* 需要双窗口，可统一 */ }
            MovingAverage => { /* 需要window_size，可统一 */ }
            EventPatternMatch => { /* 复杂，需要pattern匹配引擎 */ }
        }
    }
}

// 4. Graph - 可统一 ✅
impl GraphExecutor {
    fn execute(&self, method: GraphAnalysisType, config: GraphConfig) -> Result<Value> {
        // 使用统一的dimension/dimension_value字段
        // dimension2表示第二维度（关联维度）
    }
}

// 5. Expression - 简单统一 ✅
impl ExpressionExecutor {
    fn execute(&self, method: ExpressionType, config: ExpressionConfig) -> Result<Value> {
        match method {
            CustomExpression => { /* 表达式引擎 */ }
            MLModelScore => { /* 模型推理 */ }
        }
    }
}

// 6. Lookup - 最简单 ✅
impl LookupExecutor {
    fn execute(&self, config: LookupConfig) -> Result<Value> {
        // 直接Redis GET操作
        self.redis.get(&config.key).or(config.fallback)
    }
}
```

### 总结

✅ **所有类型都可以用统一的Executor实现**
- Aggregation, State, Expression, Lookup: 高度统一
- Graph: 高度统一（已使用一致的字段命名）
- Sequence: 简单method可统一，复杂的(pattern)需特殊处理

✅ **DSL命名一致性**
- 所有类型统一使用 `dimension` / `dimension_value` 作为主维度
- Graph类型使用 `dimension2` 表示第二维度（关联维度）
- 保持跨类型的字段命名一致性

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
  - *Example: 当前交易金额Z-score为2.8，异常偏高*
  - **实际应用场景**:
    - 异常交易检测：用户交易金额Z-score > 3，可能被盗刷
    - 登录频率异常：登录频率Z-score > 2.5，可能是暴力破解
    - 动态阈值：根据Z-score自动调整风控策略，而非固定阈值
  - **YAML示例**:
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
  - *Example: 当前登录频率比历史平均高150%*
  - **实际应用场景**:
    - 行为突变检测：用户日均登录2次，今天登录20次，偏离900%
    - 消费习惯变化：历史日均消费¥200，今天消费¥5000，偏离2400%
    - 账号接管：行为模式突然偏离基线，可能被他人控制
  - **YAML示例**:
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
  - *Example: 当前交易金额处于历史第92百分位*
  - **实际应用场景**:
    - 大额交易验证：当前交易金额超过历史P95，需要二次验证
    - 异常活跃度：当前登录频率超过历史P99，可能异常
    - 风险分级：P0-P50低风险，P50-P90中风险，P90+高风险
  - **YAML示例**:
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
  - *Example: 当前行为判定为统计异常值（true）*
  - **实际应用场景**:
    - 自动异常标记：统计学判断为异常值，直接触发人工审核
    - 欺诈检测：交易金额/频率/地点等多维度异常值检测
    - 机器学习特征：异常值标记作为ML模型输入特征
  - **YAML示例**:
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
  - *Example: 用户连续失败登录3次*
  - **实际应用场景**:
    - 暴力破解：连续失败登录≥5次，锁定账户15分钟
    - 支付失败：连续3次支付失败，可能卡被冻结或余额不足
    - 异常操作：连续10次快速点击，可能是脚本攻击
  - **YAML示例**:
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
  - *Example: 用户连续7天每天都有交易（活跃度高）*
  - **实际应用场景**:
    - 用户活跃度：连续活跃7天的用户，流失风险低
    - 刷单检测：连续30天每天都有订单，且金额相似，疑似刷单
    - 习惯养成：连续3天使用某功能，推荐相关服务
  - **YAML示例**:
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
  - *Example: 检测到"修改密码→登录→大额转账"可疑序列*
  - **实际应用场景**:
    - 账户接管：密码重置→修改邮箱→大额转账（15分钟内），高风险
    - 欺诈模式：注册→实名认证→申请贷款→提现（1小时内），疑似欺诈
    - 正常流程：浏览商品→加入购物车→结算→支付，转化漏斗分析
  - **YAML示例**:
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
  - *Example: "登录→浏览→加购→支付"完整路径出现5次*
  - **实际应用场景**:
    - 刷单检测：相同操作序列重复出现>10次，疑似刷单
    - 用户行为分析：高价值用户的典型路径频率
    - 异常模式：异常操作序列频繁出现，可能是攻击
  - **YAML示例**:
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
  - *Example: 用户交易金额呈上升趋势（+15%/周）*
  - **实际应用场景**:
    - 消费趋势：交易金额持续上升，用户价值增长
    - 风险趋势：失败交易比例上升趋势，可能卡出问题
    - 异常检测：登录频率突然上升趋势（斜率陡增），可能被盗号
  - **YAML示例**:
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
  - *Example: 本周交易次数比上周增加120%*
  - **实际应用场景**:
    - 行为突变：本周交易比上周增加500%，异常活跃
    - 促销效果：活动期间交易量增加200%，效果显著
    - 休眠唤醒：本周交易比上周增长从0到10，账户被重新激活
  - **YAML示例**:
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
  - **计算逻辑**:
    - 当前窗口：[now - 7d, now]
    - 基线窗口：[now - 14d, now - 7d]
    - 百分比变化 = (当前窗口值 - 基线窗口值) / 基线窗口值 × 100%

- `rate_of_change` - Rate of change over time
  - *Example: 用户登录频率增长率为+5次/天*
  - **实际应用场景**:
    - 加速度检测：交易频率增长率从1次/天加速到10次/天，异常
    - 渐进式攻击：失败登录率每小时增加2次，逐步升级攻击
    - 趋势预警：订单量下降率-3单/天，可能流失
  - **YAML示例**:
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
  - *Example: 序列异常评分8.5/10，高度可疑*
  - **实际应用场景**:
    - 综合异常检测：基于时序模型计算异常分数，>7分触发审核
    - 账户行为画像：行为序列与历史模式差异度评分
    - 欺诈概率：序列异常分数作为欺诈模型输入特征
  - **YAML示例**:
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
  - *Example: 用户7天移动平均交易额 ¥800/天*
  - **实际应用场景**:
    - 平滑趋势分析：7日移动平均消除日常波动，观察真实趋势
    - 异常检测：当前交易额超过7日移动平均3倍，异常
    - 动态基线：使用移动平均作为动态基线，自适应用户行为变化
  - **YAML示例**:
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

**字段语义说明：**

Graph类型分析二部图（Bipartite Graph）结构，其中：
- `dimension` - 主实体类型（如 user_id）
- `dimension2` - 关联实体类型（如 device_id）
- 形成图结构：User <--> Device <--> User <--> Device

**单节点方法**（如 graph_centrality, community_size）：
- `dimension_value` - 要分析的节点
- `dimension2` - 该节点如何与其他节点关联

**双节点方法**（如 shared_entity_count, network_distance）：
- `dimension_value` - 起点/源节点
- `dimension_value2` - 终点/目标节点（同一类型）
- `dimension2` - 两个节点通过什么建立连接（中间节点类型）

**Planned:**
- `graph_centrality` - Network centrality score
  - *Example: 设备在用户网络中心度0.65，可能是共享设备*
  - **实际应用场景**:
    - 核心节点识别：中心度>0.8的设备，可能是欺诈团伙核心设备
    - 风险源定位：高中心度账户被标记欺诈，关联账户需审查
    - 黑产识别：中心度异常高的IP，可能是黑产操作节点
  - **YAML示例**:
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
  - *Example: 该用户所在欺诈团伙社区规模23人*
  - **实际应用场景**:
    - 团伙欺诈：社区规模>20人且交易模式相似，疑似欺诈团伙
    - 洗钱网络：资金在大社区内循环流转，可能洗钱
    - 正常社交：小社区(<5人)且行为正常，可能是家庭/朋友
  - **YAML示例**:
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
  - *Example: 两个用户共享5个相同设备*
  - **实际应用场景**:
    - 虚假账户：两个账户共享>3个设备，可能是同一人多账户
    - 关联欺诈：多个高风险账户共享设备/IP，协同欺诈
    - 家庭识别：共享2个设备(手机+电脑)，可能是家庭成员
  - **YAML示例**:
    ```yaml
    - name: shared_devices_between_users
      type: graph
      method: shared_entity_count
      datasource: neo4j_graph
      dimension: user_id                      # 节点类型
      dimension_value: "{event.user_id}"      # 节点1（源）
      dimension_value2: "{event.target_user_id}"  # 节点2（目标）
      dimension2: device_id                   # 共享实体类型
      window: 30d
    ```
  - **字段说明**:
    - `dimension: user_id` - 主实体类型（两个节点都是User）
    - `dimension_value` - User1
    - `dimension_value2` - User2
    - `dimension2: device_id` - **共享的中间节点类型**
    - 计算结果：User1和User2共享多少个Device
  - **图结构示例**:
    ```
    User1 --使用--> Device1 <--使用-- User2
          --使用--> Device2 <--使用--
          --使用--> Device3 <--使用--

    结果 = 3（共享3个设备）
    ```
    - User1和User2都是 `user_id` 类型（dimension）
    - Device1/2/3是 `device_id` 类型（dimension2，共享节点）
    - 查找：有多少个Device同时连接到User1和User2

- `network_distance` - Distance between entities in graph
  - *Example: 两个账户的网络距离为3跳（间接关联）*
  - **实际应用场景**:
    - 风险传播：距离已知欺诈账户≤2跳，需要审查
    - 关联分析：虽无直接关联，但网络距离≤3跳，间接关联
    - 社交推荐：网络距离2-3跳的用户，可能有共同兴趣
  - **YAML示例**:
    ```yaml
    - name: network_dist_to_fraud_account
      type: graph
      method: network_distance
      datasource: neo4j_graph
      dimension: user_id                      # 节点类型
      dimension_value: "{event.user_id}"      # 起点节点
      dimension_value2: "{known_fraud_user_id}"   # 终点节点
      dimension2: device_id                   # 连接路径
      window: 90d
    ```
  - **字段说明**:
    - `dimension: user_id` - 主实体类型（起点和终点都是User）
    - `dimension_value` - 起点User
    - `dimension_value2` - 终点User
    - `dimension2: device_id` - **中间连接节点类型**（不是终点类型！）
    - 计算结果：从起点到终点的最短跳数
  - **图结构示例**:
    ```
    UserA --使用--> Device1 <--使用-- UserC --使用--> Device2 <--使用-- UserB
    起点                                                              终点

    跳数 = 2跳（UserA -> Device1 -> UserC -> Device2 -> UserB）
    ```
    - UserA和UserB都是 `user_id` 类型（dimension）
    - Device1和Device2是 `device_id` 类型（dimension2，中间连接）
    - 图遍历：User -> Device -> User -> Device -> User

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
  - *Example: 计算登录失败率 = failed_count / total_count*
  - **实际应用场景**:
    - 失败率计算：login_failure_rate = failed_login_count_1h / login_count_1h
    - 复合评分：risk_score = 0.4 * transaction_anomaly + 0.3 * device_risk + 0.3 * location_risk
    - 比率分析：large_transaction_ratio = transactions_above_1000 / total_transactions
    - 转化率：conversion_rate = purchase_count / view_count
  - **YAML示例**:
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
  - *Example: 从Redis查询用户90天风险评分*
  - **实际应用场景**:
    - 批量计算的风险评分：每天凌晨批量计算用户风险分数，存储在Redis
    - 用户细分标签：数据分析团队生成的用户分群标签，缓存在Redis
    - 设备指纹：安全团队维护的设备信誉库，存储在Redis
    - 缓存的聚合特征：定时任务预计算的聚合指标，加速实时查询
  - **YAML示例**:
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
- [1. Aggregation (数东西)](#1-aggregation-数东西)
- [2. State (看最近状态)](#2-state-看最近状态)
- [3. Sequence (看过程)](#3-sequence-看过程)
- [4. Graph (看关系图)](#4-graph-看关系图)
- [5. Expression (算分数)](#5-expression-算分数)
- [6. Lookup (查预算值)](#6-lookup-查预算值)
- [Implementation Roadmap](#implementation-roadmap)
- [By Risk Domain](#by-risk-domain)

---

## 1. Aggregation (数东西)

**Purpose:** Count events, aggregate values, and compute statistical measures.

### Key Concepts

**Understanding `dimension` vs `field`:**

- **`dimension`** - 聚合的**分组维度**（GROUP BY）
  - 表示"按什么来分组"
  - 例如: `dimension: user_id` 表示按用户分组
  - 相当于SQL中的 `GROUP BY user_id`

- **`field`** - 聚合的**计算字段**（聚合函数作用的字段）
  - 表示"对哪个字段进行计算"
  - 例如: `field: amount` 表示对金额字段进行聚合
  - 相当于SQL中的 `AVG(amount)`, `SUM(amount)` 等

**示例理解:**
```yaml
- name: avg_userid_order_amt_30d
  dimension: user_id    # 按用户分组
  field: amount         # 计算金额的平均值
```
相当于SQL:
```sql
SELECT user_id, AVG(amount)
FROM events
WHERE type='order' AND timestamp > now() - 30d
GROUP BY user_id
```

**什么时候需要`field`:**
- `count` - ❌ 不需要（只计数，不关心具体字段值）
- `sum`, `avg`, `max`, `min`, `stddev` - ✅ 需要（必须指定对哪个字段进行计算）
- `distinct` - ✅ 需要（统计某字段的不同值数量）

### `when` 条件中的字段引用语法

在 `when` 条件中过滤数据库行时，可以引用两种类型的字段：

**1. 数据库字段（来自 entity 指定的数据表）**
- 不需要前缀，直接引用列名
- 示例：`type`, `status`, `amount`, `country`
- 支持 JSON 嵌套字段：`attributes.device.fingerprint`, `metadata.user.tier`

**2. 请求字段（来自 API 请求的 context.event）**
- 使用模板语法加花括号：`{event.field_name}`
- 示例：`{event.user_id}`, `{event.min_amount}`, `{event.threshold}`
- 用于动态过滤和模板替换

**示例：**

```yaml
# 数据库字段过滤（无需前缀）
when: type == "transaction"

# 数据库 JSON 嵌套字段访问
when: attributes.risk_level == "high"

# 组合数据库字段和请求字段
when:
  all:
    - type == "payment"                      # 数据库字段
    - amount > {event.threshold}             # 请求字段（动态值）
    - metadata.country == "{event.country}"  # 数据库 JSON 字段匹配请求值

# 复杂的 JSON 嵌套字段
when: user.profile.verification_status == "verified"
```

**SQL 生成示例：**

配置：
```yaml
when:
  all:
    - type == "transaction"
    - amount > {event.min_amount}
    - attributes.device_type == "mobile"
```

生成的 SQL：
```sql
SELECT COUNT(*)
FROM events
WHERE user_id = $1
  AND event_timestamp >= NOW() - INTERVAL '24 hours'
  AND type = 'transaction'                           -- 数据库字段
  AND amount > $2                                     -- 请求值替换
  AND attributes->>'device_type' = 'mobile'          -- JSON 字段访问
```

### `dimension` vs `when` - 不能互相替代

**重要概念区分:**

| 字段 | 作用 | SQL等价 | 目的 |
|------|------|---------|------|
| `dimension` | 分组维度 | `GROUP BY` | 决定"**为谁**计算"（分组） |
| `when` | 过滤条件 | `WHERE` | 决定"**计算什么**"（过滤事件） |
| `field` | 计算字段 | `SUM(field)` | 决定"**计算哪个字段**" |

**示例对比:**

```yaml
# 正确：使用dimension进行分组
- name: cnt_userid_login_24h
  dimension: user_id              # 为每个用户分别计算
  when: type == "login"           # 只统计登录事件（数据库字段，无需前缀）
```

SQL等价:
```sql
SELECT user_id, COUNT(*)
FROM events
WHERE type = 'login' AND timestamp > now() - 24h
GROUP BY user_id               -- dimension的作用
-- 结果：每个用户都有自己的登录次数
```

**如果去掉dimension会怎样？**

```yaml
# 错误：缺少dimension，没有分组
- name: cnt_all_login_24h
  when: type == "login"           # 数据库字段，无需前缀
  # 没有dimension = 没有分组
```

SQL等价:
```sql
SELECT COUNT(*)
FROM events
WHERE type = 'login' AND timestamp > now() - 24h
-- 没有 GROUP BY
-- 结果：所有用户的登录次数加在一起，只有一个总数！
```

**dimension 和 when 的配合使用:**

```yaml
- name: cnt_userid_txn_24h_large
  dimension: user_id              # 按用户分组
  when:
    all:
      - type == "transaction"        # 过滤：只要交易事件（数据库字段，无需前缀）
      - amount > 1000                # 过滤：金额大于1000（数据库字段，无需前缀）
```

SQL等价:
```sql
SELECT user_id, COUNT(*)
FROM events
WHERE type = 'transaction'          -- when条件1（数据库字段）
  AND amount > 1000                 -- when条件2（数据库字段）
  AND timestamp > now() - 24h
GROUP BY user_id                    -- dimension
-- 结果：每个用户的大额交易（>1000）次数
```

**总结:**
- ✅ `dimension` 创建分组，每组一个结果值
- ✅ `when` 过滤事件，只有符合条件的事件参与计算
- ❌ **不能**用 `when` 替代 `dimension`，它们的作用完全不同！

### 为什么 `dimension_value` 不能用 `when` 替代？

**常见误解:** "既然 `dimension_value: "{event.user_id}"` 也是从请求事件中取值，为什么不能用 `when: user_id == ...` 替代？"

**关键区别:**

| 方式 | 性质 | 结果数量 | 适用场景 |
|------|------|---------|---------|
| `dimension + dimension_value` | **动态分组** | 为数据中**每个不同的值**都计算一个结果 | 为所有用户计算 |
| `when` 条件 | **静态过滤** | 只能针对**一个写死的值**计算 | 只为特定用户计算 |

**示例对比:**

```yaml
# 方式1: 使用 dimension（正确） - 动态分组
- name: cnt_userid_login_24h
  dimension: user_id
  dimension_value: "{event.user_id}"    # 动态提取每个事件的user_id
  when: type == "login"               # Database field (no prefix)
```

**执行逻辑:**
```
事件流:
  event1: {user_id: "user_A", type: "login"}  → 归入 user_A 组
  event2: {user_id: "user_B", type: "login"}  → 归入 user_B 组
  event3: {user_id: "user_A", type: "login"}  → 归入 user_A 组
  event4: {user_id: "user_C", type: "login"}  → 归入 user_C 组

结果（为每个用户计算）:
  user_A: 2次
  user_B: 1次
  user_C: 1次
  ... （系统中所有有登录的用户）
```

```yaml
# 方式2: 用 when 条件（错误） - 静态过滤
- name: cnt_login_24h_for_userA
  when:
    all:
      - type == "login"              # 数据库字段
      - user_id == "user_A"          # 写死了只看user_A！
```

**执行逻辑:**
```
事件流:
  event1: {user_id: "user_A", type: "login"}  → ✅ 计入
  event2: {user_id: "user_B", type: "login"}  → ❌ 过滤掉
  event3: {user_id: "user_A", type: "login"}  → ✅ 计入
  event4: {user_id: "user_C", type: "login"}  → ❌ 过滤掉

结果（只有一个值）:
  总计: 2次  （只有user_A的登录次数，其他用户全部丢失！）
```

**实际运行时的区别:**

当风控引擎评估一个用户（比如 user_B）的交易时：

```yaml
# 使用 dimension（正确）
dimension: user_id
dimension_value: "{event.user_id}"  # 运行时自动替换为 "user_B"

→ 查询: SELECT COUNT(*) WHERE user_id = 'user_B' AND ...
→ 返回: user_B 的登录次数
```

```yaml
# 使用 when（错误）
when: user_id == "user_A"           # 写死了！

→ 查询: SELECT COUNT(*) WHERE user_id = 'user_A' AND ...
→ 返回: user_A 的登录次数（错误！我们要的是 user_B 的数据）
```

**`dimension_value` 的真正作用:**

`dimension_value` 是一个**模板表达式**，在运行时会被替换：
- 配置时写: `dimension_value: "{event.user_id}"`
- 运行时评估当前事件，自动变成: `dimension_value: "user_123"` (当前用户)
- 然后去查询这个用户的历史数据，按 user_id 分组聚合

**关键理解:**
- ✅ `dimension_value` 是**分组依据**，告诉系统"从当前事件中提取哪个值作为分组key"
- ❌ `when` 是**固定的过滤条件**，只能写死一个具体值，无法动态适应不同用户
- 💡 你需要为**每个用户**都计算独立的统计值，必须用 `dimension`，不能用 `when`！

### 实时风控场景下 `dimension` 的真正作用

**你的疑问可能是：** "在实时决策时，只针对当前用户计算，为什么不能直接用 `when` 条件过滤 `user_id`？"

**答案：技术上可以，但设计上不应该。** 原因如下：

#### 1. **实时计算时的实际查询**

当评估 user_B 的交易时，实际的 SQL 查询：

```sql
-- 使用 dimension 的方式
SELECT COUNT(*)
FROM events
WHERE user_id = 'user_B'      -- 来自 dimension_value
  AND type = 'login'          -- 来自 when 条件
  AND timestamp > now() - 24h
```

你说得对！在这个场景下，`user_id = 'user_B'` 确实是一个 WHERE 条件，**理论上可以放在 `when` 中**。

#### 2. **为什么还要用 `dimension` 而不是 `when`？**

**原因一：语义清晰**

```yaml
# 方式1: 使用 dimension（推荐） - 语义明确
dimension: user_id              # 明确：这是"为谁"计算的维度
dimension_value: "{event.user_id}"
when: type == "login"           # 明确：这是过滤条件（数据库字段，无需前缀）

# 方式2: 全部放在 when（不推荐） - 语义混乱
when:
  all:
    - user_id == "{event.user_id}"     # 这不是过滤，是指定查询主体
    - type == "login"                  # 这才是过滤（数据库字段）
```

**原因二：支持多种计算模式**

系统需要支持两种模式：

| 模式 | 场景 | SQL | 需要 GROUP BY？ |
|------|------|-----|----------------|
| **在线模式** | 实时决策单个事件 | `WHERE user_id = :current_user` | ❌ 不需要 |
| **离线模式** | 批量预计算特征 | `GROUP BY user_id` | ✅ 需要 |

使用 `dimension` 的好处：**同一个配置可以用于两种模式**

```yaml
# 同一份配置
dimension: user_id
dimension_value: "{event.user_id}"
when: type == "login"              # 数据库字段，无需前缀

# 在线模式执行时:
SELECT COUNT(*) WHERE user_id = 'user_B' AND type = 'login'

# 离线批量计算时:
SELECT user_id, COUNT(*) WHERE type = 'login' GROUP BY user_id
```

**原因三：查询优化**

系统可以识别 `dimension` 是分组维度，做针对性优化：
- 自动选择合适的索引
- 识别分区键（如果数据库按 user_id 分区）
- 并行计算优化

如果全部放在 `when` 中，系统无法区分哪个条件是"分组维度"，哪个是"过滤条件"。

**原因四：配置复用**

```yaml
# 使用 dimension - 可以轻松改变维度
dimension: user_id       # 按用户
# dimension: device_id   # 改成按设备，只改一行
# dimension: ip_address  # 改成按IP，只改一行

# 使用 when - 要改多处
when:
  all:
    - user_id == "{event.user_id}"        # 要改这里
    - type == "login"                     # 数据库字段
```

#### 总结：设计原则

| 字段 | 语义 | 作用 |
|------|------|------|
| `dimension` | "**为谁**计算"（查询主体） | 指定聚合维度，支持在线/离线两种模式 |
| `when` | "**什么样的**事件参与计算"（业务过滤） | 过滤符合业务条件的事件 |

**虽然在纯在线场景下，技术上可以把 dimension_value 放进 when，但为了：**
- ✅ 语义清晰（明确区分"为谁"和"什么"）
- ✅ 支持离线批量计算
- ✅ 便于系统优化
- ✅ 配置更容易维护和复用

**我们仍然建议使用 `dimension` 字段。**

### DSL一致性分析 ✅

**结论：所有Aggregation操作符的DSL结构高度一致，可以用统一的Rust函数实现！**

| 字段 | count | sum | avg | max | min | distinct | stddev | 一致性 |
|------|-------|-----|-----|-----|-----|----------|--------|--------|
| `type` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `operator` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `datasource` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `entity` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `dimension` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `dimension_value` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `window` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `when` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ 100% |
| `field` | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ 仅count不需要 |

**实现建议：**

```rust
// ✅ 统一的配置结构
struct AggregationConfig {
    datasource: String,
    entity: String,
    dimension: String,
    dimension_value: String,
    field: Option<String>,      // count时为None，其他为Some(field_name)
    window: Duration,
    when: Option<Condition>,
}

// ✅ 统一的执行器
impl AggregationExecutor {
    // 一个函数处理所有operator！
    fn execute(&self, op: AggregationType, config: &AggregationConfig) -> Result<Value> {
        // 唯一的区别是生成的SQL聚合函数不同：
        // COUNT(*) vs SUM(field) vs AVG(field) vs MAX(field) ...
        let sql = self.build_query(op, config)?;
        self.datasource.query(&sql)
    }
}
```

**优势：**
- ✅ 代码复用率高（共享时间窗口、维度分组、条件过滤逻辑）
- ✅ 易于扩展（添加新operator只需在enum中增加variant）
- ✅ 统一测试（一套测试框架覆盖所有operator）
- ✅ 配置一致（用户学习一次就能使用所有operator）

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

## 4. Graph (看关系图)

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

## 6. Lookup (查预算值)

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
- name: sum_userid_txn_amt_24h     # name 使用缩写
  field: amount                    # field 使用完整词
  when: type == "transaction"      # when 使用完整词（数据库字段，无需前缀）

# ❌ Wrong - don't use abbreviations in config
- name: sum_userid_txn_amt_24h
  field: amt                       # ❌ 错误（不要使用缩写）
  when: type == "txn"              # ❌ 错误（不要使用缩写）
```

### Examples

**Aggregation Features:**

```yaml
# Basic counting
cnt_userid_login_24h               # 用户24小时登录次数
cnt_userid_txn_7d                  # 用户7天交易次数
cnt_deviceid_login_1h              # 设备1小时登录次数

# Sum/Avg with field
sum_userid_txn_amt_30d             # 用户30天交易金额总和
avg_userid_pay_amt_7d              # 用户7天支付平均金额
max_userid_txn_amt_24h             # 用户24小时最大交易金额

# Distinct counting
distinct_userid_device_7d          # 用户7天内不同设备数
distinct_userid_ip_24h             # 用户24小时内不同IP数
distinct_ip_userid_1h              # IP 1小时内不同用户数

# With modifier for conditions
cnt_userid_login_1h_failed         # 用户1小时失败登录次数
cnt_userid_pay_24h_success         # 用户24小时成功支付次数
```

**State Features:**

```yaml
# Statistical comparison (planned)
zscore_userid_txn_amt              # 用户交易金额Z-score
deviation_userid_login_freq        # 用户登录频率偏离度
percentile_userid_txn_amt          # 用户交易金额百分位
timezone_userid_login_7d           # 用户时区一致性检测

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
consec_userid_login_1h_failed      # 用户1小时连续失败登录次数
streak_userid_txn_7d               # 用户7天交易连续性

# Trend detection
pctchg_userid_txn_amt              # 用户交易金额变化百分比
trend_userid_login_7d              # 用户7天登录趋势

# Session analysis
avg_userid_sess_dur_7d             # 用户7天平均会话时长
```

**Graph Features:**

```yaml
# Entity linking - use distinct (not Graph operators)
distinct_ip_device_24h             # IP 24小时关联设备数（用 distinct）
distinct_deviceid_userid_7d        # 设备7天关联用户数（用 distinct）

# Network analysis (planned)
centrality_userid_device_30d       # 用户30天设备网络中心度
community_userid_network_30d       # 用户30天所在社区大小
shared_userid_device_30d           # 用户间共享设备数
```

**Expression Features:**

```yaml
# Computed scores
score_userid_fraud                     # 用户欺诈评分
score_userid_risk                      # 用户风险评分

# Ratio/Rate (complex expressions)
rate_userid_login_1h_failure           # 用户1小时登录失败率
ratio_userid_txn_7d_change             # 用户7天交易比率变化
```

**Lookup Features:**

```yaml
# Pre-computed values (no operator, descriptive names)
user_risk_score_90d                    # 用户90天风险评分（预计算）
user_segment                           # 用户细分标签
device_reputation_score                # 设备信誉评分
ip_risk_level                          # IP风险等级

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
