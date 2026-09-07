# 文档导航

本文面向策略作者、引擎开发者和部署维护者。按下列入口查阅当前契约、使用步骤和验收范围。

语言定义与语义统一位于根目录 [CDL/](../CDL/overall.md)；本目录收录工具、引擎和集成的使用说明。

## CDL 定义与描述

- [CDL 语言概览、规范导航与完整示例](../CDL/overall.md)
- [通用文档与资源约束](../CDL/overall.md#document-and-resource-constraints)
- [Rule 严格参考与可执行示例](../CDL/rule.md)
- [Ruleset 严格参考与可执行示例](../CDL/ruleset.md)
- [Pipeline 严格参考](../CDL/pipeline.md)
- [Registry 入口路由与完整示例](../CDL/registry.md)

## 工具链与使用指南

- [CLI 使用](cli.md)
- [行为测试](testing.md)与[导入解析](resolution.md)
- [源码包构建、验证与交换](packages.md)
- [生成 API 与 Provider 配置](generation.md)与[决策回放](replay.md)
- [执行入口、Trace 与运行验证](runtime-validation.md)及[性能测量](core-development.md#performance-measurement)
- [Pipeline 编译与入口差异](runtime-validation.md#pipeline-编译与入口差异)
- [Feature 用例、命名与数据源配置](feature-configuration.md)
- [List 加载、后端与运维参考](list-configuration.md)
- [Service 加载、重载与 SDK 集成](SERVICE_GUIDE.md)

## 实现与验收

- [架构与未实现规划](ARCHITECTURE.md)
- [Core 编译器 API、文档校验与发布验收](core-development.md)
- [结果与 Trace 格式](runtime-validation.md#结果与-trace-格式)

## 公共契约与服务

- [公共契约概览](contracts/README.md)
- [资源、评估审批与反馈契约](contracts/phase0.md)
- [严格 Core repository 服务](contracts/core-server.md)
- [兼容 HTTP/gRPC 共享快照](contracts/compatibility-server-snapshots.md)与[HTTP 请求](API_REQUEST.md)
- [Feature 运行时能力、错误与缓存](contracts/feature-runtime.md)

## 阅读步骤与输入输出

先确认使用严格 Core 还是兼容入口，再阅读对应 schema、示例与测试说明。
策略源码、输入 schema、目标和证据契约的格式以对应参考为准；兼容性报告与行为结果不自动构成业务评估或发布批准。

所有页面的阅读范围由 [文档清单](inventory.json) 记录，路径以仓库根目录为基准。新增页面需更新清单；CI 检查整个 `CDL/` 和 `docs/` 的本地链接、示例测试绑定，并执行已绑定的 REST 与 CDL 契约测试。

## 常见问题

**文档中的所有能力是否都已支持？** 以 [能力清单](contracts/schema/capabilities.json) 和指定入口为准。
兼容参考标记为未验收的示例不构成支持声明；离线契约测试不等于真实产品集成。

## 修订历史

| 日期 | 变更 |
|---|---|
| 2026-09-05 | 增加当前 CDL、公共契约与服务快照的阅读入口。 |

在线 Service 调用：[语言契约](../CDL/service.md) · [集成指南](SERVICE_GUIDE.md)。
