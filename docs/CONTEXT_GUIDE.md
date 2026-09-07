# Context guide

The online runtime uses seven namespaces. `event` is input; `features`, `service`,
`vars` and `llm` hold computed or explicitly supplied runtime values; `sys` and
`env` provide runtime metadata. The [CDL context reference](cdl/context.md) describes
the boundary between these namespaces and the strict Core profile.

All service results, including internal services and third-party HTTP services,
use `service`. The old `api` namespace is removed from runtime contexts, SDK
requests and the REST request model. Passing `api`, even as null, is an unknown
field error at the public REST boundary.

A service step named `customer_check` stores its response at
`service.customer_check`. A later expression can read
`service.customer_check.score`. The service's provider name does not determine the
result path. Calling the same provider from two steps therefore keeps two results.
An explicit `output: vars.customer_risk` instead stores the response at that path.

`vars` is for intermediate values; it is not an external resource or connector.
Reading a service result does not trigger a call. A missing value is not evidence
that an external check passed. Invocation errors propagate according to the
[Service contract](cdl/service.md).

The SDK exposes `ContextInput::with_service` and `DecisionRequest::with_service`
for trusted runtime integrations. Public callers cannot populate computed
namespaces: the [request contract](API_REQUEST.md) admits caller event data and
rejects non-null computed namespaces. Strict Core additionally validates the
explicit input schema and does not admit service calls.
