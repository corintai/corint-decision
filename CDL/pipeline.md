# Pipeline — CDL Core Risk draft-1

This reference applies to `cdl-core-risk-draft-1`, language version `"0.1"`,
with the language and execution rules defined below.
The profile is experimental; the named capabilities below are supported
only within its tested scope. See [CDL reference](overall.md), the
[language scope](overall.md#language-scope) and [resource schema](schema/core.json).

Historical compatibility syntax does not extend this profile. Writing `version: "0.1"`
alone does not enable strict Core at a compatibility entry point.

## 1. Pipeline structure

A Pipeline runs an explicit, bounded DAG and chooses a final decision. A Rule
contributes its score once when its complete condition matches; Rulesets aggregate
their own rule scores and produce local signals. The selected Pipeline decision
emits the final result and action intents.

Each source contains one YAML document with explicit string `version: "0.1"` and
one top-level `pipeline` object. The fields below belong inside that object.
Unknown fields, duplicate keys and invalid types are rejected.

| Field | Core contract |
|---|---|
| `id` | Required ASCII identifier matching `^[A-Za-z_][A-Za-z0-9_]*$`; unique across resources in the supplied bundle. `end` is reserved. |
| `name` | Required string containing at least one non-whitespace character. |
| `description` | Optional descriptive string, with no execution semantics. |
| `when` | Optional Pipeline guard, evaluated before steps; false has the entry/child behavior specified in [Guard behavior](#24-guard-behavior). |
| `entry` | Required ID of an existing first step, which must have no predecessor. |
| `steps` | Required nonempty array of wrappers, each containing exactly one `step` object. Step IDs are unique within this Pipeline. |
| `decision` | Required ordered array; exactly one default, at the end |

The shape is `steps: [{step: {...}}]`; placing `{id, name, type, ...}` directly in
the array is invalid. Inside each wrapper, the step requires `id`, `name` and
`type`; optional `when` guards its execution. Step IDs follow the same identifier
format and reserved-word rule as resource IDs; step names must be nonblank strings.
Unsupported capabilities, missing references, unreachable steps and cycles fail
before execution.

## 2. Supported steps

| Step `type` | Required type-specific fields | Execution / local result |
|---|---|---|
| `rule` | `rule: <rule_id>`, explicit `next` | Synchronous Rule call; returns status, score and matched state. |
| `ruleset` | `ruleset: <ruleset_id>`, explicit `next` | Synchronous Ruleset call; returns status, local score and signal. |
| `pipeline` | `pipeline: <pipeline_id>`, explicit `next` | Synchronous child Pipeline call; returns status, local score and signal. |
| `router` | Nonempty ordered `routes`, explicit `default` target | Selects one successor; does not create a resource-call result. |

Call steps specify only the resource selector corresponding to their `type` and
cannot declare `routes` or `default`. Routers cannot declare a resource selector
or a step-level `next`. A `next` or router `default` target names a step in the
same Pipeline or `end`. Pipeline/step guards, router routes and decision conditions
use the shared [Core condition contract](expression.md#operator-precedence).

### 2.1 Ruleset step

A Ruleset step names its `ruleset` and an explicit `next`, including `next: end`.
It executes all ordered rules and the Ruleset conclusion before returning.
Later routing can read the completed result, subject to the result-scope rules below.

### 2.2 Router step

A router has a nonempty ordered `routes` list of `{when, next}` and an explicit
`default` target. The first matching route wins. It must not also declare `next`.
Conditions use the shared pure expression/boolean-group model.

Node order in YAML never determines successors. `next: end` terminates step
execution and proceeds to `decision` once; omitting `next` is invalid on any
resource-call step. Use a router to select or bypass work explicitly.

### 2.3 Call sites and graph limits

Within one Pipeline, each directly called Rule, Ruleset or child Pipeline may
have at most one call site. This restriction applies across all its steps, including mutually
exclusive branches; it is not merely a limit of one execution per selected path.
Different caller Pipelines may each call the same resource.

The call graph allows at most 16 Pipeline levels, including the entry Pipeline,
and a conservative expanded budget of 4096 resource/step/rule nodes. Recursive
calls are forbidden. All supplied Pipelines are checked, including unregistered
ones; guards and routing do not remove nodes from these compile-time checks.
See [direct-call results](#32-direct-call-results) for call scoping.

### 2.4 Guard behavior

| Guard whose condition is false | Result and continuation |
|---|---|
| Registry-selected entry Pipeline's `when` | Returns `E_PIPELINE_SKIPPED`. Runs neither steps nor decision, and does not try another Registry entry. |
| Child Pipeline's own `when` | Returns `status: skipped` to its caller without running child steps or decision. The parent follows the call step's `next`. |
| Rule/Ruleset/Pipeline call step's `when` | Skips invoking the resource, records `status: skipped`, and follows the step's `next`. |
| Router step's `when` | Skips evaluation of all routes and follows the router's `default`. |

Input validation and compilation still precede guard execution. A skipped call
contributes no score and exposes no score, signal or matched value; it is distinct
from a completed Rule that did not match. Guard conditions short-circuit using
the same evaluator as other Core conditions.

## 3. Results, decisions and actions

### 3.1 Local accumulated score

`total_score` is the current Pipeline invocation's local accumulated score. It
starts at zero and adds each completed direct Rule/Ruleset/child-Pipeline call's
returned score once. Child-internal scores are not added a second time. A child
Pipeline starts its own score at zero rather than inheriting the parent's total.
Skipped calls add nothing. Negative totals are allowed; checked i32 accumulation
returns `E_SCORE_OVERFLOW` on overflow.

| Condition scope | Declared `event` fields / `exists` | `total_score` | Direct-call `results.<resource_id>.*` |
|---|---|---|---|
| Pipeline's own `when`, whether entry or child | Allowed | Not allowed | Not allowed |
| Step `when`, router route, Pipeline decision | Allowed | Current local total | Only calls reached on every incoming path |

For example, the first step may use `when: total_score == 0` before any call has
run. The same expression in the Pipeline's own `when` fails with `E_INVALID_REF`.

### 3.2 Direct-call results

`results.<resource_id>` uses the called resource's ID, not the call step's ID.
After the call has been reached on every incoming path, its fields are:

| Call state | Available fields |
|---|---|
| Completed Rule | `status: completed`, `score` / `total_score`, `matched` |
| Completed Ruleset or child Pipeline | `status: completed`, `score` / `total_score`, `signal` |
| Skipped call | `status: skipped` only |

Here `.score` and `.total_score` are aliases for that call's local score, whereas
bare `total_score` is the caller Pipeline's accumulated score. A completed Rule
miss returns zero and `matched: false`; a zero-score match has `matched: true`.

Reading a skipped call's score, signal or matched value returns
`E_RESULT_UNAVAILABLE`. Guard such reads with short-circuiting, for example
`results.risk.status == "completed" && results.risk.score > 50`, provided the
`risk` call was reached on every incoming path. A status check cannot make a
result from an unexecuted branch available; such references fail compilation.
Parent/child result scopes are isolated. Child-internal results and implicit
last-result references are rejected. Enabling execution observation does not evaluate
skipped conditions or create results for unexecuted calls.

### 3.3 Decision rows and action intents

Each decision row has exactly one of `when` or `default: true`, a required
`result`, and optional `actions` and `reason`. Allowed results are
`approve / decline / review / hold / pass`; `reason` is a string. There must be
exactly one default row, at the end. The first matching condition selects a row;
the default is selected only if no preceding condition matches. Only the selected
row supplies the final result, actions and reason.

`actions` is an array of unique strings, each containing at least one
non-whitespace character. An empty array is valid. Duplicate entries such as
`["BLOCK", "BLOCK"]`, empty strings and whitespace-only strings are rejected
with `E_INVALID_STRUCTURE`. Actions are opaque intent strings; the engine does
not execute external side effects.

Ruleset and child-Pipeline signals do not implicitly override the parent decision,
and child action intents do not automatically become final actions. The response
score is the parent Pipeline's accumulated score.

## 4. Invalid structures and unsupported capabilities

Unsupported step types and malformed step structures fail during validation.

A malformed Pipeline `when: []` is rejected as `E_INVALID_STRUCTURE`; valid conditions are supported.

A malformed step `when: []` is rejected as `E_INVALID_STRUCTURE`; valid guards skip execution when false.

Changing only the type while retaining `ruleset` fields is rejected as `E_INVALID_STRUCTURE`; a correctly declared `pipeline` target is supported.

API steps, including `params`, `on_error` and fallback, are outside Core and
return `E_UNSUPPORTED_CAPABILITY`. The online extension uses the unified
[Service node](service.md) and supports evaluated parameters.

API `any` is outside Core and returns `E_UNSUPPORTED_CAPABILITY`. The online
extension does not support this combination either.

API `all` and `min_success` are outside Core and return `E_UNSUPPORTED_CAPABILITY`.
The online extension does not support this combination either.

Service steps are outside Core and return `E_UNSUPPORTED_CAPABILITY`. The online
Service node requires `operation`; the historical `endpoint` spelling is rejected.

Metadata, rule parameters, inheritance, dynamic Feature/List
access and structured actions remain outside this Core profile. Their presence in
an AST or in compatibility documentation is not execution evidence.

## 5. Related language definitions

[Import](import.md) defines source composition. [Rule](rule.md), [Ruleset](ruleset.md),
[Registry](registry.md), [Context](context.md) and [Expression](expression.md) define
resource behavior, selection, visible values and conditions.

## Revision History

| Date | Changes |
|---|---|
| 2026-09-05 | 以可执行 fixture 重整严格 Core Pipeline 参考及不支持能力门禁。 |
| 2026-09-07 | 补齐源码包装、四类 step、guard 行为、调用限制、分数作用域和 actions 约束。 |
