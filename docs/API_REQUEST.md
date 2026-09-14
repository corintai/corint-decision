# 兼容 HTTP 决策 API

本文描述当前兼容 REST 服务的已实现请求边界。严格 Core 使用独立服务及
[Core server 契约](contracts/core-server.md)。兼容 HTTP/gRPC 的认证、版本和重载协议见
[共享引擎快照](contracts/compatibility-server-snapshots.md)。

## 请求

`POST /v1/decide`，请求头为 `Content-Type: application/json` 和
`Authorization: Bearer <decision-token>`。凭据由操作员配置，不应写入策略或仓库。

<!-- executable-example: decide-request -->
```json
{
  "event": {
    "type": "transaction",
    "amount": 100,
    "user_id": "example-user"
  },
  "options": {
    "return_features": false,
    "enable_trace": false
  }
}
```

`event` 是必填对象，其业务字段由已加载策略定义。`options` 可省略，上述两个选项默认
均为 `false`。`async` 仅接受缺省或 `false`；`async: true` 返回 400，不提供任务轮询接口。
未知请求字段和未知选项会被拒绝。

调用者只能提交 event。值不为 `null` 的 `user`、`features`、`service`、`llm`、`vars`
命名空间以及 `event.tenant_id` 会被拒绝，不能用来覆盖可信数据。
租户由操作员注入；业务用户属性如需作为输入，应放入策略约定的 event 字段。

## 响应

成功返回 HTTP 200。响应字段如下：

| 字段 | 当前含义 |
| --- | --- |
| `request_id` | 本次请求标识 |
| `status` | 成功时为 200 |
| `process_time_ms` | 引擎报告的处理耗时 |
| `pipeline_id` | 选中的流程标识；无标识时为 `default` |
| `decision.result` | 小写 `approve`、`decline`、`review`、`hold` 或 `pass` |
| `decision.actions` | 策略输出的动作字符串数组 |
| `decision.scores.raw` | 规则聚合原始分数 |
| `decision.scores.canonical` | 兼容服务归一化分数，不构成业务风险校准 |
| `decision.evidence.triggered_rules` | 命中的规则标识 |
| `decision.cognition` | `summary` 与 `reason_codes` |
| `features` | 仅 `return_features=true` 时返回，当前兼容实现为引擎结果 context 的 JSON 映射 |
| `trace` | 请求启用且引擎生成时返回执行 trace |

自动生成的 `request_id` 使用 `rq_<6位Base62随机串>_<11位Base62雪花ID>`，
例如 `rq_a3F2eZ_9oVW9PpkHy8`，总长 21 字符。随机段每次请求重新生成，
字符集为 `0-9A-Za-z`。Base62 区分大小写，存储、比较和传输必须保留完整 ID 及大小写。
生成器无需外部服务；跨进程唯一性仍属于概率保证，不能把该字段当作业务幂等键。
兼容服务的错误响应使用同一生成器。SDK 显式传入的 `metadata.request_id` 继续原样沿用；
这不是 HTTP 请求新增的输入字段。算法细节见 [SDK 说明](../crates/corint-decision-sdk/README.md#request-id-generation)。

响应头 `x-corint-revision` 标识当前运行快照，`x-corint-compiled-sha256` 标识编译策略。
两者不是严格 Core 的发布批准或业务验收证据。HTTP 与 gRPC 的信号均为小写；
FFI 原生响应结构不同，不能直接套用本页的 HTTP 响应格式。

## 异步保存结果

策略计算仍在本次请求中完成，结果保存使用后台队列；这与请求选项 `async`（异步计算/任务轮询）不同。
配置结果 writer 时，HTTP/gRPC 返回 `x-corint-persistence: queued`，表示入队成功而非数据库提交；未配置时为 `disabled`。
每个 writer 最多保留 64 个未完成任务、32 MiB 序列化数据，队列满或关闭时请求报错。数据库后续失败不能撤回已返回结果，由日志及 publisher 专用 `GET /v1/persistence` 的状态计数报告。
兼容 PostgreSQL 写入不自动重试，避免不确定提交重复产生明细；Core 的幂等 journal 支持有界重试，见 [运行保障](contracts/core-operations.md)。
正常服务停机先停止请求，再最多等待 30 秒排空；崩溃或强制退出可能丢失尚未落库记录。该行为不提供“响应成功必已持久化”的保证。

## 错误与管理

非法 JSON、非法字段或不支持的异步请求返回 400；认证与角色不足分别返回 401/403。
执行失败返回错误响应，不应当作 `approve` 或“规则未命中”处理。
`POST /v1/repo/reload` 使用独立 publisher 凭据，可携带 `x-corint-expected-revision`；
冲突和 busy 返回 409，候选加载失败保留现有快照。
服务限定 loopback；部署前置网络边界由操作员管理。

本页请求示例由 [生产 REST router 测试](../crates/corint-decision-server/tests/shared_snapshots.rs)
直接执行，覆盖请求接受、被禁止字段、异步拒绝及小写响应。

`api` 已从请求模型移除；即使值为 `null` 也按未知字段拒绝。在线 SDK 的服务结果统一使用 `service`。

## 修订历史

| 日期 | 变更 |
|---|---|
| 2026-09-14 | 更新 Base62 请求 ID 格式、异步保存状态、队列限制和停机语义。 |
