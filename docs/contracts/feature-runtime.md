# Feature 运行时边界

Feature 是兼容运行时扩展，不属于当前严格 Core 支持范围。
本页记录实际准入与错误语义；业务用例见 [Feature 用例与命名](../feature-configuration.md#use-cases-and-naming)，未实现的方向见 [架构规划](../ARCHITECTURE.md#feature-directions)。
数据源连接、连接池和配置示例见 [Feature 配置指南](../feature-configuration.md)，语言字段见 [Feature DSL](../../CDL/feature.md)。

## 查询能力

| 方法 | SQLite | PostgreSQL | 其他后端 |
| --- | --- | --- | --- |
| count / sum / avg / min / max / distinct | 已有本地数据库回归测试 | 有 SQL 生成实现，需目标数据库验收 | 按连接器分别验收 |
| median / stddev / percentile | 明确拒绝，不生成替代 SQL | 有 SQL 生成实现，需目标数据库验收 | 按连接器分别验收 |

MySQL SQL 执行尚未实现。不能将方法出现在枚举、配置可解析或生成 SQL 等同于已验证的后端支持。
注册 Feature 时，如果对应数据源已存在，会校验方法能力；先注册 Feature、后添加数据源时，
`FeatureExecutor::add_datasource` 返回 `Result` 并校验已有定义，失败不替换原数据源。
直接 Query 调用也在 SQL 生成阶段拒绝 SQLite 不支持的统计方法。

数据库 `when` 支持单个谓词或 `all`。`any` 尚未实现并明确拒绝；未知结构、非法谓词和
未解析的模板变量都返回错误，不会删除过滤器后执行扩大范围的查询。
完整 `${event.path}` 值按原类型解析，兼容旧 `{event.path}` 写法；嵌套路径必须完整匹配。
过滤器不支持部分字符串插值或字面量数组内的模板，可用完整模板传入数组。
`contains`、`starts_with`、`ends_with` 分别生成包含、前缀、后缀匹配；字面量 `%`、`_`、`!`
按显式 LIKE 转义处理。大小写行为取决于数据库的 LIKE 规则。
当前字面量字符串过滤只在 SQL 连接器实现，OLAP 连接器会明确拒绝；原始 Query 的 Like 保留后端原有模式语义。
这些数据库过滤器与 Rule/Registry 的通用表达式是不同入口。

## 依赖与错误

单项注册允许后续补充依赖，但不能形成循环；批量注册和目录加载必须满足完整依赖图。
目录加载失败不提交部分结果。图验证与执行使用有界的迭代遍历：最多 4,096 个 Feature，
依赖路径最多 128 层；同一请求或批次中的共享依赖只计算一次。

Feature 查询或计算失败会向上传播，规则不能把它转换成成功的“零分、未命中”。
Lookup 未找到键时返回显式 fallback，未配置时返回 null；查询失败只有显式 fallback
才能降级，否则传播错误。Redis 连接不可用属于错误，不是缺失键。
Lookup 配置在反序列化时完整保留，fallback 支持 JSON 对象和数组；YAML null 表示未配置。
State `time_since` 按最早匹配时间戳计算完整的 minutes/hours/days，拒绝 `window`；
没有有效时间戳或查询/时间计算失败时可用显式 fallback。
缺失数据源、模板和过滤器错误、未支持的方法不能使用 fallback。

## 时间与示例

普通 SQL/ClickHouse 相对窗口使用数据库时钟，区间为 `[now - window, now)`，排除未来记录。
聚合默认时间列为 `event_timestamp`；State 使用宿主 UTC 时钟计算经过的时间。
固定截止点入口使用 `[as_of - window, as_of)`，详见 [FeaturePipeline](feature-pipeline.md)。
[Feature 文档示例](../../crates/corint-decision-engine/tests/feature_documentation.rs) 直接加载
CDL 中的 YAML，以本地 SQLite 验证动态过滤、实体隔离、时间窗口、State 和 Rule 结果，
并以本地 RESP 服务器验证 Redis 键映射、缺失值及错误处理。
运行：`cargo test -p corint-decision-engine --features sqlx,redis --test feature_documentation --locked`。

## 缓存与新鲜度

Runtime `DataSourceConfig.query_cache_ttl_secs` 默认 `0`，查询每次读取数据源。
非零值显式启用查询缓存，单位秒；缓存完整结果（包括多行与空结果），缓存键包含完整 Query。
配置 TTL 表示调用方接受该时间内的数据陈旧，不能把缓存查询当作实时最新值。
写入后可调用 `DataSourceClient::clear_query_cache()` 主动失效。

server.yaml 的数据源配置通过 `options.query_cache_ttl_secs: "60"` 传递该设置；
无效数值会被拒绝。FeatureStore 的键值缓存使用其 `default_ttl`。
新 Feature 定义暂不支持单独的缓存策略或 Redis L2 Feature 缓存，不能依赖旧 Operator
缓存字段配置新 Feature 的行为。

回归证据见 [Feature 安全与 SQLite 测试](../../crates/corint-decision-runtime/tests/feature_safety.rs)。
运行：`cargo test -p corint-decision-runtime --all-features --test feature_safety --locked`。
固定截止点的 PostgreSQL/SQLite 聚合输入绑定另有真实后端验收，见 [FeaturePipeline](feature-pipeline.md)。其余 PostgreSQL 方法、Redis 与 OLAP 能力仍需分别验收。
