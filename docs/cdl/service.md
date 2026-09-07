# Service

A Service is a reusable named capability with one or more operations. Its concrete
definition lives in `services/<name>.yaml` under the repository root.
A Pipeline invokes an operation by referencing the service name; it does not
contain the service definition. Internal services and third-party APIs use the
same Service concept. Deployment location does not determine the resource type.

| Location | Responsibility |
| --- | --- |
| `services/<name>.yaml` | Define the service name, connection settings, operations, and request/response mappings. |
| Pipeline `type: service` step | Reference a service operation, supply invocation parameters, choose an output path, and control execution order. |
| Runtime adapter | Execute the invocation using the configured protocol. |

The configuration's `name` is the reference key; the filename is an organizational
convention. The current file-based implementation supports HTTP services. Other
protocols require an SDK adapter; there is no generic declarative connector schema
yet. A separate service-contract file or binding file is not required.

This contract covers service configuration and invocation in the online runtime.
The strict [Core profile](cdl-core.md) remains closed to I/O and rejects service
invocation steps. Feature and List remain separate semantic resources; a transport
does not determine whether a resource is a Service, Feature or List.

## Service definition

Each YAML file defines one concrete service. For example,
`services/customer_risk.yaml` defines the `customer_risk` service and its
`assess` operation:

<!-- executable-example: service-http -->
```yaml
name: customer_risk
base_url: https://risk.example.com
timeout_ms: 3000
operations:
  assess:
    method: GET
    path: /customers/{customer_id}
    response:
      mapping:
        score: risk_score
```

This file contains both the logical operation names and their concrete HTTP
implementation settings. In runtime terminology it also supplies the HTTP
binding; that is a role of this definition, not another required configuration
file. See the repository's [IPInfo definition](../../repository/services/ipinfo.yaml)
for another example.

### Service fields (current HTTP configuration)

| Field | Contract |
| --- | --- |
| `name` | Required non-empty logical service name, referenced by Pipeline steps. |
| `base_url` | Required HTTP or HTTPS base URL with a host. |
| `auth` | Optional authentication; currently supports `type: header` with explicit `name` and `value`. |
| `timeout_ms` | Positive default timeout in milliseconds; defaults to 10000. |
| `operations` | Required non-empty map of named operations. |

### Operation fields

| Field | Contract |
| --- | --- |
| `method` | Required HTTP method: GET, POST, PUT, PATCH or DELETE. |
| `path` | Required request path; `{name}` placeholders use resolved parameters. |
| `timeout_ms` | Optional positive timeout overriding the service default. |
| `params` | Optional parameter defaults from context paths or literals. Invocation parameters override these defaults. |
| `query_params` | Optional list of parameter names to include in the query string. |
| `request_body` | Optional JSON request template using `${name}` parameter placeholders. |
| `response` | Optional `mapping` of output fields to JSON response fields and an explicit `fallback` value. |

For reusable definitions, supply event-specific values in the Pipeline step and
use their parameter names in the service's request mapping. The example above
expects the caller to supply `customer_id`. Context-dependent defaults in an
operation's `params` are supported, but couple that definition to a particular
context shape. No input/output type schema is enforced by these mappings.

## Service invocation in a Pipeline

The following Pipeline references the separately defined `customer_risk.assess`
operation. Its `params` describe this invocation, not the service implementation.

<!-- executable-example: service-parameters -->
```yaml
pipeline:
  id: checkout
  name: Checkout
  entry: lookup
  steps:
    - step:
        id: lookup
        name: Assess customer
        type: service
        service: customer_risk
        operation: assess
        params:
          customer_id: event.customer_id
          amount: '${event.amount + 1}'
          channel: web
        timeout_ms: 1000
        next: end
```

| Field | Contract |
| --- | --- |
| `id`, `name`, `type` | Required common step fields; `type` is `service`. |
| `service` | Required non-empty logical service name, resolved through a runtime binding. |
| `operation` | Required non-empty capability name within the service. |
| `params` | Optional map of named values/expressions, evaluated before invocation. |
| `timeout_ms` | Optional positive integer, in milliseconds. |
| `output` | Optional result path in `service` or `vars`; default `service.<step_id>`. |
| `next` | Next step ID or `end`; omission ends this execution path. |

Parameters accept JSON-compatible literals, including arrays, objects and null.
A complete `${expression}` string evaluates an expression. Bare paths beginning
with `event.`, `service.`, `vars.`, `features.`, `sys.` or `env.` also evaluate as
expressions. Other strings are literal, including URLs and dotted business values.
Expressions inside literal arrays/objects are not recursively interpolated.
Parameter names must be non-empty strings. Evaluation order is lexical by name.

A successful invocation stores its returned value at `service.<step_id>` unless
`output` is specified. Two steps invoking the same service have separate defaults;
explicitly choosing the same output path overwrites its previous value. Subsequent
steps and conditions can read `service.lookup.<field>`.

The node does not accept `api`, `endpoint`, `method`, `topic`, `query`, `timeout`,
`any`, `all`, `min_success` or `on_error`. Transport details belong in the service
definition or its SDK adapter.
Step-level `when` is currently rejected by the online compiler; use a router for
conditional invocation. This limitation is separate from Core guard support.

## Execution semantics

The runtime resolves one logical name to either a registered HTTP binding or a
custom `ServiceClient`. Duplicate or ambiguous bindings are errors. No automatic
internal/external classification, fallback to a different provider, retry, cache
or discovery is implied.

The adapter receives the service name, operation name and evaluated parameters.
Its successful response data becomes the step result. Missing bindings, connector
errors, non-success adapter statuses and timeouts fail execution, subject to the
explicit HTTP fallback described below. Errors are not converted into a normal
string or implicit null.

Timeout precedence is step, operation, service default. Custom adapters use the
step deadline or 10000 ms. Timeout stops awaiting the operation; it cannot undo
side effects already performed remotely. The engine does not retry automatically.

Step parameters override HTTP operation parameter defaults before unused defaults are resolved.
Missing declared context paths fail; they are not sent as literal path strings. `{name}` in paths,
named query parameters and `${name}` in request bodies use the resolved values.
Response mapping selects fields from JSON. An explicitly configured HTTP
`response.fallback` applies to unsuccessful HTTP status or invalid JSON; transport
failures and timeouts remain errors. No fallback exists unless configured.

Internal/external ownership, credentials, network access and any future trust-domain
metadata are deployment concerns. Changing a provider or protocol while preserving
the service contract should not require changing the policy node.

## Loading and extension boundaries

The filesystem engine loads service definitions from `services/*.yaml`
and `*.yml` under the repository root using the current HTTP configuration schema.
Malformed files, unknown fields and duplicate names fail engine initialization.
Policy reload retains startup service bindings; rebuild the engine to change them.
Authentication values do not undergo implicit environment-variable substitution;
resolve secrets in deployment configuration before registering the service.

SDK adapters can implement other protocols while retaining the same Pipeline
reference shape. gRPC, MQ and MCP declarative configurations and a built-in MCP
client are not implemented. Existing test clients are not production adapters.
Registration APIs and the adapter interface are documented in the
[Service integration guide](../SERVICE_GUIDE.md).

The former `api` node, `api` results namespace, `configs/apis` directory and
`endpoints` configuration field are not supported aliases.
