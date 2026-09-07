---
name: cdl-policy-authoring
description: "根据业务需求创建、修改或修复完整 CDL：Rule、Ruleset、Pipeline、Registry、Feature、List、Service；仅生成必要的策略资源文件，编写后调用 Corint CLI 静态校验并修复诊断。校验结果直接反馈，不默认生成分析、测试、脚本或报告文件。"
---

# CDL 策略编写

将业务需求转为可审查的 CDL 文件，最后通过公开 CLI 静态校验。默认工作流为：编写或修改 → 校验 → 按诊断修复 → 再次校验。使用本地文件，不依赖 Work 或特定 Agent 工具。

## 默认产物边界

默认只允许新增或修改本次策略必需的 CDL 资源文件：Rule、Ruleset、Pipeline、Registry，以及策略实际依赖的 Feature、List、Service。按需选择资源类型，不为凑齐七类资源搭建模板、空目录或重复版本。修改已有策略时沿用现有布局，只改必要文件。

- 写入前确定本次需要的资源文件及用途；这份范围说明留在对话中，不另建清单文件。
- 用户提供的 CSV、Schema、样例和已有测试是可读取的输入，不默认复制、转换或扩展成新的交付文件。已有输入 Schema 可用于校验；静态校验不要求新建 `input-schema.yaml`。
- 不默认生成 `behavior.yaml`、测试用例、分析或评估代码、Python/Shell 脚本、notebook、数据副本、指标 CSV、README、特征说明文档、验证脚本、日志、JSON 报告、包或回放产物，也不创建 `analysis/`、`reports/` 等配套目录。不能通过把这些文件放到策略目录之外规避此边界。
- CLI 校验直接读取 stdout 和退出码，在最终回复中给出结果；不默认重定向、`tee` 或保存报告，也不把命令保存为脚本。构建 CLI 所需的正常编译缓存不作为策略交付物。
- 只有用户明确要求或此前已明确授权的额外交付才可扩展范围；“生成策略”“修改策略”“保证质量”本身不授权生成辅助文件或开展离线评估。额外产物按该项要求的范围和位置处理。
- 保留用户已有的辅助文件；本 Skill 的产物限制不授权清理、迁移或重写这些文件。

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
- 传入一个或多个文件时只校验指定文件；传入目录时递归扫描其中全部 YAML/JSON 文件，支持文件和目录混用。若要额外解析 imports 并检查跨文件引用，加 `--root`。目录发现会按内容识别并跳过输入 Schema、行为用例和已知报告，查看 `skipped_sources` 确认范围；未知结构和损坏的 CDL 仍报错。显式指定的辅助文件不会跳过。跳过已有辅助文件不意味着应当生成这些文件。
- 已有输入 Schema 时传给 CLI 增加字段和类型检查。没有 Schema 时仍可完成静态校验，明确字段类型未验证；不得编造或放宽 Schema 来通过。
- 不为单纯编写/语法校验默认生成行为用例、打包、运行策略或发布产物。保留已有验收用例，不从实际输出反向改写预期。

## 校验与修复

读取[CLI 操作流程](references/cli-workflow.md)，使用当前源码构建的 CLI 或已确认版本的 `corint`。不要另写 YAML/表达式校验器替代 CLI。

1. 对编写或修改的文件或目录运行 `validate PATH... --format json`。需要解析 imports 和检查跨文件引用时加 `--root`；需要字段检查时加 `--input-schema`。
2. 同时检查进程退出码和报告 `valid`，按 `source`、`field_path`、`stage`、`code` 定位错误。解析器提供时还可用 `line`/`column`。
3. 修复原因后重新运行相同范围的校验。涉及引用或依赖变更时重新验证完整仓库。不得删除合法引用、替换外部资源或改变业务逻辑只为通过。
4. 在回复中说明 `references_checked`、`input_schema_checked`、`skipped_sources` 和 `unchecked` 所表示的实际检查范围，不另存报告。`execution_checked: false` 是默认静态验证的正常结果。

退出码 `0` 为校验通过，`1` 为 CDL/Schema/引用错误，`2` 为用法或文件读取错误。构建失败另行报告。重复诊断无法定位时保留命令和可复现错误，不进行无依据改写。工具不可用时可交付草稿，但没有实际运行不能宣称验证通过。

## 按需增加验证

以下流程均不属于默认策略编写范围。仅在用户明确要求或此前已明确授权相应检查时使用；策略存在行为变更本身不构成额外授权：

- **Core 编译与行为测试**：读取[Core Schema](../../CDL/schema/core.json)、[测试契约](../../docs/testing.md)，先运行 `validate --profile cdl-core-risk-draft-1`，再运行 `test`。这一路径要求完整 Core 闭包、输入 Schema 和独立预期用例，不能直接承接外部扩展资源。`test` 会运行 Trace 开关两种模式。
- **在线集成**：依据 Feature/List/Service 契约和用户提供的运行环境设计集成测试；静态通过不证明数据源、名单内容或 HTTP 服务可用。
- **Core 导入、目标检查、打包或回放**：读取[CLI 流程](references/cli-workflow.md)对应入口。静态 `--root` 导入不等于 Core `resolve`，也不改变执行能力。

## 交付

交付前核对本次新增、修改文件是否都属于必要的策略资源或用户明确要求的额外产物。最终回复列出修改的文件，简述业务假设、实际执行的校验命令、通过/失败和未验证范围，不为此新建 README、总结或报告文件。单文件通过要标明未做跨文件引用检查；结果仅对应本次文件版本。
语法通过不能证明业务效果。编写与校验不会执行动作、发布或激活策略；已有明确授权的后续工作按相应流程处理。
