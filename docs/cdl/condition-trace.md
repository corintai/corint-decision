# Core condition observations v1

Status: experimental, only `DecisionEngine::from_core` with `enable_trace: true`.
The additive `trace.core_conditions_v1` field follows the public
[JSON Schema](schema/condition-trace.json). Compatibility traces omit it; existing
trace JSON without this field still deserializes. This is not a full audit or
cross-product DecisionRecord contract.

## Observation contract

Records cover Registry guards actually invoked, Rule conditions, Ruleset
conclusions, Pipeline/step guards, Router routes and Pipeline decision conditions. Each boolean tree
contains comparison/boolean leaves, `all`/`any` groups and `not` nodes. Comparisons
are atomic: nested scalar operands and their raw values are not exposed.

- `evaluated` carries the actual boolean `result`, including `false`.
- `skipped` carries `reason`, never a `result`. `short_circuit` means the enclosing
  condition started but this child was bypassed. `not_reached` means the whole
  condition was bypassed within an invoked program (e.g. a later first-match row).
- A Rule/Ruleset that was never called, or a Registry guard after the first match,
  has no record. Absence is not a false result. Existing step/call traces identify
  unselected program paths. Default rows have no condition node.
- Failed requests still return the existing error, not a fabricated successful
  trace. Partial failure traces and unknown propagation are not implemented.

Each record contains a source label, resource type/ID, `field_path` pointing to
the source `when`, and a `node_path` in the **normalized** boolean tree. Empty
`node_path` is the root; `/children/N` addresses a zero-based child. Normalization
can introduce singleton groups, so these child paths are not YAML pointers or
line/column spans and need not match across different source spellings.

`invocation` is a request-local zero-based VM invocation number, including empty
condition-map programs. It distinguishes shared rules invoked by different
Rulesets. Records are grouped by invocation and compiler map order, not a global
timeline of condition evaluations. No duration or stable cross-request identity
is claimed. Identical source/request content produces the same condition records;
the rest of the existing trace still contains noncanonical ordering/timing.

## Execution and data boundaries

The compiler verifies boolean-node instruction ranges against the existing
expression compiler output. Core disables legacy instruction-removal optimization
until it can relocate jumps and observation maps together. No new interpreter or
Trace-only policy program is used. The observer reads booleans already on the VM
stack and never loads fields, runs comparisons or evaluates skipped operands.
Trace off does not collect records. Internal collection state is removed from the
returned decision context, so the complete `DecisionResult` stays identical across
Trace modes.

No raw input values or expression literals are copied into this new field.
Source labels, IDs, structure and boolean outcomes can still be sensitive; callers
must restrict access to trace-enabled requests. This does not add sampling,
enterprise authorization, redaction to legacy trace fields, or production storage.

For resolved imports, labels and clause pointers refer to the **normalized frozen
closure**, not original multi-document header line numbers. Policy/package hashes
and the resolver receipt provide the separate content/provenance binding. Trace
alone is unsigned and must not be accepted as approval or business-effect evidence.

## Executable example and evidence

<!-- cdl-example: condition_trace_short_circuit -->
The complete `condition_trace_short_circuit` case in the
[fixture manifest](../../tests/conformance/cdl_core/manifest.yaml) supplies the
input, dependency closure and expected outcomes. Its
[Rule](../../tests/conformance/cdl_core/trace_rule.yaml) short-circuits at amount
1000 and evaluates the nested `not` at 1001; the same cases assert scores, signals,
actions, calls, step paths and Trace-off/on equality through the real engine.

Additional [conformance tests](../../crates/corint-decision-engine/tests/cdl_core_conformance.rs)
inject an invalid boolean operand at VM level to prove skipped operands remain
unexecuted with Trace on; public requests still fail the input gate before execution.
Tests also cover all supported condition scopes, later first-match skips, repeated
rule invocations, schema validity and repeated-request determinism.

The [example registry](examples.json) governs this page,
[CDL Core](cdl-core.md) and [Pipeline](pipeline.md). CI rejects missing/duplicate markers, invalid case IDs,
missing fixture links and copied inline YAML on those pages. This bounded check
does not certify historical examples or LLM prompts elsewhere in `docs/cdl/`.
