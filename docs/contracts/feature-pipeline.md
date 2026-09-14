# 固定时间截止点的 Feature → Core 输入绑定

## 背景与目标

面向 SDK 集成者和严格 Core 服务维护者。公共执行层
`corint_decision_engine::decision_host::DecisionHost` 统一原始输入校验、特征准备、Core 执行和执行证据，
由严格 Core HTTP 与 SDK 复用。原有 `FeaturePipeline` SDK 接口继续可用，复用相同的输入准备实现。
Core 计算内核仍只处理已准备的输入，外部查询由宿主执行层协调。

## 范围

`FeaturePlan` 包含格式版本 `1`、计划 revision、数据源部署 revision、超时预算与 1–64 个输出。
每个输出绑定一个唯一的、必填的 Core number 输入字段和唯一 Feature 名称。
当前接纳 enabled aggregation / expression；聚合必须有合法 window 和已绑定数据源，表达式依赖必须在计划内完整解析。
DecisionHost 的数据源配置当前支持 SQLite/PostgreSQL，最多 64 项，查询缓存必须为 TTL 0。
SDK 使用 SQL 数据源时需启用 `sqlx` crate feature；未启用时构造失败，纯表达式计划不需要该 feature。
这不启用 Core CDL 中的动态 Feature/List/Model/Service 节点，也不改变兼容引擎的策略语义。
严格 Core gRPC/FFI 尚未接入该宿主；兼容 HTTP/gRPC 保持现有入口。

## 方案与执行步骤

1. HTTP 验证凭据并取得一个不可变的策略/资源快照；SDK 调用者负责其自身的认证和发布验收。
2. 宿主验证原始 event，拒绝调用方预填绑定输出；此时尚未查询数据库。
3. HTTP 用服务端当前 Unix 秒固定 `as_of`，窗口为 `[as_of - window, as_of)`。客户端不能传入或覆盖截止点。
4. 查询数据源并按依赖顺序执行，同一请求共享依赖只执行一次。每个宿主最多接纳 64 个并发特征决策，超限返回 `E_HOST_BUSY`；无等待队列。
5. 输出必须是有限 number；null、缺失、错误或超时均失败，不隐式转成 0 或 false。
6. 填入绑定字段后调用严格 Core，形成决策及输入/资源证据。响应 `processing_time_ms` 包含宿主取数和策略执行时间。
7. HTTP 默认完成本地 journal 可靠接收后返回；下游异步导出。显式 `best_effort` 模式才仅等待内存入队。

计划查询超时为 1–60000 ms；数据源 `timeout_ms` 也限制单次查询等待。
连接初始化使用总计 60 秒的截止时间，连接失败或超时拒绝候选，保留已有快照。
取消等待不保证数据库服务器立即停止已经发出的 SQL；当前只执行只读计算。
聚合维度支持 `event.user.id` 和 `${event.user.id}`、多个插值及嵌套路径；缺失字段、未闭合模板和错误类型报错。

SQL 聚合空集合可能产生 null，因此 sum/avg 等会按输出契约失败；count 返回 0。
固定事件截止点本身不能排除事后补录。可为输出配置 `available_at_field`（数据首次可用时间，数值 Unix 秒），查询会同时要求该列 `<= as_of`。生产数据必须保留不可变的首次可用时间，历史改写、删除及跨表/跨源事务快照仍由数据层负责。
历史回放使用保存的特征值，不重新查询当前数据库。

## 配置、输入与输出

Core 配置 v2/v3 可增加 `"feature_pipeline": "features.json"`，路径相对 Core 配置目录。
下面是 `features.json` 的 SQLite 聚合配置形状；实际部署需替换绝对数据库路径并配置相应只读权限：

```json
{
  "activation_cases": [{
    "event": {"user_id":"u1"}, "as_of":120,
    "expected_values": {"amount":1100.0}, "expected_score":60
  }],
  "plan": {
    "format_version": "1",
    "revision": "volume-v1",
    "datasource_revisions": {"events": "db-v1"},
    "timeout_ms": 1000,
    "outputs": [{
      "field": "amount",
      "definition": {
        "name": "volume", "type": "aggregation", "method": "sum",
        "datasource": "events", "entity": "events", "dimension": "user_id",
        "dimension_value": "${event.user_id}", "field": "amount",
        "window": "60s", "timestamp_field": "occurred_at"
      }
    }]
  },
  "datasources": {
    "events": {
      "revision": "db-v1",
      "config": {
        "name": "events", "type": "sql", "provider": "sqlite",
        "connection_string": "sqlite:///absolute/path/events.sqlite?mode=ro",
        "database": "test", "pool_size": 1,
        "timeout_ms": 1000, "query_cache_ttl_secs": 0
      }
    }
  }
}
```

Core 完整输入 schema 和业务 context 需同时声明 `user_id` 和必填 number 字段 `amount`。
调用方仅提交 `{"business_event_id":"payment-123","event":{"user_id":"u1"}}`，`amount` 由宿主计算。
独立策略行为用例仍使用包含 `amount` 的完整输入。启用 HTTP 特征计划还必须提供 1–16 个
`activation_cases`，固定原始 event、Unix 秒截止点、完整预期特征值和决策分数。示例对应 `[60,120)` 内 u1 的 400+700 两条记录，需部署方提供其可信验收数据。
启动和重载用候选实际数据源执行每个用例的 Trace 开/关路径；缺少用例、执行失败、取值或分数不符均拒绝激活，保留旧快照。
验收总预算 60 秒，使用独立禁用指标的宿主，避免污染在线指标；用例本身纳入配置批准指纹。
SDK 构造不隐式执行验收，宿主可显式调用 `validate_activation_cases`。

原始输入、输出与依赖联合检查：`event.<输出字段>` 被拒绝，即便该字段属于另一个特征；
特征间引用应写成 `features.<特征名>`。表达式原始字段必须在 schema 中声明为 number，
嵌套路径必须有显式对象 schema；聚合维度与过滤模板须引用已声明的原始标量字段。

过滤器支持单谓词、`all`、`and`/`&&`；按完整 AST 解析，字符串里的操作符不参与切分。
`or`/`any`、正则、未引用的字符串、尾随垃圾、非字面量数组 IN、集合内模板、非有限数字和 NULL 大小比较在发布前拒绝。
`== null`/`!= null` 使用 IS NULL/IS NOT NULL。IN/NOT IN 采用集合成员语义：NULL 可作为显式成员，
空 IN 恒假、空 NOT IN 恒真；例如 NULL 不属于 `['good']`，但属于 `['good', null]`。

可给 aggregation 输出增加以下字段（表达式输出不接受它们）：

```json
{
  "available_at_field": "available_at",
  "freshness": {
    "entity":"source_watermarks", "key_field":"source_id", "key":"events",
    "watermark_field":"complete_through", "max_lag_seconds":5
  }
}
```

水位表必须准确返回一行，`complete_through` 是非负整数 Unix 秒，表示源数据已完整到达的事件时间。
每次决策读取并要求 `watermark >= as_of - max_lag_seconds`，未来墙钟水位、缺行、多行或陈旧水位都报 `E_FEATURE_FRESHNESS`，不执行 Core。
新鲜度查询与特征查询共享计划超时预算，`max_lag_seconds` 为 0–86400。
水位由数据生产者正确维护；取水位和特征不是跨查询事务快照，水位并不证明历史版本不可变。
证据 `feature_evidence.freshness` 按输出字段记录水位、允许延迟和可用时间过滤状态。
未配置时明确 `watermark_checked: false` / `availability_filtered: false`，不宣称新鲜度或历史可用性保证。

`FeatureHostConfig::binding_sha256()` 对解析后的计划、数据源 revision 及完整有效连接配置计算规范化 SHA-256。
修改连接目标或凭据也会改变指纹，即使声明的 revision 不变。配置只从操作员文件读取，不通过 HTTP 接收或导出。
可用以下只读命令计算指纹；它不建立数据库连接，也不表示验收通过：

```sh
cargo run -p corint-decision-engine --example feature_host_binding -- features.json
```

将输出填入对应 Core `approvals` 条目的 `feature_binding_sha256`，与现有策略/context/target/cases 指纹共同批准。
启用特征时，缺少或不匹配的特征指纹拒绝启动/重载；禁用时该批准字段应省略。
配置连接采用运行进程的路径语义，推荐绝对路径；服务不会把相对数据库路径自动转为相对 Core 配置目录。

成功响应增加 `feature_evidence`：包含计划指纹、计划 revision、数据源 revisions、`as_of` 和实际特征值。
`GET /v1/core/target` 与决策 `snapshot` 包含 `feature_binding_sha256`。
现有 `binding_sha256` 仍指 Core target 兼容绑定，不能与新指纹互换。
数据源 revisions 是部署方声明，不是数据库内容指纹或远端身份认证。

SDK 可从 `corint_decision_sdk` 导入 `DecisionHost`、`FeatureHostConfig`、`FeaturePlan`、`FeatureInput`：
先调用 `DecisionHost::new(sources, schema, Some(config), enable_metrics).await`，
再调用 `host.decide(raw_event, as_of, trace).await`。`as_of` 必须来自可信宿主。
返回的 `HostExecution.result` 表示执行成功或失败，其他字段保留可交给存储适配器的证据；SDK 不自动写 journal。
无外部输入准备时传入 `None`，仍使用相同宿主接口。

## 发布、记录与影响

每个候选读取一次资源配置，指纹校验后创建宿主；提交前再次验证配置没有变化。
重载失败保留原快照。重载成功后新请求使用新宿主，旧请求继续使用旧宿主及其数据源。
运行期间编辑资源文件不会立即改变当前请求；必须通过受批准的重载，或重启。

业务证据的 subject 将原 Core bindings 指纹与 `feature_host_sha256` 组合哈希；
策略、资源配置变化均使旧证据失效。启用 `business_evidence` 时，评估必须覆盖 `config.resources()`
返回的完整特征集合及固定样本；启动、重载和每次请求都会检查批准/覆盖/时效。
`subject_with_features` 定义组合规则，指纹生成工具本身不生成批准或业务评估。
Core target 的 `resources: []` 仍描述纯计算内核，宿主输入资源单独绑定，不扩大其 CDL 能力声明。

异步决策记录的 `resources` 保存已完成输入准备的特征引用（输出绑定指纹，包括定义、字段和时效约束，以及计划 revision）；
`input_evidence.sha256` 绑定 journal 私有输入包，内容为：

- `raw_event`：解析后的原始事件，数值已按引擎类型规范化。
- `event`：实际交给 Core 的完整输入；准备失败时为 null。
- `feature_evidence`：成功准备的取值和版本；准备失败时为 null。
- `feature_binding_sha256`、`as_of` 和包格式版本 `1`。

Core 执行报错仍保留已经准备好的输入和特征证据。取数失败不执行 Core，并写入错误记录，
不伪造完成的资源列表。未配置 FeatureHost 时保持原有 journal 输入对象格式。
回放应提取输入包的 `event`，再交给对应版本的纯 Core；不能把整个输入包当作 Core event。

## 验证

[Feature 集成测试](../../crates/corint-decision-engine/tests/feature_pipeline.rs) 使用真实 SQLite 和独立临时 PostgreSQL，
核对截止窗口、实体隔离、Core 决策、回放及缺失值。
[Core HTTP 测试](../../crates/corint-decision-server/tests/core_activation.rs) 的 `feature_host_*` 用例验证真实 SQLite 取数、
输入覆盖拒绝、错误记录、数据库超时、取值回放、资源变更批准、业务证据覆盖和在途重载。
[宿主测试](../../crates/corint-decision-engine/src/decision_host.rs) 验证并发准入、指标开关和配置约束。

```sh
cargo test -p corint-decision-server --test core_activation feature_host
cargo test -p corint-decision-engine --all-features --test feature_pipeline
python3 tests/scripts/run_feature_postgres_tests.py
```

测试使用合成数据，业务评估用例使用测试专用声明，不代表真实业务评估已完成。
PostgreSQL 用例需独立临时环境，未运行时不得计为通过。

## 常见问题

- **要部署新服务吗？** 不需要。DecisionHost 是引擎内部公共执行模块。
- **结果是否已落库？** 默认 `persistence: durable` 表示本地可靠接收；显式易失模式的 `queued` 只表示后台入队，约束见[运行保障](core-operations.md)。
- **会接管反馈和策略调优吗？** 不会；这些由外部 Agent 系统负责。
- **所有入口和资源都已统一了吗？** 本次统一 SDK 与严格 Core HTTP 的已支持特征输入路径；兼容策略迁移、严格 gRPC/FFI、List/Model/Service 宿主步骤仍需各自实现和验收。

## 修订历史

| 日期 | 变更 |
|---|---|
| 2026-09-14 | 增加公共 DecisionHost、Core HTTP 特征接入、完整资源配置绑定、业务证据覆盖和异步输入记录。 |

| 2026-09-14 | 修复过滤 NULL/集合/复合语义，增加原始输入依赖校验、真实宿主发布用例、水位及历史可用时间约束。 |
