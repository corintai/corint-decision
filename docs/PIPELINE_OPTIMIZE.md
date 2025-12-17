# Pipeline DSL 设计优化建议

本文档基于 `mypipeline.yml` 的实际设计，提出 Pipeline DSL 的优化方向和最佳实践。

**参考示例**: `docs/mypipeline.yml` 展示了完整的 Pipeline 设计模式，包括显式入口、路由条件、步骤类型等核心特性。

---

## 一、当前设计优点总结

### 1.1 架构层面

| 优点 | 说明 | 示例位置 |
|------|------|----------|
| Pipeline = Orchestration | 明确定位为编排器，而非规则容器 | `mypipeline.yml:116-325` |
| 显式入口点 (entry) | DAG 编译友好，支持可视化 | `mypipeline.yml:123` |
| Step 类型清晰划分 | router/api/service/ruleset/pipeline/trigger | `mypipeline.yml:132-325` |
| 条件路由格式统一 | routes + when + default，与 registry 一致 | `mypipeline.yml:145-158, 169-181` |
| Convention over Configuration | api.<name>, service.<name>, function.<name> | `mypipeline.yml:36-43, 144` |
| 无 loop 设计 | 避免风控场景中的潜在灾难 | - |

### 1.2 DSL 层面

- **显式路由**: 使用 `router` 步骤类型，纯路由判断，无副作用 (`mypipeline.yml:162-181`)
- **统一条件格式**: `when` 支持 `all/any/not` 任意嵌套 (`mypipeline.yml:60-66`)
- **Step ID 唯一性**: 编译器强制校验，支持 DAG 分析 (`mypipeline.yml:82-88`)
- **终端节点清晰**: `next: end` 明确流程结束点 (`mypipeline.yml:281, 295, 309`)

---

## 二、核心设计原则

### 2.1 关注点分离 (Separation of Concerns)

**核心原则**: Pipeline 只负责"调用什么"和"何时调用"，不关心"如何调用"。

```
┌─────────────────────────────────────────────────────────────┐
│                     Pipeline (编排层)                        │
│  - 定义执行顺序和流程控制                                      │
│  - 引用 API/Service (只需 name/id)                           │
│  - 条件路由和分支逻辑                                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                repository/configs/ (配置层)                   │
│  ├── apis/          # 外部 API 配置 (timeout, retry, etc.)   │
│  ├── services/      # 内部服务配置 (connection, queries)      │
│  ├── datasources/   # 数据源配置                              │
│  └── features/      # 特征计算配置                            │
└─────────────────────────────────────────────────────────────┘
```

**在 Pipeline 中的体现** (`mypipeline.yml:136-144`):
```yaml
- step:
    id: get_ip_info
    name: Get IP Geolocation
    type: api
    api: ip_geolocation           # 只引用名称，不定义实现
    params:
      ip: event.ip_address
    # output: api.ip_geolocation (约定自动存储)
```

**在 configs/ 中定义实现**:
```yaml
# repository/configs/apis/ip_geolocation.yaml
name: ip_geolocation
base_url: "https://api.ipgeolocation.io"
timeout: 3s
retry:
  max_attempts: 3
  backoff: exponential
  on: [timeout, 5xx]
on_error:
  action: fallback
  default_value:
    country_code: "unknown"
    risk_level: "medium"
```

### 2.2 Step 类型设计

根据 `mypipeline.yml:45-53` 的定义:

| 类型 | 职责 | 输出位置 | 示例 |
|------|------|----------|------|
| router | 纯路由判断 (无计算) | 无 | `mypipeline.yml:162-181` |
| function | 内部纯函数计算 | `function.<name>` | 特征工程、计算衍生值 |
| rule/ruleset | 执行业务规则 | 影响 score/decision | `mypipeline.yml:186-192` |
| pipeline | 调用子流程 | 合并到主流程 context | `mypipeline.yml:208-214` |
| service | 内部服务调用 | `service.<name>` | DB, Redis 查询 |
| api | 外部 API 调用 | `api.<name>` | `mypipeline.yml:136-144` |
| trigger | 触发外部动作 | 无 | `mypipeline.yml:273-281` |

**关键区别**:
- **router**: 纯路由，使用 `routes` 条件分发，**不执行任何计算或 I/O**
- **function**: 有计算，无 I/O，结果供后续步骤复用
- **service/api**: 有 I/O，实现细节在 configs/ 中定义

### 2.3 路由条件格式 (与 Registry 一致)

**格式**: 目标在外，条件在里 (`mypipeline.yml:68-79`)

```yaml
routes:
  # 复杂条件: 高价值 + VIP 用户
  - next: vip_fast_track
    when:
      all:
        - event.amount > 10000
        - event.user.vip_status == true

  # 简单条件
  - next: high_value_flow
    when:
      all:
        - event.amount > 1000

default: standard_scoring
```

**支持任意嵌套** (`mypipeline.yml:486-497`):
```yaml
routes:
  - next: high_risk_review
    when:
      any:
        - all:
            - event.amount >= 3000
            - geo.country in ["NG", "PK", "UA"]
        - all:
            - device.is_emulator == true
            - risk.login_fail_count_1h >= 3
```

---

## 三、优化建议

### 3.1 A/B 测试和流量分配 (P1 - 重要)

**设计原则**: 使用简单的 hash 函数 + when 条件实现，无需复杂的实验框架。

**方案 1: Registry 层流量分配**

```yaml
# registry.yaml
registry:
  - pipeline: fraud_detection_v1
    when:
      all:
        - event.type == "payment"
        - hash(event.user.id) % 100 < 90  # 90% 流量

  - pipeline: fraud_detection_v2
    when:
      all:
        - event.type == "payment"
        - hash(event.user.id) % 100 >= 90  # 10% 流量
```

**方案 2: Pipeline 内分支 (使用 function step)**

```yaml
steps:
  # 计算实验分组
  - step:
      id: calc_variant
      type: function
      function: hash_mod
      params:
        key: event.user.id
        mod: 100
      # output: function.hash_mod

  # 路由到不同变体
  - step:
      id: route_variant
      type: router
      routes:
        - next: variant_a
          when:
            all:
              - function.hash_mod < 50  # 50% 流量

        - next: variant_b
          when:
            all:
              - function.hash_mod >= 50  # 50% 流量
```

**收益**:
- 简单直接，无需额外基础设施
- 基于用户 ID 的一致性分流
- 易于理解和调试

### 3.2 Shadow Mode / Observe Mode (P1 - 重要)

**问题**: 新版本 Pipeline 需要在不影响业务决策的情况下验证效果。

**建议语法**:
```yaml
pipeline:
  id: fraud_detection_v2
  mode: shadow                     # decision | shadow
  # decision: 影响业务决策（默认）
  # shadow: 完整执行并记录，但不影响决策

  # 可选：流量采样
  traffic:
    percentage: 10                 # 10% 流量
    hash_key: event.user.id        # 分流依据
```

**实现方式**:
- 运行时标记: `context._mode = "shadow"`
- 执行所有步骤，但不更新 `sys.decision`
- 完整记录 trace 用于对比分析

### 3.3 条件语义分层 (P1 - 文档化)

**问题**: 当前存在多层条件 (registry.when / pipeline.when / routes / step.when)，需明确职责。

**推荐语义划分**:

| 层级 | 职责 | 示例 | 位置 |
|------|------|------|------|
| Registry.when | 粗粒度路由 (事件类型 → pipeline) | `event.type == "payment"` | registry.yaml |
| Pipeline.when | 细分路由 (国家/实验/shadow) | `event.country in ["US", "UK"]` | `mypipeline.yml:126-129` |
| Router.routes | 流程分支控制 | `amount > 1000` | `mypipeline.yml:169-181` |
| Step.when | 步骤级门控 (成本控制) | `context.score > 30` | 步骤可选 when 字段 |

**文档建议**: 在规范中明确声明各层职责，避免混用。

### 3.4 成本控制和 Early Exit (P1 - 已支持)

**场景**: 成本敏感场景需要在特定条件下跳过后续昂贵的步骤。

**实现方式**: 使用 step-level `when` 条件 (可选字段)

```yaml
pipeline:
  id: credit_application

  steps:
    # 第一阶段: 基于本地规则的快速评估
    - step:
        id: basic_scoring
        type: ruleset
        ruleset: basic_risk_scoring
        next: credit_check

    # 成本控制: 只有中等风险以上才调用付费 API
    - step:
        id: credit_check
        type: api
        api: credit_bureau           # 每次调用 $0.5
        when:                         # 步骤级条件 (可选)
          all:
            - context.basic_score > 30
        next: detailed_scoring

    # 后续详细评估
    - step:
        id: detailed_scoring
        type: ruleset
        ruleset: detailed_scoring
        when:
          all:
            - context.decision != "approve"
        next: end
```

**与 Router 的区别**:
- **Router**: 互斥分支，选择不同路径
- **Step.when**: 跳过当前步骤，继续执行 next 指向的步骤

### 3.5 Execution Mode: any / all (已实现)

根据 `mypipeline.yml:354-384`，已支持 api/service 的并发执行模式:

**any 模式 - 降级/备选** (顺序尝试，首个成功即返回):
```yaml
- step:
    id: get_ip_info
    type: api
    any:
      - maxmind_api         # 首选
      - ipinfo_api          # 备选 1
      - ip2location_api     # 备选 2
    params:
      ip: event.ip_address
    next: next_step
```

**all 模式 - 聚合** (并行执行，等待全部完成):
```yaml
- step:
    id: fetch_external_data
    type: api
    all:
      - credit_bureau_api
      - fraud_detection_api
      - identity_verification_api
    timeout: 5s              # 总超时时间 (Pipeline 关注点)
    on_error: partial        # partial | fail_fast (Pipeline 关注点)
    min_success: 2           # 最少成功数 (Pipeline 关注点)
    next: scoring
```

**说明**:
- 单个 API 的 timeout/retry 在 `configs/apis/` 中定义
- 编排层的 timeout/on_error 在 Pipeline 中定义

---

## 四、API/Service 配置层设计

### 4.1 API 配置示例

```yaml
# repository/configs/apis/credit_bureau.yaml
name: credit_bureau
base_url: "https://api.creditbureau.com"

# 单次调用配置
timeout: 5s
cost: 0.5                          # 单次成本 (用于报表和成本追踪)

# 容错配置
retry:
  max_attempts: 2
  backoff: exponential             # fixed | linear | exponential
  on: [timeout, 5xx]               # 重试条件

on_error:
  action: fallback                 # skip | fail | fallback
  default_value:
    credit_score: null
    available: false

# 端点定义
endpoints:
  get_credit_score:
    method: POST
    path: "/v1/credit-score"
    headers:
      Authorization: "Bearer ${CREDIT_API_KEY}"
    response_mapping:
      credit_score: response.score
      report_date: response.date
```

**在 Pipeline 中引用**:
```yaml
- step:
    id: check_credit
    type: api
    api: credit_bureau
    endpoint: get_credit_score
    params:
      user_id: event.user.id
    # output: api.credit_bureau (约定)
    next: scoring
```

### 4.2 Service 配置示例

```yaml
# repository/configs/services/user_db.yaml
name: user_db
type: postgres
connection: ${DATABASE_URL}

# 查询定义
queries:
  get_user_profile:
    sql: "SELECT name, age, risk_level FROM users WHERE id = $1"
    params:
      - user_id: string
    output:
      schema:
        type: object
        properties:
          name: string
          age: integer
          risk_level: string
    timeout: 1s
    cache:
      ttl: 300s
      key: "user:${user_id}"
```

**在 Pipeline 中引用**:
```yaml
- step:
    id: fetch_user
    type: service
    service: user_db
    query: get_user_profile
    params:
      user_id: event.user.id
    # output: service.user_db (约定)
    next: check_rules
```

**收益**:
- 关注点分离：配置集中管理，Pipeline 保持简洁
- 可复用性：同一配置可被多个 Pipeline 引用
- 可测试性：配置可独立测试和 mock
- 安全性：敏感配置 (API key, DB password) 与业务逻辑分离

---

## 五、调试与可观测性

### 5.1 执行追踪 (已实现)

**请求级别** (已支持):
```json
{
  "event": {...},
  "enable_trace": true
}
```

**Pipeline 级别** (建议):
```yaml
pipeline:
  id: payment_risk_pipeline
  trace:
    enabled: true                  # Pipeline 级别启用追踪
    sample_rate: 0.1               # 10% 采样（避免性能影响）
    include_context: true          # 包含上下文快照
```

### 5.2 调试节点 (P2 - 开发体验)

```yaml
steps:
  - step:
      id: debug_score
      type: log
      level: debug                   # debug | info | warn
      message: "Risk score calculated"
      data:
        score: context.risk_score
        user_id: event.user.id
      when:
        all:
          - config.debug_mode == true
      next: next_step
```

### 5.3 断点支持 (P3 - 高级调试)

```yaml
steps:
  - step:
      id: inspect_context
      type: breakpoint
      enabled: ${DEBUG}              # 环境变量控制
      next: next_step
```

---

## 六、复杂度控制

### 6.1 Pipeline 复杂度限制

**建议在规范中声明软限制**:

| 指标 | 推荐上限 | 说明 |
|------|----------|------|
| steps 数量 | ≤ 30 | 超过应拆分子流程 |
| routes 分支数 | ≤ 5 | 避免过度复杂 |
| nesting depth | ≤ 3 | all/any/not 嵌套层级 |
| include depth | ≤ 3 | 子流程调用深度 |
| 总节点数 | ≤ 50 | 编译后 DAG 节点 |

### 6.2 禁止的模式

在规范中明确禁止:
- Pipeline 内定义 API/Service 实现细节 (timeout/retry 等应在 `repository/configs/` 中定义)
- Pipeline 内再声明 imports (只能在文件顶层)
- 循环引用 (A include B, B include A)
- Router step 中包含计算逻辑 (router 只做路由判断)

**设计原则**: Pipeline = 编排层，只负责"调用什么"和"何时调用"。

---

## 七、场景适配最佳实践

### 7.1 反欺诈场景 (高并发、低延迟)

**需求**: Scatter-Gather 模式，并行调用多个服务

**使用 all 模式**:
```yaml
- step:
    id: parallel_checks
    type: api
    all:
      - ip_reputation
      - device_fingerprint
      - blacklist_check
    timeout: 50ms              # 编排层总超时
    on_error: partial          # 允许部分失败
    min_success: 2             # 至少成功 2 个
    next: aggregate_results
```

**说明**: 单个 API 的 timeout/retry 在 `configs/apis/` 中定义，编排层只关注整体超时和失败策略。

### 7.2 信贷场景 (多阶段依赖、成本敏感)

**需求**: 串行短路，昂贵 API 仅在必要时调用

**使用 step.when 条件**:
```yaml
steps:
  # 第一阶段: 本地规则
  - step:
      id: basic_check
      type: ruleset
      ruleset: basic_eligibility
      next: credit_check

  # 第二阶段: 付费 API (条件调用)
  - step:
      id: credit_check
      type: api
      api: credit_bureau         # 成本 $0.5/次
      when:
        all:
          - context.basic_score > 30
      next: final_scoring

  # 第三阶段: 详细评估
  - step:
      id: final_scoring
      type: ruleset
      ruleset: detailed_scoring
      next: decision
```

### 7.3 账户安全场景 (实验和对比)

**需求**: Champion/Challenger 测试

**方案**: 使用 hash 函数 + router

```yaml
steps:
  # 计算实验分组
  - step:
      id: calc_experiment_group
      type: function
      function: hash_mod
      params:
        key: event.user.id
        mod: 100
      next: route_experiment

  # 路由到不同变体
  - step:
      id: route_experiment
      type: router
      routes:
        - next: champion_flow
          when:
            all:
              - function.hash_mod < 90  # 90% Champion

        - next: challenger_flow
          when:
            all:
              - function.hash_mod >= 90  # 10% Challenger

  - step:
      id: champion_flow
      type: ruleset
      ruleset: login_rules_v1
      next: decision

  - step:
      id: challenger_flow
      type: ruleset
      ruleset: login_rules_v2
      next: decision
```

---

## 八、实现优先级

| 优先级 | 特性 | 理由 | 状态 |
|--------|------|------|------|
| **已实现** | 显式 entry point | DAG 编译必需 | ✅ `mypipeline.yml:123` |
| **已实现** | router step type | 纯路由判断 | ✅ `mypipeline.yml:162-181` |
| **已实现** | routes 条件格式 | 与 registry 一致 | ✅ `mypipeline.yml:145-158` |
| **已实现** | Convention over Configuration | api.<name>, service.<name> | ✅ `mypipeline.yml:36-43` |
| **已实现** | any/all execution mode | 降级和聚合 | ✅ `mypipeline.yml:354-384` |
| **已实现** | enable_trace | 请求级追踪 | ✅ 已支持 |
| P0 | API/Service 配置分离 | 生产环境必需 | 在 `repository/configs/` 中实现 |
| P1 | step.when 条件 | 成本控制和 Early Exit | 扩展已有 when 支持 |
| P1 | Shadow mode | A/B 测试和新版本验证 | 运行时标记 |
| P1 | 条件语义文档化 | 降低认知负担 | 完善文档 |
| P2 | Pipeline 级 trace 配置 | 可观测性 | 基于请求级扩展 |
| P2 | log step type | 调试体验 | 新增步骤类型 |
| P2 | 复杂度检查 | 规模化治理 | 编译器警告 |
| P3 | breakpoint step | 高级调试 | 开发工具 |

---

## 九、总结

当前 Pipeline DSL 设计已经达到**生产可用**水平，`mypipeline.yml` 展示了完整的最佳实践：

### 9.1 核心优势

1. **架构清晰**: Pipeline (编排) 与 Configs (实现) 分离
2. **路由统一**: `router` + `routes` + `when` 格式，与 registry 一致
3. **类型明确**: 8 种步骤类型，职责清晰，易于理解
4. **可视化友好**: 显式 entry + step.id + next，支持 DAG 生成
5. **LLM 友好**: 结构化 Schema、约束明确、示例丰富

### 9.2 优化方向

优化方向不是推翻重来，而是在现有基础上补齐和完善：

1. **P0 - 配置分离**: 将 API/Service 的 timeout/retry/fallback 移至 `repository/configs/`
2. **P1 - 成本控制**: 扩展 step.when 支持，实现步骤级条件跳过
3. **P1 - 实验支持**: Shadow mode + hash 函数实现简单 A/B 测试
4. **P1 - 文档完善**: 明确条件语义分层，降低认知负担
5. **P2 - 可观测性**: Pipeline 级 trace 配置、log step type
6. **P3 - 开发工具**: 断点、复杂度检查

### 9.3 设计原则 (重要)

```
Pipeline = 编排层
  - 定义执行顺序 (entry → steps → next → end)
  - 条件路由 (router + routes + when)
  - 步骤级门控 (step.when)
  ✗ 不包含实现细节 (timeout/retry/connection)

Configs = 实现层
  - API 配置 (base_url, timeout, retry, fallback)
  - Service 配置 (connection, queries, cache)
  - Datasource 配置
  ✗ 不包含业务逻辑
```

这些改进可以分阶段实施，优先完成 P0/P1 级别的能力补齐。
