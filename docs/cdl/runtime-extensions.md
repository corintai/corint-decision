# Core Runtime 与表达式扩展

本扩展适用于严格 `cdl-core-risk-draft-1` 的 `compile_core`、`DecisionEngine::from_core` 及共享 CLI/Core HTTP 路径。继续使用语言版本 `"0.1"`，支持项登记在[能力清单](schema/capabilities.json)。兼容加载器不会自动启用这些调用或 guard。

## 调用与结果

| 节点 | 声明 | 同步返回的局部结果 |
|---|---|---|
| 单 Rule | `type: rule`、`rule: <id>`、`next` | `status`、`score`/`total_score`、`matched`，不产生 signal/actions |
| Ruleset | `type: ruleset`、`ruleset: <id>`、`next` | `status`、局部 `score`/`total_score`、`signal` |
| 子 Pipeline | `type: pipeline`、`pipeline: <id>`、`next` | `status`、局部 `score`/`total_score`、`signal` |

每次调用从零分开始。父 Pipeline 将本次调用的分数加一次；子 Pipeline 内的 Ruleset 不会再被父层重复汇总。一个 Rule 在不同调用中各自执行，零分命中仍记为 `matched: true`。
父子结果作用域隔离：子 Pipeline 只能读自己调用的结果，父层只能读 `results.<child_id>`，不能读取子层内部的 Ruleset 结果。子 Pipeline 的 signal 和 actions 不会自动成为最终输出；最终结果由父 Pipeline 的 decision 明确选择。子 actions 保留为局部意图 Trace，不在 Runtime 内执行。

每个 Pipeline 内，一个资源只能有一个调用点。所有供给资源都接受引用、类型和 DAG 检查，包括未注册的 Pipeline；禁止递归、不可达步骤及跨分支未定义结果。调用图最多 16 层 Pipeline，保守展开预算最多 4096 个资源/步骤/规则节点，不靠 guard 或路由裁剪绕过预算。

## Guard

Pipeline 与 step 的 `when` 使用同一条件编译器和短路执行器。

- 调用 step 的 guard 为 false：跳过调用并走该 step 的 `next`。
- Router 的 guard 为 false：不求值 routes，直接走 `default`。
- 子 Pipeline 自身 guard 为 false：返回 `status: skipped`，父 Pipeline 继续执行。
- Registry 选中的入口 Pipeline 自身 guard 为 false：返回 `E_PIPELINE_SKIPPED`，不执行 decision，也不选择新的 Registry 条目。

已到达但被跳过的调用只有 `results.<id>.status == "skipped"`，没有 score/signal/matched。可以先检查 status，再通过 `&&` / `||` 短路读取值；直接读取返回 `E_RESULT_UNAVAILABLE`。未到达的分支结果仍在编译阶段拒绝。被跳过的规则不计分，也不记为执行成功。

## 嵌套与可选输入

[输入 schema](schema/input.json) 支持 `number`、`string`、`boolean` 和带完整子 schema 的封闭 object。对象类型沿用模型的序列化格式：`{"object":{"schema":{"name":"payment","fields":{...}}}}`。字段的 `required: false` 允许缺失；嵌套对象缺失时不要求其子字段出现，但对象一旦出现就递归检查它的必填字段。

`exists(event.payment.amount)` 在声明路径缺失时返回 false；普通读取缺失的可选字段返回 `E_MISSING_INPUT`。例如 `exists(event.payment.amount) && event.payment.amount > 1000` 可安全处理缺失。`exists` 仅接受一个已声明 event 路径，不接受任意函数或结果路径。

可选不等于 nullable：显式 null、错误类型、未声明字段和非有限数字均返回 `E_INPUT_SCHEMA`。不注入默认值，不将缺失转成 0/false；数组、开放 object、Any 类型仍未启用。schema 最多嵌套 16 层、1024 个字段。

算术支持数值 `+ - * / %`、一元负号、比较和布尔短路。无类型隐式转换；对象不可整体比较。数值沿用 binary64，除零和取模零返回 `E_DIVISION_BY_ZERO`，非有限算术结果返回 `E_NUMBER_OVERFLOW`，整数分数累加溢出返回 `E_SCORE_OVERFLOW`。

## Trace 与错误

条件继续输出 `trace.core_conditions_v1`，新增 Pipeline/step guard 的条件边界。`trace.core_calls_v1` 输出资源来源、完整调用路径、completed/skipped、局部分数、signal 和 actions，按调用完成顺序排列；格式见[调用 Trace schema](schema/call-trace.json)。嵌套调用的条件仍有独立 invocation，Trace 开关不改变求值或错误。

错误统一携带 `stage/code/source/field_path`，运行错误指出失败资源，不回退为业务通过。代码包括 `E_CALL_CYCLE`、`E_CALL_LIMIT`、`E_MISSING_INPUT`、`E_RESULT_UNAVAILABLE`、`E_DIVISION_BY_ZERO`、`E_NUMBER_OVERFLOW`、`E_SCORE_OVERFLOW` 和 `E_PIPELINE_SKIPPED`。声明/请求错误沿用既有结构化代码。
Core HTTP 保留 `E_CORE_DECISION` 外层错误，`diagnostic.cause` 提供具体诊断；启用 v3 journal 后，错误 DecisionRecord 的 `error_code` 保存具体代码，并与请求快照绑定。发生执行错误时不返回伪造的完整成功 Trace 或动作。

## 通用 Agent 验收

完整合成示例位于 [core_extensions](../../tests/conformance/core_extensions/pipeline.yaml)，包含父子 Pipeline、单 Rule、Ruleset、Registry、[嵌套输入 schema](../../tests/conformance/core_extensions/input-schema.yaml)、[业务上下文](../../tests/conformance/core_extensions/business-context.yaml)和[独立行为样例](../../tests/conformance/core_extensions/behavior.yaml)。JSON 是合法 YAML，这些 `.yaml` 文件采用 JSON 表示以精确对应公开 schema。

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
