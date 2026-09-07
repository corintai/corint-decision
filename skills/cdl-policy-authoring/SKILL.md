---
name: cdl-policy-authoring
description: "根据业务需求创建、修改或修复完整 CDL：Rule、Ruleset、Pipeline、Registry、Feature、List、Service；编写后调用 Corint CLI 静态校验并修复诊断。默认不执行策略或连接外部资源；按需增加输入 Schema 检查、行为测试和目标兼容性检查。"
---

# CDL 策略编写

将业务需求转为可审查的 CDL 文件，最后通过公开 CLI 静态校验。默认工作流为：编写或修改 → 校验 → 按诊断修复 → 再次校验。使用本地文件，不依赖 Work 或特定 Agent 工具。

## 定位规范与工具

本 Skill 随 `corint-decision` 分发。先检查本文件所在目录的 `../..` 是否包含 `CDL/schema/authoring.json`、`docs/contracts/schema/capabilities.json` 和 `crates/corint-decision-cli/Cargo.toml`。
若安装在其他位置，从当前工作区或用户指定位置定位源码仓库。仍无法定位才询问路径，不把 Skill 安装目录误当作源码仓库，也不自动下载另一版本。下列链接按原始仓库布局给出，迁移后按已确认的仓库根目录解析。

编写前读取[能力清单](../../docs/contracts/schema/capabilities.json)、[语言概览](../../CDL/overall.md)及[静态资源 Schema](../../CDL/schema/authoring.json)，再按涉及的资源读取专题文档。
默认校验 Profile 是 `cdl-static-1`，覆盖七类资源；Core 专题中的限制描述严格执行入口，不能据此把完整 CDL 缩减为 Core，也不能把静态通过解释为 Core 可执行。
不要生成当前语言尚未实现的 Feature graph/sequence 等操作符。语言版本、静态检查 Profile、执行 Profile 分别确认，不通过改版本号绕过诊断。

## 确定语义并修改文件

从需求和现有策略提取输入字段、单位、阈值、优先级、评分、最终结果和动作意图。保留用户的 ID、Schema、预期行为和无关文件。金额单位、关键阈值、默认行为或字段来源不明时询问会影响语义的问题，继续处理独立部分，未确认内容标为草稿。

- 按资源类型采用当前定义：Rule/Ruleset/Pipeline/Registry、[Feature](../../CDL/feature.md)、[List](../../CDL/list.md)、[Service](../../CDL/service.md)。完整静态示例见[公共 fixture](../../tests/conformance/cdl_authoring/registry.yaml)及同目录的资源文件。
- 用 `entry`、`next` 和 routes 表达控制流；first-match 分支按业务优先级排列。区分 Rule 评分、Ruleset 信号、Pipeline 最终结果。
- 外部 Feature/List/Service 需要声明与引用；确认数据源名、窗口、单位、操作和失败行为，不需要为了语法校验启动数据库或调用服务。
- 单文件编辑可单独校验；若相关资源已在本地，用仓库模式同时检查引用与依赖图。Schema、测试、报告和备份不要放进资源扫描目录。
- 已有输入 Schema 时传给 CLI 增加字段和类型检查。没有 Schema 时仍可完成静态校验，明确字段类型未验证；不得编造或放宽 Schema 来通过。
- 不为单纯编写/语法校验默认生成行为用例、打包、运行策略或发布产物。保留已有验收用例，不从实际输出反向改写预期。

## 校验与修复

读取[CLI 操作流程](references/cli-workflow.md)，使用当前源码构建的 CLI 或已确认版本的 `corint`。不要另写 YAML/表达式校验器替代 CLI。

1. 对编写或修改的资源运行 `validate --format json`。存在完整仓库时加 `--root`；需要字段检查时加 `--input-schema`。
2. 同时检查进程退出码和报告 `valid`，按 `source`、`field_path`、`stage`、`code` 定位错误。解析器提供时还可用 `line`/`column`。
3. 修复原因后重新运行相同范围的校验。涉及引用或依赖变更时重新验证完整仓库。不得删除合法引用、替换外部资源或改变业务逻辑只为通过。
4. 记录 `references_checked`、`input_schema_checked` 和 `unchecked`。`execution_checked: false` 是默认静态验证的正常结果。

退出码 `0` 为校验通过，`1` 为 CDL/Schema/引用错误，`2` 为用法或文件读取错误。构建失败另行报告。重复诊断无法定位时保留命令和可复现错误，不进行无依据改写。工具不可用时可交付草稿，但没有实际运行不能宣称验证通过。

## 按需增加验证

仅在用户要求或任务本身包含行为变更验证时增加相应检查：

- **Core 编译与行为测试**：读取[Core Schema](../../CDL/schema/core.json)、[测试契约](../../docs/testing.md)，先运行 `validate --profile cdl-core-risk-draft-1`，再运行 `test`。这一路径要求完整 Core 闭包、输入 Schema 和独立预期用例，不能直接承接外部扩展资源。`test` 会运行 Trace 开关两种模式。
- **在线集成**：依据 Feature/List/Service 契约和用户提供的运行环境设计集成测试；静态通过不证明数据源、名单内容或 HTTP 服务可用。
- **Core 导入、目标检查、打包或回放**：读取[CLI 流程](references/cli-workflow.md)对应入口。静态 `--root` 导入不等于 Core `resolve`，也不改变执行能力。

## 交付

交付修改的文件和实际执行的校验结果，简述业务假设、命令、通过/失败和未验证范围。单文件通过要标明未做跨文件引用检查；报告仅对应本次文件版本。
语法通过不能证明业务效果。编写与校验不会执行动作、发布或激活策略；已有明确授权的后续工作按相应流程处理。
