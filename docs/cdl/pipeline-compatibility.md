# Online Pipeline runtime

<!-- cdl-scope: compatibility-unverified -->
> This page is an unverified compatibility reference. Its snippets are not Core support evidence.
> Core admission is defined by [CDL Core](cdl-core.md) and the [capability inventory](schema/capabilities.json).
> This page describes the online runtime separately from the strict Core profile.
> Examples from other historical pages are not evidence of supported execution.

For pure, validated policies use the [Core Pipeline contract](pipeline.md).
For online service invocation use the [Service contract](service.md).

The online compiler admits router, ruleset and service steps. It rejects unknown
node types and unsupported fields before reachability filtering, including unused
steps. A service has one logical name and operation, evaluated parameters, an
optional deadline and an output path. There is no separate API node.

Router conditions select the first matching route, then the default. Online
ruleset execution remains deferred, so conditions depending on ruleset results
must use the strict Core execution path. Service results are available immediately
to following nodes. Pipeline guards are supported; online step guards are not.

`rule`, subpipeline, function, trigger and extract execution are not implemented by
this online compiler. Strict Core has its own supported rule/subpipeline calls.
Do not infer support from legacy AST variants or historical DSL examples.

Transport binding belongs in the runtime. HTTP bindings use `services`;
custom adapters bind through the SDK. The node does not encode whether a service
is internal or external. MCP may be added as an adapter without adding a DSL node.
