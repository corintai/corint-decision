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

响应头 `x-corint-revision` 标识当前运行快照，`x-corint-compiled-sha256` 标识编译策略。
两者不是严格 Core 的发布批准或业务验收证据。HTTP 与 gRPC 的信号均为小写；
FFI 原生响应结构不同，不能直接套用本页的 HTTP 响应格式。

## 错误与管理

非法 JSON、非法字段或不支持的异步请求返回 400；认证与角色不足分别返回 401/403。
执行失败返回错误响应，不应当作 `approve` 或“规则未命中”处理。
`POST /v1/repo/reload` 使用独立 publisher 凭据，可携带 `x-corint-expected-revision`；
冲突和 busy 返回 409，候选加载失败保留现有快照。
服务限定 loopback；部署前置网络边界由操作员管理。

本页请求示例由 [生产 REST router 测试](../crates/corint-decision-server/tests/shared_snapshots.rs)
直接执行，覆盖请求接受、被禁止字段、异步拒绝及小写响应。

`api` 已从请求模型移除；即使值为 `null` 也按未知字段拒绝。在线 SDK 的服务结果统一使用 `service`。
