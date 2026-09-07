# CDL 六项增强与验收入口

本轮实现覆盖明确的 Core、SDK Feature 输入绑定和本地服务范围。语言版本仍为 `0.1 / experimental`。
各项实现与外部部署验收独立记录，不能用本地通过代替真实 Work 或多节点能力。

| 项目 | 交付物 | 可执行验收 |
| --- | --- | --- |
| 1. 规范与版本 | [稳定性契约](stability.md)、唯一[能力清单](schema/capabilities.json)、修正发布状态 | 文档门禁、Core capability/schema conformance |
| 2. 语义一致性 | 固定随机种子的表达式/图生成与解析器变异测试 | [semantic_properties](../../crates/corint-decision-engine/tests/semantic_properties.rs)：128 表达式、64 分支图、4096 变异输入 |
| 3. 文档和生成模板 | [逐片段清单](snippets.json)、生成模板直接嵌入[公共 fixture](../../tests/conformance/generation/rule.yaml) | [document_assets](../../crates/corint-decision-engine/tests/document_assets.rs)、[prompt_assets](../../crates/corint-decision-llm/tests/prompt_assets.rs) |
| 4. Feature 接入 | [固定截止点与版本绑定](../contracts/feature-pipeline.md)、查询超时与完整模板路径 | 真实 SQLite 与独立临时 PostgreSQL → Feature → Core → 保留输入回放 |
| 5. 诊断与回放 | [record/replay API 和 CLI](replay.md)、完整性校验、选择性输入记录、结构化失败记录 | 工具链与 CLI 的成功、失败、缺失、变更和拒绝覆盖测试 |
| 6. 性能与部署证据 | Release 引擎基准、真实 HTTP/持久日志/并发重载基准、Core 内部日志复制优化 | 下述脚本及 [手动性能 CI](../../.github/workflows/cdl-performance.yml) |

历史片段的 `core-negative` 验证其严格 Core 拒绝边界，不证明旧兼容运行时行为。
可接受的 Registry 片段提供完整输入与 Pipeline 包装，真实执行 first-match；其他语法说明保持 `syntax-reference`。
Rule/Ruleset/Pipeline/flow 生成模板使用已执行 fixture；API 配置生成器仍是兼容能力，不能用于声明严格 Core Connector 支持。

## 性能测量

```sh
python3 tests/scripts/run_core_benchmark.py --samples 1000 --output target/core-benchmark.json
python3 tests/scripts/run_core_http_benchmark.py --samples 200 --output target/http-benchmark.json
```

输出必须是新文件。引擎报告覆盖 16/128/512 条规则、并发 1/4/16、Trace 开关，包含编译耗时、P50/P95/P99、吞吐和子进程峰值 RSS。
HTTP 报告使用真实 CLI/server、随机 loopback 端口、合成输入和 v3 SQLite 持久日志，对比是否并发重载。
每组负载在预热后清空该脚本独占的临时日志，从 0 条历史开始测量，不接触用户数据库。
它包含连接建立、Python 客户端与日志开销，不能直接与进程内数据比较。
`--profile debug` 仅用于 HTTP 功能检查，正式测量默认 release。

```sh
python3 tests/scripts/run_core_benchmark.py --samples 1000 --baseline target/core-benchmark.json --max-regression 0.25 --output target/core-benchmark-next.json
```

比较前检查平台、构建模式、线程数和负载一致性；超阈值返回非零并保留报告。
共享 CI 主机波动可能造成噪声，应在同一受控硬件重复确认后设置性能门禁。
业务容量目标由部署方提供；基准结果不自动形成生产 SLA。

## 独立部署范围

当前明确验收的是本地真实 PostgreSQL/SQLite、严格 Core HTTP 和现有兼容协议测试。
FeaturePipeline 为 SDK 入口；严格 Core gRPC/FFI、真实 Work 产品、Model 在线推理与多节点发布均未由本轮启用。
接入这些目标时，应运行相同策略、输入及固定依赖的互操作用例，单独记录目标、版本和执行证据。

## 本地测量记录（2026-09-06）

同机、200 次/组的 Release 引擎测量中，512 条规则、并发 1 的 P95 从 31.19 ms 降至 6.45 ms；Trace 开启时从 39.16 ms 降至 7.70 ms。
这是本次样本的观察值，不是通用性能承诺。原始[优化前](../../tests/performance/baselines/2026-09-06-macos-aarch64-before.json)与[优化后](../../tests/performance/baselines/2026-09-06-macos-aarch64-after.json)报告包含全部 18 组负载和构建状态。
[HTTP 功能基准](../../tests/performance/baselines/2026-09-06-macos-http-debug.json)包含 12 组真实服务负载，使用 debug 构建，不能作为生产吞吐估计。
