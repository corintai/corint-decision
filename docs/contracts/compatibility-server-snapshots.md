# 兼容服务的共享引擎快照

普通服务启动一次 `DecisionEngine`，将同一个 `Arc<EngineManager>` 传给生产 REST router 和
`DecisionGrpcService`。HTTP 与 gRPC 的决策、健康查询和 repo 重载都使用这一个当前版本指针。
Rust 嵌入调用方也应按这一方式装配两个入口；`create_router` 和 `DecisionGrpcService::new`
现在接收 `Arc<EngineManager>` 和必须显式提供的 `AccessPolicy`。管理器实现位于 engine crate，FFI 复用同一实现。

## 请求与重载

请求短暂持锁取得 `Arc<EngineSnapshot>` 后释放锁，整个决策使用同一引擎与身份。
重载从配置的 repository 准备独立候选，在阻塞 worker 中完成加载、解析、编译和指纹计算，
不会持有版本指针的写锁。准备成功后比较运行 revision，再原子替换指针。
旧请求继续执行旧快照，新请求取得新快照；准备失败或版本冲突时保留当前快照。

每个进程同时只允许一个候选准备任务。并发调用立即返回 busy，不排队重复发布；
调用方取消请求时，后台准备任务仍持有占用名额直到结束，取消本身不会提交候选。
若取消发生在切换已经完成之后，则切换已生效；响应丢失后应先查询当前 revision。

兼容引擎的 `prepare_reload(&self)` 保留启动时的 executor、Feature/List 服务、连接池、
metrics 与结果写入器。该接口只重载策略与 registry，连接器配置变更仍需重启。
原有 `DecisionEngine::reload(&mut self)` 复用此候选构建逻辑。
文件 repo 的导入与启动 API 配置解析使用配置的 repo 根目录；列出的 pipeline 加载失败、
编译失败或已提供的 registry 解析失败均返回错误，不以跳过策略的方式报告重载成功。

## 版本与协议

HTTP 成功响应 header 和 gRPC 成功响应 metadata 使用同名字段，覆盖 decide、health 和 reload：

| 字段 | 含义 |
| --- | --- |
| `x-corint-revision` | 运行快照 UUID；每次成功重载和重启均变化，用于并发控制 |
| `x-corint-compiled-sha256` | 已编译 Program 与 registry 的确定性 JSON 指纹，标识本次执行的策略 |

该指纹不包含连接器配置，不是 repo commit、源码闭包哈希或严格 Core `policy_sha256`。
兼容 repo 尚未统一发布声明；本轮不伪造 repository revision。重启重新读取 repo，
在相同编译器下同一编译策略具有相同指纹，运行 UUID 则不同。
文件 repo 发布方应提供读取期间稳定的完整目录；目录跨文件变更的事务性发布仍由 repo 管理。

`POST /v1/repo/reload` 和 gRPC `ReloadRepository` 可通过 request header/metadata
`x-corint-expected-revision` 指定预期运行版本。原有空请求仍可使用；策略内容始终来自配置的 repo。

| 情况 | HTTP | gRPC |
| --- | --- | --- |
| 预期版本已过期 | 409 / `REVISION_CONFLICT` | `Aborted` |
| 已有候选准备任务 | 409 / `RELOAD_BUSY` | `ResourceExhausted` |
| 加载、编译或 worker 失败 | 500 / `RELOAD_FAILED` | `Internal` |

HTTP/gRPC 的 signal 统一小写；score/actions/命中/原因沿用同一引擎结果。
gRPC 返回请求的 features，`trace.canonical_json` 保存完整共享 Trace；旧摘要字段继续提供。
FFI 返回原生 DecisionResponse（signal 为 tagged object），metadata 增加 `runtime_revision` / `compiled_sha256`；
新增 `corint_engine_reload(engine, expected_revision)`，返回相同版本冲突/忙/失败语义。
FFI 属于可信嵌入边界，宿主负责调用者授权；不是网络匿名接口。

兼容服务启动必须设置 `CORINT_DECISION_TOKEN`、`CORINT_PUBLISHER_TOKEN`、`CORINT_TENANT_ID`，
两个角色凭据必须不同。HTTP/gRPC 决策和管理操作都认证，health 仍允许本地探测。
服务限定 loopback，移除 permissive CORS；配置 Debug 和服务端错误不输出连接凭据。
客户端只能提交 event，user/features/api/service/llm/vars 和 event.tenant_id 不能覆盖可信数据；
tenant 由操作员配置注入 vars。HTTP async、gRPC pipeline_id/score_normalization 及任意 metadata 覆盖明确拒绝。
此收紧会使依赖原先未认证端点、大小写或注入字段的客户端需要迁移。

## 严格 Core 边界与验证

`CORINT_CORE_CONFIG` 仍启动独立的严格 Core HTTP 服务，不启动兼容 gRPC。
其 repo 发布声明、目标校验、操作员批准、独立行为验收与授权维持
[Core server 契约](core-server.md)，不会调用兼容候选加载器或降级加载。

[共享快照集成测试](../../crates/corint-decision-server/tests/shared_snapshots.rs) 直接调用生产
Axum router 与 tonic service，覆盖任一入口重载后两侧版本/结果一致、repo 回滚、重新构建、
无效 pipeline/registry/import、过期版本，以及两个决策请求等待真实本机 HTTP 连接器时仍可切换。
该测试不冒充 gRPC 网络部署测试；连接器测试需要允许绑定本机临时端口。
快照管理器单元测试覆盖候选提交竞争、busy 和准备失败释放名额。

## Revision History

| Date | Changes |
|---|---|
| 2026-09-05 | 记录兼容 HTTP/gRPC 共享快照、重载与版本协议及验证范围。 |
