# 文档导航

本文面向策略作者、引擎开发者和部署维护者。按下列入口查阅当前契约、使用步骤和验收范围。

## CDL 与工具链

- [演进建议与完成进度](DSL_EVOLUTION_RECOMMENDATIONS.md)
- [严格 Core 规范](cdl/cdl-core.md)
- [Pipeline 严格参考](cdl/pipeline.md)
- [Pipeline 兼容参考及已知缺口](cdl/pipeline-compatibility.md)
- [CLI 使用](cdl/cli.md)
- [源码包](cdl/packages.md)与[源码交换](cdl/exchange.md)

## 公共契约与服务

- [公共契约概览](contracts/README.md)
- [资源、评估审批与反馈契约](contracts/phase0.md)
- [严格 Core repository 服务](contracts/core-server.md)
- [兼容 HTTP/gRPC 共享快照](contracts/compatibility-server-snapshots.md)与[HTTP 请求](API_REQUEST.md)
- [Feature 运行时能力、错误与缓存](contracts/feature-runtime.md)

## 阅读步骤与输入输出

先确认使用严格 Core 还是兼容入口，再阅读对应 schema、示例与测试说明。
策略源码、输入 schema、目标和证据契约的格式以对应参考为准；兼容性报告与行为结果不自动构成业务评估或发布批准。

所有页面的阅读范围由 [文档清单](inventory.json) 记录。新增页面需更新清单；CI 检查整个 docs/ 的本地链接、示例测试绑定，并执行已绑定的 REST 与 CDL 契约测试。

## 常见问题

**文档中的所有能力是否都已支持？** 以 [能力清单](cdl/schema/capabilities.json) 和指定入口为准。
兼容参考标记为未验收的示例不构成支持声明；离线契约测试不等于真实产品集成。

## 修订历史

| 日期 | 变更 |
|---|---|
| 2026-09-05 | 增加当前 CDL、公共契约与服务快照的阅读入口。 |
