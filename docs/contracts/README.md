# 公共契约：Core 目标兼容性（实验性）

这是改进提案 §5.4 / §8.3 的首个可执行增量，面向通用 Agent、Corint Work 和独立 CLI，
不是新增的 CDL 顶层语法，也不增加 DecisionPolicy 层。
当前仅支持 `cdl-core-risk-draft-1`；不代表完整跨产品契约或真实 Work/生产发布集成已完成。

## 版本与职责

| 契约 | 生产者 → 消费者 | 当前内容 |
|---|---|---|
| [BusinessContext v1](schema/business-context.json) | 业务拥有者或 Work → Agent、检查器 | 输入 Schema、字段含义/单位/实体/时间口径、目标、约束、动作名称 |
| [TargetCapabilities v1](schema/target-capabilities.json) | 目标运维方 → Agent、检查器 | 引擎/Profile/语言版本、能力集合、上下文指纹、声明状态、动作名称、源码数量预算 |
| [CompatibilityReport v1](schema/compatibility-report.json) | 同一共享检查器 → 调用者 | 成功检查的策略/上下文/目标/检查程序绑定与明确的信任边界 |

前两个契约使用 YAML 或 JSON，必须显式指定 `contract_version: "1"`、`id`、`revision` 和
`provenance: {producer, reference}`。来源信息只是声明：检查器不联网、不验证身份，也不会访问 reference。
未知版本、额外字段、重复键和缺失必填字段均拒绝；`permissions`、`approval` 等自述字段不授予权限。
报告 schema 只描述成功结果；失败通过现有 Diagnostic 返回，不生成成功报告。

BusinessContext 的 `input_schema` 直接复用已有 [输入模型及 schema](../cdl/schema/input.json)，
不是第二套类型系统。外部验证器须离线注册 `urn:corint:core-input` 为该 schema；Rust 工具链已内置注册。
实际输入仍需通过共享编译器的语义检查。传入的 input-schema 必须与上下文中反序列化后的 Schema 一致，
包括元数据；字段说明必须恰好覆盖输入字段，且实体引用必须存在。

单位、时间口径、业务目标和约束作为生成上下文保留并绑定指纹，**尚不进行业务语义证明或单位转换**。
例如把“元”改成“分”会使旧绑定失效，但新检查通过不表示阈值已正确换算，仍需人工审核和独立测试。

目标检查采用保守的精确匹配：目标声明的引擎版本须等于本地 `ENGINE_VERSION`，语言/Profile 须匹配，
能力列表须恰好包含 [能力清单](../cdl/schema/capabilities.json) 中全部 `supported` 能力。
draft-1 暂按整体 Profile 检查，不协商能力子集或跨版本兼容；新增未知能力也拒绝。
`status: unavailable` 拒绝；`ready` **只是声明，不证明远端在线**。
所有 Pipeline 决策分支的 action（包括样例未触达的分支）须同时出现在两份契约的动作列表中。
动作列表是兼容性约束，不是执行授权。`max_sources` 限制整个显式源码闭包，范围 1–10000；
不是运行耗时或内存预算。`resources` 必须为空；Feature/Model/Connector 部署绑定尚未启用。

## 运行示例

在仓库根目录构建 CLI 后运行：

```sh
cargo build -p corint-decision-cli --locked --offline
./target/debug/corint check-target --format json \
  --input-schema tests/conformance/cdl_core/input-schema.yaml \
  --context tests/conformance/contracts/business-context.yaml \
  --target tests/conformance/contracts/target-capabilities.json \
  tests/conformance/cdl_core/rule.yaml \
  tests/conformance/cdl_core/ruleset.yaml \
  tests/conformance/cdl_core/pipeline.yaml \
  tests/conformance/cdl_core/registry.yaml
```

这是本地声明 fixture，不是真实客户或在线环境。缺少依赖缓存时构建需去掉 `--offline`；运行不需要网络。
命令不修改输入，不运行行为样例，不发布策略。CLI 输出一个 JSON 对象，`scope: compatibility`，
成功时其 `compatibility` 字段包含上述报告。退出码 0 为声明兼容，1 为校验失败，2 为用法/I/O 错误。
行为检查请另行运行 [`corint test`](../cdl/testing.md)。

## 指纹与旧证据

- 目标的 `context` 必须精确固定 BusinessContext 的 ID、revision 和原始 UTF-8 字节 SHA-256。
  注释或排版变化也会改变这个指纹；不因 ID/revision 相同而信任旧内容。
- `policy_sha256` 复用现有 [source package 内容身份](../cdl/packages.md)，包括输入契约和整个源码闭包。
- `checker_version` 是共享工具链版本，`checker_sha256` 是当前宿主可执行文件的 SHA-256，
  不是远端引擎指纹；CLI、测试程序或 Work 宿主不同会产生不同的检查程序绑定。
- `binding_sha256` 复用包的规范化 JSON 哈希算法：SHA-256 的输入为
  `corint-canonical-json-v1\0` 字节前缀，后接递归按键排序的紧凑 JSON：
  `{"domain":"core-target-binding-v1","value":{...}}`。
  value 恰含 `policy_sha256`、`context_sha256`、`target_sha256`、`checker_version`、`checker_sha256`。
  文件路径不进入契约指纹，同一宿主中的整体目录搬迁不会改变绑定。

可在同一命令增加 `--expected-binding <此前保存的64位小写SHA256>`，重新检查并比较当前绑定。
策略、上下文、目标声明或检查程序改变后，旧指纹不能继续通过（`E_STALE_BINDING`，或更早的契约错误）。
这只是防止误用旧结果，**不是签名、防篡改认证或审批验证**；调用者替换 expected-binding 并不构成授权。
报告明确记录 `execution_checked: false`、`business_semantics_checked: false`、
`live_target_verified: false`、`business_evaluation: not_performed`、
`publication_approval: not_granted`、`authenticity: unsigned`。

当前报告是独立结果，不写入 source package v1 或 source bundle v1。
`build / verify / export / import` 仍不检查目标契约，也不携带目标兼容性的可信历史证据。
需要部署约束的调用方必须额外运行本检查。[实验性严格 Core 服务端](core-server.md) 现已在本地操作者信任域内接入共享检查、固定样例重跑、批准指纹列表和原子激活；完整生产授权、可信远端证明与持久发布仍待完成。

## 共享 API 与验证证据

`corint_decision_toolchain::contracts::TargetContracts::load(context, target)` 验证并保存不可变契约；
`check(sources, input_schema, expected_binding)` 使用共享 Core 编译器与包内容身份计算。
CLI 和严格生成器使用同一个实现，没有另一套表达式解释器。

生成器提供 `generate_for_target` / `revise_for_target`，详见 [生成 API](../cdl/generation.md)。
调用者明确选择这些入口后，两份契约会发送给选定模型；请先审核其中的敏感信息。
独立验收样例仍不发送。兼容性检查通过也不能替代行为验收。

证据来自 [CLI 契约测试](../../crates/corint-decision-cli/tests/contracts.rs) 和
[固定模型响应测试](../../crates/corint-decision-llm/tests/core_generation.rs)：
覆盖版本/字段/实体/能力/动作错误、上下文精确绑定、过期指纹、源码搬迁、原有 Core 反例、
拒绝伪造权限以及兼容但行为错误时不构建包。尚无真实模型、Work 数据连接或部署安全认证的证据。
