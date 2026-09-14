# 多租户决策宿主

## Audience

面向决策 API 接入方及负责租户配置、凭据和数据隔离的操作员。

## Feature Overview

业务决策统一使用 `POST /v1/decide`，请求体顶层指定 `tenant_id`，策略由 Registry 按 `event.type` 匹配。
服务端内部以 `tenant_id + environment + deployment` 为隔离范围，复用严格 Core 和统一 DecisionHost。租户逻辑位于认证、运行管理和数据资源层，不改变 CDL 规则语义。

## 本地单租户：默认 local

原单租户入口保持兼容：不设置 `CORINT_TENANT_ID` 时，兼容 HTTP/gRPC 使用 `local`；严格 Core 的 Journal 未指定 `tenant_id` 时也使用 `local`。显式配置会覆盖默认值，空值仍拒绝。身份凭据仍然必需。

操作员配置中的多租户 Scope 必须显式填写三个字段，不会因为遗漏配置自动并入 `local`。调用方不传 environment 或 deployment；测试、生产通过不同域名对应的服务配置区分，不从请求 Host/Header 推断可信环境。

## Steps

### 快速运行

在仓库根目录执行，输出目录必须不存在：

```sh
python3 quickstart/tenant_demo.py --output /tmp/corint-tenants-demo
```

脚本构建 CLI/server，以合成策略生成 `local/dev/risk`、`acme/dev/risk` 两个部署，启动真实 loopback HTTP，验证鉴权、幂等、Agent 权限、独立暂停，以及数据库凭据创建／轮换／撤销，然后正常停机。随机凭据只写入权限为 `0600` 的 `credentials.env`，不打印到终端。

再次启动：

```sh
source /tmp/corint-tenants-demo/credentials.env
./target/debug/corint-decision-server
```

生成配置使用端口 `0`，实际监听地址见启动日志。需要固定端口时，修改 `tenants.json` 的 `listen`。`--prepare-only` 仅生成配置。示例使用合成数据与本地操作员批准，不构成真实业务评估或生产审批。

## 入口与配置

`CORINT_TENANT_CONFIG` 指向操作员拥有的 JSON 文件，与 `CORINT_CORE_CONFIG` 互斥。选择多租户模式后不启动兼容 REST/gRPC 入口，也没有不带租户范围的 `/v1/core/decide`。

配置结构如下；完整有效的 Core 配置和批准指纹由示例脚本生成：

```json
{
  "format_version": "1",
  "listen": "127.0.0.1:3001",
  "credentials": "credentials.json",
  "control_store": {"type": "sqlite", "path": "control.sqlite"},
  "platform_limits": {"max_inflight":64,"requests_per_second":1000,"burst":1000,"max_connections":128},
  "tenant_limits": {
    "local": {"max_inflight":16,"requests_per_second":100,"burst":100,"max_connections":32}
  },
  "max_loaded": 8,
  "max_preparations": 2,
  "deployments": [{
    "scope": {"tenant_id":"local","environment":"dev","deployment":"risk"},
    "root": "local",
    "core_config": "core.json",
    "limits": {"max_inflight":16,"requests_per_second":100,"burst":100,"max_connections":32},
    "timeout_ms": 10000,
    "idle_seconds": 300,
    "resources": []
  }]
}
```

每个部署目录必须位于平台目录内，不能相同或相互嵌套。上下文、目标、样例、特征配置及审批文件限制在自己的部署目录。平台凭据文件位于部署目录之外。最多注册 256 个部署，所有配置读取限制为 8 MiB；未知字段拒绝。

Core 使用 v3 和可靠 Journal。多租户身份由外层统一认证，Core 配置的 `decision_token_env`、`publisher_token_env` 可填空字符串，Journal 的 `consumer_token_env` 不参与多租户授权。

控制存储也支持 PostgreSQL：

```json
{"type":"postgres","url_env":"CORINT_CONTROL_POSTGRES_URL","schema":"corint_tenant_control"}
```

它保存暂停状态、并发版本、管理审计及平台凭据。租户运行状态读写带完整 Scope；凭据存储属于平台管理面，只允许平台管理员操作。控制存储不可用时，在线入口拒绝请求。使用专用 schema 和平台管理账号，不要向租户授予该数据库账号。

## 身份、权限与 Agent 委托

数据库是凭据的唯一权威来源，SQLite 与 PostgreSQL 使用相同机制。首次启动空控制库时，`credentials` 指向的文件和环境变量用于导入初始凭据；数据库已有凭据后不再读取它们，可以移除文件、环境变量和该配置项。文件修改或重启不会恢复已撤销的凭据。每个主体有独立 token，可为同一租户的多个系统和 Agent 分别授权。以下文件仅用于首次初始化：

```json
{
  "format_version":"1",
  "principals":[
    {"id":"platform","token_env":"PLATFORM_TOKEN","platform_admin":true,"grants":[]},
    {"id":"operator","token_env":"LOCAL_OPERATOR_TOKEN","grants":[
      {"scope":{"tenant_id":"local","environment":"dev","deployment":"risk"},
       "permissions":["decide","inspect","publish","consume","export","manage"]}
    ]},
    {"id":"agent","token_env":"LOCAL_AGENT_TOKEN","delegated_by":"operator","expires_at_ms":2000000000000,"grants":[
      {"scope":{"tenant_id":"local","environment":"dev","deployment":"risk"},"permissions":["decide"]}
    ]}
  ]
}
```

示例中的到期时间需要替换成实际短期授权期限。委托只能来自直接父主体，权限和 Scope 必须是父主体授权的子集，到期时间不能超过父主体；委托凭据不能成为平台管理员。父主体过期或撤销也会使子凭据失效。普通主体可配置 `expires_at_ms`。

| 权限 | 可执行操作 |
| --- | --- |
| `decide` | 在线决策与业务幂等重试 |
| `inspect` | 查看本部署目标、指标、持久化状态、运行状态和管理审计 |
| `publish` | 按预期 revision 重载已发布且经批准的策略 |
| `consume` | 领取和确认本部署 outbox |
| `export` | 在配置允许时，额外获取私有输入和完整历史响应；同时需要 consume |
| `manage` | 按版本暂停、恢复和卸载本部署 |

`platform_admin` 提供平台部署清单以及凭据创建、轮换、撤销、刷新权限，不自动获得任何租户决策、发布或数据权限。重复 Authorization Header、过期凭据和未知凭据都拒绝。

平台管理员携带自己的 Bearer token 调用 `POST /v1/tenancy/credentials` 管理凭据。例如为租户 local 创建决策客户端：

```json
{
  "action": "create",
  "id": "payment_client",
  "grants": [{
    "scope": {"tenant_id": "local", "environment": "dev", "deployment": "risk"},
    "permissions": ["decide"]
  }]
}
```

成功响应为 `{"principal_id":"payment_client","revision":1,"token":"<服务端生成的 token>"}`。token 使用操作系统随机源生成 256 位随机值，编码为 64 位十六进制字符串，只在创建或轮换响应中返回一次；响应带 `Cache-Control: no-store`。服务端只持久化 SHA-256 摘要、主体、租户授权、到期时间和撤销状态，不保存 token 明文或环境变量引用。可用 `expires_at_ms`、`delegated_by` 创建有期限的 Agent 凭据。

同一接口支持 `{"action":"rotate","id":"payment_client"}` 和 `{"action":"revoke","id":"payment_client"}`。轮换返回新 token；撤销返回 `token: null`。已过期或撤销的凭据不能通过轮换恢复。主体 ID 不重复使用，凭据登记上限为 2048 个（包含已撤销主体）。禁止撤销最后一个有效平台管理员；授权越界、非法配置返回 422，跨节点并发版本冲突返回 409，管理员应重新读取业务状态后重试。若创建响应丢失，可根据原主体 ID 轮换取得新 token。

`tenant_credentials` 保存有版本的完整摘要注册表，使两个数据库后端都能一次读取一致快照；凭据管理是低频操作，修改用版本条件更新和事务审计，避免并发覆盖。`tenant_credential_audit` 只记录操作者、动作、目标主体、版本和时间，不记录 token。数据库已有但当前宿主未加载的 Scope 授权仍保留，只能用于对应 Scope，不能转为其他租户权限。控制库应随配置一起备份，不应在普通重启中清空。

请求认证只查询内存，并逐次检查摘要、有效期、撤销状态及目标租户权限。凭据变更提交数据库后立即更新本机缓存；共享同一控制库的其他节点每 60 秒自动刷新。同步失败时最多使用自上次确认起 120 秒的缓存，随后认证返回 401；数据库恢复后自动刷新。`POST /v1/tenancy/credentials/reload` 可强制刷新当前节点，无需等待定时任务，来源始终是数据库；需要携带平台管理员 Bearer token，成功返回 `{"reloaded":true}`。多节点强制刷新时应逐个调用节点地址。撤销和轮换作用于后续认证，已开始执行的请求不被中断；其他节点存在上述同步窗口。管理修改额外从数据库重新验证管理员权限，避免使用缓存中的旧权限修改凭据。

该机制适用于多租户 HTTP 入口；兼容 HTTP/gRPC 设置 `CORINT_AUTH_CONFIG` 后也使用同一凭据服务。配置和迁移说明见[兼容服务契约](compatibility-server-snapshots.md)。未配置数据库认证的兼容服务保留环境变量认证。

## Inputs and Outputs

### 业务决策 API

```http
POST /v1/decide
Authorization: Bearer <token>
Content-Type: application/json

{
  "tenant_id": "acme",
  "business_event_id": "payment-123",
  "idempotency_key": "payment-123:decision-1",
  "event": {"type": "payment", "amount": 1500}
}
```

`tenant_id` 是请求元数据，必填且只允许在 JSON 顶层出现一次；不放在 URL、query 或 `event` 中。
顶层仅接受 tenant_id、event、business_event_id、idempotency_key、enable_trace，重复字段和未知字段拒绝。
Bearer 认证先于请求体读取；正文有 8 MiB 上限、10 秒读取超时及独立的有界读取并发。
解析 tenant_id 后验证该主体是否有对应内部 Scope 的 decide 权限，再执行原有配额、策略及持久化流程。
未知租户或无权访问返回 403；无效/过期凭据返回 401；无效正文和调用方指定环境/部署返回 400。

同一服务里某租户只有一个运行实例时，入口自动绑定它，无需额外配置。
若操作员注册了同租户多个内部 Scope，则必须在平台配置中指定唯一决策入口，否则启动失败：

```json
{"decision_bindings":{"acme":{"tenant_id":"acme","environment":"prod","deployment":"risk"}}}
```

此映射属于服务端配置，不是请求参数；绑定的租户必须一致，目标 Scope 必须已注册。
凭据只负责授权，不能通过携带另一个环境的凭据更改默认入口。
一个绑定中的 Registry 可以按 event.type 路由多个业务策略，不需要为支付、登录等场景分别指定部署。
修改绑定随服务重启生效；请求重试仍在绑定的 Journal 范围内幂等，切换存储目标不自动迁移旧幂等键。

## 管理与兼容 API

管理接口和已有调用的兼容前缀保留为：`/v1/tenants/{tenant}/environments/{environment}/deployments/{deployment}`。

| 方法与后缀 | 内容 |
| --- | --- |
| `POST /decide` | 沿用 Core event、business_event_id、idempotency_key、enable_trace |
| `GET /target` | 当前冻结快照与 tenant_scope、resource_scope_sha256 |
| `POST /repo/reload` | `{"expected_revision":"当前 revision"}` |
| `GET /metrics` | 当前快照的有界引擎指标 |
| `GET /persistence` | 当前 Journal 容量与持久化状态 |
| `POST /outbox/claim` | 领取本部署记录；私有字段由 export 权限控制 |
| `POST /outbox/ack` | 沿用 lease、idempotency_keys |
| `GET /runtime` | 持久暂停状态、控制 revision、加载状态、配额和错误/超时计数 |
| `POST /runtime` | `{"expected_revision":0,"paused":true}`，恢复用 false |
| `POST /runtime/unload` | 仅在暂停且其他请求已排空时释放本部署缓存 |
| `GET /runtime/audit` | 本部署最近 100 次暂停/恢复记录，包含可信 actor 和请求 ID |

`GET /v1/tenancy/deployments` 对普通主体仅返回具备 inspect 权限的部署。管理状态的数字 revision 与策略快照 revision 是不同的并发控制标识。租户输入不能覆盖 event 中的 tenant_id/environment/deployment/tenant_context 保留名；额外 query 参数拒绝。

认证请求获得服务端 `x-corint-request-id`。决策记录中的 `tenant_context` 保存 tenant、environment、deployment、principal_id、actor_id、request_id；幂等重放返回原决策记录，其原始 actor 不会被重试者改写。请求日志另记录当前重试者。

## 数据源、仓库和批准边界

当前多租户执行范围是严格 Core，以及 SQLite/PostgreSQL 聚合特征。每个特征数据源必须列在部署的 resources 中：

```json
{
  "datasource":"events",
  "revision":"db-v1",
  "config_sha256":"<规范化 DataSourceConfig 的 SHA-256>",
  "entity":"events",
  "tenant_column":"tenant_id",
  "environment_column":"environment"
}
```

宿主先验证数据源名、版本和完整配置指纹，再构造不可变的 DataAccessScope。SQL 客户端将可信 tenant/environment 等值条件追加为 AND 条件，校验授权表及标识符，并使用只读连接。配置改变必须重新授权；同名用户不会合并不同租户/环境的数据。它不使用连接池里可残留的可变 current_tenant 会话变量。

scoped 查询不接受原始 SQL 字段表达式、子查询或未授权关系。租户和环境列必须是不同的简单标识符；列不存在时查询失败。共享业务表需要先完成两列的数据回填、索引与授权。数据库账号应使用最小权限，PostgreSQL 可再配置 RLS 作为额外边界；不会自动修改业务数据库的角色和策略。

DataSourceConfig 指纹通过 Rust 的 `canonical_sha256(&config)` 计算，config 需先反序列化以包含默认值。资源范围指纹是 `canonical_sha256({"scope":scope,"resources":resources})`。本地 OperatorApproval 必须同时声明 `tenant_scope`、`resource_scope_sha256` 以及原有策略/上下文/目标/样例/特征指纹；复制另一租户的批准列表会被拒绝。配置业务审批证据时，evaluation 和 approval 都必须含有匹配 Scope 的 `audience`，该字段也在原始证据摘要的覆盖范围内。

客户端及可变缓存不跨部署共享。查询缓存键纳入资源授权 namespace；严格在线链路继续要求查询缓存 TTL=0，Feature L1 仍未启用。不能把兼容模式的 Redis、名单、LLM 或任意服务调用直接挂入这个入口；这些适配器仍保持原有单实例使用范围，后续接入必须实现相应资源授权。

文件仓库限定在部署目录。SQLite/PostgreSQL 仓库使用单独的 `corint_tenant_publication` 表，按完整 Scope 查询；可参考 [SQL 表定义](tenant-publication.sql)。这允许同一库里不同租户使用相同规则 ID。HTTP 仓库尚未具有租户授权协议，多租户入口明确拒绝该后端。

## 持久化、扩容与生命周期

在线数据库访问强制绑定 tenant：Journal 的所有读取、插入、更新、删除、容量统计、幂等预留/过期清理和 outbox 操作，都通过 `journal/queries.rs` 的固定操作枚举构造。`Storage` 持有不可变 tenant，自动绑定 SQL 的第一个参数；业务层不能直接取得连接池或提交 SQL 字符串。控制状态和策略发布读取使用 `tenancy/queries.rs`，自动绑定 tenant 与完整部署范围。请求体无法覆盖这些参数。

Journal 的 `events`、`request_keys`、`journal_usage` 及控制库的运行状态、管理审计表均有显式 `tenant_id` 列。数据库约束拒绝遗漏或空 tenant 的新写入，决策正文/控制 Scope 中的 tenant 必须与该列一致。幂等预留对象也绑定 tenant 和 Scope，不能跨租户提交或放弃。平台初始化与 schema 迁移属于独立的操作员路径，不受单次业务请求范围限制。

首次打开旧格式存储时，在事务内补列、校验并回填历史归属、建立索引和写入约束。Journal 从已验证的存储归属回填，控制库从原有 `scope_key` 回填；缺失/非法身份会使整个迁移回滚，不默认为 local。迁移完成后正常启动和请求不会再扫描历史数据。升级前停止旧版本写入进程并备份；旧写入程序缺少 tenant 列时会被新约束拒绝。

当前按应用层 WHERE 条件隔离，未启用 RLS。此约束覆盖严格 Core/多租户入口的引擎数据库访问；外部特征入库程序、SQL 仓库发布者及数据库管理员直接执行的 SQL，仍需自行携带正确租户范围。兼容单实例适配器不作为共享多租户接口开放。

每个部署使用独立 SQLite 文件，或 PostgreSQL 独立 schema。Journal 除固定 tenant 外，还持久绑定完整 Scope；同租户不同环境也不能混用。未带 Scope 的旧入口不能打开已绑定的 Journal。已经标记为单租户的 Journal 即使为空也不能被多租户入口接管；已有非空历史 Journal 同样不能静默改归属。切换模式前先停止旧实例，保留原审计存储并显式迁移。

同一 Scope 的多个实例使用相同 PostgreSQL Journal schema 时共享幂等和投递租约；独立 SQLite 文件不提供跨节点去重。幂等身份不包含策略版本，重载和重启不会让同键请求重新执行。私有回放输入只在配置 `export_replay=true` 且消费者有 export 权限时返回；确认不删除审计记录。即使当前策略准备失败，持久化状态和 outbox 仍可独立访问。

控制数据库保存暂停状态和乐观并发版本。PostgreSQL 控制库可在多个实例间共享暂停状态；暂停后拒绝新决策和发布，已开始的请求排空。卸载只影响该部署，不调用进程级 shutdown。

部署按需加载，空闲后回收连接和宿主；max_loaded 控制驻留数量。内容相同的租户快照使用稳定 revision，单纯回收/重新加载不会制造新的逻辑版本。平台、租户、部署分别限制在线速率、突发量和在途数量；管理流量使用独立的有界准入（每部署 2、每租户 4、每宿主 16 个并发），确保在线配额耗尽后仍能暂停。编译并发另受 max_preparations 限制。

数据库连接按平台/租户/部署三级预算预留，旧快照、发布探测和新快照按实际存活期占用预算。Journal 预留 SQLite 1 个或 PostgreSQL 8 个连接，SQL 仓库在准备期间另预留 1 个连接；控制库另使用 SQLite 1 个或 PostgreSQL 最多 4 个连接。预算需容纳更新时的新旧宿主。请求体最多 8 MiB，读取超时不超过 10 秒。HTTP 执行超时返回 504 后，已经开始的决策仍持有许可并尝试可靠提交，调用方应以原幂等键重试；正常停机会额外等待这些请求排空。

配额是每个 worker 的配额，跨 worker 的总预算需要部署平台分配。当前没有跨节点策略发布调度器：发布方要同步批准的仓库内容并协调各实例重载；暂停、共享 Journal 幂等不等于策略发布已在全群完成。强 CPU/内存故障隔离仍可采用独立进程/工作节点。

新增/删除部署和变更资源授权通过操作员配置及重启生效。建议下线顺序：暂停 → 排空 → 卸载 → 撤销凭据 → 按保留策略处理存储 → 移除配置。备份/恢复单位为 Scope 对应的 Journal 文件/schema，并同时保存批准的仓库、特征配置、凭据控制库和控制审计。恢复后必须通过 Scope 检查，不能通过删元数据绕过。管理审计、决策与私有输入的保留和删除由操作员制定；本实现不会自动删除它们。

所有监听保持 loopback；TLS、企业身份联邦和租户自助注册属于上层平台集成。该入口实现应用内租户授权，不把公网网关或操作员文件权限当作由本组件自动部署完成的能力。

## 验证

```sh
cargo test -p corint-decision-server --test tenancy --locked
cargo test -p corint-decision-server --test tenant_database --test journal_backends --test durable_decisions --locked
cargo test -p corint-decision-runtime --features sqlx scoped_sql_and_cache --locked
python3 tests/scripts/check_tenant_sql.py
python3 tests/scripts/run_tenant_postgres_tests.py
```

测试覆盖同名用户/规则/幂等键、跨租户和跨环境访问拒绝、委托越权/过期、凭据原子轮换、共享真实 SQL 特征、SQL 表达式逃逸、缓存范围、资源配置漂移、私有导出、租约确认、单租户暂停/重载、控制审计、空闲回收、超时后可靠完成，以及 PostgreSQL 多实例幂等和暂停。测试使用合成数据，不代表生产负载下的容量或延迟承诺。

## FAQ

**调用方需要传环境和部署吗？** 不需要。业务请求只传顶层 tenant_id、凭据和事件；服务端配置决定内部 Scope。

**原来的完整路径还能调用吗？** 保留兼容；业务集成使用新的 `/v1/decide`。管理接口继续使用完整 Scope 路径。

**能通过 event.type 切换租户吗？** 不能。event.type 只在已授权租户的 Registry 中匹配策略，不改变租户身份或数据权限。

## 修订历史

- 2026-09-14：新增统一业务决策入口，tenant_id 改为 JSON 顶层参数，环境与部署由操作员绑定，Registry 保持按事件匹配策略。
- 2026-09-14：凭据改为数据库持久化、内存校验；新增创建、轮换、撤销与跨节点缓存刷新。
- 2026-09-14：自动凭据刷新改为每分钟一次，缓存失效上限调整为 120 秒，明确强制刷新接口。
