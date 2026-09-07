# Expression and template syntax

Service invocation uses the [unified Service contract](cdl/service.md). The syntax
of a policy expression is distinct from the template syntax used by an HTTP
binding. Neither implies that arbitrary YAML fields are evaluated.

## Conditions

Conditions contain CDL expressions, such as `event.amount > 100`. Quoting the YAML
string does not turn it into an HTTP template. See [expressions](cdl/expression.md)
and the [strict Core grammar](cdl/cdl-core.md) for their supported operators.

## Service step parameters

```yaml
params:
  customer_id: event.customer_id
  adjusted_amount: '${event.amount + 1}'
  source: checkout
  endpoint_label: example.com
  flags: [new_customer, web]
```

A complete `${expression}` value is evaluated by the expression compiler. A bare
path beginning with a declared runtime namespace (`event`, `service`, `vars`,
`features`, `sys`, `env`) followed by a dot is also an expression. Other strings,
including dotted literal values and URLs, remain literal. Arrays and objects are
literal values; nested strings are not recursively expanded.

A malformed `params` map or non-string/empty parameter name is rejected. Reading
a missing required context field fails execution instead of treating the path as
a successful service response.

## HTTP operation defaults

```yaml
name: customer_directory
base_url: https://directory.example.com
timeout_ms: 1000
operations:
  get_customer:
    method: GET
    path: /customers/{customer_id}
    params:
      customer_id: event.customer_id
```

Operation defaults resolve bare namespace paths against runtime context. Step
parameters override these defaults before default expressions are resolved.
Other values are literal; arithmetic belongs in the step's `params` expression.
`{customer_id}` in the HTTP path substitutes a resolved parameter.

## HTTP request bodies

```yaml
name: risk
base_url: https://risk.example.com
operations:
  assess:
    method: POST
    path: /assess
    request_body: '{"customer_id": "${customer_id}", "amount": ${amount}}'
```

`${name}` in an HTTP body references a resolved request parameter, not a CDL
expression. Full-value placeholders are serialized as JSON with their value type
preserved. The resulting body must be valid JSON. Supply the parameters from the
service step or operation defaults.

## Deployment configuration

`base_url`, authentication header values and timeouts are configuration values.
The HTTP service loader does not expand `@{config.path}`, `${ENV_VAR}` or
`{{env.NAME}}` in those fields. Resolve deployment values before registering a
binding. This keeps policy expressions separate from deployment and credentials.

Feature lookup-key templates and other resource-specific syntax are defined by
their own [Feature](cdl/feature.md) and [List](cdl/list.md) references. A template
example in a design page is not evidence that strict Core executes that resource.
