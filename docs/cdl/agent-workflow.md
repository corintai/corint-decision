# 通用 Agent 的 Core 执行流程

当前目标是让能够读写文件、调用 CLI 和授权 HTTP 的通用 Agent 跑通策略流程。
不依赖 Work 客户端、Work 账号、特定模型或 `CoreGenerator` SDK。
本流程仅适用于 [首批严格 Core](cdl-core.md) 与 [单目标文件 repo 服务](../contracts/core-server.md)，
不表示完整 P0/P1、线上业务效果或多租户部署已验收。

## 1. 提供明确输入

给 Agent 提供业务要求、字段 schema、上下文、目标能力，以及可独立检查的行为样例。
资源必须在 [能力清单](schema/capabilities.json) 的支持范围内。
Agent 编写、修改 YAML 后，必须检查 CLI 的退出码和 JSON 诊断，不能根据“文件已生成”判断成功。
规则、规则集、Pipeline 和 Registry 的完整闭包必须齐全；未支持字段不能靠猜测或宽松解析补齐。

## 2. 用公开 CLI 准备候选 repo

```sh
cargo build -p corint-decision-cli -p corint-decision-server --locked --offline
./target/debug/corint prepare-repository \
  --root tests/conformance/cdl_core \
  --input-schema input-schema.yaml \
  --cases tests/conformance/cdl_core/behavior.yaml \
  --context tests/conformance/contracts/business-context.yaml \
  --target tests/conformance/contracts/target-capabilities.json \
  --revision agent-v1 \
  --output ./agent-candidate \
  --format json \
  rule.yaml ruleset.yaml pipeline.yaml registry.yaml
```

这是完整的合成示例，无客户业务数据。实际使用时替换为自己的文件，输出目录必须不存在。
缺少依赖缓存时可去掉构建的 `--offline`；CLI 本身不访问网络。
输入 schema 和 entry 标签相对 `--root`；cases/context/target/output 路径相对当前工作目录。
支持现有 `cdl-core-import-draft-1` 的显式文件导入，复用 resolver 的路径、大小和闭包约束。

该命令依次冻结原始源码、严格编译、检查声明目标、用真实引擎运行行为样例（Trace 开/关），
通过后将已检查的原始文件与 `published.json` 写入一个新候选目录。
不会再次读取作者目录来构建输出，因此准备期间之后的作者修改不会混入候选。
写入完成后使用同一 server repository loader 复核产物。
上下文、验收样例和批准不写入候选策略目录；服务继续使用操作员自己的控制文件。

| 退出码 | 含义 |
|---|---|
| 0 | 候选准备成功，仍未批准、未激活 |
| 1 | 源码、目标或行为校验失败 |
| 2 | 命令用法或文件读写失败 |

`--format json` 输出一个对象，包含 `scope: repository_candidate`、`valid`、`diagnostics`，
成功时还包含 `test_results`、`compatibility` 与 `candidate` 的 repo/策略指纹。
行为断言失败时也返回 `test_results` 中的实际值、期望值和逐例诊断，便于 Agent 修正源码。
始终声明 `publication_approval: not_granted`、`activated: false`、`business_evaluation: not_performed`。
失败输出不得部署；写入故障可能留下不完整目录，应保留诊断并使用新的输出目录重试。
命令不覆盖已有目录，不修改活动 repo，不生成授权，不访问服务端。

现有 `validate/test/build/verify/export/import/check-target/resolve` 仍可单独使用。
需要交换源码包时复用 [源码包](packages.md) 和 [导入导出](exchange.md)，不以完整跨产品 PolicyPackage 为前置条件。

## 3. 操作员批准与发布

操作员审核候选策略，使用独立的服务端验收样例，并在可信配置中批准精确指纹。
随后将候选作为服务配置指向的 repo 发布，或在首次启动前将 `repository` 指向该候选目录。
审批与 repo 版本选择由操作员/授权发布系统管理；Agent 的成功报告不能为自己授予发布权限。
操作员应通过 repo 的部署机制确保重载读取的是完整稳定版本，不逐文件修改正在读取的目录。
完整配置、token 环境变量和批准匹配规则见 [Core server](../contracts/core-server.md)。

已获 publisher 权限的调用方先读取 `GET /v1/core/target`，再调用
`POST /v1/core/repo/reload`，正文只携带 `{"expected_revision":"<当前运行 revision>"}`。
服务从 repo 重新读取并独立验收；不接受 Agent 上传策略正文、批准或验证报告。
过期 revision、错误批准或验收失败均不切换当前引擎。

## 4. 执行与核对

有 decision 权限的 Agent 调用 `POST /v1/core/decide`：

```json
{"event":{"amount":1001},"enable_trace":true}
```

核对响应中的实际 `snapshot.repository`、`policy_sha256`、运行 `revision` 和 `decision`。
重启后 repo 身份与策略指纹保持，运行 revision 更新；回滚也必须从 repo 选择历史版本并重新验收。
配置 v2 提供响应级追溯；启用 v3 journal 后具备持久 DecisionRecord、可靠投递与通用标签反馈，详见下文“持久运行与反馈”。外部业务消费者和真实数据效果仍需部署方独立验收。

## 5. 可执行验收

```sh
bash tests/scripts/run_core_e2e_tests.sh --offline
```

进程测试使用 Cargo 构建的 CLI 和服务端二进制，通过真实 TCP HTTP 校验决策。
`public_candidate_command_runs_without_work_or_generator_sdk` 从普通 YAML 文件调用公开
`prepare-repository`，不使用生成器 SDK 或测试专用 repo 发布函数，验证操作员独立批准后的
服务启动、边界输入、角色隔离、授权重载与重启。
另有固定模型响应的生成闭环测试；两者都使用合成数据，不代表已连接真实模型或验证业务效果。

## 持久运行与反馈

候选目录现在还包含 `publication.json`，可供 SQLite/PostgreSQL/HTTP repo 原子发布。JSON 报告的 `evidence_subject` 供独立评估/审批系统绑定。部署 Core v3 后，决策请求带 business_event_id，响应获得实际版本绑定的 record；通用消费者领取和确认出箱事件，并提交标签/动作回执。配置与完整协议见 [Core 运行保障](../contracts/core-operations.md)。
