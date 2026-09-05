# 严格 Core 服务端：本地操作者授权与原子激活（实验性）

这是[目标兼容性检查](README.md)之后的服务端增量。默认不开启，不改变 CDL 语法，
不增加 DecisionPolicy，也不修改 source package / source bundle v1。
服务端仅对**本进程构建并持有的严格 Core 引擎**负责，不远程核验任意目标。

## 信任与部署边界

- 显式设置 `CORINT_CORE_CONFIG` 后，服务端只启动严格 Core HTTP 路由。
  配置错误、初始策略失败或密钥缺失时退出，不回退到兼容模式。
- 该模式仅允许 loopback 监听，单进程、单目标，不加载旧 repository/datasource 配置，
  不启动旧 gRPC 引擎，不开放 `/v1/decide` 或 `/v1/repo/reload`。
  未启用该模式时旧入口仍保持原状，**其安全问题并未因此修复**。
- 业务上下文、目标声明、独立验收样例、初始源码 bundle、批准指纹列表均由本地操作者配置。
  控制这些文件和进程环境的主体就是当前信任根；应通过操作系统权限保护它们。
  YAML/JSON 中的来源、Agent 身份、`compatible: true`、历史测试或审批字段都不能授权激活。
- 两个不同的 Bearer token 分别用于决策和发布接口。凭据从指定环境变量读取，不写入配置，
  不记录到日志，也不回显。必须为 32–1024 字符的无空格可打印 ASCII，操作者应使用高熵随机值。
  长度检查不证明熵。HTTP 鉴权先于正文读取，比较固定长度 SHA-256 摘要时使用恒定时间比较。
- 不提供 SSO/RBAC、多租户隔离、TLS、在线吊销或凭据轮换；不应通过公网代理暴露该模式。
  Rust 嵌入者可以复用 router，但改变监听/部署边界需要自行承担相应鉴权和传输安全设计。

## 操作者配置

`CORINT_CORE_CONFIG` 指向严格 JSON 配置；所有文件路径相对于配置文件所在目录。
文件必须为常规 UTF-8 文件，每份最多 8 MiB。以下是**配置结构示意，指纹占位符必须替换**：

```json
{
  "config_version": "1",
  "listen": "127.0.0.1:8080",
  "context": "business-context.yaml",
  "target": "target-capabilities.json",
  "cases": "acceptance-cases.yaml",
  "initial_bundle": "initial.core-sources.json",
  "decision_token_env": "CORINT_CORE_DECISION_TOKEN",
  "publisher_token_env": "CORINT_CORE_PUBLISHER_TOKEN",
  "approvals": [
    {
      "policy_sha256": "<64位策略内容指纹>",
      "context_sha256": "<上下文原文SHA256>",
      "target_sha256": "<目标声明原文SHA256>",
      "cases_sha256": "<服务端验收样例原文SHA256>"
    }
  ]
}
```

列表须有 1–128 个明确条目，不接受通配符。初始策略和后续候选都必须命中同样的列表。
`policy_sha256` 来自 `corint check-target` 的兼容性报告或既有 source package 内容身份；
其余指纹对各文件原始 UTF-8 字节计算 SHA-256。不要把“检查通过”自动转换为操作者批准。
批准前需要人工/外部治理审核业务意图、风险和独立验收样例。

上下文、目标、验收样例任意内容变化（包括排版）都会使不匹配的批准条目失效。
检查程序指纹不纳入操作者 allowlist：升级后仍强制在新程序下重新编译、兼容性检查、执行样例并构建引擎，
新 receipt 的 `binding_sha256` 绑定新宿主程序。不是复用旧程序的检查结果。

使用 [`corint export`](../cdl/exchange.md) 产生初始及候选 source bundle。
在完成独立审核并设置两个环境凭据后启动：

```sh
cargo build -p corint-decision-server --locked --offline
CORINT_CORE_CONFIG=/absolute/path/core-server.json ./target/debug/corint-decision-server
```

构建需要 Rust、缓存的依赖以及 `protoc`；无依赖缓存时去掉 `--offline`。
该指令不会自动创建凭据或批准。配置和凭据在启动时固定；修改文件不热加载，须重启并重新验收初始策略。

## HTTP 契约

所有请求须提供对应 `Authorization: Bearer ...`，无权限为 401。以下 JSON 是结构示意。

| 方法 / 路径 | 凭据 | 作用 |
|---|---|---|
| `GET /v1/core/target` | publisher | 返回当前活动版本、策略/契约/样例指纹及本地验收事实，不返回密钥或验收样例 |
| `POST /v1/core/decide` | decision | 只接受 `event` 和可选 `enable_trace`，通过严格引擎执行 |
| `POST /v1/core/policies/activate` | publisher | 验收候选 bundle，匹配操作者批准后原子激活 |

决策请求：`{"event":{"amount":1001},"enable_trace":true}`。
不接受客户端 `features/api/service/llm/vars` 命名空间注入；未知顶层字段拒绝。
响应包含 `snapshot` 和原生引擎 `decision`，不沿用兼容 REST 的结果改写；
没有路由命中等执行错误返回 422，不将其默认为 approve/pass。

激活请求：

```json
{
  "expected_revision": "<GET target 返回的 revision>",
  "bundle": {"format": "corint-core-source-bundle", "...": "完整source bundle v1"}
}
```

`bundle` 须遵守 [source bundle schema](../cdl/schema/source-bundle.json)，不是 package，不能携带历史证据。
源码标签是诊断标识，不作为文件读取/写入路径。不接受 context、target、cases、approval 或 passed 等额外字段。
正文限制为 8 MiB；结构错误 400，超限 413，编译/兼容性/行为拒绝 422，未命中操作者批准 403。
行为失败只返回错误码，不回传私有样例输入和期望值。

## 激活顺序与快照

1. 鉴权并检查 `expected_revision`，拒绝过期请求。
2. 在阻塞 worker 中验证完整源码 bundle，复用共享目标兼容性检查器。
3. 匹配本地操作者 allowlist 中的策略、上下文、目标与样例指纹。
4. 用服务端固定样例重新跑真实引擎（含 Trace 开/关），全部通过后构建新的严格引擎。
5. 再次比较活动 revision，然后一次性替换引擎及身份快照。

准备阶段限一个并发任务，冲突为 429；版本比较失败为 409。失败不更改活动策略。
决策请求只短暂持锁取得一个 `Arc` 快照；执行期间不持全局锁，结果身份与实际执行引擎来自同一快照。
每次激活使用新的 UUID revision，重启也使用新 revision，因此旧激活请求不能跨重启直接重放。
网络响应丢失时先查询当前状态，不盲目重试旧 revision。

返回 receipt 的 `scope: local_operator_activation`，区分：
`local_engine_constructed`、`server_owned_cases_passed`、`operator_allowlist_matched`。
这些是当前服务实例的执行事实，不把原有兼容性报告里的 `live_target_verified: false` 改为远端认证。
`business_evaluation` 仍为 `not_performed`；receipt 无签名，不是跨宿主可移植的审批证明。

**激活仅在内存中生效，不持久化、不修改初始 bundle。** 重启会重新加载操作者配置的初始策略。
没有持久审计、分布式一致性、灰度/回滚流程、持久化发布记录、OutcomeEvent 或真实 Work 接入。
这些是后续生产发布控制面的工作，当前版本不可据此宣称生产发布体系完成。

## 验证证据

[Core HTTP 集成测试](../../crates/corint-decision-server/tests/core_activation.rs) 使用真实 Axum router、
严格编译器和引擎，不访问业务数据、不启动外部监听。覆盖角色分离、请求字段拒绝、初始策略验收、
批准绑定失效、错误行为拒绝、失败不切换、并发竞争及决策快照一致性。
另有 [Core 进程级 e2e](../../tests/CORE_E2E.md)：固定模型响应经真实生成器及 CLI
构建、验证、导出，再启动真实 server 二进制，通过随机 loopback 端口验证鉴权、激活、
决策与条件 Trace、更新失败保持旧状态，以及重启后重新加载初始策略。
它使用临时目录和合成样例，不连接业务数据库或在线模型。
独立 CI Core job 已配置这两类测试；本地通过不等于部署端或完整 CI 已通过。
