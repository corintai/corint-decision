# Execution context

<!-- cdl-scope: compatibility-unverified -->
> Historical snippets on this page are unverified compatibility references. They are not Core support evidence.
> Core admission follows the [document and resource constraints](overall.md#document-and-resource-constraints) and the [language scope](overall.md#language-scope).
> Online runtime namespaces described here are separate from strict Core admission.
> The strict input and result rules below do not enable the compatibility namespaces.

## Strict Core input and results

For `cdl-core-risk-draft-1`, input fields use the [input schema format](schema/input.json).
Only finite binary64 numbers, strings, booleans and closed nested objects are admitted.
Fields explicitly declare whether they are required. Defaults, null, arrays, open objects
and undeclared input fields are not admitted. The caller supplies field meanings and units;
binary64 does not provide exact decimal arithmetic.

Optional objects may be absent, including their children; a present object must satisfy its
child schema. Schema nesting is limited to 16 levels and 1024 fields. A declared optional
path can be checked with `exists(event.path)`; reading it when missing returns `E_MISSING_INPUT`.
Missing required fields, explicit null, wrong types and nonfinite values return `E_INPUT_SCHEMA`.

Declared `event` paths are available in all condition scopes. `total_score` is the current
resource's local score and is allowed in Ruleset conclusions and Pipeline step guards,
Router routes and decisions. `results.<resource_id>` refers only to a direct call reached
on every incoming path; parent and child results are isolated. The singular
`result.<ruleset_id>` spelling is a recognized alias; use `results` in new policies.
Implicit last-result references are rejected.

Completed Rule calls expose `status`, `score`/`total_score` and `matched`; Ruleset and child
Pipeline calls expose `status`, `score`/`total_score` and `signal`. A reached but skipped
call exposes only `status: skipped`. Safe access and unavailable-result errors follow
the [Pipeline result rules](pipeline.md#32-direct-call-results). Unknown namespaces or results absent
on an incoming path fail compilation, including in a boolean operand that would be skipped.

## Compatibility namespaces

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

See [Service](service.md) for invocation parameters and failures. Host request formats
are separate from the language namespaces defined here.
