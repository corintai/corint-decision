# Service

A Service is a reusable named capability with one or more operations. Its concrete
definition is separate from the invoking Pipeline.
A Pipeline invokes an operation by referencing the service name; it does not
contain the service definition. Internal services and third-party APIs use the
same Service concept. Deployment location does not determine the resource type.

| Language element | Responsibility |
| --- | --- |
| Service definition | Define the service name, HTTP fields, operations, and request/response mappings. |
| Pipeline `type: service` step | Reference a service operation, supply invocation parameters, choose an output path, and control execution order. |

The definition's `name` is the reference key. This reference defines the HTTP
configuration shape; no generic declarative connector schema is defined here.
File layout, protocol adapters and registration are host responsibilities.

This contract covers service configuration and invocation in the online runtime.
The strict [Core profile](overall.md) remains closed to I/O and rejects service
invocation steps. Feature and List remain separate semantic resources; a transport
does not determine whether a resource is a Service, Feature or List.

## Service definition

Each Service document defines one concrete service, including its logical operation
names and their concrete HTTP implementation settings. In runtime terminology it also supplies the HTTP
binding; that is a role of this definition, not another required configuration
file.

### Service fields (current HTTP configuration)

| Field | Contract |
| --- | --- |
| `name` | Required non-empty logical service name, referenced by Pipeline steps. |
| `base_url` | Required HTTP or HTTPS URL with a host. An optional path prefix and existing query parameters are preserved; fragments are rejected. |
| `auth` | Optional authentication; currently supports `type: header` with explicit `name` and `value`. |
| `timeout_ms` | Positive default timeout in milliseconds; defaults to 10000. |
| `operations` | Required non-empty map of named operations. |

### Operation fields

| Field | Contract |
| --- | --- |
| `method` | Required HTTP method: GET, POST, PUT, PATCH or DELETE. |
| `path` | Required non-empty request path appended to the base URL's path prefix; `{name}` placeholders use resolved parameters. |
| `timeout_ms` | Optional positive timeout overriding the service default. |
| `params` | Optional defaults from explicit context paths or scalar/null literals. Invocation parameters override these defaults; see Operation defaults below. |
| `query_params` | Optional list of parameter names to include in the query string. |
| `request_body` | Optional string containing a JSON template with whole-value `${name}` placeholders. Allowed only for POST, PUT and PATCH; GET and DELETE reject it. |
| `response` | Optional `mapping` of output fields to JSON response fields and an explicit `fallback` value. |

For reusable definitions, supply event-specific values in the Pipeline step and
use their parameter names in the service's request mapping. Context-dependent defaults
in an operation's `params` are supported, but couple that definition to a particular
context shape. No input/output type schema is enforced by these mappings.

### Operation defaults

Operation `params` and Pipeline step `params` use different evaluation rules:

| Value | HTTP operation default | Pipeline invocation parameter |
|---|---|---|
| String, number, boolean or null literal | Supported | Supported |
| Direct array or object literal | Rejected when the default is resolved | Supported |
| String beginning with `event.`, `service.`, `vars.`, `features.`, `sys.` or `env.` | Reads that exact dot-separated context path | Parsed as an expression |
| Complete `${expression}` string | Kept as literal text | Evaluates the expression |

Step parameters override defaults before the remaining defaults are resolved.
An overridden default is not evaluated. A missing operation-default context path
or traversal through a non-object fails the call; an explicitly present null is
a valid value. Context references may return objects or arrays, subject to the
request mapping's value restrictions. Invocation expressions retain the online
expression evaluator's behavior; these default-path checks do not change it.

## Service invocation in a Pipeline

A Pipeline step references a separately defined service and operation. Its `params`
describe the invocation.

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

## HTTP request mapping

The operation path is appended to the base URL's path prefix with one slash at
their boundary. For example, `https://host/v1/` and `/customers/{id}` produce a path
under `/v1/customers/`. The operation path cannot contain an absolute URL, a raw
query or fragment, backslashes, control characters, or `.`/`..` path segments
(including percent-encoded dots).

Path parameters are percent-encoded as data before insertion: `id: "a/b"` produces
`/customers/a%2Fb`, not another path segment. Missing, empty-name or malformed
placeholders fail the call. Substituted values cannot introduce a dot segment.
Substitution examines the original template once; text inside a parameter value
is never interpreted as another placeholder.

`query_params` selects resolved parameters in list order. Names with no resolved
value are omitted. Query names and values are URL-encoded and appended to any
existing base URL query. Path and query values accept strings, numbers, booleans
and null; null becomes an empty string, while arrays and objects are rejected.

`request_body` is YAML string content, for example
`request_body: '{"id": ${customer_id}, "amount": "${amount}"}'`.
Both `${name}` and a quoted whole value `"${name}"` insert the parameter's JSON
value, preserving numbers, booleans, arrays, objects and null. Strings are escaped
as JSON strings. Quotes around a whole placeholder do not force the result to be
a string. Partial string interpolation such as `"customer-${id}"` and placeholders
in object keys are rejected; prepare a complete parameter value in the Pipeline.
Missing parameters, malformed placeholders and invalid resulting JSON fail before
the request is sent. Inserted parameter data is never interpolated recursively.

## HTTP response mapping

The connector reads the full response body before parsing JSON. Without a non-empty
`response.mapping`, the parsed JSON value is returned unchanged. With a mapping:

- An object response produces a new object containing only the declared output
  keys. Mapping values select response fields, including dot-separated nested
  object paths such as `risk.score`; array indexes and JSONPath are unsupported.
- A missing source field or traversal through a non-object produces null for that
  output field. This does not trigger fallback.
- Output keys are literal keys: `result.score` does not create a nested object.
- A non-object JSON response (including an array or null) is returned unchanged.

These mappings do not enforce an output type schema.

An explicit `response.fallback` is returned directly for a final non-2xx HTTP
status or invalid JSON after a complete response-body read. It is not passed through
`mapping`, so it must already have the output shape the caller expects. Omitting
fallback, or setting `fallback: null` in YAML, disables it. Connection failures,
truncated response bodies and timeouts remain execution errors even when fallback
is configured. Request construction and parameter errors never use fallback.

## Execution semantics

The runtime resolves one logical name to one bound implementation.
Duplicate or ambiguous bindings are errors. No automatic
internal/external classification, fallback to a different provider, retry, cache
or discovery is implied.

The adapter receives the service name, operation name and evaluated parameters.
Its successful response data becomes the step result. Missing bindings, connector
errors, non-success adapter statuses and timeouts fail execution, subject to the
explicit HTTP fallback described above. Errors are not converted into a normal
string or implicit null.

Timeout precedence is step, operation, service default. Custom adapters use the
step deadline or 10000 ms. Timeout stops awaiting the operation; it cannot undo
side effects already performed remotely. The engine does not retry automatically.

HTTP parameters, request construction and fallback follow the mapping rules above.

The former `api` node, `api` results namespace and `endpoints` configuration field
are not supported aliases.
