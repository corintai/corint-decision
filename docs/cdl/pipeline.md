# Pipeline — CDL Core Risk draft-1

This reference applies to `cdl-core-risk-draft-1`, language version `"0.1"`,
through `compile_core` / `DecisionEngine::from_core` and their strict CLI/server
adapters. The profile is experimental; the named capabilities below are supported
only within its tested scope. See [CDL Core](cdl-core.md), the
[capability inventory](schema/capabilities.json) and [resource schema](schema/core.json).

The [compatibility reference](pipeline-compatibility.md) retains historical syntax,
including unverified and incomplete extensions. Writing `version: "0.1"` alone
does not enable strict Core at a compatibility entry point.

## 1. Pipeline structure

A Pipeline runs an explicit, bounded DAG and chooses a final decision. Rules score
individual conditions; Rulesets aggregate their own rule scores and produce local
signals. Only the selected Pipeline decision emits the final result and action intents.

| Field | Core contract |
|---|---|
| `id`, `name` | Required, nonempty; ID is globally unique |
| `description` | Optional descriptive text |
| `when` | Optional Pipeline guard, evaluated before steps; false skips this invocation |
| `entry` | Required ID of the first step |
| `steps` | Required nonempty array of `step` objects with unique IDs |
| `decision` | Required ordered array; exactly one default, at the end |

Every step requires `id`, `name` and `type`; optional `when` guards its execution. Core accepts `ruleset`, `router`, `rule` and `pipeline` types. Unknown fields, unsupported capabilities, missing references,
unreachable steps and cycles fail before execution. `end` is reserved.

## 2. Supported steps and executable examples

### 2.1 Ruleset step

A Ruleset step names its `ruleset` and an explicit `next`, including `next: end`.
It executes all ordered rules and the Ruleset conclusion before returning.
Later routing can read the completed result. This increment permits at most one
call site for each Ruleset in a Pipeline.

<!-- cdl-example: pipeline_ruleset -->
Supported complete example: `payment_boundaries` in the
[conformance manifest](../../tests/conformance/cdl_core/manifest.yaml) supplies the
[Pipeline](../../tests/conformance/cdl_core/pipeline.yaml), Rule, Ruleset, Registry,
input schema and boundary expectations. At amounts 1001 / 1000 / 999 it checks
scores 60 / 0 / 0, results, action isolation and actual rule calls. Evidence:
`pipeline.first_match_and_action_intents` and `ruleset.local_score_and_first_match`.

### 2.2 Router step

A router has a nonempty ordered `routes` list of `{when, next}` and an explicit
`default` target. The first matching route wins. It must not also declare `next`.
Conditions use the shared pure expression/boolean-group model.

<!-- cdl-example: pipeline_router -->
Supported complete example: `result_dependent_router` in the
[conformance manifest](../../tests/conformance/cdl_core/manifest.yaml) uses the
[router Pipeline](../../tests/conformance/cdl_core/router.yaml). It reads the
completed risk Ruleset score, invokes an extra Ruleset only on the selected branch,
and checks that the other path makes zero extra calls. Evidence:
`pipeline.synchronous_ruleset_router`.

Node order in YAML never determines successors. `next: end` terminates step
execution and proceeds to `decision` once; omitting `next` is invalid on a Ruleset
step. Use a router to select or bypass work explicitly.

Single Rule calls, sub-Pipeline calls, Pipeline/step guards and their executable examples are specified in [Runtime extensions](runtime-extensions.md). Call steps require an explicit `next`; guarded routers use `default` when skipped.

## 3. Results, decisions and actions

Conditions may read declared `event` fields. Step guards, Router routes and
decision conditions may also read `total_score` and direct-call
`results.<resource_id>.score`, `.total_score` and `.signal`, only after the call
has been reached on every incoming path. Rule results use `matched` instead of
`signal`. Reached calls expose `status`; guarded skipped calls have no score,
signal or matched value, so check status before a short-circuited value read.
Pipeline entry guards cannot read call results. Child-internal results, implicit
last-result references and reads from unexecuted branches are rejected.

Decision rows use `when` or `default: true`, with required `result` and optional
`actions` (string array) and `reason`. Allowed results are
`approve / decline / review / hold / pass`. The first matching row wins; there must
be exactly one final default row. Only that row's actions and reason are selected.
Actions are intent strings; the engine does not execute external side effects.

Ruleset signals do not implicitly override the final decision. The response score
sums direct Rule/Ruleset/sub-Pipeline call scores once with checked i32 accumulation. The
`payment_boundaries` fixture above verifies first-match behavior even when two
conditional decision rows match. See [condition Trace](condition-trace.md) for
observed conditions and skipped branches.

## 4. Invalid structures and unsupported capabilities

The following examples are **negative cases**, not supported policies. Each is a
mutation of the complete Core fixture and must fail at `validate` with the code declared in the
[conformance manifest](../../tests/conformance/cdl_core/manifest.yaml).

<!-- cdl-example: pipeline_guard_rejected -->
`N07_pipeline_guard`: malformed Pipeline `when: []` is rejected as `E_INVALID_STRUCTURE`; valid conditions are supported.

<!-- cdl-example: step_guard_rejected -->
`N07_step_guard`: malformed step `when: []` is rejected as `E_INVALID_STRUCTURE`; valid guards skip execution when false.

<!-- cdl-example: subpipeline_rejected -->
`N07_subpipeline`: changing only the type while retaining `ruleset` fields is rejected as `E_INVALID_STRUCTURE`; a correctly declared `pipeline` target is supported.

<!-- cdl-example: api_params_rejected -->
`N08_api_params`: API steps, including `params`, `on_error` and fallback, are outside
Core. The compatibility compiler also rejects unsupported parameters and error policies.

<!-- cdl-example: api_any_rejected -->
`N08_api_any`: API `any` is outside Core. The compatibility compiler also rejects this combination.

<!-- cdl-example: api_all_rejected -->
`N08_api_all`: API `all` and `min_success` are outside Core. The compatibility compiler also rejects this combination.

<!-- cdl-example: service_endpoint_rejected -->
`N08_service_endpoint`: Service steps are outside Core. The historical `endpoint`
spelling is also absent from the compatibility step field whitelist.

Metadata, rule parameters, inheritance, dynamic Feature/List
access and structured actions remain outside this Core profile. Their presence in
an AST or in compatibility documentation is not execution evidence.

## 5. Authoring and delivery

Use [strict import resolution](resolution.md) to freeze a local dependency closure,
then [validate](cli.md), [run independent behavior cases](testing.md),
[build/verify](packages.md) and [check the declared target](../contracts/README.md).
The [strict Core server](../contracts/core-server.md) performs its own authorization,
compilation and independent acceptance checks before loading a policy.

This page's complete examples and rejections are registered in [examples.json](examples.json).
The conformance runner checks their mappings and rejects copied inline YAML;
[compatibility references](pipeline-compatibility.md) are classified separately and
cannot serve as support evidence for this profile.

## Revision History

| Date | Changes |
|---|---|
| 2026-09-05 | 以可执行 fixture 重整严格 Core Pipeline 参考及不支持能力门禁。 |
