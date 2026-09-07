# Offline behavior testing with `corint test`

Status: experimental, `cdl-core-risk-draft-1`. This command runs **declared
examples through the real DecisionEngine**, not a second expression interpreter.
It does not evaluate business effectiveness, execute action intents or publish policies.

## Run the checked-in examples

From the repository root (omit `--offline` when build dependencies are not cached):

```sh
cargo build -p corint-decision-cli --locked --offline
./target/debug/corint test --format json \
  --input-schema tests/conformance/cdl_core/input-schema.yaml \
  --cases tests/conformance/cdl_core/behavior.yaml \
  tests/conformance/cdl_core/rule.yaml \
  tests/conformance/cdl_core/ruleset.yaml \
  tests/conformance/cdl_core/pipeline.yaml \
  tests/conformance/cdl_core/registry.yaml
```

The [example suite](../../tests/conformance/cdl_core/behavior.yaml) has three
threshold cases and two expected input failures. It is synthetic, not a real-data
evaluation. The [validation CLI](cli.md) documents the same explicit file loading,
input schema, path handling, complete resource closure and strict Core rules.
`--cases` is required exactly once for `test` and is rejected by `validate`.

The suite and bundle must both validate before any case runs. The command uses
`DecisionEngine::from_core`, with no legacy loader, repository discovery or Work
dependency. The Core executor does not initialize HTTP/feature/service/list clients.
Unknown capabilities (including connectors) still fail at compilation. A direct
Service instruction given to the offline executor fails when no binding is explicitly installed.
Compatibility engine constructors retain their existing behavior.

## Versioned suite format

The [test-suite JSON Schema](schema/test-suite.json) is embedded in the shared toolchain and
checks YAML or JSON test files. This format is **not CDL**, a BusinessContext,
a PolicyPackage or a signed evaluation report.

- Required top-level fields: `version: "1"`, `profile: cdl-core-risk-draft-1`,
  and `cases` (1–1000 entries). Unsupported versions/profiles, duplicate YAML keys,
  multiple YAML documents and unknown fields fail before execution.
- Each case needs a unique `id`, `input: {event: {...}}`, and **exactly one** of
  `expect` or `expect_error`. No skip/ignore flags, automatic expectations or
  permissive fallback are supported.
- Input schema enforcement happens at the real engine entry. Thus missing fields,
  wrong scalar types, nulls and undeclared event fields can be tested as expected
  errors. Extra request namespaces such as `features`, `vars` or `service` are not
  accepted by the test format.

`expect` must assert all of the following; `explanation` is optional:

| Field | Assertion |
|---|---|
| `pipeline_id` | Exact selected Pipeline ID. |
| `score`, `signal` | Raw i32 score and final string signal. |
| `actions` | Exact ordered action-intent strings; none are executed. |
| `triggered_rules` | Exact ordered rule IDs, including repeated invocations if any. |
| `steps` | Exact ordered executed step IDs; skipped steps must not appear. |
| `calls` | Exact ordered `{ruleset_id, rule_id, triggered, score}` rule-invocation records; `ruleset_id` is null for a direct Rule call. `score` is that invocation's contribution, including zero for a miss. |
| `local_results` | Exact map of direct call resource IDs: completed Rulesets/sub-Pipelines use `{score, signal}`, completed Rules use `{score, matched}`, and guarded skipped calls use `{status: "skipped"}`. Parent results exclude child-internal calls; missing/extra entries fail. |
| `explanation` | If supplied, exact final explanation string. |

Array order and object key sets matter; no subset matching for arrays or maps.
Integral numeric spellings such as `60` and `60.0` compare equally. Signals use
`approve / decline / review / hold / pass`. The report's string signals are an
assertion projection, not a change to the engine's existing transport signal object.

`expect_error` accepts only these stage/code pairs in this increment:

- `input / E_INPUT_SCHEMA`
- `execute / E_NO_PIPELINE_MATCH`
- `execute / E_SCORE_OVERFLOW`
- `execute / E_MISSING_INPUT`
- `execute / E_RESULT_UNAVAILABLE`
- `execute / E_DIVISION_BY_ZERO`
- `execute / E_NUMBER_OVERFLOW`
- `execute / E_PIPELINE_SKIPPED`

The error must actually occur. Success where an error is expected fails, as does
an unexpected error or a different error code/stage. Compile/load errors abort the
suite; they cannot be swallowed by a case's runtime-error expectation. Existing
Core diagnostics, including structured execution errors, are preserved with their
stage/code/source/field_path. The adapter also recognizes the legacy typed
score-overflow marker. Unclassified engine failures become `E_ENGINE` and cannot
be configured as a passing expectation. The [runtime extension suite](../../tests/conformance/core_extensions/behavior.yaml)
exercises direct and nested calls, optional inputs and guards through this format.

## Execution and report semantics

Each case calls the engine **twice**, first with Trace off, then on, using the same
input. It checks the deterministic result/evidence projection, Trace presence,
Trace rule count and step execution flags. Random request IDs and timings are not
compared or included in the projection. Error parity compares stage/code.
This does not claim full operand-level tracing or production audit replay.

All valid cases run, even after an assertion fails. Results use the same JSON
envelope as `validate`, with `scope: "behavior"`, `cases_file`, and `test_results`:

- `total`, `executed`, `passed`, `failed` count cases, **not** engine invocations.
  `executed` means both engine-entry calls were attempted; expected input failures
  may execute no policy instructions.
- Each case reports `id`, `passed`, `trace_parity`, `expected`, `actual`,
  `trace_actual`, and `diagnostics`. Actuals contain either `result` or `error`.
- A mismatch has `E_TEST_MISMATCH`, stage `test`, and a suite field pointer such
  as `/cases/0/expect/score`. `E_TRACE_PARITY` means Trace on/off outcomes differ.
  Missing/malformed engine evidence is `E_TEST_EVIDENCE`, never an implicit pass.
- Expected engine errors remain in case diagnostics as evidence, even for a
  passing case. Use case `passed` and aggregate `failed`, not diagnostic severity
  alone, to determine test success. Top-level diagnostics describe preflight
  failures; assertion diagnostics live inside `test_results.cases`.
- `execution_checked` is true only after cases have been attempted. Preflight
  failure leaves it false and omits `test_results`; no skipped cases are counted
  as passed. `valid` is true only when the command completed and all cases passed.
- `business_evaluation` always remains `"not_performed"`.

JSON output is one document on stdout; text output gives counts and failed-case
diagnostics. Exit `0` means all declared cases passed, `1` means invalid
suite/schema/CDL or failed behavior checks, and `2` means usage/I/O failure.
Help/version still return text with exit 0 without validating or testing a policy.

Reports omit raw input events but may contain sensitive policy explanations,
actions, expectations and diagnostic text; handle report files accordingly.
Internal context keys are adapted into this experimental test projection, not
published as a stable engine audit schema. Standalone test reports have no artifact
binding. The separate [source-package commands](packages.md) bind fresh reports to
source/test/tool fingerprints, but still provide no signature, data lineage,
approval or deployment binding and are not publication authorization.

## Agent workflow and verification

After each policy edit: run `corint validate`, then `corint test` with independent
expected results derived from the user's requirements. Inspect failed assertions
before changing either policy or expectations; never rewrite expectations merely
to make the test green. A passing suite establishes only those examples, not
general correctness, test coverage sufficiency or real-business risk performance.

The [process tests](../../crates/corint-decision-cli/tests/behavior.rs) exercise the
checked-in suite and adapt every positive case in the existing real-engine fixture
manifest, including result-dependent routing and skipped rule calls. They also
deliberately break every result assertion, exercise unexpected/expected errors,
reject invalid suites before execution, and verify clean output and unchanged
source files from temporary working directories. Run them with:

```sh
cargo test -p corint-decision-cli --locked --offline
```

An initial [source-package build/verify workflow](packages.md) is now available;
imports/locking, generator and Work integration, real-data evaluation and production
publication enforcement remain separate work. Standalone local
validation and behavior testing cover only part of W01, not all W01–W10 contracts.
