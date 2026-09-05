# 阶段 0：资源、评估审批与反馈契约 v1

本页交付 W04、W07、W08 的离线公共契约，并覆盖相关 W05/W06/W09 证据约束。
8 份自包含 JSON Schema Draft 7、完整正例和反例清单由共享工具链执行。
这是实验性接口验收：未接入真实 Work、在线 Feature/Model、业务评估后端或持久化反馈服务。
Core draft-1 的运行时能力与 `TargetCapabilities v1.resources: []` 保持原有约束。

## 三项独立交付

| 交付 | 版本化 schema | 生产者 → 明确消费者 | 当前可执行验收 |
|---|---|---|---|
| W04 资源与绑定 | [FeatureDescriptor](schema/feature-descriptor.json)、[ModelDescriptor](schema/model-descriptor.json)、[ResourceBindings](schema/resource-bindings.json) | 资源拥有者、部署控制面 → `phase0::check_resources` | 精确依赖闭包、定义内容、目标身份、就绪声明、后端绑定、服务能力 |
| W07 评估与审批证据（含 W05/W06/W09） | [EvaluationEvidence](schema/evaluation-evidence.json)、[ApprovalEvidence](schema/approval-evidence.json) | 评估程序、审批服务 → `phase0::check_evaluation` / `check_approval` | 固定样例口径与时间、完整特征覆盖、精确版本、外部信任、授权角色与有效期 |
| W08 决策与反馈 | [DecisionRecord](schema/decision-record.json)、[OutcomeEvent](schema/outcome-event.json)、[ActionReceipt](schema/action-receipt.json) | 决策适配器、标签来源、动作执行器 → `phase0::FeedbackLedger` | 关联、重复、冲突、更正链、历史查询、动作回执与意图对应 |

实现入口：[phase0.rs](../../crates/corint-decision-toolchain/src/phase0.rs)。
`Contract::load(kind, &CoreSource)` 使用同一严格 YAML/JSON 解析和 schema 校验，拒绝重复键、
未知版本、额外字段和缺失字段；随后调用相应消费者执行跨字段和跨文档校验。
仅 `load` 成功不等于绑定、评估或事件入库成功。
`Contract` 保存不可变值，可通过 `value()` 读取、序列化，再从另一目录加载。
不解释 CDL 表达式、不访问文件引用或网络、不启动 SQL/训练任务。

每份文档都有 `kind`、`contract_version: "1"`、`id`、`revision`、`provenance`。
来源是可审计声明，不自动授予权限。`id` 是稳定身份，`revision` 是精确版本标签，
资源引用同时固定 kind/id/revision/sha256；即使版本标签未变，内容变化也会使旧引用失效。
时间字段统一为 UTC Unix 毫秒，限定在 0–9007199254740991 的整数范围。
未知未来版本直接拒绝，不自动降级。

`Contract::sha256()` 使用现有规范化 JSON 哈希算法：
`corint-canonical-json-v1\0` 前缀，加递归按键排序的紧凑 JSON
`{"domain":"phase0-contract-v1","value":<完整文档>}`，随后计算 SHA-256。
数组顺序和文档全部字段参与身份；路径、YAML 注释与排版不参与。
数字的不同 JSON 表示可能产生不同内容身份，不做业务单位或数值口径转换。
这里的内容身份独立于已有 BusinessContext/TargetCapabilities 的原文指纹，不替换其算法。

## W04：定义与部署声明分开

Feature 描述实体键、输入/输出类型与单位、窗口、去重规则、事件时间与实际可用时间字段、
缺失和过期处理、离线/在线后端标识。Model 固定产物和预处理指纹、输入特征引用、输出业务含义、
所需能力、超时及失败策略，并区分 defined/evaluated/ready。
这些字段目前固定资源语义声明；检查器不执行单位转换、窗口计算、过期处理或模型推理。

调用方从实际策略依赖中取得完整 descriptors，并向 `check_resources` 传入当前目标内容指纹和
允许的能力集合。绑定必须恰好覆盖该闭包；多余、重复、缺失、版本不符或未 ready 均失败。
Model 的全部输入特征必须在闭包内。Feature 离线/在线绑定须与定义一致；
Model 所需服务能力须与绑定一致，并在目标能力集合中。
检查不猜测替代版本，不读取 `latest` 服务来补齐依赖。

`ResourceBindings` 是独立的未来资源扩展契约，不写入现有 Core 目标契约或源码包。
非空资源需 `feature-read-v1` 或 Model 明确要求的能力；现有 Core 没有这些能力，仍会拒绝。
`ready` 是目标声明，不是健康探测结果。实际发布方还需认证部署控制面、验证目标实时状态，
并通过语言能力门禁；不能凭本检查结果直接发布带资源调用的 Core 策略。

## W07：固定评估证据与外部审批信任

EvaluationEvidence 分开记录 `behavior` 和 `business`；合成数据不能声明 business。
报告固定数据快照、训练/调参/评估分区指纹、划分方法、特征闭包、样例输入指纹、指标和结果。
业务报告的三个分区指纹必须不同；这只能发现同一分区被直接复用，真实数据行的隔离仍由可信评估后端验证。

每个所需特征至少有一个固定样例。样例固定实体、输入、定义和离线/在线输出指纹；
两侧定义必须与特征内容身份一致，输出指纹相等。
时间必须满足 `event_time ≤ available_time ≤ decision_time ≤ report_created_time`。
迟到补录或更正在历史决策之后才可用的数据不能通过。
该比较是契约证据核对，不是在线/离线计算后端的真实等价性证明。

审批消费者还要求调用方传入当前特征闭包和当前 subject。subject 同时固定策略、业务上下文、
目标、资源绑定、检查程序的 SHA-256；审批再固定完整评估报告 SHA-256，避免报告与审批循环哈希。
阈值、依赖、预处理、绑定、目标或检查程序变化时，调用方必须重新取得当前指纹，旧证据失败。
当前特征闭包须被报告完整覆盖，不能删掉难以通过的特征样例来获得批准。

`check_approval` 要求 passed business 报告、approved 决定、合法审批时间和未过期状态：
`report_created ≤ approval_issued ≤ now < approval_expires`。
`EvidenceTrust` 由宿主的可信配置提供，包含独立核实的报告/审批内容指纹及认证生产者、允许审批者集合；
它没有来自报告的反序列化入口。审批 producer 必须等于认证 approver。
未进入可信集合、报告内容被改动或审批者不获授权均失败。字段自述和 Agent 来源不会建立信任。

参考消费者只演示本地信任域准入。宿主负责认证、可信时钟、撤销/更新允许集合和证据访问控制；
本模块不实现签名基础设施，不读取来源 URL，也不替代现有 server 的激活授权。
`check-target` 和现有 server 尚未消费这些新评估/审批契约；已有兼容性报告继续声明
`business_evaluation: not_performed`、`publication_approval: not_granted`。

## W08：关联、迟到、更正和动作回执

DecisionRecord 固定租户、决策/业务事件 ID、决策时间、策略/上下文/目标/绑定/检查程序指纹、
引擎版本、输入证据引用及指纹、资源版本、命中与原因、结果/错误/耗时、动作意图及幂等键。
它是新的事件接口，不是现有 HTTP 决策响应字段已扩展的声明。

`FeedbackLedger` 按 tenant/decision 关联。DecisionRecord 相同内容重放返回 Duplicate，
同一决策 ID 的不同内容冲突。OutcomeEvent 先关联已记录决策及业务事件；
未知决策或租户、错误业务事件均拒绝，消费者可在决策到达后重试。
标签按 tenant/decision/label_name 保存完整历史，事件 ID 在租户内去重。
相同 ID 的不同内容冲突；更正使用新事件 ID、递增 label_version 和前一事件 ID 的 supersedes。
缺失前驱的更正拒绝，待前驱到达后重试；不会猜测或覆盖历史。
更正的 available_at 不能早于上一版本；历史查询按 available_at 和版本顺序确定当时已知标签。

`outcome_as_of` 在标签缺失或尚未可用时返回 None；显式 `unknown` 与 `negative` 也是不同状态。
未到达标签不被解释成安全。标签发生/观察/可用时间必须有序；发生时间可以晚于决策，允许迟到反馈。
关联字段仅提供隔离键，真实租户身份与标签来源仍由接入层认证。

ActionReceipt 必须关联已记录的动作 ID 与幂等键，执行时间不能早于决策。
v1 每个动作只接收一份终态回执（成功、失败或人工干预）；相同内容可重放，冲突终态拒绝。
重试流水和终态更正需要后续版本；消费者不会执行动作或因回放产生副作用。
当前 ledger 仅为内存参考实现；生产反馈还需持久化、可靠投递、认证和保留策略。

## 固定用例与运行

完整正例均为虚构 fixture，其中标记 real 的评估样例只用于验证声明约束，不包含真实业务数据：

| 契约 | 完整正例 |
|---|---|
| FeatureDescriptor | [feature-descriptor.json](../../tests/conformance/contracts/phase0/feature-descriptor.json) |
| ModelDescriptor | [model-descriptor.json](../../tests/conformance/contracts/phase0/model-descriptor.json) |
| ResourceBindings | [resource-bindings.json](../../tests/conformance/contracts/phase0/resource-bindings.json) |
| EvaluationEvidence | [evaluation-evidence.json](../../tests/conformance/contracts/phase0/evaluation-evidence.json) |
| ApprovalEvidence | [approval-evidence.json](../../tests/conformance/contracts/phase0/approval-evidence.json) |
| DecisionRecord | [decision-record.json](../../tests/conformance/contracts/phase0/decision-record.json) |
| OutcomeEvent | [outcome-event.json](../../tests/conformance/contracts/phase0/outcome-event.json) |
| ActionReceipt | [action-receipt.json](../../tests/conformance/contracts/phase0/action-receipt.json) |

[反例清单](../../tests/conformance/contracts/phase0/negative-cases.json) 用完整正例 + JSON Pointer 替换描述反例，
固定用例 ID、消费者及预期错误码。[契约测试](../../crates/corint-decision-toolchain/tests/phase0_contracts.rs)
执行清单全部用例，并补充版本/未知字段/重复键、规范化往返、信任集合、审批有效期边界、
缺失标签、迟到、更正、去重冲突及动作回执测试。失败写入不改变既有标签历史。

```sh
cargo test -p corint-decision-toolchain --test phase0_contracts --locked --offline
```

[CI Core job](../../.github/workflows/ci.yml) 的共享工具链测试会运行同一目标。
实际 Work 对照、在线资源依赖校验和持久化反馈集成分别保留到后续阶段，不能用这些 fixture 宣称通过。

## Revision History

| Date | Changes |
|---|---|
| 2026-09-05 | 新增资源绑定、评估审批和反馈事件 v1 契约、消费者及固定验收用例。 |
