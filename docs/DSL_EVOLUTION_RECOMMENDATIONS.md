# Corint 解决方案：Work、Decision 与 CDL 演进建议

> 状态：已完成优化见 **§1.4 进度标记**；阶段 0 首批实现及 W04/W07/W08 离线契约已落地，完整核心规范与跨产品一致性验收仍未完成。当前边界见 [CDL Core 首批规范](cdl/cdl-core.md)。
>
> 修订日期：2026-09-05。本文区分当前实现、首期候选契约与后续设计；提案示例不代表当前引擎已经支持。
>
> 目标：以 Corint Definition Language（CDL）及共享工具链连接通用 Agent、Corint Work 与 Corint Decision，建立可生成、可验证、可执行、可审计的风险策略闭环。执行确定性以固定的策略版本、输入和依赖证据为前提。

## 1. 结论

Corint 解决方案包含两个产品：**Corint Work** 是 Agentic Risk Operations Platform，承担数据分析、特征工程、建模、策略生成与优化；**Corint Decision** 执行已发布策略并做出风险判断。CDL 是两个产品与外部创作工具之间的公共执行契约，不是只对 Decision 内部开放的配置格式。

按当前产品规划，通用 Agent 默认根据用户要求及用户提供的上下文生成、修改策略，不直接接入客户真实业务数据；Work 在授权范围内连接真实业务数据开展分析与评估。两条路径必须使用相同的 CDL、验证规则和执行语义，不形成 Work 私有方言。

现有 CDL 的核心分层是正确的：规则负责检测与评分，规则集负责聚合，Pipeline 负责组织流程并产生最终决策，Registry 负责选择入口。YAML 也适合作为业务策略的载体：可阅读、可评审、可进行 Git 版本管理。

需要调整的并不是“是否使用领域语言”，而是语言的边界和执行语义。目前策略、编排、连接器配置、特征定义和副作用意图之间有重叠；同时部分可写语法尚无完整运行时语义。对于决策场景，任何“接受配置但静默忽略其含义”的行为都应视为设计缺陷。

面向 Agent / LLM，首先意味着机器能够可靠地生成、检查、模拟、解释和迁移策略，不等于必须在实时请求中调用大模型。语言还需要机器可读的结构约束、统一的类型与错误语义，以及明确的权限边界。

**首个交付目标是可执行验证的 CDL Core 规范与跨产品公共契约：每个标记为“支持”的能力和示例，都必须经过真实的解析、编译和行为验证。** 同时定义业务上下文、资源依赖、策略包、验证证据与反馈协议；这些接口先行，不等于首期实现完整 Work 或所有运行时扩展。

“任一 Agent 可以生成”是开放接入目标，不是保证模型每次都能写对。可执行性由统一工具链验证；业务有效性依赖具体数据和评估证据。CDL 规范本身不能替代客户的字段定义、可用特征目录或业务验收标准。

### 1.1 核心语言与领域能力分开

- **CDL Core**：定义资源、引用、表达式、执行顺序、结果、错误和版本语义。
- **Risk Profile**：保留现有 score 与 `approve / decline / review / hold / pass` 等领域约定；这些枚举不应成为所有未来决策领域的唯一结果类型。
- **能力扩展**：Connector、Feature、List、子 Pipeline、预测模型评分与 LLM 证据等逐项声明支持范围、权限和测试集。传统预测模型与在线 LLM 调用不是同一类能力。

首期继续使用现有 `rule / ruleset / pipeline / registry` 结构，不立即引入第二套顶层语法。结构化表达式和结构化 action 属于后续扩展；策略包先定义交付契约，具体格式示例仍是待实现提案，不改变当前 CDL 顶层语法。

### 1.2 严格 Core 当前状态与兼容入口遗留问题

最初问题清单来自 `3fad78f`；以下按当前严格 Core 与兼容入口分别记录，避免把已经完成的 Core 修复重复列为待办。严格 Core 指 `compile_core` / `DecisionEngine::from_core` 及明确接入它们的 CLI、生成器和 Core HTTP 模式；旧 parser、builder 与 REST/gRPC 入口不会因声明 `version: "0.1"` 自动获得这些保证。

| 核对项 | 严格 Core 当前状态 | 兼容入口遗留问题 | 验收证据 |
|---|---|---|---|
| 版本校验 | 拒绝未知、缺失和非字符串版本 | `parse_with_imports` 仍保留读取/默认版本语义，不能作为严格发布门禁 | `N02_unknown_version` / `N02_missing_version` / `N02_numeric_version` |
| 未知条件字段 | 未知字段、重复键及错误结构在执行前拒绝 | Rule / Pipeline parser 已拒绝未知键、混合条件表示及错误字段类型 | `N01_unknown_condition` / `N01_unknown_field` / `N01_duplicate_key` |
| Step guard | 已实现短路条件与显式 skipped；调用走 next、router 走 default | 兼容编译器仍在可达性筛选前拒绝，包括不可达节点；已删除空操作 | core_zero_score_match_and_guarded_router_do_not_fall_through；N07_step_guard |
| API 调用 | Connector 整体拒绝，包括参数、失败策略和组合调用 | 兼容编译器明确拒绝 `params / on_error / min_success / any / all`；解析器拒绝歧义目标和错误类型 | `N08_api_params` / `N08_api_any` / `N08_api_all` |
| 子 Pipeline | 已实现同步调用、局部结果隔离、单次分数汇总和有界调用图 | 兼容编译器明确拒绝子调用、function、单 rule、trigger、Service 和未知步骤 | core_calls_guards_and_nested_optional_inputs_share_the_vm；N07_subpipeline |
| Service 字段 | Service 节点及历史 `endpoint` 写法均拒绝 | `endpoint` 不在兼容 step 字段白名单，参考示例不代表可执行支持 | N08_service_endpoint |
| 默认与必填 | Registry 必须显式 `when`；Pipeline 必须 `decision`；conclusion/decision 唯一末尾 default | 兼容 parser 的默认/必填行为仍需单独迁移；文档中省略 Registry `when` 的兜底例已更正 | N03/N04；malformed_defaults_and_ids_are_rejected；input_errors_and_registry_no_match_are_not_approval |
| 调用执行时序 | Core `CallRuleset` 同步完成求值，后续 router 能看到真实结果 | 兼容编译器明确拒绝依赖规则集结果的 router，需使用严格 Core；其最终 decision 仍使用兼容后处理 | result_dependent_router；node_order_does_not_change_control_flow |

上述 Nxx / 完整场景来自 [conformance manifest](../tests/conformance/cdl_core/manifest.yaml)，具名测试来自 [真实引擎 runner](../crates/corint-decision-engine/tests/cdl_core_conformance.rs)。实现落点是 [Core 编译门禁](../crates/corint-decision-compiler/src/core.rs)、[Pipeline 指令生成](../crates/corint-decision-compiler/src/codegen/pipeline_codegen/instruction_gen.rs) 与 [Pipeline Runtime](../crates/corint-decision-runtime/src/engine/pipeline_executor.rs)。拒绝某项能力证明门禁有效，不证明该能力已实现。

已有 AST、IR、类型模型和 [Validator / Diagnostic](../crates/corint-decision-compiler/src/validator.rs) 可以复用。后续工作是按入口收敛语义与证据，不需要另建一套互不相通的校验器。

### 1.3 支持状态必须由证据决定

| 状态 | 含义 | 文档与发布要求 |
|---|---|---|
| `supported` | 在指定语言版本、能力配置和执行入口下通过全部规定测试 | 绑定 fixture ID；CI 失败即不能发布该支持声明 |
| `experimental` | 有部分实现，但完整边界或行为验收未完成 | 仅可通过标明的实验入口启用；不自动纳入严格 Core 的支持能力，不能作为生产支持示例 |
| `planned` | 设计提案或尚未实现 | 明确标注，严格 Core 拒绝 |
| `deprecated` | 曾受支持、仍有兼容测试的旧写法 | 给出替代写法、诊断和移除版本 |

本提案的完整 Core 清单仍待验收；实验性 `cdl-core-risk-draft-1` 已有绑定 fixture 和真实引擎测试的部分 `supported` 能力，范围以 [机器可读清单](cdl/schema/capabilities.json) 为准。这不代表阶段 0 整体验收通过。历史参考页现已统一标为 `compatibility-unverified`，撤下未绑定证据的实现/生产支持徽标；它们的片段仍需逐项验收。仅能解析或编译，不足以升级为 `supported`。

这里的状态描述语言能力，不描述某份客户策略的业务效果。策略的语言校验、行为测试、业务评估和发布审批必须分别记录，见第 9.3 节。

### 1.4 进度标记（2026-09-05）

**本轮范围调整：先跑通通用 Agent，真实 Work 客户端集成移出当前 P0/P1。**
验收主线为“Agent 编写/修改 → 共享 CLI 严格校验与行为测试 → repo 发布 → 授权重载 → 真实决策与追溯”。
不要求 Work 账号、客户端或测试环境；保留该主线必需的源码包、导入/导出、目标检查与发布授权。
完整跨产品 PolicyPackage 和 Work 双向集成留待后续产品阶段，不作为本轮完成条件。
通用 Agent 不拥有隐式审批权限，repo 仍是策略唯一权威来源。见 [通用 Agent 执行流程](cdl/agent-workflow.md)。

**“已完成”仅指下表写明的交付范围**；“契约已完成”表示 schema、正反例及离线消费者已验收，不表示在线资源或真实产品集成已完成。语言能力是否 `supported` 仍以 §1.3 的能力清单和对应入口为准。

| 优化项 | 状态 | 已完成范围与证据 | 剩余工作 |
|---|---|---|---|
| 验收基线与 CI 配置修正 | **已完成（配置与基线修复）** | 修正 server 二进制名、安装 `protoc`、统一 fmt/Clippy 命令，修复已发现的基线失败；见 [CI](../.github/workflows/ci.yml) | 后续并发改动需重新验收；不代表远端 CI 已通过，release 工作流迁移另行处理 |
| 文档支持声明与示例门禁 | **已完成（登记范围内）** | 3 个严格参考页绑定 fixture；13 个兼容参考页统一范围声明；补充 5 个 guard/API/Service 反例；见 [示例清单](cdl/examples.json) 与 §11.3 | 历史片段逐例包装、LLM 兼容模板映射仍待补齐 |
| 严格 Core 校验与执行语义 | **已完成（首批 Core 范围）** | 版本/未知字段/必填/default 门禁、同步 Ruleset 调用、显式跳转及条件 Trace；见 §1.2 与 [conformance](../crates/corint-decision-engine/tests/cdl_core_conformance.rs) | 兼容入口收敛和未启用扩展仍待实现；拒绝能力不等于实现能力 |
| 独立工具链、生成校验与源码交换 | **已完成（离线首批）** | validate/test/build/verify、严格生成/修改、export/import 与冻结文件 import；见 [CLI](cdl/cli.md)、[生成接口](cdl/generation.md)、[源码交换](cdl/exchange.md) | 本轮优先验收通用 Agent 的 repo 发布与运行闭环；真实 Work 客户端、完整跨产品 PolicyPackage 移出本轮 |
| 业务上下文与目标兼容性 | **已完成（声明契约）** | BusinessContext / TargetCapabilities v1、共享 check-target 与旧绑定拒绝；见 [公共契约](contracts/README.md) | 远端就绪证明与生产权限治理仍待完善 |
| 严格 Core repo 发布 | **已完成（单实例）** | 发布声明与内容指纹、共用启动/重载验收、失败保留旧快照、repo 回滚与重启；见 [Core server](contracts/core-server.md) | 新增文件/SQLite/PostgreSQL/HTTP 完整发布文档消费；多节点与现有客户后端迁移后续处理 |
| 兼容 HTTP/gRPC/FFI 共享快照 | **已完成（现有协议）** | 一次初始化、锁外候选准备、原子切换、版本冲突与慢请求测试；见 [快照契约](contracts/compatibility-server-snapshots.md) | 共享引擎管理器、FFI 条件重载、gRPC 完整 Trace/特征与不支持参数拒绝已补齐；严格 Core 其他协议适配后续独立验收 |
| 通用 Agent 候选准备与执行 | **已完成（本地合成场景）** | 公开 `prepare-repository` 冻结源码、校验目标、运行样例并生成新 repo；普通文件经真实 CLI、独立操作员批准与真实 HTTP 执行/重载/重启；见 [流程](cdl/agent-workflow.md) | 后续补真实业务数据评估；新增运行保障见 §1.5 |
| W04：Feature / Model 描述与绑定 | **契约已完成** | 3 份 v1 schema、精确版本/内容/目标/能力检查、缺失或未部署绑定反例；见 [阶段 0 契约](contracts/phase0.md) | 在线 Feature/Model、真实部署状态校验及发布入口接入 |
| W07 及 W05/W06/W09：评估与审批证据 | **契约已完成** | 2 份 v1 schema、固定样例口径与历史可用时间、证据绑定、外部信任与审批有效期检查；见 [契约测试](../crates/corint-decision-toolchain/tests/phase0_contracts.rs) | 发布入口与本地信任/期限/撤销已接入；真实评估后端与签名身份服务仍由部署方提供 |
| W08：决策记录、标签与动作回执 | **契约已完成** | 3 份 v1 schema、内存消费者、关联/去重/更正/历史查询与回执校验；见 [阶段 0 契约](contracts/phase0.md) | Core HTTP 记录、SQLite 日志、租约投递、反馈消费者与重启恢复已完成；规模化存储/外部业务执行后续处理 |

本轮公共契约验收：新增 **8 组测试（包含 16 个清单反例）**、CLI 契约 **9 项**、Core conformance **16 项**通过，全仓 Clippy 通过。本次新增文件格式通过；全仓格式检查曾发现并发引擎文件差异，后续以最新工作区检查为准。这里记录本地验收结果，不将阶段 0 或 W01–W10 整体标为完成。

### 1.5 剩余 P0/P1 实施清单（本轮）

本轮以通用 Agent、单实例、操作员固定租户为验收范围，Work 与完整跨产品 PolicyPackage 不属于完成条件。
代码、配置及迁移说明见 [Core 运行保障](contracts/core-operations.md)。下表区分机制实现和外部生产接入。

| 优先级 | 优化项 | 状态 | 本轮交付与验收 |
|---|---|---|---|
| P0 | 兼容编译器不再静默忽略语义 | 已完成 | 全量步骤先验收再筛选可达节点；拒绝未实现 guard/调用/API 参数与组合；未知/混合 when、错误参数类型拒绝；终止分支不再穿透，结果依赖 router 在兼容入口明确拒绝；`compatibility_admission`、`when_admission` |
| P0 | 访问权限、凭据和可信输入边界 | 已完成（单租户本地信任根） | HTTP/gRPC 决策/发布角色分离、认证先于正文；禁用任意 CORS；客户端只提交 event，tenant 来自操作员；配置 Debug 与外部错误脱敏；`both_transports_enforce_roles...` |
| P1 | 实际版本绑定的 DecisionRecord | 已完成（Core HTTP） | 同一执行快照产出业务事件 ID、repo/runtime/完整 subject、输入证据、hold/pass/no_match/error 与动作幂等身份；v3 强制日志 |
| P1 | 持久化和可靠投递 | 已完成（单实例） | SQLite 原子提交、容量限制、租约/退避/重启恢复/确认；满或失败返回错误；旧 PostgreSQL writer 改为有界、等待事务确认；`durable_feedback` |
| P1 | 反馈与动作回执消费者 | 已完成（通用 HTTP） | 单租户关联、持久去重、冲突拒绝、标签更正及历史查询；回执匹配意图；并发标签只有一个成功；不依赖 Work |
| P1 | 评估/审批接入发布门禁 | 已完成（消费操作员认证结果） | 精确 subject、证据哈希、期限和角色检查；启动/重载/新决策重读信任文件以消费撤销；未配置业务评估不声称通过 |
| P1 | 共享领域执行与现有协议收敛 | 已完成（兼容 HTTP/gRPC/FFI） | manager 下沉引擎层；FFI 决策绑定版本、原子 repo 重载；gRPC signal/score/actions/完整 Trace/特征等价，不支持 options 明确拒绝；`shared_snapshots`、FFI ABI 测试 |
| P1 | 非文件 repo 的冻结发布消费 | 已完成（单文档协议） | 文件、SQLite、PostgreSQL、HTTP 共用严格闭包和发布身份；候选 CLI 输出 publication.json；SQLite/HTTP 与临时真实 PostgreSQL 事务测试 |

本地验证：全仓测试 1428 项通过、0 失败；全仓 Clippy（含所有 targets，warnings 为错误）、格式检查通过。另有 3 个真实进程端到端用例、临时 PostgreSQL 事务测试，以及本轮补充的控制流/持久化反例。忽略的历史外部集成用例不计入通过数；这不代表远端 CI 或客户环境已验证。

后续仍待推进：Work/完整 PolicyPackage、在线 Feature/Model 与真实业务评估、企业身份联合和多租户、
多节点发布、严格 Core 的 gRPC/FFI 适配、日志归档与高吞吐消费者。上述能力不因本轮局部验收升级为 supported。
已有数据库/API repo 的逐资源接口不会自动获得新协议保证，需发布方迁移到完整 publication.json 消费协议。

### 1.6 统一 Runtime 与表达式扩展

**已完成（严格 Core 共享执行路径）**：单 Rule、子 Pipeline、Pipeline/step guard、嵌套/可选输入、`exists`、数值算术和结构化错误。Rule/Ruleset/Pipeline 由同一个 Runtime 执行器同步求值；父子局部结果隔离，分数只汇总一次，子 actions 不隐式进入最终输出。

验收与边界见 [Runtime 扩展](cdl/runtime-extensions.md)：Trace 开/关的真实引擎测试、循环/深度/展开预算门禁、缺失输入/除零/溢出诊断，以及公开 CLI 候选 repo 验收。能力清单只升级这些已验收能力；兼容入口、Connector、在线资源和完整跨协议扩展仍需分别推进。

## 2. 目标架构与产品边界

```text
通用 Agent                              Corint Work
用户要求 + 提供/导出的业务上下文          授权业务数据 + 分析 / 特征 / 建模 / 优化
        │                                      │
        └─────────────────┬────────────────────┘
                          ▼
                 共享 CDL 工具链
          规范 / 校验 / 编译 / 测试 / 诊断
                          │
                          ▼
             策略包 + 验证 / 评估证据
                          │ 审批 / 依赖绑定 / 目标构建 / 发布
                          ▼
                  Corint Decision
          固定版本执行 / Trace / 动作意图
                          │
                          ▼
        决策记录 ──→ 与业务系统的动作/后验结果关联 ──→ Corint Work
```

### 2.1 产品独立性

- **Work** 负责持续业务上下文、受治理的数据访问、分析与实验、策略协作和优化流程；通过公共契约调用工具链和发布接口。
- **Decision** 只依赖已发布策略、明确的运行时依赖和请求输入，不依赖 Work 会话或其私有数据库来解释规则。Work 不可用时，已部署策略仍应正常执行。
- **共享工具链** 是可独立使用的基础能力，不是必须再部署的第三个产品。客户可不使用 Work，仍完成策略校验、测试、构建及授权发布。
- 数据能力按授权和工具可用性控制，不以“是否由 Work 内置 Agent 生成”授予特殊权限。未来若允许外部 Agent 接入受控数据工具，也应复用相同接口和治理规则。

Work 的差异化在于业务上下文、数据证据与运营闭环，而不是隐藏语法或另一套编译器。策略可移植指在具备等价依赖的环境中执行，不代表脱离所需业务数据也能得出有效判断。

### 2.2 可独立分发的 CDL 工具链

先复用现有 parser / compiler / engine 的公共实现，提供独立库与可运行的 CLI，再按需要提供 SDK、HTTP 和 MCP 适配；所有入口使用同一套规范与验证内核。MCP 是可选接入层，不是 CDL 的语言依赖。

建议的接口职责包括：读取规范与目标能力、验证资源与引用、运行行为测试、比较策略语义差异、构建策略包及输出结构化诊断。这是接口设计，不表示当前已经存在对应命令或服务。

离线工具必须能使用本地业务上下文和固定测试输入工作，不要求 Work 账号或业务数据连接。真实数据评估属于单独的受授权能力；不能因为调用方没有该能力，就把“未评估”改写为“评估通过”。

### 2.3 仓库解析与编译对象

`ResolvedRepository` 是控制面的结果，负责：

- 解析 import、相对路径和稳定资源 ID；
- 校验 registry、Pipeline、ruleset、rule 和 connector 的所有引用；
- 校验 schema、CDL 版本、连接器能力和权限；
- 生成可读的诊断信息（文件、行号、引用链）。

`CompiledRepository` 是数据面的不可变产物，包含已编译的 Program、依赖版本、内容哈希及运行时所需配置。每次请求只能读取其中一个确定版本；重载必须“加载 → 校验 → 编译 → 预热 → 原子替换”，失败时继续服务旧版本。

跨工具交换的 `PolicyPackage` 是源码、契约、依赖及关联证据的交付单元；它通过上述加载链路成为针对目标环境的编译产物。两者需要可追溯关联，但源码包可交换不代表 IR 可以跨任意引擎版本直接加载。

这是目标架构，不要求在首期验收前一次性重写全部模块。可以先复用现有装配链路建立一致性测试，但任何尚未满足执行契约的能力都不能绕过门禁。

### 2.4 Repository 作为唯一权威策略来源

策略源码、依赖和已发布版本的选择统一由 repository 管理。运行中的编译引擎是 repository 某个确定版本的派生快照，不另建独立的策略持久化或内存上传发布通道。Agent、Work 与 API 创作最终都应进入同一 repository 发布流程。

- 发布方先验证候选源码、目标契约和独立行为样例，再在 repo 中发布确定版本及内容指纹。
- 服务启动与重载共用严格加载链路：读取 repo 已发布声明 → 冻结依赖闭包 → 核对内容指纹 → 严格编译、目标检查、批准匹配和行为验收 → 构建不可变引擎快照。
- 重载只在验收成功且运行版本未被其他重载替换时原子切换；请求取得一次快照后完成整个决策。失败继续使用旧快照。
- 重启重新读取 repo 的已发布版本，不恢复上次进程的临时内存状态。若 repo 被外部修改为无效版本，启动失败，不静默回退到旧策略或宽松入口。
- 回滚由发布方在 repo 恢复历史版本，再触发同一验收和重载流程；回滚不绕过当前授权或验收。
- HTTP 重载请求不接受策略正文、文件路径、审批或验收样例。发布历史和版本存储由 repo 后端承担，服务端仅报告实际加载的 repo 版本及指纹。

当前增量已将上述流程接入严格 Core 服务端的文件 repository（配置 v2 与 published.json），启动和重载复用同一验收链路。兼容 HTTP/gRPC 已共享同一进程快照与 repo 重载管理器，见 §10.2；新增 SQLite/PostgreSQL/HTTP 完整发布文档已共用严格验收，FFI 已共用兼容快照；旧逐资源后端与严格 Core 的其他协议适配仍需迁移。

## 3. CDL 的职责边界

| 层级 | 应负责 | 不应负责 |
|---|---|---|
| Rule | 纯条件、命中事实、分数贡献；后续可扩展 typed fact / reason code | 调外部 API、写数据库、触发通知 |
| Ruleset | 聚合规则结果，输出局部决策候选 | 隐式修改全局流程或外部状态 |
| Pipeline | 有界 DAG、组织 ruleset；通过显式 `decision` 产出最终结果；扩展中准备数据、调用 connector | 依赖隐含顺序决定多 ruleset 的最终优先级 |
| Registry | 按事件类型、租户、场景选择入口 Pipeline | 重复承载复杂业务决策 |
| Connector 配置 | Endpoint、凭据引用、超时、重试、熔断、数据源 | 混入具体业务规则 |

最终决策统一由 `pipeline.decision` 承担：读取 `ruleset.conclusion` 产生的局部 signal 与规则集得分，通过显式条件和优先级确定最终结果、动作及原因。保持 Rule → Ruleset → Pipeline 三层，不增加独立的最终决策实体，也不让规则集执行先后隐式决定最终结果。

规则集的局部输出通过现有结果上下文供 Pipeline 读取；输出契约可以完善，但不需要新增一类语言资源。现有 Rule 的 `score` 是必填整数，不能因目标架构允许纯事实输出，就在当前示例中将它省略。

## 4. 单一、可验证的执行模型

Runtime 应持有只读的 `ProgramRegistry`，并原生支持：

```text
CallRule → CallRuleset → CallPipeline → CallConnector
```

Pipeline 的 `CallRuleset` 必须具有实际调用语义：返回时规则集结果已可供后续节点读取，不能仅向上下文写入约定名称，再在整个 Pipeline 扫描完毕后补执行。严格 Core 已将 Rule/Ruleset/Pipeline 同步调用下沉 Runtime；兼容路径的历史后处理仍需独立迁移。SDK 的职责应限于装配、请求入口、版本切换、结果封装和观测；它不应重演 Runtime 的控制流。

这样可以让以下行为拥有唯一语义：

- 单条 rule 调用；
- ruleset 调用；
- 子 Pipeline 调用；
- step guard；
- 失败、超时、fallback 和取消；
- 嵌套调用的 Trace、score 和 action 继承规则。

验收应覆盖 `ruleset → router（读取该 ruleset 的结果）→ decision`，并断言未选中的分支没有执行。只核对 IR 中是否存在 `CallRuleset`，无法证明此契约成立。

## 5. 表达式与上下文规范

### 5.1 只保留一个表达式模型

首期保留现有字符串表达式及 `all / any / not` 写法，并归一化为同一个有类型的 AST；Registry、Rule、Pipeline 条件和 Trace 使用相同语义。不要让字符串、模板、相对字段和多套布尔结构各自拥有不同的运行时解释器。

后续可以提供以下结构化序列化形式，便于 Agent 受约束生成；**当前解析器尚不支持这套 `op / ref` 写法，它不是首期必需的新语法**：

```yaml
when:
  all:
    - op: gt
      left: { ref: event.amount }
      right: 1000
    - op: eq
      left: { ref: event.currency }
      right: CNY
```

### 5.2 强制命名空间

字段引用应始终显式使用命名空间：

```text
event.amount
features.user_7d_txn_count
api.ip_reputation.country
service.account.status
vars.channel
results.payment_rules.score
```

禁止无法判定来源的裸字段，例如 `amount` 或 `risk_score`。现有 Ruleset conclusion 中的 `total_score`、`triggered_rules` 等是有明确作用域的保留聚合变量，应在符号表中显式定义，不应与任意裸字段混为一谈。

输入、结果和步骤输出还应具备 schema，以便在编译期发现拼写错误、类型错误和缺失依赖。复用现有 [Schema 模型](../crates/corint-decision-model/src/types/schema.rs)，把它接入编译与请求校验；仅存在类型定义不等于已经完成静态类型检查。

### 5.3 首期表达式与求值契约（待实现验收）

当前增量：[条件观察 v1](cdl/condition-trace.md) 已从真实 VM 收集五类 Core 条件作用域的布尔树结果，区分 `evaluated(false)`、`short_circuit` 与 `not_reached`，并以完整结果比较和故障注入验证 Trace 不改变求值。该字段不暴露原始操作数；未调用资源不生成伪造记录，执行失败仍返回错误。完整操作数、失败部分 Trace、脱敏/采样与审计存储仍待设计。

- 首期覆盖布尔、字符串和有限数值的字面量、已声明字段、比较、布尔组合和括号；函数、隐式类型转换和非确定性求值不自动进入 Core。
- `all / any` 必须为非空条件列表；`not` 只接受一个条件，可通过嵌套 `all / any` 表达组合取反。空条件组、多个组键并存、未知键均报错；旧版多条件 `not` 不得在未提示的情况下改变含义。
- 布尔组合从左到右短路；启用 Trace 不得使原本不求值的条件被执行，也不得改变最终结果。
- 输入 schema 区分必填、可选和允许 `null`；缺失必填字段或类型不符在求值前返回输入错误。`null` 是值，不等价于缺失、未执行或调用失败；可空值参与非空运算前必须显式保护。
- 首期 Risk Profile 中单条规则分数沿用 `i32`；累加必须检查溢出。数值表示、比较边界和允许范围写入类型契约，禁止静默截断、溢出或把字符串自动转成数字。

结果状态也需要独立建模，不能只剩一个布尔值：

| 状态 | 含义 | 决策处理要求 |
|---|---|---|
| `success(value)` | 已完成求值，值可以是 `false` 或显式 `null` | 按声明类型使用 |
| `unknown` | 所需证据缺失、不可用或无法判断 | 不得自动等同于“不命中” |
| `skipped` | 分支未选择或 guard 不满足，未执行 | 不产生分数，不冒充成功输出 |
| `error` | 输入校验、执行、超时或依赖失败 | 不得自动变为 `approve` |

这些是目标状态模型，不是当前对外响应字段。首期纯输入 Core 对无法满足前置条件的请求返回显式错误，不承诺完整的 `unknown` 传播代数；后续扩展必须定义这些状态如何进入 fallback、review 或拒绝路径。表达式层的 `false` 与策略层的“没有选中 Pipeline”也必须区分。

### 5.4 业务上下文与目标环境能力契约

生成可执行策略需要以下三类版本化信息。后两类是包/控制面的公共契约，不是新增的 CDL 顶层语法。[公共契约首批 schema 与检查器](contracts/README.md) 已实现 Core 子集；下表描述完整目标，不表示特征/模型、在线资源或权限核验已支持。

| 契约 | 必须描述的信息 | 解决的问题 |
|---|---|---|
| CDL 规范 | 资源 schema、表达式、执行与错误语义、语言能力 | 规则怎样表达、怎样执行 |
| `BusinessContext` | 事件与实体、字段类型/单位/含义、时间口径、特征与模型目录、业务目标与验收约束、允许引用的动作类型 | 客户实际有哪些可用概念，如何正确使用 |
| `TargetCapabilities` | 引擎版本、CDL/Profile 版本、已启用能力、资源版本与部署可用性、预算及权限约束 | 草案能否在指定目标环境运行 |

业务上下文可由客户维护或由 Work 辅助整理、经业务负责人确认后发布。它应支持导出最小必要的字段说明、目录和脱敏样例，不要求导出真实记录；目录、样例和统计信息本身也需经过数据分类与访问控制。

例如，`amount` 是元还是分、24 小时次数按用户还是设备计算、是否包含当前事件，都必须由契约说明，不能由 Agent 猜测。没有上下文时可以生成带假设的草案，但缺失字段、特征或绑定必须成为明确诊断，不能伪造资源后声称已可执行。

上下文与目标能力声明应包含版本、来源和完整性标识。编译器根据它们检查字段、类型与引用；目标环境在部署时重新核验实际依赖和授权。Agent 提供的目录或权限描述不是授权本身。

### 5.5 特征与模型从研究到执行的契约

当前增量：[Feature/Model 描述及资源绑定 v1](contracts/phase0.md) 已有 schema、离线消费者和 W04/W07 固定契约用例；在线计算、健康验证及运行时调用仍未启用。

Work 中分析有效的资源，不一定已具备在线服务能力。以下契约应在首期定义，计算、训练与服务实现仍按能力扩展逐项落地：

| 资源契约 | 最小内容 |
|---|---|
| Feature | 稳定 ID 与定义版本、实体键、输入/输出类型和单位、窗口与去重口径、事件时间及数据实际可用时间、缺失值与过期处理、离线计算和在线获取的绑定 |
| Model | 稳定 ID 与精确产物版本、输入特征及预处理版本、输出类型与业务含义、所需服务能力、超时/失败策略及部署状态 |

同名特征在分析与执行中必须保持同一业务口径；不同计算后端通过固定输入的等价性用例验证，而不是仅比较名称。历史评估只能使用决策当时已经可用的特征，不能利用后来补录、更正或标签泄漏的数据。训练、调参和评估数据应按预先确定的方法隔离。

资源需要区分“已有定义”“经过离线评估”和“已在目标环境就绪”。引用了未部署的模型、未物化的特征或不兼容的资源版本，必须阻断发布或明确报告待绑定；不能在请求时自动运行实验 SQL、训练任务或静默换用另一版本。

CDL 引用逻辑资源，目标绑定解析到具体服务或存储；不把 Notebook 路径、训练流程或 Work 内部对象 ID 当作唯一执行依据。声明这些契约不自动启用模型节点或新的命名空间，仍需满足 Core 的能力门禁。

## 6. Pipeline 与 Connector 设计

Pipeline 应是有界、显式输入输出的 DAG。每个步骤都需要明确：输入、输出、失败策略、超时预算和下一个节点。

以下统一 Connector 写法属于后续提案，不是当前 `type: api / service` 的可直接替换语法：

```yaml
step:
  id: ip_reputation
  kind: connector
  connector: ipinfo.lookup
  with:
    ip: { ref: event.ip_address }
  output: api.ip_reputation
  timeout_ms: 150
  on_error: use_fallback
  fallback:
    country: unknown
  next: payment_rules
```

连接器配置与策略文件分离。Secret 只能使用引用，例如 `secretRef: ipinfo_token`，不能出现在 Pipeline、规则库或 quickstart 中。

所有声明的 step 都必须满足二选一：

1. 编译为具有完整确定语义的 IR 指令；或
2. 在编译期失败，并指出不支持的字段和位置。

禁止 no-op、丢弃参数、只执行列表中的第一项等静默降级行为。

首期 Core 暂不包含 Connector；现有 API / Service 能力可继续单独维护，但不据此宣称通过 Core 验收。扩展进入支持集前必须验证输入绑定、输出 schema、超时、取消、fallback，以及 `any / all / min_success` 的实际调用次数与结果。

## 7. 副作用与最终决策

决策引擎应产生动作意图，而不是在规则匹配时直接实施外部副作用。以下是**后续结构化决策结果契约示意，不是当前 Pipeline 的 `decision` 数组语法**：

```yaml
decision:
  signal: review
  actions:
    - type: create_review_case
      payload:
        queue: high_risk
```

下游可靠执行器负责实际发消息、封禁账户或创建工单，并提供幂等键、重试、失败补偿和审计。这样重放历史决策不会重复执行副作用。

action 的类型、payload schema、允许调用方和权限由可信配置声明。LLM 或外部响应给出的 action 名称不是授权；动作执行器仍需独立鉴权。

`pipeline.decision` 的最终决策语义需要明确：

- 多 ruleset 的 signal 优先级；
- score 与 signal 冲突时的规则；
- actions 的累积、去重和覆盖；
- reason / explanation 的构成；
- 所有决策条件均未命中时的 default 结果。

没有命中 Pipeline 属于 Registry / 引擎入口的处理范围，不由未执行的 `pipeline.decision` 兜底。若使用 raw score 到 canonical score 的标定，应由 Risk Profile 定义并锁定标定版本，最终决策使用含义明确的分数。

首期沿用明确的 first-match 规则：`conclusion` 和 `decision` 按数组顺序选择第一条命中项，各自必须且只能有一个末尾 default。仅采纳被选中项的结果；不隐式累积未命中项的 action，也不猜测跨 Ruleset 的 signal 优先级。新聚合模式、结构化 action 和通用 typed result 后续另行版本化。

## 8. Registry、版本与兼容性

### 8.1 路由与版本边界

Registry 只负责选择入口 Pipeline；复杂业务判断应在 Pipeline 或 Ruleset 内完成。Pipeline 的 `when` 可作为防御性前置条件，但不应与 Registry 重复维护同一套路由规则。

需要分清三种版本：语言语法/语义版本、能力配置版本、策略包版本。现有 `version: "0.1"` 不足以表达某个引擎是否完整支持某项能力；本提案的 “CDL Core 0.1” 只是工作名称，不能据此认定当前 `0.1` 文件已符合新契约。

严格发布入口必须拒绝未知版本、未知语义字段和未启用能力。旧文件缺省版本、旧表达式和旧 Pipeline 格式只能通过显式兼容入口迁移，输出诊断及规范化结果；不能在严格校验失败后自动退回宽松解析。`metadata` 可保留声明过的扩展空间，但 metadata 中的内容不能改变执行语义。

### 8.2 可交换的策略包

当前已有[实验性源码包构建与核验](cdl/packages.md)：`corint build / verify` 保存完整 Core 源码与输入 schema，以指纹绑定独立保存的测试集、工具二进制及重新运行的结果摘要。默认不导出测试输入，不包含签名、环境绑定或发布审批，不代表下述完整 PolicyPackage 已完成。

用户可以只编辑一个 YAML 文件，但发布交付物应是依赖明确、可验证的 `PolicyPackage`。包契约在首期定义，具体封装格式和分发服务分阶段实现；它不要求立即新增 CDL 顶层关键字。

策略包至少包含或以不可变引用关联：

- CDL 源文件、入口及语言/Profile/能力要求；
- 输入与资源契约、业务上下文版本、精确依赖及锁定信息；
- 行为测试样例与业务验收条件；
- 验证/评估报告、来源与发布审批记录。

真实数据集、凭据和 Work 私有会话不应随包默认导出。报告可引用受控证据，导出时遵守独立的访问权限；移除受保护证据不能被解释成该证据不存在或已验证通过。

首期保留文件 import，校验完整引用闭包、重复 ID、循环依赖和路径越界。资源引用后续可迁移到稳定 ID 与包版本；以下包 manifest 只是尚未实现的格式提案，不表示语言版本已升级为 `corint/v2`：

```yaml
apiVersion: corint/v2
kind: Package
metadata:
  name: payment-risk
  version: 1.4.0
dependencies:
  - name: common-fraud-rules
    version: ^2.1
```

版本范围只能在控制面解析，发布时必须锁定为精确版本与内容哈希，运行时不得重新解析 `^2.1` 或读取浮动的 `latest`。

每个 `CompiledRepository` 应记录：CDL 版本、能力配置、包版本、依赖图、编译器版本、内容哈希和发布时间，以支持回放、审计、灰度发布和回滚。

### 8.3 环境绑定与证据有效性

当前增量：`corint check-target` 与严格生成器的目标入口共用检查器，绑定策略内容、上下文原文、目标声明和检查程序指纹，拒绝复用不匹配的旧绑定。此报告独立于 source package v1，仅证明声明兼容；不核验远端状态、业务语义或授权，不可充当发布审批。见 [契约边界与指纹算法](contracts/README.md)。

策略的逻辑资源引用与环境绑定分开管理。发布时解析到精确 Feature / Model / Connector 版本与物理服务，检查目标兼容性、就绪状态及授权，再构建和激活编译产物。不能因 Work 中存在该资源，就跳过目标环境检查。

策略内容哈希覆盖执行相关源文件、输入契约和精确逻辑依赖；环境绑定具有独立版本/哈希。验证报告按这些标识关联，报告和审批记录不反向参与策略哈希计算，避免循环引用。发布记录还应绑定目标引擎及最终编译产物的标识。

策略阈值、依赖版本、输入契约、绑定或目标执行语义发生变化后，旧证据必须按适用范围重新判定，必要时重新验证和审批；旧报告保留用于审计，但不能自动为新版本背书。由可信工具产生的运行记录与可信审批者的授权才可作为证据，Agent 自行填入的“已通过”字段不具备效力。

### 8.4 通用 Agent 与 Work 的双向编辑

必须支持“外部编辑 → Work 导入 → Work 修改 → 导出后再编辑”，并满足：

- 资源 ID 稳定，纯导入/导出与格式规范化不改变执行语义；业务修改只产生可评审的预期差异，所有执行必要信息可通过公共契约取得。
- 会话、图布局、实验笔记等作为非执行附属信息保存，不含 Work 专属的隐藏执行逻辑；注释和作者信息尽量保留。
- 修改携带基准版本，发生并发修改时报告冲突，不直接覆盖；评审同时展示文本差异与语义差异。
- 未识别的语义字段明确拒绝，不能在导入或导出时悄悄丢弃；格式迁移给出诊断并经过等价性测试。
- 修改后按第 8.3 节更新证据适用状态，不能保留界面上的旧“已验证”标记冒充新版本结论。

## 9. Agent / LLM 的生成、验证与权限闭环

LLM 适合首先位于控制面：生成规则草案、解释 Trace、发现冲突、生成测试用例、辅助迁移 CDL。它不应默认进入实时决策热路径。

### 9.1 与人工编写共用同一套发布门禁

已落地的服务端子集：[严格 Core 本地模式](contracts/core-server.md) 由 `CORINT_CORE_CONFIG` 显式启用，服务端持有上下文、目标、独立样例和操作者批准指纹列表；HTTP 决策/重载凭据分离，客户端不能提交审批或替换验收样例。策略统一从 repo 已发布声明读取；必须重新编译、检查并通过真实引擎样例后才能切换运行快照，HTTP 不接受策略正文。只允许 loopback、单目标，策略持久化与版本历史由 repo 管理，尚无企业身份集成；旧 REST/gRPC 入口未自动获得这些保证。

推荐流程：需求与授权范围 → 读取受支持 schema、业务上下文及目标能力 → 生成草案 → 解析、引用解析、类型检查与编译 → 沙箱行为测试 → 按场景进行业务评估 → 评审与审批 → 发布不可变产物。

- 为 Agent 提供机器可读的资源 schema、表达式/函数签名、能力清单、错误码和通过验证的例子；结构合法仍不代表策略符合业务意图。
- 复用现有 `Diagnostic` 并补齐 source path、字段路径、阶段、稳定错误码和可用的源码范围；同一个错误在 CLI、服务端和生成器中含义一致。
- Prompt 和生成器必须来自同一版本契约。已新增可选 [严格生成入口](cdl/generation.md)，直接嵌入 Core 规范、schema 和 conformance fixture；复用共享工具链完成生成/修改后的编译、真实引擎样例测试和源码包构建。调用方固定输入契约和独立验收样例，模型不能修改期望或提交验证声明，样例不发送给模型。现有 [规则生成器](../crates/corint-decision-llm/src/generator/rule_generator.rs) 与 [模板](../crates/corint-decision-llm/src/generator/prompt_templates.rs) 保留兼容语义，不能据此声称通过 Core 门禁；旧字段/缺失字段已纳入严格入口反例。
- 可以基于结构化诊断进行有次数上限的自动修复，但修复不能修改 schema、提升权限、跳过测试或自行批准发布。
- 文档、检索内容、事件数据和模型响应均是非可信数据，不能成为引擎指令、凭据来源或授权依据。编写、测试、审批、发布应具有分开的权限。

### 9.2 在线模型仅作为受控的证据来源

若未来提供 LLM connector，必须显式声明模型及版本、输入数据范围、输出 schema、超时、token/成本上限、缓存、fallback 和适用的人工审批要求；它应被视为不确定且可失败的外部依赖。

输出需要记录来源、请求/响应指纹和必要的受控证据。模型建议不能直接覆盖可信规则或执行高权限 action；最终处置仍由已发布策略决定。测试使用固定响应的 mock，回放使用历史证据，不能重新询问模型后声称重现了历史决策。

### 9.3 分级验证，不使用笼统的“已验证”

| 证据类别 | 能证明的范围 | 不能据此推断 |
|---|---|---|
| 语言与环境校验 | 结构、类型、引用与目标能力合法，可针对所声明环境编译/构建 | 策略符合业务意图或具有业务收益 |
| 行为测试 | 指定输入及边界条件下，结果、路径与调用符合预期 | 对真实客户群体同样有效 |
| 业务效果评估 | 在特定数据、标签、时间窗口、基线及评估方法下得到某些指标 | 上线后一定改善业务，或具有已证实的因果收益 |
| 发布审批 | 具备该环境要求的证据与权限，批准发布该精确版本 | 可以跳过前面的验证或任意扩大授权 |

这四类证据分别记录状态，例如未运行、通过、失败、不适用及其原因，不应压缩成一个 `validated: true`。语言校验可保留现有 `ValidationResult` 的含义；业务评估报告与发布审批应是独立对象。

通用 Agent 在具备声明上下文和本地工具时可以完成语言校验与行为测试；没有真实数据时应明确标记业务评估未进行。Work 提供受授权的真实数据评估能力，但使用 Work 不自动获得更高验证等级。发布要求由可信的场景/环境治理规则决定，不强制将 Work 作为唯一验证者或发布入口。

Agent 同时生成规则与测试可能重复同一种理解错误，必须保留业务方维护或独立评审的验收样例。效果评估需记录样本覆盖、标签成熟度、留出集、基线、误报/漏报与人工审核容量等约束；不能只选择表现良好的样本或只报告单一指标。

### 9.4 可追溯的验证与评估报告

报告契约应至少包含：

- 策略内容哈希、业务上下文/依赖/绑定版本、目标引擎及能力配置；
- 验证工具与版本、运行 ID、执行时间、证据类别、状态及诊断；
- 测试集或数据快照标识、输入 schema、数据时间范围和标签定义/版本；
- 评估方法、基线版本、指标、适用范围、限制和必要的受控证据引用。

报告需要明确 mock、合成样例与真实数据的区别。缺少某类证据时保留缺口，不用空数据集、默认成功或 Agent 自述替代。报告来源须可验证，签名或可信服务记录的具体机制可分阶段实现；生产发布不能接受不可验证的自我声明。

## 10. 系统架构优化

除 CDL 本身外，系统架构需要优先收敛三组边界：**控制面与数据面、同步决策与异步副作用、引擎版本与请求生命周期**。

```text
Control Plane
  编辑 / 校验 / 编译 / 审批 / 发布 / 回滚 / LLM 辅助
                              │
                              ▼
                    Repository（权威源码与发布版本）
                              │ 严格加载并构建不可变编译快照
                              ▼
Data Plane
  API Gateway ──> Decision Worker ──> Connector Layer ──> 数据源
                         │
                         ├─> Trace / Audit / Replay Store
                         └─> Durable Outbox ──> Action Worker
```

### 10.1 控制面与数据面分离

在线请求路径不应同时承担 CDL 编辑、校验、编译和发布。先做到逻辑边界与权限分离，是否拆成独立服务可按部署规模决定。

Work 是控制面的业务创作与运营入口之一，不等于全部控制面。共享工具链和授权发布接口也要可被外部工具独立使用，避免 Decision 的部署与持续服务依赖 Work 在线。

| 平面 | 职责 |
|---|---|
| 控制面 | 策略编辑、schema 校验、编译、测试、审批、灰度、发布、回滚、Agent / LLM 辅助 |
| 数据面 | 读取已发布的 `CompiledRepository`，执行实时决策、记录受控 Trace、返回结果 |

发布动作应产出不可变版本；数据面只切换已验证的版本。仓库重载失败时继续服务旧版本，不能让半成品配置进入请求路径。管理操作需要单独认证、授权和审计，而不应作为可任意调用的业务 API。

### 10.2 请求快照与原子热更新

当前证据：上述严格 Core HTTP 模式已将引擎与策略身份放入同一不可变 `Arc` 快照，在准备候选后以 revision 比较并原子替换；失败保留旧版本，每个决策响应附带其执行快照指纹。已有并发激活与切换前后真实决策测试。文件 repo 增量使启动、重载和回滚均读取唯一已发布来源，响应附带 repo 版本/声明指纹；不包含跨进程协调、企业发布审计或完整 HTTP/gRPC/FFI 统一快照。

兼容服务增量：普通 HTTP 与 gRPC 现在只初始化一次引擎，共享 `EngineManager` 的当前 `Arc<EngineSnapshot>`。请求执行期间不持全局锁；repo 候选在 worker 中准备后以 revision 比较并切换，失败保留旧快照，并发准备被拒绝。两个入口的决策、健康与重载响应携带相同语义的运行 revision 和 compiled SHA-256；重载可携带预期 revision。该哈希标识编译策略，不冒充 repo 发布版本或严格 Core 策略哈希。真实处理器测试覆盖跨入口切换、回滚/重新构建、错误 repo 和等待连接器的旧请求。严格 Core 的独立验收与鉴权边界保持不变。见 [兼容服务快照契约](contracts/compatibility-server-snapshots.md)。

每个请求应在入口处绑定一个不可变引擎快照，例如持有 `CompiledRepository` 的 `Arc<CompiledEngine>`，随后立即释放版本索引锁。决策中的 API、数据库和特征调用可以耗时，但不能持续占用全局读锁、阻塞发布或重载。

推荐流程：

```text
加载候选版本 → 引用校验 → 编译 → 预热/健康检查 → 原子替换当前指针
                                                    ↓
请求开始 → 获取 Engine 快照 → 执行完整决策 → 返回带版本号的结果
```

同一服务实例中的 HTTP、gRPC 和 FFI 入口必须读取同一份当前版本指针，而不是分别初始化独立引擎；跨进程部署则按精确版本协调发布。决策结果与 Trace 至少记录 repository version、内容哈希、发布时间和 CDL version，以支持准确回放。

### 10.3 Connector 韧性与资源生命周期

Feature、List、外部 API、Service 和数据源应通过统一 Connector Layer 接入，并拥有一致的运行时契约：

- 输入/输出 schema 与参数绑定；
- 请求级 deadline、连接器超时和总延迟预算；
- 连接池、限流、熔断、重试和明确 fallback；
- 健康检查、能力声明与版本兼容性；
- 从 Secret Provider 注入凭据，禁止将 secret 写入 CDL 或 Trace；
- 记录数据来源、耗时、错误和降级路径。

外部依赖不应让单个慢请求拖垮整个决策节点；每个 connector 都必须声明失败时是拒绝、降级、跳过还是进入人工审核。

### 10.4 同步决策与异步副作用分离

Decision Engine 只生成决策和动作意图；实际封禁、通知、工单、消息推送等由异步执行器完成：

```text
Decision Result + Action Intent
             ↓
       Durable Outbox / Event Bus
             ↓
        Idempotent Action Worker
```

Action Worker 负责幂等键、重试、失败补偿、权限和执行审计。这样重放历史决策不会重复发短信、重复封禁账户或重复创建工单。

### 10.5 API、身份与多租户边界

REST、gRPC 和 FFI 应共用版本化的领域契约，统一 request namespaces、signal、score、action、trace 和错误码。API Gateway 或 Server 边界负责：

- 身份认证、细粒度授权和租户识别；
- CORS、限流、请求大小和超时限制；
- idempotency key、API 版本协商和输入 schema 校验；
- 敏感数据脱敏及 Trace 访问控制。

若面向多个业务线或客户，还需隔离 tenant 的 repository、connector、secret、缓存、审计数据和发布权限。外部 URL、连接器类型和数据源权限必须使用白名单，避免策略配置成为 SSRF 或越权访问入口。

查看业务上下文、查询数据、修改草案、执行评估、审批、发布及执行动作应分别授权。Work 的真实数据访问优先通过只读副本、数仓或受控查询服务，限制字段、租户、查询成本、导出范围和模型可接收的数据；默认不允许 Agent 任意操作生产主库。

Decision 还必须区分调用方可声明的数据与可信运行时证据。请求携带 `features`、模型分数或权限字段，不代表这些值可信；哪些主体可以提供或覆盖它们，应由目标环境的输入与权限契约决定。测试注入能力不得绕过生产入口的信任边界。

### 10.6 审计、回放与数据治理

风险决策需要可重建的证据链，而不只是日志。审计记录应包含：

- 受控保存的输入快照，以及用于检索和完整性检查的输入指纹；
- 所使用的 CDL、Pipeline、Rule、Connector、编译器精确版本；
- 参与求值的 Feature、List、API / Service 和模型输出证据及其版本；
- 请求使用的时钟、随机值等非确定性输入（如对应扩展允许使用）；
- 特征来源、路由路径、规则命中、错误、跳过和 fallback；
- 最终 decision、action intent 与 action 执行状态。

在线决策结果库、审计/回放库和分析数仓应按用途分离，避免分析查询或长期保存策略影响实时请求延迟。Trace 还应支持采样和字段脱敏，防止可观测性系统本身泄露敏感数据。

指纹本身不能重建输入；脱敏也可能移除重新求值所需信息。审计摘要与受访问控制的回放证据应分开管理，按保留期限存储必要数据。只有精确策略产物与全部必要证据仍可取得时，才能宣称该请求可确定性重放；证据不完整应报告不可重放，不得查询实时数据补齐后冒充历史结果。

### 10.7 部署与容量模型

Decision Worker 应尽量无状态：规则包和版本来自受控分发，持久状态落在外部存储，连接池和缓存按节点管理。这样可以水平扩容，并允许同一版本在多个节点上稳定运行。

发布应支持灰度、版本锁定、快速回滚和容量预热；性能压测则应按典型 Pipeline、连接器延迟、缓存命中率和降级路径分别评估，而不是只测纯规则计算。

### 10.8 Decision 到 Work 的反馈契约

当前增量：[DecisionRecord / OutcomeEvent / ActionReceipt v1](contracts/phase0.md) 已有 schema 和内存参考消费者，覆盖 W08 关联、去重、更正与历史可用时间；现有 HTTP 响应、持久化投递和真实 Work 消费尚未接入。

持续优化需要把决策、实际动作和后验业务结果关联起来，而不只是收集最终 signal。以下是首期需要确定的版本化事件契约，不表示现有 API 已具备这些字段：

| 记录 | 最小关联与证据信息 |
|---|---|
| `DecisionRecord` | 租户、决策 ID、业务事件 ID、决策时间、策略/引擎/绑定版本、必要输入证据引用、特征/模型版本、命中/原因、结果、错误和耗时 |
| 动作执行回执 | 决策 ID、动作 ID/幂等键、实际执行状态与时间、失败或人工干预记录 |
| `OutcomeEvent` | 决策/业务事件关联、事实或标签来源、标签版本、事件发生时间与观察/可用时间、更正记录 |

后验标签可能延迟、缺失或被更正；应可去重、版本化并按评估时间重建。被拦截的交易没有产生损失，不等于原本没有风险；未观察到标签也不等于安全。评估必须区分决策造成的干预、标签选择偏差和真实业务事实。

反馈通过异步、受控接口送达 Work 或其他分析系统，不在 Decision 同步请求链路中调用 Work。最小关联字段应在生产上线前落地，Work 的完整消费与优化功能可以后续实现；不能期望事后补回从未记录的关联与证据。

## 11. 落地顺序与首期交付

### 阶段 0：公共契约、CDL Core 与一致性门禁

这一阶段应先于大规模架构重写，分为两条相互约束的交付线：

- **可执行核心**：规范、独立工具链、真实行为测试与严格发布校验。
- **跨产品公共契约**：业务上下文、目标能力、Feature / Model 资源、策略包、验证/评估报告及反馈事件的 schema 与契约测试。

先固定接口并验证最小交互，不要求同时完成真实数据连接、模型训练服务和完整 Work 产品。首批已新增实验性 `cdl-core-risk-draft-1`：公开结构 schema、显式严格 Rust 入口、闭包校验、同步规则集调用和真实引擎 runner。现已增加 [离线 CLI](cdl/cli.md)：`corint validate` 校验编译；[`corint test`](cdl/testing.md) 用真实引擎核对声明样例、路径与 Trace 一致性；[`corint build / verify`](cdl/packages.md) 构建源码快照、绑定内容指纹并重新核验测试证据。[严格生成/修改 API](cdl/generation.md) 已复用 `corint-decision-toolchain` 完成同一闭环；源码 export/import 已支持可编辑源码交换与新证据重建。本轮增加 [BusinessContext / TargetCapabilities 和 check-target](contracts/README.md)，并接入严格生成器的可选目标入口。固定模型响应测试覆盖失败拒绝和最小往返。上述工具均不宣称真实业务效果已验证，也不授予发布权限；生成测试通过不等于真实模型或 Work 产品接入完成。新增的 [严格 import 创作入口](cdl/resolution.md) 已将文件依赖解析为冻结闭包，覆盖 C08 的有界本地文件场景。单实例 repo 发布/激活已实现；多节点分发、真实 Work 接入、跨宿主可信证据与完整跨产品公共契约仍待实现；完整交付计划如下。

#### 11.1 首期候选支持范围

| 范围 | 候选契约 | 首期边界 |
|---|---|---|
| 文档结构 | 明确的语言版本；`rule / ruleset / pipeline / registry`；现有 import 形式 | 不新增 `apiVersion / kind: Package` 语法；拒绝重复键、未知语义字段和错误字段类型 |
| Rule | 非空 `id / name`、显式 `when`、`i32 score`；条件满足时加分一次 | 无外部调用，无隐式 action；首期例子仅使用已校验的事件输入 |
| 表达式 | 第 5 节的纯表达式与布尔组合；所有作用域一致 | 无隐式取特征、字符串模板执行、任意函数或模型推理 |
| Ruleset | 有序 rule 引用、命中分数聚合、first-match `conclusion`、唯一末尾 default | 不引入新的跨 Ruleset 分数聚合策略；继承等能力单独验收 |
| Pipeline | `id / name / entry / steps / decision`；`ruleset / router` 节点；结果完成后可见 | 所有跳转显式，决策 first-match 且有唯一末尾 default；循环和未定义目标拒绝 |
| Registry | 按数组顺序 first-match；每项有显式 `when` | 兜底使用显式恒真表达式 `when: "true"`；不以省略条件推断兜底 |
| 引用与上下文 | 完整资源闭包、唯一资源 ID、Pipeline 内唯一 step ID、明确读写作用域 | 拒绝越界路径、未声明引用，以及读取未执行分支结果 |
| 输入与结果 | 输入 schema 校验；局部 score / signal 与最终 result 分开 | 首期沿用 Risk Profile；通用 typed fact / result 后续扩展 |

Core 草案要求非 router 节点显式声明 `next`（包括 `next: end`）；router 必须有明确的 routes 和 default。旧写法中省略 `next` 的含义必须先在兼容入口归一化并验证，不能依赖 YAML 中步骤排列顺序来推断后继。

Pipeline / step guard、单 Rule 节点、子 Pipeline 与嵌套/可选输入已作为后续增量通过严格 Core 验收，见 §1.6。Connector、动态 Feature / List、结构化 action 等仍未进入支持集。它们的既有实现不因此被删除，但在严格 Core 中必须拒绝；具备完整契约与测试后，再以能力扩展纳入。若 `ruleset / router` 等候选项仍不满足执行时序，也不能仅因列在表中而标为支持。

Feature / Model 的资源描述和就绪性契约在首期设计，不代表首期 Core 接受相应运行时调用。包与上下文 schema 可以描述未来依赖，发布检查仍必须拒绝目标环境不支持的执行能力。

#### 11.2 交付物与代码落点

下表描述完整阶段 0 目标。Core 规范/schema、fixture 与 runner、独立 CLI、共享工具链及可选严格生成/修改 API 已有首批实现。新增 [源码交换](cdl/exchange.md)：`corint export` 导出不含历史证据的可编辑 YAML 源码集合（JSON 容器），`corint import` 使用调用方样例重新编译/执行并生成当前程序的新证据。已覆盖生成器宿主与 CLI 的真实跨程序往返，但不宣称 Work 集成、历史证据跨宿主互认或发布授权；其余落点和完整完成定义仍需逐项验收：

| 交付物 | 当前进度 | 建议落点 | 完成定义 |
|---|---|---|---|
| 规范性文本 | **首批 Core 已完成** | `docs/cdl/cdl-core.md` | 给出字段、默认、类型、引用、求值、错误和兼容性规则；每条要求有用例 ID |
| 机器可读契约 | **首批 Core 已完成** | `docs/cdl/schema/` | 资源 schema 与版本化能力清单可供编辑器、Agent、验证器共用；表达式类型检查仍由编译器完成 |
| 独立工具链 | **离线首批已完成；产品集成待办** | 现有 parser / compiler / engine 公共库、`crates/corint-decision-toolchain`、`crates/corint-decision-cli` 及严格生成适配层 | CLI 与生成器已复用严格编译、真实引擎样例测试、源码包构建/指纹核验、源码导出/导入和新证据重建；单实例发布/激活已实现；多节点分发、跨宿主可信历史证据及完整公共契约待完成 |
| 跨产品公共契约 | **已完成本轮契约；完整交付待办** | `docs/contracts/` | 已实现 Core BusinessContext / TargetCapabilities v1、声明兼容性报告、CLI/严格生成共享检查及旧绑定拒绝；已补充 [W04/W07/W08 v1 契约](contracts/phase0.md) 的资源描述与绑定、评估/审批证据、决策/标签/动作回执 schema 和离线消费者；完整 PolicyPackage、可信基础设施及真实产品集成仍待实现 |
| 严格校验入口 | **严格 Core 已完成；兼容收敛待办** | 现有 parser / compiler / repository 装配链 | 拒绝未知版本、字段、类型、引用和不支持能力，输出结构化诊断；已有宽松入口不能绕过发布门禁 |
| 用例与真实示例 | **首批 fixture 已完成；历史示例待办** | `tests/conformance/cdl_core/` | 每例包含完整依赖、输入、预期输出或预期错误；不依赖在线服务或本机私有仓库 |
| 端到端 runner | **首批 Core 已完成** | `crates/corint-decision-engine/tests/cdl_core_conformance.rs` | 使用公开解析器、编译器及真实 DecisionEngine 执行，不另写测试专用解释器 |
| Core 进程级 e2e | **本地单目标用例已完成** | `tests/scripts/run_core_e2e_tests.sh`、`crates/corint-decision-cli/tests/core_process_e2e.rs` | 固定模型响应经真实生成器、CLI、服务进程和 TCP HTTP；验证激活失败保持旧状态、决策/Trace 等价和重启语义，不代表真实 Work/在线模型集成 |
| 互操作与证据测试 | **离线契约已完成；真实集成待办** | `tests/conformance/contracts/` | 验证上下文/依赖检查、规范化往返、报告与版本绑定、事件关联；与真实 Work/在线依赖集成测试分开报告 |
| 文档与生成器同步 | **部分完成；范围见 §1.4** | `docs/cdl/`、LLM prompt templates、现有 CI | 支持示例引用同一份 fixture；更改语义、示例、schema 或 prompt 都触发门禁 |

schema 约束 YAML 对象形状；符号、类型、引用和控制流检查约束语义；行为用例验证实际执行。三者不能互相代替。需对 schema 接受集与解析器接受集作一致性检查，避免再形成两套定义。

#### 11.3 示例即测试资产

当前 [example registry](cdl/examples.json) 管理 `cdl-core.md`、`condition-trace.md` 与新的严格 `pipeline.md`：完整支持例与反例绑定同一 conformance manifest；CI 校验分类、标记、fixture 链接和可执行用例，并禁止在这些页面复制内联 YAML。Pipeline 反例覆盖 guard、子调用、API 参数/any/all 与 Service endpoint；条件例绑定精确的布尔节点结果/跳过原因。原 Pipeline 细节保存在 [兼容参考](cdl/pipeline-compatibility.md)。清单另行登记 13 个 `compatibility-unverified` 页面：片段按页分类为未验收兼容资料，CI 检查范围声明并阻断重新添加实现/生产支持徽标；这些片段尚不是逐例可执行资产，LLM 兼容模板也未完成映射。

每个示例必须声明用途：完整支持例、片段、反例或未来提案。完整支持例直接来自 fixture；片段必须有可运行的包装用例；反例绑定预期错误；未来提案不得进入可发布清单。CI 应拒绝没有分类或 fixture 映射的支持声明。

建议用 manifest 维护 `case_id`、规范条款、语言版本、能力配置、入口资源、依赖、输入和期望。以下仅演示**待实现的测试描述格式**，不是 CDL 语法，也不是当前 HTTP 请求格式：

```yaml
case_id: C01_rule_score_boundary
status: candidate
language_version: "0.1"
profile: cdl-core-risk-draft
entry: rules/large_amount.yaml
input_schema:
  event.amount: required_integer
runs:
  - input: { event: { amount: 1001 } }
    expect: { score: 60, triggered_rules: [large_amount] }
  - input: { event: { amount: 1000 } }
    expect: { score: 0, triggered_rules: [] }
  - input: { event: {} }
    expect_error: { stage: input, code: E_INPUT_SCHEMA }
```

该 fixture 对应规则条件 `event.amount > 1000`、分数 `60`，并包含必填的 `id / name / when / score`。当前严格 Core 已覆盖高于/等于/低于阈值，以及缺失值和错误输入类型；实际用例见 `tests/conformance/cdl_core/behavior.yaml`，不表示所有历史兼容入口都满足同一输入契约。

#### 11.4 阶段退出条件

- 所有首期支持项及文档例子均通过第 12 节规定的解析、编译和行为测试；未完成项保留非支持状态，不以跳过测试通过门禁。
- 未知或不支持的语义在发布前失败，当前发现的 no-op / 参数丢弃路径有回归测试。
- 支持状态清单可以由 CI 检查，文档和 LLM 示例不存在第二份手工漂移的语义定义。
- 明确区分严格 Core 与兼容入口；现有策略不被未经说明地更改含义。
- 独立工具链可以在无 Work 的环境运行；相同策略、输入、依赖和引擎版本得到相同执行语义。
- 公共契约具备版本化 schema、正反例及第 12.3 节的首期契约测试；缺失依赖、伪造验证声明和证据版本不匹配不能被当作成功。
- 接口 mock、核心执行测试与真实产品集成的验收分别标注，不能用前两者宣称 Work 或在线 Feature / Model 已完整实现。

### 阶段 1：执行模型与发布安全收敛

- 以 repo 为策略唯一权威来源，沿用严格 Core 文件 repo 的启动/重载验收；逐步收敛 `ResolvedRepository` 与 `CompiledRepository`，运行时快照是 repo 确定版本的派生产物；
- 将完整的 Rule / Ruleset / Pipeline 调用模型纳入 Runtime，沿用阶段 0 的行为测试验证等价性；
- 对 guard、子调用、API 参数等扩展逐项实现、测试，再更新能力清单；
- 将首期源码包构建与导入/导出接入通用 Agent 的真实 repo 发布流程，落实可信报告、审批校验和目标依赖绑定；本轮不接入真实 Work 客户端，不以完整跨产品 PolicyPackage 为前置条件；
- 根据实际接入场景实现首批 Feature / Model 在线绑定，验证与离线口径一致，再纳入支持清单；
- 完善 `ruleset.conclusion` 的局部输出契约与 `pipeline.decision` 的最终映射，验证优先级、默认结果和动作语义，不增加独立决策层；
- 兼容 HTTP/gRPC/FFI 共享快照、版本、Trace/特征映射及失败保留旧版本机制已落地；严格 Core 的跨协议支持后续独立验收；
- 生产上线前记录最小 DecisionRecord、动作回执与后验结果关联字段，提供不依赖 Work 在线的异步反馈接口；
- 移除硬编码仓库路径和明文 token-like 配置，补齐管理入口的认证、授权和审计。

安全整改不应等待阶段 1 才开始；凭据、授权、租户隔离等是生产发布的独立阻断项。Core 一致性通过不等于系统已经适合生产。

### 阶段 2：平台能力

- 后续再接入真实 Work 客户端与完整跨产品 PolicyPackage，复用通用 Agent 已验收的工具链和 repo 发布流程；

- 完善包分发、签名、版本与依赖治理，沿用首期锁定与兼容性契约；
- Connector 能力模型、Secret Provider、超时/熔断/fallback；
- Work 的受控数据接入、数据快照、特征/模型实验及业务评估报告，连接已定义的发布与反馈接口；
- 审计、回放、数据血缘和敏感字段脱敏；
- 发布审批、灰度和回滚；
- Durable Outbox 与幂等的异步动作执行器；
- 多租户隔离、配额和容量治理。

### 阶段 3：智能化与生态扩展

- 扩展 Work 的 Agent / LLM 策略创作、自动建模与优化、冲突发现、回放解释和受控自动迁移；基础生成校验闭环应在阶段 0 完成；
- 可视化 CDL 编辑器与编译诊断；
- 更多领域 Profile、数据源、连接器，以及有预算的可选模型证据能力。

## 12. 可执行验收标准

### 12.1 测试链路

每个标记为 `supported` 的完整示例，都执行同一条真实链路：

```text
原始 YAML + 依赖 + 输入 schema
  → 解析并规范化 AST
  → 解析完整引用 / 类型与能力检查
  → 编译为 IR
  → 实际 DecisionEngine / Runtime 执行
  → 断言结果、命中、路径、调用次数与关键 Trace
```

正例至少验证解析、编译和行为三层。反例验证预期失败阶段及错误码，且断言没有继续执行或产生外部调用。涉及导入的用例必须通过真实加载链路，不能只测试单文件 parser。

不能只比较最终 `approve / decline`：遗漏步骤可能刚好仍产生相同结果。应同时验证局部 score / signal、执行与跳过节点、默认分支和副作用调用次数。相同用例在 Trace 开关、兼容迁移前后和不同公开入口下应保持语义等价。

### 12.2 首期必备用例

以下为完整验收要求，不是整体已通过的测试报告。首批覆盖范围以 [能力清单](cdl/schema/capabilities.json) 及所绑定用例为准；C08 已有独立 `cdl-core-import-draft-1` 创作入口的多级文件解析、隔离读取、真实引擎等价性和冻结包互操作证据，运行时 draft-1 仍拒绝未解析 import。C02 新增条件树观察、短路故障注入、共享规则调用区分与冻结 import 经 HTTP 返回 Trace 的证据；原始操作数和完整审计 Trace 仍未完成。首批已复用现有 Diagnostic 并补充来源、字段路径与阶段，其余错误码仍需逐项收敛。

| ID | 场景 | 预期断言 |
|---|---|---|
| C01 | 单 Rule：阈值上下与等于阈值 | `> 1000` 在 1001 时贡献 60，1000 / 999 时为 0；仅命中时记录 rule ID |
| C02 | `all / any / not`、括号、同一条件跨作用域 | 真值与短路语义一致；Trace 开关不改变结果；未求值项不伪装为已求值 |
| C03 | Ruleset 聚合、多个 conclusion 同时满足、default | 分数只来自命中规则；第一条命中结论获选，否则仅选择末尾 default |
| C04 | Pipeline 的多个 decision 同时满足与兜底 | 第一条命中 result 获选；不执行或累积其他项的动作 |
| C05 | Registry 多项匹配、显式恒真兜底、无项匹配 | 仅第一条匹配 Pipeline 执行；无匹配返回可识别状态，不伪造业务放行 |
| C06 | `ruleset → router` 读取前一步结果 | 路由看到已完成的真实结果；未选中的分支执行次数为 0 |
| C07 | `next: end`、提前终止、调整 YAML 节点排列 | 后继只由显式跳转决定；终止后仅进入 decision，不继续执行其他节点 |
| C08 | 文件 import 与多级引用 | 依赖完整解析，执行结果与已解析的等价策略一致 |
| N01 | `when.al`、未知字段、重复键、错误字段类型 | 解析/校验阶段失败，不变为恒真、不丢字段；`E_UNKNOWN_FIELD / E_INVALID_STRUCTURE` |
| N02 | 未知版本、严格入口缺失版本、版本冲突 | 发布前失败；`E_UNSUPPORTED_VERSION / E_INVALID_VERSION`；不得回退到宽松解析 |
| N03 | 缺 `id / name / when / score / decision` 等必填项 | 按所属资源 schema 失败；`E_MISSING_FIELD` |
| N04 | default 缺失、重复、非末尾，空条件组、多组键、多条件 `not` | 拒绝含糊结构；`E_INVALID_STRUCTURE` |
| N05 | 重复 ID、未解析引用、越界 import、循环引用或 DAG 环 | 加载/编译失败；`E_DUPLICATE_ID / E_UNRESOLVED_REF / E_INVALID_GRAPH / E_INVALID_IMPORT` |
| N06 | 读取未执行分支结果、非法命名空间、写只读输入 | 编译或权限校验失败；`E_INVALID_REF / E_FORBIDDEN_WRITE` |
| N07 | guard 错误结构、调用 type/字段不匹配 | 严格 Core 拒绝；`E_INVALID_STRUCTURE`。正确 guard/单 Rule/子调用已由 §1.6 正反例验收 |
| N08 | API `params / on_error / any / all` 与 Service endpoint | 首期 Core 拒绝 Connector；扩展启用后必须分别验证参数、fallback、调用次数和输出，不能只删除反例 |
| N09 | 必填输入缺失、错误类型、未经保护的可空值、数值越界 | 输入/类型检查或受控执行错误，不静默转为 false 或放行 |
| G01 | LLM 生成合法 YAML，但遗漏 `name` 或生成旧 Ruleset 字段 | 复用同一校验入口失败；不得以 YAML 合法或前缀正确判定可发布 |

新增能力必须同时添加成功、边界、失败、未执行路径和兼容性用例。Connector 的行为测试使用记录调用的 mock；回放测试禁止真实网络和动作执行。模型类用例不依赖实时模型回复。

### 12.3 跨产品契约与互操作验收

严格 Core 服务端新增 W05/W09 的局部证据：操作者批准绑定精确策略/上下文/目标/样例，拒绝越权角色与客户端伪造字段；启动失败不回退，过期 revision 和并发竞争不能覆盖新快照。该证据局限于本地单目标信任域，不等于多租户治理、可信远端证明或真实 Work 发布流程通过。

新增 [阶段 0 W04/W07/W08 契约证据](contracts/phase0.md)：8 份版本化 schema 和离线消费者已覆盖精确资源绑定、固定样例口径/历史可用时间、精确评估审批信任绑定，以及反馈关联、去重、更正和历史标签查询。W05/W06/W09 增加旧证据、合成数据业务声明和越权审批的反例。该证据不启用 Core 在线资源，不接入真实 Work、评估后端或生产反馈；单实例 Core 发布与决策入口已消费评估/审批及反馈契约；真实 Work 与外部评估后端仍待接入。

下列要求尚未整体验收。W01 已有离线工具链证据；W02 已有源码导出/导入、编辑往返及新证据重建测试；W03 已有 Core 字段和契约版本/能力检查；W05 已覆盖策略/上下文/目标/检查程序变化使旧绑定失效；W06 已区分样例通过与业务评估并拒绝自述审批字段。这些只是局部证据，不能将 W01–W10 全部标为完成。阶段 0 验证公共 schema、接口约束、独立工具链和最小往返；涉及真实 Work 客户端、在线资源或生产反馈的用例，在对应集成阶段完成后才能声明产品能力。契约 mock 的通过不等于真实产品集成通过。

| ID | 场景 | 预期断言 | 当前进度与后续验收 |
|---|---|---|---|
| W01 | 无 Work 的环境中验证、测试和构建 Core 策略 | 不需要 Work 账号、会话或网络调用；固定依赖下与其他创作入口的执行结果一致 | **离线工具链已完成**；后续补真实 Work 对照 |
| W02 | 外部修改 → 导入 → 修改 → 导出；并发编辑 | 纯往返不改变语义，业务修改仅产生预期差异；ID 稳定、冲突不覆盖；附属信息不影响结果 | **契约往返已完成**；Work 接入后补完整流程与协作冲突验收 |
| W03 | Agent 编造字段，或上下文/目标能力版本不匹配 | 给出明确诊断，不用猜测、隐式转换或宽松解析补齐 | **首批 Core 契约已完成**；后续扩展沿用同一门禁 |
| W04 | 特征/模型有定义，但未部署、版本不符或绑定缺失 | 目标检查阻断发布，不自动运行研究任务或换版本 | **契约已完成**：v1 schema 与离线消费者；扩展启用后验证真实依赖 |
| W05 | 阈值、依赖或绑定变更后复用旧报告/审批 | 保留历史证据，但不能为不匹配的新版本提供有效授权 | **Core 门禁已完成**：启动/重载/新决策消费精确证据、失效与撤销；外部评估系统后续接入 |
| W06 | 只有合成样例，却声明真实数据效果已验证 | 行为测试与业务评估状态分开；拒绝伪造来源和越权审批 | **契约已完成**：行为/业务评估分离及伪造反例；真实数据来源核验待接入 |
| W07 | 离线/在线特征同名不同口径，或使用事后补录数据 | 固定样例暴露差异；按历史可用时间取证，不能把泄漏数据当成有效回测 | **契约已完成**：v1 固定样例与证据消费者；后续真实计算后端对照 |
| W08 | 后验标签迟到、重复、更正或尚未到达 | 按决策/事件 ID 正确关联和去重；保留标签版本；缺失不等同于安全 | **Core 持久消费者已完成**：真实 HTTP 记录、SQLite 日志、关联/去重/更正与历史查询；Work 消费后续接入 |
| W09 | 越权数据查询、伪造可信特征、冒充 Work 发布 | 按主体/租户/环境权限拒绝；Agent 来源或 YAML 自述不授予权限 | **单实例本地边界已完成**：HTTP/gRPC 角色鉴权、固定租户、可信 namespace 拒绝覆盖；企业身份与跨租户治理后续处理 |
| W10 | Work 断连或反馈消费滞后 | Decision 不同步调用 Work、不自动更换策略；反馈故障按声明策略处理 | **通用消费者路径已完成**：持久出箱与故障重试不依赖 Work；真实 Work 断连场景后续集成验收 |

### 12.4 CI 与支持声明

严格 Core server 的独立 HTTP 鉴权/激活测试已加入 Core job，并安装其构建所需的 `protoc`。router 集成测试不启动外部监听；另有 [Core 进程级 e2e](../tests/CORE_E2E.md) 启动真实 CLI/server 二进制，在临时目录和随机 loopback 端口验证固定模型生成、构建/验证/导出、服务端独立验收、鉴权激活、失败不切换、决策与条件 Trace、重启加载 repo 已发布版本及 repo 历史版本回滚。两类测试均不使用真实客户数据或在线模型；运行时依赖的业务与生产发布治理仍须独立验收。

现有 [CI 配置](../.github/workflows/ci.yml) 在推送 main 和 PR 到 main/develop 时触发。独立 Core job 已包含真实引擎 conformance、CLI、共享工具链和固定模型响应生成器测试；条件 Trace schema、三个严格参考页的示例映射、兼容参考页范围声明及反例门禁已纳入 conformance 目标；另有 workspace tests。这是已配置的门禁，不是本地修改已在远端 CI 通过的声明，也不等于覆盖全部文档示例。尚需扩充历史示例/片段、生成模板映射与跨产品集成证据。产品集成测试与公共契约测试分别报告，不通过移除某个适配入口来隐藏语义差异。

门禁应阻断：支持例解析失败、编译失败、行为不符、反例意外被接受、示例未映射、能力状态与测试不符，以及未经兼容性说明的语义变更。不得用 `ignore`、只解析不执行，或无说明地修改期望输出来维持绿色结果。

### 12.5 平台级验收（后续阶段）

完成上述收敛后，至少应满足：

- 每份可发布 CDL 都能完整编译，且不存在静默忽略的语义字段；
- 未解析引用、类型不匹配、循环依赖和未支持能力在发布前被拒绝；
- 同一请求在相同 `CompiledRepository`、输入和完整依赖证据下可确定性重放；证据不足明确报告不可重放；
- Trace 中可定位规则、ruleset、Pipeline、connector、版本与输入血缘；
- HTTP、gRPC、FFI 对同一请求得到语义等价的结果；
- 所有外部副作用可幂等、可审计、可重试，且不会因决策回放重复触发；
- 热更新不会因慢 connector 或长决策请求而阻塞，并且失败时保留上一个可用版本；
- 不同 tenant 之间无法读取彼此的策略、secret、缓存或审计数据；
- 相同策略包可经外部工具或 Work 创作、修改和发布，在等价依赖与目标版本下执行语义一致；
- 未经真实数据评估的策略不会被展示为效果已验证；报告、审批及部署产物可追溯到精确版本；
- Decision 不依赖 Work 在线，反馈足以关联实际动作与后验业务结果，并能表达标签缺失、延迟和更正。

## 13. 非目标

- 不把 CDL 发展成任意代码执行环境或通用 Agent 工作流编排器；
- 不把 Work 的分析、训练和实验流程全部塞进 CDL，也不以 Work 私有语义或账号作为独立使用 Decision 的前提；
- 不把“面向 Agent / LLM”等同于默认启用在线大模型推理；
- 不承诺 Agent 每次生成都正确，不把可编译、样例通过或使用真实数据等同于业务效果保证；
- 不在决策判断中引入隐式网络调用或不受限模型推理；
- 不在首期同时改写全部语法、替换运行时和建设完整平台；
- 不以牺牲确定性、可解释性和审计能力来换取语法“灵活”。

## 修订历史

| 日期 | 变更 |
|---|---|
| 2026-09-05 | 更新严格 Core、验收基线、文档门禁和阶段 0 公共契约的完成状态，列明证据与后续集成边界。 |
