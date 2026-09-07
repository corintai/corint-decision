# 固定时间截止点的 Feature → Core 输入绑定

实验性 SDK 入口 `corint_decision_engine::feature_pipeline::FeaturePipeline`。
该入口在严格 Core 之前执行显式声明的特征计划；不启用 Core 文档中的动态 Feature 语法，现有 HTTP/gRPC/FFI 不会自动接入。

`FeaturePlan` 包含格式版本 `1`、计划 revision、数据源部署 revision、超时预算与 1–64 个输出。
每个输出绑定一个唯一的、必填的 Core number 输入字段和唯一 Feature 名称。
当前接纳 enabled aggregation / expression；聚合必须有合法 window 和已绑定数据源，表达式依赖必须在计划内完整解析。
构造时要求计划指纹与 expected_binding 相同，并逐项匹配宿主传入的数据源 revision。
这些 revision 是部署方声明，不能视为数据库内容指纹或远端身份认证。

## 执行约定

1. 校验原始 event，拒绝输入错误以及调用方预填绑定输出；此时尚未查询数据库。
2. 使用宿主独立传入的 Unix 秒 `as_of` 将窗口固定为 `[as_of - window, as_of)`。
3. 查询数据源并按依赖顺序执行，同一请求共享依赖只执行一次。查询缓存必须为 TTL 0。
4. 输出必须是有限 number；null、缺失、错误或超时均失败，不隐式转成 0 或 false。
5. 填入绑定字段后执行同一个严格 Core 引擎，返回决策、FeatureEvidence 和可用于显式保留的 replay_event。

计划超时为 1–60000 ms；数据源 `timeout_ms` 也限制单次查询等待。
取消等待不保证数据库服务器立即停止已经发出的 SQL；当前只执行只读计算。
聚合维度支持 `event.user.id` 和 `${event.user.id}`，支持多个插值，完整解析嵌套路径。
缺失字段、未闭合模板及错误类型报错。过滤器模板沿用其独立的条件语法。

SQL 聚合空集合可能产生 null，因此 sum/avg 等会按输出契约失败；count 返回 0。
固定截止点排除未来事件，但不能排除事后补录。历史回放使用保留的特征值，不能重新查询后声称原始历史值不变。

## 验收与边界

[Feature 集成测试](../../crates/corint-decision-engine/tests/feature_pipeline.rs) 使用真实 SQLite 和独立临时 PostgreSQL，
核对窗口两端、实体隔离、离线期望、Core 决策、回放、缺失值、绑定变更、输入覆盖及数据库失败。

```sh
python3 tests/scripts/run_feature_postgres_tests.py
```

PostgreSQL 测试使用合成数据，仅监听临时 Unix socket，结束后销毁临时数据库；无环境时失败，不计作通过。
当前不包含真实 Work 客户端、Model 在线推理、结构化副作用或生产数据质量认证。
