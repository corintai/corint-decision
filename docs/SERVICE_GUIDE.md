# Service integration guide

Define concrete services in `services`; reference their names and
operations from Pipeline `type: service` steps. Internal and external capabilities
use the same resource and invocation model. The [CDL Service contract](../CDL/service.md)
defines service configuration, invocation parameters, output paths and execution
semantics. This guide covers runtime registration and SDK integration.

The default repository layout stores one service per `services/<name>.yaml` file.
The definition's `name` is the reference key; filenames organize files and do not
override that name. The current filesystem loader accepts HTTP configurations.
Other protocols require a custom SDK adapter; no separate contract or binding file
is required for the HTTP definition.

1. Choose a logical service name and operation, such as `customer_risk.assess`.
2. For HTTP, create one `HttpServiceConfig` YAML file under `services`.
   Use `operations` to map business operation names to HTTP methods and paths.
3. Add a `type: service` step with `service`, `operation` and optional `params`.
4. Read the response from `service.<step_id>`, or specify an output under `vars`.
5. For another protocol, supply a `ServiceClient` through
   `DecisionEngineBuilder::with_service(name, adapter)` before building the engine.

The runtime rejects missing bindings and propagates failed invocations. HTTP
fallback must be explicit. There is no implicit retry or automatic switch between
internal and external providers. Do not assume a timeout cancels remote effects.

The repository sample `pipelines/service_pipeline.yaml` invokes the HTTP binding
`services/ipinfo.yaml`. This is an online example; the strict Core entry point remains closed to network I/O.

## Runtime registration

The filesystem engine loads HTTP service definitions from `services/*.yaml`
and `*.yml` under the repository root. Repository and runtime share the
`HttpServiceConfig` model. SDK callers can also supply this configuration through
`DecisionEngineBuilder::with_http_service`.
Malformed files, unknown fields and duplicate service names fail initialization.

For direct runtime use, deserialize `HttpServiceConfig`, call
`HttpServiceClient::register_service`, then install it with
`PipelineExecutor::with_http_service_client`. The YAML definition supplies the
runtime binding; no separate binding file is required.

For a custom adapter, use `DecisionEngineBuilder::with_service(name, adapter)` or
`PipelineExecutor::with_service(name, adapter)`. Duplicate or ambiguous names fail.
`ServiceRequest` carries `service`, `operation` and evaluated `params`.
`ServiceClient::call` returns `Result<ServiceResponse>`; success requires
`status: success`, and `data` becomes the step result. Adapters may connect to
internal or external systems using the same interface.

## Reload and deployment

Policy reload retains startup service bindings. Rebuild the engine to change
service configurations or adapters. Authentication values must already be resolved;
the runtime does not expand environment-variable placeholders in credentials.
Resolve secrets in deployment configuration before registering the service.

Internal/external ownership, credentials, network access and any future trust-domain
metadata are deployment concerns. A provider or protocol change that preserves the
logical service contract does not require a different Pipeline node type.

## Protocol extension boundaries

The former API node, API result namespace and API configuration directory have
been removed. Native gRPC/MQ test clients remain SDK examples, not declarative
repository configuration. Declarative gRPC, MQ and MCP configurations and a built-in
MCP client are not implemented. Existing test clients are not production adapters.
