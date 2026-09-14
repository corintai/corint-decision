# 单实例 Core 发布与在线决策持久化

本轮 P0/P1 的交付范围是通用 Agent 可独立使用的单实例执行链路。repo 是唯一策略来源；
在线主链路负责执行策略、将本次决策证据入队、立即返回结果，由后台保存；反馈关联、标签更正、动作回执管理和策略调优由外部 Agent 系统负责。事件日志只新增决策记录，可选提供结果投递。真实 Work、完整跨产品 PolicyPackage、在线 Model、
企业身份联合、多节点发布及严格 Core 的其他协议适配仍属于后续阶段。

## 配置与角色

保留配置 v2 的离线/本地兼容模式。需要后台保存决策记录时使用 **`config_version: "3"`**，v3 必须配置 journal。
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
`consumer_token_env` 可省略或为空：此时只启用在线决策落库，不需要消费者凭据，也不注册 outbox HTTP 接口。
配置 consumer 后，consumer、decision、publisher 三个凭据必须各不相同，均为 32–1024 字节可打印非空格 ASCII。
consumer 只能领取和确认该实例的决策记录，不能提交业务反馈、切换租户或发布策略。
固定 tenant 写入数据库元数据，其他 tenant 的记录和复用该数据库的配置均拒绝。
所有服务监听仍限定 loopback；这不是企业多租户身份系统或公网 TLS 服务。

兼容编译入口另已拒绝未实现的步骤/guard/API 参数及结果依赖 router；需要根据已执行 ruleset 结果路由时使用严格 Core。省略 next 的兼容分支显式终止，不会穿透至其他分支。

## 真实决策记录

启用 journal 后，`POST /v1/core/decide` 必须携带业务事件 ID：

```json
{"business_event_id":"payment-123","event":{"amount":1500},"enable_trace":true}
```

成功响应包含 `snapshot`、`decision`、`record` 和 `persistence`。启用 journal 时 `persistence: "queued"` 只表示记录已进入内存后台队列，不代表事务已提交；禁用时为 `"disabled"`。执行/输入校验失败返回 422，已入队的错误记录位于
`diagnostic.record`，入队状态位于 `diagnostic.persistence`，对应版本位于 `diagnostic.snapshot`。缺失业务事件 ID、鉴权失败或无法解析请求正文时
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

在线决策不等待数据库 I/O：校验本次记录并进入后台队列后即可返回。每个写入器最多保留 64 个未完成任务、32 MiB 序列化记录/输入，单个 Core 事件仍限 8 MiB；该预算不包括对象和任务本身的内存开销。
队列满、字节预算不足或正在停机时，入队失败返回 503 / `E_PERSISTENCE_QUEUE_UNAVAILABLE`。数据库不可用或持久容量耗尽发生在响应之后，记录后台失败，不能撤回已返回的业务结果。
SQLite 使用 WAL、`synchronous=FULL`；后台仍在一个事务中保存决策记录、私有输入与待投递事件。Core 对同记录的写入最多尝试 3 次，间隔 100/200 ms，依靠同 ID/内容幂等避免重复插入。
`GET /v1/core/persistence` 使用 publisher 凭据，返回 `accepting`、`pending`、`written`、`failed`、`retries` 和 `last_failed_id`。写入失败同时产生结构化错误日志；计数是当前进程状态，重启归零。

这是有界的易失内存队列，不是可靠消息队列。强制退出、崩溃或主机故障可能丢失未提交记录；重试耗尽的记录不持久保留在失败队列中。需要无丢失保障时，应另行接入持久消息系统或改用等待可靠提交的模式。
服务收到 SIGINT/SIGTERM 后停止 HTTP/gRPC 接收并等待在途请求，随后最多等待 30 秒排空后台写入；超时或已知写入失败报告错误。outbox 只能领取已经提交的记录，紧跟决策响应的查询可能暂时看不到新记录。

后台写入只校验本条 DecisionRecord，通过 `(tenant_id, decision_id)` 唯一索引查重；相同 ID 和内容返回 duplicate，不同内容或冲突输入证据拒绝，绝不覆盖旧记录。
条数与逻辑字节数由数据库触发器在同一事务中维护，每次写入不再统计或重放全部历史。
重启直接使用持久化索引、计数和投递状态，不重建 FeedbackLedger。

可选的结果导出接口均使用 consumer 凭据：

| 接口 | 请求 / 响应 |
|---|---|
| `POST /v1/core/outbox/claim` | 空请求；返回 `lease` 和最多 100 个、合计不超过 8 MiB 计费正文/输入的 `events` |
| `POST /v1/core/outbox/ack` | `{"lease":"…","idempotency_keys":["…"]}` |

每个出箱项包含契约内容哈希 `idempotency_key`、`attempt`、`lease_expires_at_ms` 和事件正文。
首次租期 1 分钟；未确认事件按 1、2、4…60 分钟退避后重新可领取。租约和次数持久化，消费者崩溃或响应丢失不会删除事件。
消费者须按幂等键完成自身持久处理，再确认。旧/过期/被替换的租约返回 409；有效租约下重复确认允许。
这是至少一次投递，不保证副作用恰好一次。服务不主动请求消费者 URL，通用消费者可按自己的调度轮询。

持久容量由操作员的 `max_records` / `max_bytes` 配置限制，包含已确认记录及旧库保留的数据；达到上限时后台拒绝新增并计为写入失败，不自动删除审计记录。`max_records` 为正 u32，`max_bytes` 范围为 1024 到 i64 最大值；已移除首期硬编码的 100000 条/1 GiB 上限。这不是吞吐或磁盘容量承诺，SQLite 索引和 WAL 另占空间。

### 旧库升级与职责迁移

首次打开旧 SQLite journal 时，在一个事务中校验已有决策记录、建立唯一索引及容量计数。旧库有同决策 ID 的冲突记录或损坏的决策内容时，升级失败，事务回滚。此一次性升级仍与旧决策数量有关，后续启动和在线写入不再全量校验历史。
原有 outcome-event、action-receipt、输入证据及投递状态不删除。旧反馈不重放、不参与决策校验，也不再通过 outbox 导出；如需迁移，由外部 Agent 系统从旧库读取。outbox 只投递 DecisionRecord。

`POST /v1/core/feedback/outcome`、`/receipt`、`/query` 已从在线服务移除，返回 404。外部 Agent 根据决策 ID/业务事件 ID 关联业务反馈、维护标签更正和回执，再进行策略评估。原有离线反馈契约和 `FeedbackLedger` 参考实现仍保留在 toolchain，供外部系统复用；在线服务不依赖该账本。
历史全量完整性核验应作为独立离线工作，不放在每次在线写入或常规启动路径。

兼容 PostgreSQL 的在线引擎调用 `enqueue_decision`，同样不等待数据库提交，采用 64 个任务/32 MiB 的后台准入限制。兼容写入暂不自动重试，以免提交结果不确定时重复写入规则明细。失败通过日志和状态报告；`GET /v1/persistence` 使用 publisher 凭据读取状态。HTTP/gRPC 的 `x-corint-persistence` 响应头/元数据表明 `queued` 或 `disabled`。
显式调用底层 `DecisionResultWriter::write_decision` 仍等待事务提交；SDK 宿主退出前应调用 `engine.shutdown_persistence(timeout)`，并保持 Tokio runtime 存活直到排空。
兼容表格式保留原协议，不冒充上述 Core 版本化记录/反馈日志。

## 外部特征输入

可通过操作员 `feature_pipeline` 文件配置公共 DecisionHost，在严格 Core 前执行固定截止点的 SQLite/PostgreSQL 聚合或表达式。
策略和资源配置共同批准，成功与错误均可异步保存本次实际输入及特征证据，详见[Feature → Core 契约](feature-pipeline.md)。
此时业务评估必须覆盖当前完整特征集合，不能复用没有资源绑定的旧批准。

## 有界运行指标

Core 配置 v2/v3 支持顶层 `enable_metrics`，缺省为 `true`；设置 `false` 后执行器不采集指标，
该设置在策略重载后仍然生效。修改操作员配置需要重启服务。
`GET /v1/core/metrics` 仅允许 publisher，返回当前 `revision` 和 `metrics` 聚合快照。
关闭时仍可查询，返回 `enabled: false` 及空指标列表。

耗时以秒记录到固定 23 个桶，保留总次数与总耗时，不保留原始样本。每个 collector 的 counter 和
histogram 分别最多 128 个，名称限制为 1–128 UTF-8 字节；超限注册被忽略并计入
`rejected_registrations`。指标格式与[兼容 HTTP 指标](../API_REQUEST.md#运行指标)相同，
最后一个桶用 `upper_bound: null` 表示正无穷，各桶计数累计到上界（含上界）。
负数、非有限样本和导致总和溢出的样本不计入统计。

Core 成功重载会创建新 collector，统计从零开始；旧版本在途请求继续更新旧 collector，
其后续指标不再从当前版本接口导出。兼容服务重载则复用 collector。导出无跨指标原子性。
SDK 的 `Histogram::percentile` 改为桶内线性插值的近似值，0/100 分位保留精确最小/最大值，
落入最后无穷桶的中间分位返回桶下界；不应依赖它得到精确的逐样本排名。

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

- `cargo test -p corint-decision-server --test core_activation --test durable_decisions --test shared_snapshots`
- `python3 tests/scripts/run_p1_postgres_tests.py`：临时 PostgreSQL、私有 Unix socket、禁止 TCP，结束后清理。
- `cargo test -p corint-decision-ffi`：真实 repo、C ABI 决策与版本条件重载。
- `tests/scripts/run_core_e2e_tests.sh`：普通 Agent 文件经公开 CLI 到真实 Core HTTP 进程。

这些测试使用合成策略/样例和独立测试凭据；证明执行、事务与权限机制，不证明客户业务效果或外部生产环境已经部署。

## 修订历史

- 2026-09-14：在线 journal 改为单条决策校验、唯一索引查重和事务插入；移除在线反馈管理和全历史账本重建，结果导出凭据改为可选；增加旧库一次性升级与索引/计数。

- 2026-09-14：响应与数据库提交解耦，增加有界后台写入、状态与失败统计、Core 幂等重试及正常停机排空；明确异步内存队列的丢失窗口。

- 2026-09-14：运行指标采用固定桶并限制名称基数，接入采集开关及 publisher 专用 JSON 导出，明确重载和近似分位语义。

- 2026-09-14：接入可选 DecisionHost 特征准备，绑定完整资源配置与业务证据，记录实际执行输入并保持异步落库。
