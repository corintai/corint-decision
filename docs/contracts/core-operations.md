# 单实例 Core 发布、审计与反馈

本轮 P0/P1 的交付范围是通用 Agent 可独立使用的单实例执行链路。repo 是唯一策略来源；
事件日志只保存决策证据、反馈和投递状态。真实 Work、完整跨产品 PolicyPackage、在线 Feature/Model、
企业身份联合、多节点发布及严格 Core 的其他协议适配仍属于后续阶段。

## 配置与角色

保留配置 v2 的离线/本地兼容模式。需要持久决策记录时使用 **`config_version: "3"`**，v3 必须配置 journal。
在 [Core server](core-server.md) 的上下文、目标、样例、repo、决策/发布凭据与批准列表之外，增加：

```json
{
  "config_version": "3",
  "journal": {
    "path": "events.sqlite",
    "tenant_id": "merchant_a",
    "max_records": 10000,
    "max_bytes": 67108864,
    "consumer_token_env": "CORINT_CONSUMER_TOKEN"
  }
}
```

这是配置增量，不是可独立启动的完整配置。路径相对操作员配置目录。
consumer、decision、publisher 三个凭据必须各不相同，均为 32–1024 字节可打印非空格 ASCII。
consumer 是被操作员信任的反馈/投递网关；它可以为该实例固定租户提交来源声明，不能切换租户或发布策略。
固定 tenant 写入数据库元数据，其他 tenant 的记录和复用该数据库的配置均拒绝。
所有服务监听仍限定 loopback；这不是企业多租户身份系统或公网 TLS 服务。

兼容编译入口另已拒绝未实现的步骤/guard/API 参数及结果依赖 router；需要根据已执行 ruleset 结果路由时使用严格 Core。省略 next 的兼容分支显式终止，不会穿透至其他分支。

## 真实决策记录

启用 journal 后，`POST /v1/core/decide` 必须携带业务事件 ID：

```json
{"business_event_id":"payment-123","event":{"amount":1500},"enable_trace":true}
```

成功响应包含 `snapshot`、`decision`、`record`。执行/输入校验失败返回 422，已落盘的错误记录位于
`diagnostic.record`，对应版本位于 `diagnostic.snapshot`。缺失业务事件 ID、鉴权失败或无法解析请求正文时
尚未进入决策，不伪造 DecisionRecord。事件 ID用于关联业务事件，不充当请求去重键；重复决策请求产生独立决策 ID。

生产者按 [DecisionRecord v1](schema/decision-record.json) 写入记录：

- `decision_id`、`business_event_id`、操作员 tenant 和服务端时钟；
- 当前策略/context/target/checker 指纹，以及实际空资源绑定契约的 `bindings_sha256`；
- `runtime.revision`、`runtime.repository_revision`、`runtime.repository_manifest_sha256` 和实际 Pipeline；
- 命中规则、原因、`approve / decline / review / hold / pass / no_match / error`、错误码和耗时；
- 动作意图的稳定 `action_id` / `idempotency_key`。记录动作不等于执行动作。

记录与执行使用同一个冻结快照。输入证据单独保存在同一 SQLite 事务中；
`input_evidence.reference = journal:<decision_id>`，SHA-256 对递归排序后的紧凑事件 JSON 计算。
出箱接口仅返回记录，不返回原始输入。持有本地数据库权限的审计工具可按记录定位输入。
`runtime` 是 v1 的可选扩展字段；本生产者始终填写，历史离线 fixture 仍可读取。

## 持久化与可靠投递

SQLite 使用 WAL、`synchronous=FULL`，日志请求采用 64 个有界准入名额。决策记录、私有输入与待投递事件在同一事务中提交后才报告成功。
数据库不可用或达到容量上限返回 503 / `E_JOURNAL_UNAVAILABLE`；不异步丢弃，也不改写为业务通过。
重启验证已有历史及内容指纹，恢复关联、去重、更正链和未确认事件。

消费接口均使用 consumer 凭据：

| 接口 | 请求 / 响应 |
|---|---|
| `POST /v1/core/outbox/claim` | 空请求；返回 `lease` 和最多 100 个、合计不超过 8 MiB 计费正文/输入的 `events` |
| `POST /v1/core/outbox/ack` | `{"lease":"…","idempotency_keys":["…"]}` |
| `POST /v1/core/feedback/outcome` | 完整 OutcomeEvent v1；返回 inserted / duplicate |
| `POST /v1/core/feedback/receipt` | 完整 ActionReceipt v1；返回 inserted / duplicate |
| `POST /v1/core/feedback/query` | `{"decision_id":"…","label_name":"fraud","available_at_ms":123456}`；返回历史时点结果或 null |

每个出箱项包含契约内容哈希 `idempotency_key`、`attempt`、`lease_expires_at_ms` 和事件正文。
首次租期 1 分钟；未确认事件按 1、2、4…60 分钟退避后重新可领取。租约和次数持久化，消费者崩溃或响应丢失不会删除事件。
消费者须按幂等键完成自身持久处理，再确认。旧/过期/被替换的租约返回 409；有效租约下重复确认允许。
这是至少一次投递，不保证副作用恰好一次。服务不主动请求消费者 URL，通用消费者可按自己的调度轮询。

日志容量包含已确认历史，因为反馈关联仍需要原决策。上限是 100000 条、1 GiB 逻辑正文/输入；
SQLite 索引和 WAL 有额外开销。不会自动删除审计历史；满后停止新增，操作员必须规划存储容量。
当前消费者重放有界历史完成跨事件校验，适用于首期单实例；高吞吐索引/归档服务不在此实现中。

OutcomeEvent 校验 tenant、decision、business_event、单调标签版本、supersedes 和可用时间。
乱序更正或尚未到达的决策返回 422，发送方保留事件后重试。历史查询不会把迟到更正写回过去。
ActionReceipt 必须匹配已有动作身份、幂等键和执行时间；冲突的终态回执拒绝，重复内容返回 duplicate。

兼容 PostgreSQL `DecisionResultWriter::write_decision` 已改成 async 确认写入，最多 64 个并发提交。
引擎等待事务提交，失败传回调用者；删除无界内存队列与“日志报错后丢记录”路径。
兼容表格式保留原协议，不冒充上述 Core 版本化记录/反馈日志。

## 评估与审批门禁

需要业务证据的部署额外配置：

```json
{"business_evidence":{"evaluation":"evaluation.json","approval":"approval.json","trust":"trust.json"}}
```

`trust.json` 是操作员独立维护的认证结果：

```json
{"evaluations":{"<报告内容哈希>":"evaluator"},"approvals":{"<审批内容哈希>":"reviewer"},"approvers":["reviewer"]}
```

报告和审批遵守 [phase0 契约](phase0.md)。`prepare-repository --format json` 输出 `evidence_subject`，
可供评估/审批系统绑定；源码、上下文、目标、资源绑定或 checker 变化必须重新取得证据。
这里 `bindings_sha256` 是固定空资源绑定契约的哈希，**不同于**兼容性报告的 `binding_sha256`。
资源绑定原文由 `contracts::core_evidence_subject` 的公开实现定义；Core 不凭空声称已有在线资源。

启动、重载以及每个新决策都核验配置的证据、期限与信任清单。删除信任项或修改批准内容立即影响后续请求，
失败返回 `E_PUBLICATION_EVIDENCE`。信任检查之后已开始的请求可在其固定快照上完成。
配置该要求后无法用一次行为测试或 Agent 自报审批绕过；未配置时继续明确报告 `business_evaluation: not_performed`。
通过时报告 `verified_operator_attestation`，不是“服务器已重新执行真实业务评估”。
该实现提供本地信任根与撤销消费，签名认证服务、真实数据评估由部署方提供。

## 文件、SQLite、PostgreSQL 与 HTTP repo

`prepare-repository` 同时输出 `published.json`/原始 YAML 和 `publication.json`。
后者是完整、冻结的发布文档：`{"manifest":"<published.json原文>","sources":[{"path":"…","yaml":"…"}]}`。
所有后端共用严格 resolver、指纹、目标、批准与行为验收。无缺文件的隐式文件系统回退。
文档上限 32 MiB、256 个文件，manifest 上限 64 KiB，资源/闭包本身仍受 resolver 限制。

文件模式保留 `repository: "repository"`。其他模式省略 repository，指定一个 backend：

```json
{"repository_backend":{"type":"sqlite","path":"policies.sqlite"}}
```

```json
{"repository_backend":{"type":"postgres","url_env":"CORINT_POLICY_DATABASE_URL"}}
```

```json
{"repository_backend":{"type":"http","url":"https://policies.example/published","token_env":"CORINT_REPOSITORY_TOKEN"}}
```

数据库由发布方建立并维护：`CREATE TABLE corint_core_publication (slot TEXT PRIMARY KEY, document TEXT NOT NULL)`。
发布方在一个事务中写入 `slot='published'` 的完整 publication.json；服务只读消费，不创建表或替发布方写策略。
HTTP 返回相同文档，必须 200；使用操作员 bearer credential，不跟随重定向。
仅接受 HTTPS，测试时允许字面 loopback IP 的 HTTP；URL 不允许嵌入用户名、密码、query 或 fragment。
重载准备完成后再次读取确认发布身份，变更则拒绝本次切换。相同语义的重排/混合原始文件也需匹配原始闭包指纹。

SQLite/PostgreSQL 的单文档查询有事务快照；文件、HTTP 后端发布方须保证发布引用更新原子。
服务固定已读取且验收通过的版本，之后的新发布不会追改执行中的快照。回滚仍由 repo 发布方恢复历史版本后重载。

## 验证入口

- `cargo test -p corint-decision-server --test core_activation --test durable_feedback --test shared_snapshots`
- `python3 tests/scripts/run_p1_postgres_tests.py`：临时 PostgreSQL、私有 Unix socket、禁止 TCP，结束后清理。
- `cargo test -p corint-decision-ffi`：真实 repo、C ABI 决策与版本条件重载。
- `tests/scripts/run_core_e2e_tests.sh`：普通 Agent 文件经公开 CLI 到真实 Core HTTP 进程。

这些测试使用合成策略/样例和独立测试凭据；证明执行、事务与权限机制，不证明客户业务效果或外部生产环境已经部署。
