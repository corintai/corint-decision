# CDL Core Risk — executable profile

Status: **experimental profile `cdl-core-risk-draft-1`**, language version `"0.1"`.
This is a bounded implementation of [phase 0](../DSL_EVOLUTION_RECOMMENDATIONS.md#阶段-0公共契约cdl-core-与一致性门禁), not completion of phase 0 or a production certification.

The contract applies to `DecisionEngine::from_core` and the shared `compile_core`
gate used by the offline [`corint validate` CLI](cli.md).
[`corint test`](testing.md) executes examples through `DecisionEngine::from_core`.
[`corint build` / `corint verify`](packages.md) use the same gates for experimental
source snapshots and content-bound example evidence, without publication authority.
The opt-in [Core repository server](../contracts/core-server.md) uses the same gates
for startup/reload and derives its runtime snapshot solely from the published repo.
Existing compatibility builders, server loading, validators and LLM generators
remain compatibility entry points. Merely
writing `version: "0.1"` does not opt a legacy entry point into this contract.

The [Runtime extensions](runtime-extensions.md) add synchronous Rule/sub-Pipeline calls, guards, optional closed objects, `exists`, checked arithmetic and structured execution errors.

## 1. Public artifacts and entry points

- [Resource schema](schema/core.json): Draft 7 JSON Schema for decoded YAML objects;
  embedded directly by the strict validator, not a second handwritten shape definition.
- [Capability inventory](schema/capabilities.json): precise support scope and test evidence.
- [Source packages](packages.md) and [package schema](schema/source-package.json):
  explicit snapshots, byte fingerprints and fresh local evidence; unsigned and not deployable IR.
- [Offline CLI](cli.md): validates an explicit file closure through the same compiler;
  structured JSON diagnostics do not imply behavior testing or business evaluation.
- [Behavior testing CLI](testing.md): runs declared examples through the real engine
  with Trace off/on, using the [test-suite schema](schema/test-suite.json).
- [Input file schema](schema/input.json): strict serialization of the existing model
  `Schema`, parsed by `parse_core_input_schema`, with a runnable
  [input fixture](../../tests/conformance/cdl_core/input-schema.yaml).
- [Fixture manifest](../../tests/conformance/cdl_core/manifest.yaml): complete synthetic
  input/expectation sets and negative source mutations. This manifest is a test format, not CDL.
- [Compiler gate](../../crates/corint-decision-compiler/src/core.rs):
  `validate_core_document` checks a single source's structure/version/capabilities;
  `compile_core` additionally checks input types, complete references and control flow,
  normalizes the existing AST and invokes the existing compiler, including Registry guards.
- [Engine entry](../../crates/corint-decision-engine/src/decision_engine/engine.rs):
  `DecisionEngine::from_core(&sources, input_schema)` constructs an immutable,
  closed-world engine. `decide` validates inputs before executing any policy.

Callers supply `CoreSource { path, yaml }` for **every** resource and an existing
`corint_decision_model::types::Schema`. Source paths identify diagnostics; this entry
does not read them from disk, resolve imports, discover a local repository or contact Work.
One YAML document is allowed per source. A bundle requires exactly one Registry.

Run the real-engine conformance suite from the repository root:

```sh
cargo nextest run -p corint-decision-engine --test cdl_core_conformance --locked
```

Without nextest, use `cargo test -p corint-decision-engine --test cdl_core_conformance --locked`.
Use `--offline` when dependencies are cached. Neither test execution nor policy
execution requires a Work account, business database or live LLM response.

## 2. Resource and structure requirements

Each document contains an explicit string `version: "0.1"` and exactly one of
`rule`, `ruleset`, `pipeline`, `registry`. Unknown fields, duplicate YAML keys,
unknown versions, null required values and wrong types fail; there is no permissive fallback.
Resource IDs are globally unique ASCII identifiers; step IDs are unique within a
Pipeline. `end` is reserved as the terminal target. Whitespace-only names are invalid.

| Resource | Current draft-1 contract | Evidence |
|---|---|---|
| Rule | Required `id`, `name`, `when`, `score`; optional description. Score is an i32 integer. Add it once iff the rule matches. | C01, N01, N03, N09 |
| Ruleset | Required `id`, nonempty unique ordered `rules`, `conclusion`; optional name/description. Only `when` or `default`, and `signal`, are accepted in a conclusion row. | C03, N04, N05 |
| Pipeline | Required `id`, `name`, `entry`, nonempty `steps`, `decision`; optional description and `when`. Keep existing `- step: {id, name, type, ...}` wrappers. | C04, C06, C07, N03, N05 |
| Ruleset step | Required `ruleset`, explicit `next` including `next: end`. At most one call site per ruleset per Pipeline in this increment. | C06, C07, N05 |
| Rule / Pipeline step | Required `rule` / `pipeline` target and explicit `next`; synchronous local result. One call site per resource per Pipeline. | Runtime extension tests |
| Pipeline / step guard | Optional `when`; false skips execution. Skipped call steps follow `next`, skipped routers follow `default`. A skipped entry Pipeline returns `E_PIPELINE_SKIPPED`. | Runtime extension tests |
| Router step | Nonempty ordered `routes` of `{when, next}` and explicit `default` target; no simultaneous `next`. First matching route wins. | C06, C07 |
| Registry | Ordered `{pipeline, when}` entries. First match wins. Explicit `when: "true"` can provide a fallback. No match returns `E_NO_PIPELINE_MATCH`, not approval. | C05 |

Both `conclusion` and `decision` require exactly one terminal default row.
The first matching row wins; only its result and actions are used. A Pipeline
decision row uses `result`, optional string-array `actions` and optional `reason`.
Signals/results use `approve / decline / review / hold / pass`. Actions are opaque
**intent strings**, never executed here and never an authorization grant.

Ruleset conclusion `actions`/`reason` are deliberately rejected in this increment:
their existing compiler behavior has not yet been brought under this contract.
Metadata, rule parameters and inheritance are also outside this initial shape.
Rejected fields are not silently dropped.
Integral score spellings such as `60.0` normalize to `60` after the i32 range
check, matching JSON Schema's mathematical integer definition.

## 3. Expressions, types and dataflow

Accepted forms: string expressions, nonempty `all`/`any`, and `not` with exactly
one item (the existing one-element sequence spelling). Nested groups are allowed.
Expressions support scalar literals, declared fields, comparisons, boolean
`&& / || / !`, unary numeric negation and parentheses. No coercion, arbitrary
functions, templates or implicit feature access. Numeric `+ - * / %` and `exists(event.path)` are supported.
`Ruleset.conclusion.when` remains string-only in this increment; use `&& / || / !`
there. Group objects are accepted in the other condition positions defined by the schema.

Boolean spellings normalize to the existing short-circuit expression AST,
including Registry, Rule, Router, Ruleset conclusion and Pipeline decision.
Operands evaluate left-to-right within the parsed tree; `&&` / `||` short-circuit.
Precedence, highest first: parentheses; unary `!` / `-`; `* / %`; `+ -`;
`< <= > >=`; `== !=`; `&&`; `||`. Binary operators of equal precedence associate
left-to-right. Thus `true || false && false` is true. Negative literals,
`-event.amount`, and scientific notation such as `1e-3` are accepted.
Strings accept single or double quotes, Unicode text, and escapes `\\`, `\"`,
`\'`, `\/`, `\n`, `\r`, `\t`, `\b`, `\f`, `\uXXXX` (including surrogate pairs).
Quote the entire expression at the YAML level when its syntax requires it; YAML
escaping and expression escaping are separate layers. Operators inside strings are
literal text. Malformed quotes, escapes or tokens return `E_INVALID_EXPRESSION`.
Expression parsing is bounded to 128 recursive levels, counting grouping, unary
operators and precedence descent. Trace does not evaluate skipped operands.
The VM fault-injection test for short-circuiting intentionally bypasses input
validation; this does not make malformed inputs valid at the public engine entry.

Input schema uses the existing model `Schema`/`SchemaField` types. This increment
accepts non-null `number / string / boolean` fields and closed nested objects, with explicit required/optional fields and no defaults. Inputs may omit optional fields; undeclared fields and nonfinite numbers are
invalid. Numbers use the existing IEEE-754 binary64 representation, not decimal
money arithmetic. Business field units must be supplied by the caller; a full
versioned BusinessContext contract is described in [public contracts](../contracts/README.md).

Allowed references:

- `event.<declared_path>` and `exists(event.<declared_path>)` in all condition scopes;
- `total_score` in Ruleset conclusion, step guard, Router and Pipeline decision, referring to the current resource's accumulated local score;
- `results.<resource_id>.score / total_score / signal` in step guards, Router routes
  and Pipeline decisions, only for direct calls reached on **every incoming path**.
  Rules expose `matched` instead of `signal`; all reached calls expose `status`.
  Pipeline entry guards, Registry guards and Rule/Ruleset conditions cannot read
  caller results. Parent and child Pipeline result scopes are isolated.
  The existing parser also recognizes the singular `result.<ruleset_id>` alias;
  new examples use `results`. Implicit last-result references are rejected.

Guarded calls additionally expose `status`; skipped calls have no score or signal. Single Rules expose `matched` instead of signal. See [Runtime extensions](runtime-extensions.md) for result scoping and safe reads.

Unknown fields/namespaces and reads of a result absent on some incoming paths fail at
compile time. All nodes must be reachable; cycles, missing targets, duplicate IDs
and unresolved dependencies fail. Node ordering in YAML never determines execution.

## 4. Execution and results

`CallRuleset` executes its ordered rules and conclusion **before returning** to
the Pipeline. Local scores start at zero for each Ruleset; completed result objects
remain visible to subsequent routing. The response score sums each direct Rule/Ruleset/sub-Pipeline call exactly once. A rule shared by two different Rulesets contributes once
per invocation, not once globally. Every accumulation checks i32 overflow and
returns `E_SCORE_OVERFLOW` rather than wrapping or panicking.

`next: end` ends node execution and proceeds to the Pipeline decision. That
decision executes once, through the same VM as the other conditions. Local
Ruleset signals do not implicitly override the final decision.

The public result remains the existing `DecisionResponse`/`DecisionResult`:
signal, raw score, triggered rules, actions and explanation. Its serialized signal
retains the existing `{ "type": "decline" }` shape; this increment does not change
the transport contract. Named result objects in context expose string signals.

Current trace evidence covers executed/skipped steps, routes and rule invocations,
plus opt-in [boolean condition observations v1](condition-trace.md).
Context keys `__executed_steps__`, `__ruleset_result__.<id>` and
`__core_rule_executions__` support this first runner; they are **internal evidence**,
not the final cross-product report or feedback schema. Raw operand-level Trace,
precise per-rule timings and deterministic audit serialization remain pending.
Request IDs, durations and trace collection order are not part of semantic equality.

## 5. Diagnostics and compatibility

Strict errors reuse `Diagnostic`, adding `source`, `field_path` (JSON pointer) and
`stage`. YAML syntax/duplicate-key errors additionally include line/column when
available; other diagnostics do not pretend to have source spans.

Stages: `parse`, `validate`, `resolve`, `type`, `input`, `execute`, `compile`.
Codes include `E_INVALID_STRUCTURE`, `E_UNKNOWN_FIELD`, `E_MISSING_FIELD`,
`E_INVALID_VERSION`, `E_UNSUPPORTED_VERSION`, `E_UNSUPPORTED_CAPABILITY`,
`E_DUPLICATE_ID`, `E_UNRESOLVED_REF`, `E_INVALID_GRAPH`, `E_INVALID_REF`,
`E_INVALID_EXPRESSION`, `E_TYPE`, `E_INPUT_SCHEMA`, `E_NO_PIPELINE_MATCH`, `E_COMPILE`,
`E_CALL_CYCLE` and `E_CALL_LIMIT`. Core execution errors are structured
`EngineError::Core` diagnostics with stage `execute`: `E_MISSING_INPUT`,
`E_RESULT_UNAVAILABLE`, `E_DIVISION_BY_ZERO`, `E_NUMBER_OVERFLOW`,
`E_SCORE_OVERFLOW` and `E_PIPELINE_SKIPPED`. No execution failure becomes approval.

Existing compatibility entry points do not become strict automatically. No legacy
strategy is silently migrated. Runtime score overflow is now a controlled error
for both entry families. Registry nested condition parsing now reuses Rule's
existing group parser. Both entry families share corrected expression precedence,
Unicode-safe string parsing and unary negation. Compatibility Ruleset conclusions
reject non-string `when` rather than dropping conditions; missing List backends and
invalid aggregation windows now fail explicitly. Other Core capabilities remain
opt-in. The strict Core executor does not initialize connector clients; compatibility
constructors continue to initialize their configured clients.

## 6. Evidence and remaining phase 0 work

The [runner](../../crates/corint-decision-engine/tests/cdl_core_conformance.rs) runs
full resource bundles through the shared schema, real parsers, compiler and engine.
It checks thresholds, local/aggregate scores, first-match choices, action isolation,
call counts, actual paths, input failures and trace parity. Negative fixtures must
fail at their declared stage and code. CI checks capability/fixture references.

Full examples in this document are links to runnable fixtures, not manually
copied YAML. The [Pipeline reference](pipeline.md) uses the same fixture gate. Historical
reference pages are classified as `compatibility-unverified`; their examples are
not certification against this profile, and CI rejects unverified support badges.

<!-- cdl-example: payment_boundaries -->
Supported complete example: `payment_boundaries` in the
[manifest](../../tests/conformance/cdl_core/manifest.yaml) binds the complete
Rule/Ruleset/Pipeline/Registry closure and boundary inputs/expected behavior.

<!-- cdl-example: result_dependent_router -->
Supported complete example: `result_dependent_router` in the
[manifest](../../tests/conformance/cdl_core/manifest.yaml) adds a branch, local
results and assertions that unselected calls do not execute.

<!-- cdl-example: unknown_condition -->
Negative example: `N01_unknown_condition` in the
[manifest](../../tests/conformance/cdl_core/manifest.yaml) must fail validation with
`E_UNKNOWN_FIELD`; it is not a supported policy. The [example registry](examples.json)
binds these declarations to the conformance runner. Its three strict pages prohibit
inline YAML copies. Thirteen compatibility pages have a separate scope/claim gate;
their individual snippets and compatibility prompts still await executable mappings.
Standalone YAML under `examples/` is also inventoried: historical source examples
must parse as YAML, carry an unverified scope marker and fail strict Core validation
with the registered diagnostic. This syntax/rejection gate is not execution evidence.

The separate [import authoring profile](resolution.md) now resolves bounded local
file imports (C08) into this profile's frozen closure. This execution profile and
strict generator response contract still reject unresolved imports; no runtime
filesystem access is enabled. Portable import provenance/locking remains pending.

Still pending: complete C02 raw operand traces, full source spans, all legacy example/prompt mappings,
live-provider/Work generator integration, production publication enforcement, complete cross-product
Feature/Model/full PolicyPackage/report/feedback contracts, and W01–W10 product-level
interoperability. The local validate/test/source-build workflow covers part of W01;
it does not certify production package distribution or complete Work interoperability.

No genuine customer data was used. Passing these tests proves the stated behavior,
not business effectiveness, production readiness or approval to deploy.
