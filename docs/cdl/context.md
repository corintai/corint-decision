# Execution context

<!-- cdl-scope: compatibility-unverified -->
> This page is an unverified compatibility reference. Its snippets are not Core support evidence.
> Core admission is defined by [CDL Core](cdl-core.md) and the [capability inventory](schema/capabilities.json).
> Online runtime namespaces described here are separate from strict Core admission.
> Consult [Core](cdl-core.md) for its caller-input and result-access constraints.

| Namespace | Meaning |
| --- | --- |
| `event` | Caller event data. |
| `features` | Feature values. |
| `service` | Results of any service invocation, regardless of deployment location. |
| `vars` | Intermediate values and explicit variable outputs. |
| `llm` | Runtime-supplied analysis values; no implied Pipeline LLM execution. |
| `sys` | System metadata. |
| `env` | Runtime environment values. |

Service results default to `service.<step_id>`, not the provider name. For example,
`service.customer_check.score` reads the result of the `customer_check` step.
Multiple calls to one provider use distinct step IDs. Explicit outputs may target
`service.<path>` or `vars.<path>` and overwrite an existing value at that path.
The former `api` namespace and `with_api` request/context builders have been removed.

A condition reads already available values. Reading a service result does not
implicitly invoke the service. `vars` remains a value-binding namespace and does
not define a separate class of connector.

See [Service](service.md) for invocation parameters and failures, and the
[request contract](../API_REQUEST.md) for which inputs the public server accepts.
