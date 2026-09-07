---
name: cdl-policy-authoring
description: "根据业务需求创建、修改或修复 CDL 规则策略文件，补齐输入 Schema、资源引用和行为测试，并使用 Corint 严格 Core 工具链验证。适用于通用 Agent 编写 Rule、Ruleset、Pipeline、Registry；不默认执行策略发布或激活。"
---

# CDL 策略编写

将业务需求转为可审查的 CDL 文件与可重复执行的验证结果。直接使用本地文件和公开 CLI；不依赖 Work、模型 SDK 或特定 Agent 工具。

## 定位规范与工具

本 Skill 随 `corint-decision` 仓库分发。默认仓库根目录为本文件所在目录的 `../..`；先确认其中存在 `docs/cdl/schema/capabilities.json`、`tests/conformance/generation/` 和 `crates/corint-decision-cli/Cargo.toml`。
若 Skill 被复制到其他位置，从用户指定的 CDL 仓库或当前工作区定位这三个入口；仍无法定位时询问仓库路径，不把 Skill 的安装目录当作源码仓库，也不自动下载另一版本。
下列仓库链接按原始目录布局给出；迁移后按已确认的仓库根目录解析同一相对路径。

开始编写前读取：

- [能力清单](../../docs/cdl/schema/capabilities.json)：确认目标 Profile、语言版本、支持状态、限制和入口；这是能力声明的权威来源。
- [Core 规范](../../docs/cdl/cdl-core.md)以及[资源 Schema](../../docs/cdl/schema/core.json)、[输入 Schema 格式](../../docs/cdl/schema/input.json)：按当前检出的版本生成。
- [稳定性契约](../../docs/cdl/stability.md)：区分实验性成熟度、已实现范围与待实现能力。

默认生成严格 Core 的完整资源闭包。用户指定其他入口时，先确认该入口的能力和验收方式；不能静默改用兼容解析器使文件通过。
不要把 `docs/cdl/overall.md`、旧兼容文档或孤立代码片段当成严格 Core 模板。也不要把 `version` 字段本身当作严格入口的选择开关。

## 确定业务语义

从用户需求和现有策略提取输入字段、类型、单位、必填性、阈值边界、优先级、评分贡献、最终结果与动作意图。
保留用户提供的字段 Schema、已有 ID 和验收预期，除非需求明确要求变更。
金额等精度敏感字段先确认单位；Core 数值不是十进制定点金额。不要自行假设币种、舍入方式或缺失值等于零。

先写出代表性输入及期望结果，再实现规则。若关键阈值、默认决策、冲突优先级或字段来源不明确，集中询问影响行为的问题；继续准备不依赖这些答案的文件，未确认部分明确标为草稿。
已有验收用例作为约束保留，不能从引擎实际输出反向生成期望值来掩盖错误。Agent 自行补充的合成用例应与用户确认的业务验收区分说明。

## 生成与修改

从[公共生成模板](../../tests/conformance/generation/rule.yaml)及同目录的 `ruleset.yaml`、`pipeline.yaml`、`registry.yaml` 起步；输入格式参照[输入 fixture](../../tests/conformance/cdl_core/input-schema.yaml)。按需求改写示例 ID、阈值和业务含义，不把示例默认决策带入新业务。

- 每个 Core 文件只含一个资源文档；使用当前规范要求的显式版本。完整闭包包含恰好一个 Registry，以及所有被引用的资源。
- 规则匹配、局部评分、Ruleset 信号和 Pipeline 最终决策分别建模。局部信号不会自动覆盖最终决策，零分也不等于未匹配。
- 按业务优先级安排 first-match 分支和默认分支。图的 `entry`、路由和 `next` 决定执行顺序，文件排列顺序不能代替控制流。
- 可选输入先用 `exists` 保护读取；读取受 guard 控制的调用结果前检查其状态。涉及子 Pipeline、结果可用性或复杂分支时，读取[运行时扩展](../../docs/cdl/runtime-extensions.md)。
- 成员匹配和字符串运算按 Core 规范当前定义使用；字面量数组成员匹配不等于外部 List 服务，也不意味着事件输入支持数组。

编辑现有策略时，将新旧需求差异映射到受影响的资源与用例，保留无关策略。即使只改一条 Rule，也要用完整闭包验证其对路由与最终决策的影响。

## 验证与修复

读取[CLI 操作流程](references/cli-workflow.md)，通过当前仓库构建的公开 CLI 执行验证，不另写表达式解释器或手工校验器。

1. 运行 `validate` 检查完整闭包、输入类型和控制流。
2. 按[行为测试契约](../../docs/cdl/testing.md)和[测试 Schema](../../docs/cdl/schema/test-suite.json)编写用例，参照[行为 fixture](../../tests/conformance/cdl_core/behavior.yaml)。覆盖相关阈值的下方/等于/上方、重叠条件的优先级、默认路径和输入错误；有可选字段、guards、子调用时补相应未执行路径。不要为不涉及的能力机械增加用例。
3. 运行 `test`；它会执行 Trace 关闭/开启两种模式。检查退出码、顶层状态、用例计数和各用例结果。
4. 根据诊断中的 `stage`、`code`、`source`、`field_path` 修复原因。资源、输入 Schema 或用例改变后重新执行相关完整闭包的 `validate` 与 `test`。

编译失败不能作为行为用例的预期运行错误吞掉。测试失败时先核对业务预期，不能删除失败用例、放宽 Schema 或改默认决策只为通过。
重复诊断无法定位时保留可复现命令与问题，不进行无依据的循环改写。
工具缺失、构建失败或无执行权限时仍可交付草稿，但明确标记“未验证”及阻碍；没有实际执行不能宣称验证通过。

## 按需扩展

- **历史聚合或外部数据**：先读取 [Feature 输入绑定契约](../../docs/contracts/feature-pipeline.md)。区分上游提供的声明字段与 SDK `FeaturePipeline` 计算的绑定字段；确认单位、窗口、截止点、revision 和失败行为。Core 测试中的合成特征值仅验证消费逻辑，不证明特征计算或数据源已接通。不要将 `features.*`、SQL、Connector 或 Model 调用直接塞入严格 Core。
- **创作期 import**：仅在需要模块化引用时读取[解析契约](../../docs/cdl/resolution.md)，使用独立创作 Profile 和 `resolve`，再对冻结闭包测试；不能将未解析 import 直接交给 `validate`。
- **目标兼容性、打包或回放**：按需要读取 [CLI 操作流程](references/cli-workflow.md)的对应段落，不默认生成额外部署产物。

## 交付

交付资源文件、输入 Schema、行为用例和实际运行的验证报告，并简述关键业务假设、目标 Profile、执行命令、通过/失败数量和未验证范围。
单个资源片段要注明缺少的上下文，不能宣称完整策略已验证。报告仅对应本次验证的文件版本。
用例通过表示所列示例符合预期，不代表真实业务效果。动作是意图字符串；本 Skill 的编写授权不包含执行动作、发布或激活策略，已有明确授权的后续交付按相应流程处理。
