# 严格 Core 服务端：以 repository 为唯一策略来源（实验性）

由 `CORINT_CORE_CONFIG` 显式启用，支持配置 **`2`** 和强制持久审计的 **`3`**。新配置、持久化反馈、业务证据和非文件 repo 详见 [Core 运行保障](core-operations.md)。
服务启动和重载均读取同一已配置 repository；内存引擎只是已验收 repo 版本的派生快照。
源码、版本历史与发布选择由 repo 管理。HTTP 不接受策略上传，不产生独立持久化策略库。

## 信任与部署边界

- 仅允许 loopback、单进程、单目标；不启动旧 gRPC 引擎或兼容 `/v1/decide` 路由。
- 配置错误、repo 无效或初始验收失败时退出，不回退到宽松加载或历史内存版本。
- repo、上下文、目标、独立验收样例和批准指纹列表由本地操作者控制。
  Agent/Work 应先经授权向 repo 提交候选，完成发布方验收后再更新发布声明。
- 两个不同的 Bearer token 分别用于决策和重载，凭据从环境变量读取。
  必须是 32–1024 个无空格可打印 ASCII 字符；操作者应使用高熵随机值。
  鉴权先于正文读取，固定长度 SHA-256 摘要采用恒定时间比较。
- 本地配置和操作者批准是当前信任根。JSON 中的来源、历史报告、Agent 身份均不授予权限。
- 不提供企业身份、TLS、多租户或跨节点发布；已配置业务证据时，新请求重读本地信任清单消费撤销；该本地模式不应通过公网代理暴露。
- 文件 repository 当前使用 Unix 的受限文件读取。SQLite/PostgreSQL/HTTP 完整发布文档已使用同一严格门禁，旧逐资源 repository 与兼容 REST/gRPC/FFI
  不会自动获得严格 Core 保证；本次没有迁移既有客户策略的语义。

## Repository 发布契约

配置中的 `repository` 指向一个目录。目录内固定的 `published.json` 是已发布版本的选择和完整性声明，
遵循 [Published Core Repository v1 schema](schema/core-repository.json)。其示意结构：

```json
{
  "format": "corint-core-repository",
  "format_version": "1",
  "revision": "payment-2026-09-05-1",
  "input_schema": "input-schema.yaml",
  "entries": ["registry.yaml"],
  "policy_sha256": "<64位小写SHA256，必须替换>"
}
```

`input_schema`、`entries` 和 import 路径均相对于 repo 根目录，遵守
[严格 import 路径与资源限制](../resolution.md)。没有 import 的文件集合可在 entries 中列出所有资源。
只读取声明可达的闭包，不扫描目录猜测入口。拒绝越界路径、符号链接、重复 ID、缺失引用和不支持语义。
`published.json` 最大 64 KiB，必须是常规文件；重复字段、未知字段/版本、非法 revision 和指纹均拒绝。

`policy_sha256` 必须使用针对**同一原始文件闭包**运行 `corint resolve` 得到的
`resolution.policy_sha256`，不能直接复用解析前的 source package 指纹。
解析器将原始源码、路径和 import 图绑定到冻结源码身份，修改源码或图后必须重新解析并验收。
该指纹用于检测混合版本和未发布编辑，不是签名或审批。

发布方流程：在稳定候选 checkout 上 resolve → 对冻结结果运行独立行为样例和目标检查 → 独立审批
→ 将确定的源码版本及 `published.json` 发布到 repo → 请求服务重载。版本历史由 Git 或所选 repo 后端保存。
推荐使用不可变 checkout/版本目录，并原子更新发布声明；不要逐文件修改服务正在读取的发布目录。
服务是只读消费者，不替发布方写入源码、历史或发布指针。

加载时核对解析闭包的指纹；解析之后及耗时验收结束时重新核对发布声明。
源码混版返回 `E_REPOSITORY_DIGEST`，发布声明在准备期间变化返回 `E_REPOSITORY_CHANGED`。
这不会把可变文件系统变成数据库事务：已经冻结且通过验收的快照可继续运行；后续 repo 更新由下一次重载消费。

## 操作者配置

`CORINT_CORE_CONFIG` 指向严格 JSON 配置。文件路径相对于配置文件目录，`repository` 为目录路径。
上下文、目标与样例文件最大各 8 MiB。以下为结构示意，批准指纹须替换为实际审核的值：

```json
{
  "config_version": "2",
  "listen": "127.0.0.1:8080",
  "repository": "repository",
  "context": "business-context.yaml",
  "target": "target-capabilities.json",
  "cases": "acceptance-cases.yaml",
  "decision_token_env": "CORINT_CORE_DECISION_TOKEN",
  "publisher_token_env": "CORINT_CORE_PUBLISHER_TOKEN",
  "approvals": [{
    "policy_sha256": "<解析后策略指纹>",
    "context_sha256": "<上下文原文字节SHA256>",
    "target_sha256": "<目标声明原文字节SHA256>",
    "cases_sha256": "<验收样例原文字节SHA256>"
  }]
}
```

批准列表要求 1–128 个精确条目，不支持通配符。初始加载、重载和回滚均必须命中该列表。
上下文/目标/样例的任何字节变化均使不匹配批准失效。程序升级后仍在新程序下重新编译和运行样例，
新的兼容性绑定使用新宿主指纹。配置、凭据和批准在启动时固定，修改这些控制文件须重启。

```sh
cargo build -p corint-decision-server --locked --offline
CORINT_CORE_CONFIG=/absolute/path/core-server.json ./target/debug/corint-decision-server
```

构建需要 Rust、依赖缓存和 `protoc`；无依赖缓存时去掉 `--offline`。启动命令不会生成凭据或批准。

## HTTP 契约

| 方法 / 路径 | 凭据 | 作用 |
|---|---|---|
| `GET /v1/core/target` | publisher | 返回当前执行快照及其 repo 身份 |
| `POST /v1/core/decide` | decision | 严格执行，仅接受 event 和可选 enable_trace |
| `POST /v1/core/repo/reload` | publisher | 重新读取配置的 repo，验收后原子替换快照 |

重载请求仅为：`{"expected_revision":"<GET target 返回的 revision>"}`。
它不接受 bundle、repo 路径、目标版本、context、cases、approval 或验证报告。
旧 `/v1/core/policies/activate` 上传路由已移除，返回 404。

决策请求示例：`{"event":{"amount":1001},"enable_trace":true}`。
不接受可信 features/api/service/llm/vars 注入。响应包含原生 `decision` 和实际执行的 `snapshot`。
执行错误返回 422，不改写成 approve/pass。

鉴权失败 401，正文/未知字段错误 400，正文超过 8 MiB 为 413；
加载/编译/目标/行为拒绝 422，未匹配批准 403，并发准备繁忙 429，活动 revision 过期 409。
行为失败仅返回错误码，不泄漏私有验收输入和期望。

## 快照、重启与回滚

启动与重载复用同一个准备函数：读取发布声明和完整引用 → 检查指纹 → 目标兼容与批准匹配
→ 服务端独立样例重跑（含 Trace 开/关）→ 构建严格引擎 → 再查 repo 发布声明。
重载在阻塞 worker 中完成准备，整个准备与切换期间限一个重载任务；最后再次比较运行 revision，
并一次性替换引擎与身份。决策只短暂持锁取得一个 Arc，执行期间不持全局锁。

响应区分两种版本：

- `revision`：当前进程运行快照的 UUID，每次重载和重启都更新，供请求并发控制。
- `repository: {revision, manifest_sha256}`：已加载 repo 版本和发布声明指纹。
  同一 repo 版本重启后保持该身份以及 `policy_sha256`。

receipt 的 `scope` 为 `local_repository_reload`，保留 `local_engine_constructed`、
`server_owned_cases_passed`、`operator_allowlist_matched`，业务评估仍为 `not_performed`。
receipt 是当前服务的执行事实，不是跨宿主审批证明。

重启始终加载 repo 当前声明的版本。若外部发布了无效 repo，正在运行的服务重载失败后继续使用旧快照，
但再次启动会失败；服务不会私自回写 repo 或静默回退。发布方应先验收再更新发布声明。
回滚需要在 repo 恢复历史源码及对应发布声明，再触发同一重载流程；仍需满足当前批准和样例。
响应丢失时先查询当前快照，避免直接重试已过期的 revision。

## 从配置 v1 迁移

v1 的 `initial_bundle` 和 HTTP 正文激活不能满足 repo 单一来源要求，明确停止支持；不会自动降级或迁移。
将策略源码放入文件 repo；按现有 resolve/测试/目标检查流程得到当前指纹，审核并维护批准列表；
创建 published.json 后改用 `config_version: "2"` 和 `repository`。调用方改为无源码正文的重载请求。
CDL 语言版本、source package/bundle v1 格式和兼容入口保持各自原有含义。

## 验证证据

[工具链 repo 测试](../../crates/corint-decision-toolchain/tests/repository.rs) 检查发布指纹、版本变化、路径、
重复字段、缺失依赖、符号链接、FIFO、大小限制和只读加载。
[Core HTTP 测试](../../crates/corint-decision-server/tests/core_activation.rs) 验证批准与独立样例、并发重载、
失败保留旧快照、输入边界、repo 更新/回滚/重启一致性，以及旧上传入口不存在。
[真实进程 e2e](../../tests/CORE_E2E.md) 覆盖固定模型、CLI、repo 发布、真实 TCP HTTP、重启和回滚。
它们使用合成数据与临时目录；不代表真实 Work、业务效果、企业审批或跨节点发布已完成。

## Revision History

| Date | Changes |
|---|---|
| 2026-09-05 | 更新 repo 唯一来源、启动/重载、授权、回滚和配置迁移契约。 |
