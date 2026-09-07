# 执行入口与运行验证

调用、guard 和计分规则见 [Pipeline](../CDL/pipeline.md)，输入规则见[执行上下文](../CDL/context.md#strict-core-input-and-results)。
本文说明严格 Core 与兼容运行时的入口差异，以及 CLI、repository 和 Core HTTP 上的验证流程。

## Pipeline 编译与入口差异

兼容在线入口使用 `PipelineCompiler::compile`；严格 Core 通过 `compile_core` 校验完整资源集合，
并由 `DecisionEngine::from_core` 执行。两者支持范围不同，不能凭 YAML 能被解析来判断运行能力。

| 能力 | 严格 Core | 当前兼容在线编译器 |
|---|---|---|
| 节点类型 | `router`、`ruleset`、`rule`、`pipeline` | `router`、`ruleset`、`service` |
| Pipeline `when` | 支持 | 支持 |
| Step `when` | 支持，按节点类型检查约束 | 拒绝 |
| 按 Ruleset 结果继续路由 | 按结果作用域和调用顺序校验 | Ruleset 执行延后，拒绝依赖其结果的路由条件 |
| Service 调用 | 当前不准入 | 参数求值后调用，结果立即供后续节点读取 |

兼容编译器在过滤不可达节点前检查每个节点，拒绝不支持的节点类型、字段组合和 Service 参数；
未到达的节点也不能绕过这些检查。`rule`、子 Pipeline、`function`、`trigger`、`extract`
在该入口尚未实现，也没有独立的 `api` 节点。Router 按声明顺序选择首个命中的路由，否则走 `default`。
严格 Core 的完整调用与 guard 语义见 [Pipeline 规范](../CDL/pipeline.md)。

在线 Service 节点声明逻辑服务名、操作、参数、可选超时和输出路径。HTTP 绑定通过运行时的
`services` 配置提供，自定义适配器通过 SDK 注册；传输方式不改变节点类型。具体配置见
[Service 集成指南](SERVICE_GUIDE.md)。MCP 等其他传输需要相应适配器，不能仅声明名称就获得支持。
当前拒绝边界见[兼容编译器测试](../crates/corint-decision-compiler/tests/compatibility_admission.rs)。

## Registry 加载与入口差异

[Registry 语言规范](../CDL/registry.md)定义有序匹配、输入作用域和失败语义。严格 Core 的
`DecisionEngine::from_core` 接收显式资源集合和输入 schema，经完整校验后建立可执行引擎；
`registry.yaml` 是示例文件名，引用本身不会触发文件发现或加载。原始 import 先按
[导入解析指南](resolution.md)组成完整资源集合。

部署可以按环境或业务域维护不同的资源集合，每次构建的集合仍必须包含且仅包含一个 Registry。
调用方、repository loader 或明确配置的路径负责提供源码。不要把“每环境一个文件”理解为
引擎会合并多个 Registry，或自动根据请求查找不同的配置文件。加载和重载需重新验证完整集合。

兼容入口不会因为源码包含 `version: "0.1"` 而采用严格语义。当前
[DecisionEngineBuilder](../crates/corint-decision-engine/src/builder.rs)可通过
`with_registry_content`、`with_registry_file` 或 repository 内容提供 Registry。具体边界为：

| 情况 | 严格 Core | 当前兼容引擎 |
|---|---|---|
| Registry 解析或条件编译返回错误 | 构建失败 | 构建失败，不是记录日志后跳过该条目。 |
| 引用的 Pipeline 缺失或类型不匹配 | 完整资源校验失败 | 若匹配到的 ID 不在 Pipeline 表中，在该次请求记录告警并继续检查后面的条目。 |
| 所有条件均为 false | 返回 `E_NO_PIPELINE_MATCH` | 返回无选中 Pipeline、`signal: None` 的结果；这不是审批通过。 |
| 未提供 Registry | 构建失败 | 可以进入旧的 Pipeline 路由分支，不提供严格 Core 的 Registry 保证。 |

此表描述当前[引擎入口](../crates/corint-decision-engine/src/decision_engine/engine.rs)，不保证兼容
解析器会拒绝所有严格 Schema 不接受的写法。不要用未知字段被忽略、宽松条件解析或旧路由行为
替代严格验证。Registry 的匹配成本取决于条目顺序、条件复杂度和实际输入；容量数据应按
[性能测量指南](core-development.md#performance-measurement)测量，语言规范不承诺固定延迟。

## Registry 回归验证

[registry_execution.rs](../crates/corint-decision-engine/tests/registry_execution.rs)加载
[测试夹具](../tests/conformance/cdl_registry/)中的四个 Pipeline、共享 Ruleset 和 Rule，
核对预期路由和结果，并在 Trace 关闭与开启时执行。[Registry 定义](../CDL/registry.md)
说明相应的选择语义。
每个 Pipeline 的 `entry`、跳转和最终 `decision` 都来自 fixture，测试不生成替代定义。

验证覆盖 shadow 与国家条件重叠、通用路由、显式 fallback、无匹配、Pipeline guard 拒绝、
输入错误、条件求值错误，以及未到达条目的引用与类型检查。重复 Pipeline 引用和提前命中后的
短路行为也有独立用例。测试命令见[文档与一致性检查](core-development.md#conformance-and-document-checks)。

## 结果与 Trace 格式

Rust 入口保留 `DecisionResponse` / `DecisionResult` 的结果对象，包含 signal、raw score、triggered rules、actions 和 explanation。
signal 的序列化沿用 `{ "type": "decline" }` 形状；这与语言中 `results.<id>.signal` 的字符串值属于不同层次。

### 条件 Trace

实验性字段 `trace.core_conditions_v1` 仅由启用 `enable_trace: true` 的严格 Core
`DecisionEngine::from_core` 产生，格式见[条件 Trace schema](contracts/schema/condition-trace.json)。
兼容入口不输出该字段；缺少此字段的旧 Trace JSON 仍可反序列化。

观察范围包括实际调用的 Registry guard、Rule 条件、Ruleset conclusion、Pipeline/step guard、
Router route 和 Pipeline decision。布尔树记录比较或布尔叶子、`all`/`any` 分组及 `not` 节点；
比较作为一个整体记录，不展开标量操作数及其原始值。

| 状态 | 含义 |
|---|---|
| `evaluated` | 已求值，包含实际 `result`，包括 `false`。 |
| `skipped`，原因为 `short_circuit` | 所属条件已开始求值，但该子节点被短路跳过；没有 `result`。 |
| `skipped`，原因为 `not_reached` | 在已调用的程序中，整个条件未到达，例如首条命中后的其他条件；没有 `result`。 |

未被调用的 Rule/Ruleset，以及 Registry 首次命中之后的 guard 不生成记录。记录缺失不表示 false；
未选择的程序路径由 step/call Trace 说明。默认分支没有条件节点。失败请求保留执行错误，
不生成伪造的成功 Trace；局部失败 Trace 和 unknown 传播不属于当前契约。

每条记录包含 source、资源类型和 ID、定位条件字段的 `field_path`，以及**规范化布尔树**中的
`node_path`。根路径为空，`/children/N` 指向从零开始的子节点。规范化可能增加单元素分组，
因此 node path 不是原始 YAML 指针或行列位置，不保证不同源码写法产生相同路径。

`invocation` 是请求内从零开始的 VM 调用序号，没有条件映射的程序也占用调用序号。
它区分共享 Rule 的不同调用。记录按调用序号及编译器映射顺序组织，不代表全局求值时间线，
不提供耗时或跨请求身份。相同源码与请求产生相同条件记录；整个 Trace 的其他部分仍可能包含
非规范化的顺序和计时信息。

### 条件观察的执行与数据边界

编译器核对布尔节点的指令范围与实际表达式程序。严格 Core 关闭会删除指令的旧优化，
直到该优化能够同时重定位跳转和观察映射。观察器读取 VM 已算出的布尔值，不另行读取字段、
比较或求值被跳过的操作数。关闭 Trace 时不收集记录；内部观察状态会从返回上下文中移除，
保证两种模式下完整 `DecisionResult` 一致。

条件记录不复制原始输入值或表达式字面量，但 source、资源 ID、结构和布尔结果仍可能包含
业务信息。宿主负责 Trace 访问控制；本契约不增加采样、企业授权、旧 Trace 字段脱敏或持久化。

解析 import 后，source 和条件指针定位规范化的冻结源码，不定位原始多文档文件的行号。
内容和来源绑定由策略/包指纹及 resolver receipt 提供。Trace 本身没有签名，也不是发布审批
或业务效果证据。

### 调用 Trace

`trace.core_calls_v1` 使用[调用 Trace schema](contracts/schema/call-trace.json)，记录以下内容：

| 字段 | 含义 |
|---|---|
| `source`、`resource_type`、`resource_id` | 被调用资源的来源、类型和 ID。 |
| `call_path` | 从入口到当前资源的完整调用路径。 |
| `status` | `completed` 或 `skipped`。 |
| `score`、`signal`、`actions` | 本次调用的局部分数、signal 和动作意图。跳过调用使用 null 分数、null signal 和空 actions。 |

调用记录按完成顺序排列，子调用先于等待它返回的父调用。子 Pipeline 的 actions 可作为局部意图出现在 Trace 中，
不会因此成为父 Pipeline 的最终输出，也不会被 Runtime 执行。未到达的调用不补造返回结果。
Trace 开关必须保持求值、短路、分数、动作和错误一致；请求 ID、耗时和内部收集顺序不参与语言语义比较。
原始操作数、精确逐规则耗时和规范化审计序列化不在当前观察契约内。

### 条件 Trace 示例与验收

<!-- cdl-example: condition_trace_short_circuit -->
[一致性清单](../tests/conformance/cdl_core/manifest.yaml)中的完整 `condition_trace_short_circuit`
用例提供输入、依赖集合和预期结果。其 [Rule 源码](../tests/conformance/cdl_core/trace_rule.yaml)
在 amount 为 1000 时短路，在 1001 时求值嵌套 `not`；相同用例核对分数、signal、actions、
调用、步骤路径和 Trace 开关下的结果一致性。

[Core 一致性测试](../crates/corint-decision-engine/tests/cdl_core_conformance.rs)还在 VM 层注入
非法布尔操作数，确认启用 Trace 后仍不会执行被短路的操作数；公开请求仍须先通过输入校验。
测试覆盖所有支持的条件作用域、首条命中后的跳过、重复 Rule 调用、schema 和重复请求的确定性。

[示例清单](../tests/conformance/documentation/examples.json)保留本页到实际执行用例的绑定。
CI 拒绝缺失或重复标记、无效 case ID、缺失源码链接及复制到已登记页面的内联 YAML。
此检查不为其他历史示例或生成提示词提供验收结论。

## HTTP 错误与执行观察

库级诊断保留 `stage/code/source/field_path`，Core 执行错误由 `EngineError::Core` 携带。
Core HTTP 保留 `E_CORE_DECISION` 外层错误，`diagnostic.cause` 提供具体诊断；启用 v3 journal 后，错误 DecisionRecord 的 `error_code` 保存具体代码，并与请求快照绑定。发生执行错误时不返回伪造的完整成功 Trace 或动作。

## 通用 Agent 验收

完整合成示例位于 [core_extensions](../tests/conformance/core_extensions/pipeline.yaml)，包含父子 Pipeline、单 Rule、Ruleset、Registry、[嵌套输入 schema](../tests/conformance/core_extensions/input-schema.yaml)、[业务上下文](../tests/conformance/core_extensions/business-context.yaml)和[独立行为样例](../tests/conformance/core_extensions/behavior.yaml)。JSON 是合法 YAML，这些 `.yaml` 文件采用 JSON 表示以精确对应公开 schema。

```sh
./target/debug/corint prepare-repository \
  --root tests/conformance/core_extensions \
  --input-schema input-schema.yaml \
  --cases tests/conformance/core_extensions/behavior.yaml \
  --context tests/conformance/core_extensions/business-context.yaml \
  --target tests/conformance/core_extensions/target-capabilities.json \
  --revision runtime-extension-v1 --output ./runtime-extension-candidate \
  --format json rule.yaml marker.yaml ruleset.yaml child.yaml pipeline.yaml registry.yaml
```

`extended_runtime_candidate_runs_public_cli_and_repo_loader` 实际执行公开 CLI，在 Trace 开/关下验收样例并从新 repo 重载。`nested_agent_repository_executes_through_core_http` 将同一候选策略经独立配置批准后在 Core HTTP 上执行，核对分数、最终动作和调用 Trace。引擎的 `cdl_core_conformance` 另覆盖调用隔离、guard、输入错误、算术错误和无效调用图。成功候选仍需操作员批准，repo 继续是唯一策略来源。
新目标能力声明需包含新增能力 ID；旧报告不能代替新 checker 和新目标的验收。Work、Connector、在线 Feature/Model、结构化动作、严格 Core 的 gRPC/FFI 仍不属于本扩展。
