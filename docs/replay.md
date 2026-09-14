# Core 决策记录与离线回放

## Audience

面向需要验证历史决策的 SDK 集成者、审计工具开发者和服务维护者。

## Feature Overview

实验性本地契约 `corint-decision-toolchain::replay`，格式见 [record schema](contracts/schema/replay-record.json)。
记录绑定策略/输入 schema、输入、实际可执行程序与输出指纹，包含确定性执行路径、局部分数、条件及调用 Trace。
失败记录保留错误阶段、错误码和来源位置，不伪造成功 Trace。单 Rule 的算术/评分错误补充明确的字段位置；源码键位置唯一时提供行列号，不推测复杂分支的行号。诊断消息使用固定文本，避免复制输入值。

## Steps

```sh
corint record --bundle sources.json --event event.json --output record.json --visible-fields amount --retain-input --trace --format json
corint replay --bundle sources.json --record record.json --format json
```

## Inputs and Outputs

`sources.json` 是 `corint export` / `corint resolve` 的冻结源码 bundle。
`event.json` 是事件字段对象，例如 `{"amount":1001}`，不再包一层 `event`。
默认记录对所有输入字段脱敏，不能回放。`--visible-fields` 选择可记录的顶层字段；选择 object 会保留其整个子树。
只有显式选择全部字段并传入 `--retain-input` 才保留完整回放输入。
Trace 中仍包含规则 ID、布尔结果和业务输出，应按调用方的数据访问规则保管。
输入上限 4 MiB、64 层及 10,000 个值节点，记录上限 8 MiB；非有限数值不能记录。输出文件必须不存在，以原子方式写入。

回放使用同一可执行程序与冻结源码运行真实 Core 引擎，不重新查询数据库，不执行动作。
使用 [FeaturePipeline](contracts/feature-pipeline.md) 时，应将 `replay_event` 作为完整输入保留，
并由宿主一起保管 FeatureEvidence；本工具只验证 Core 对这些输入的执行，不认证输入的外部来源。
晚到数据可能改变重新查询结果，固定截止点不等于历史数据库快照。

| 错误 | 含义 |
| --- | --- |
| E_REPLAY_FORMAT / E_REPLAY_VERSION | 格式或版本无效 |
| E_REPLAY_LIMIT | 输入或记录超过容量 |
| E_REPLAY_DIGEST | 记录内容指纹不匹配 |
| E_REPLAY_POLICY / E_REPLAY_ENGINE | 策略/schema 或可执行程序发生变化 |
| E_REPLAY_REDACTION / E_REPLAY_INCOMPLETE | 保留权限不足或输入不完整 |
| E_REPLAY_INPUT / E_REPLAY_MISMATCH | 输入绑定无效或实际执行结果不同 |

## FAQ

指纹不提供签名或来源认证，记录不能授予发布权限。不同可执行程序之间的升级比较须另行验收，不覆盖旧证据。
验收：[工具链测试](../crates/corint-decision-toolchain/tests/replay.rs)、[真实 CLI 测试](../crates/corint-decision-cli/tests/replay.rs)。

## Journal Replay

使用严格 Core HTTP 时，可靠 journal 原子保存输入和完整响应。操作员配置 `journal.export_replay: true`
可让授权 consumer 从 outbox 导出单项 `event`、`input_evidence`、`response` 和 `idempotency_key`。
保留原服务器二进制及对应版本的冻结源码 bundle，离线执行：

```sh
corint-decision-server --replay-journal sources.json export-item.json
```

该命令不启动服务、不读取在线配置、不查询特征数据库、不执行动作。
校验记录内容哈希、输入哈希、源码/schema 身份及原可执行文件指纹后，
从宿主输入包提取实际 Core event，比较完整确定性输出与条件/调用 Trace；执行错误比较错误码。
也可从 toolchain 调用异步 `replay::replay_journal`。
输入准备失败（`event: null`）、旧记录缺完整响应或易失模式未保存响应时返回不完整，不能伪造成功回放。
独立 CLI 二进制与原服务器的指纹不同，不能冒充同一运行时；升级效果比较是另一项评估。
此连接复现已记录输入的决策，不重建历史数据库或认证水位生产者。

## Revision History

| Date | Changes |
|---|---|
| 2026-09-14 | Document extracting actual Core input from DecisionHost journal evidence for replay. |

| 2026-09-14 | 增加原服务器二进制的 journal 导出回放入口和完整响应/Trace 对比。 |
