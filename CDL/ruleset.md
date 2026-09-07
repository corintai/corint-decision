# Ruleset — CDL Core Risk draft-1

This reference applies to experimental profile `cdl-core-risk-draft-1`, language
version `"0.1"`. See [CDL reference](overall.md), the [resource schema](schema/core.json)
and the [language scope](overall.md#language-scope).

A Ruleset evaluates an ordered collection of Rules, accumulates their local
score, and selects the first matching conclusion. It returns a local signal to
its caller. The [Pipeline decision](pipeline.md#3-results-decisions-and-actions)
selects the final result and action intents.

Writing `version: "0.1"` does not enable strict Core at a compatibility entry point.

## 1. Document and fields

Each source contains one YAML document with explicit string `version: "0.1"` and
one `ruleset` object. Unknown fields, duplicate keys and invalid types are rejected.

| Field | Required | Core contract |
|---|---|---|
| `id` | Yes | ASCII identifier matching `^[A-Za-z_][A-Za-z0-9_]*$`, unique across resources in the supplied bundle. `end` is reserved. |
| `name` | No | Human-readable string containing at least one non-whitespace character when supplied. |
| `description` | No | Descriptive string with no execution semantics. |
| `rules` | Yes | Nonempty ordered array of unique Rule IDs. Each ID must resolve to a supplied Rule resource. |
| `conclusion` | Yes | Nonempty ordered array of conclusion entries, with exactly one final default entry. |

`extends` and `metadata` are outside this Core shape and are rejected. The explicit
[import specification](import.md) describes source dependencies before
forming a complete Core bundle. This execution profile requires the complete resource
closure; it does not resolve raw imports or discover Rules by ID.

## 2. Rule evaluation and local scoring

A synchronous Ruleset invocation has two stages:

1. Evaluate every Rule in `rules` order and accumulate the contributions of
   matching Rules. A matching Rule does not stop subsequent Rule evaluation.
2. After all Rule evaluations succeed, evaluate `conclusion` entries in order
   and return the first matching entry's signal.

An execution error aborts the invocation. It is not a non-match and does not
select the default conclusion. A false guard on the calling Pipeline step skips
the entire Ruleset invocation; see [Pipeline guards](pipeline.md#24-guard-behavior).

The local score starts at zero for each invocation. Each matched Rule adds its
configured i32 score once; a miss contributes zero. Zero-score matches still count
as triggered Rule executions. Negative contributions and negative totals are
allowed; totals are not automatically clamped or normalized. Every addition is
checked for i32 overflow and returns `E_SCORE_OVERFLOW` on overflow.

A Rule shared by two different Rulesets executes and contributes separately in
each invocation. This is not global deduplication. Duplicate Rule IDs within one
Core Ruleset are rejected. A Pipeline adds each direct Rule/Ruleset/sub-Pipeline
call's returned score once, without counting child-internal contributions again.
Each resource has at most one call site per Pipeline in this profile.

## 3. Conclusions

### 3.1 Entry structure and first-match selection

| Field | Core contract |
|---|---|
| `when` | Nonempty expression string whose result is boolean. Mutually exclusive with `default`. |
| `default` | Literal YAML `true`, used only in the last entry. Mutually exclusive with `when`. |
| `signal` | Required in every entry; one of `approve`, `decline`, `review`, `hold`, `pass`. |

Exactly one default entry is required, at the end. A Ruleset whose conclusion
contains only that default is valid. A missing default, multiple defaults, an
early default, or an entry containing both `when` and `default` is invalid.
`reason` and `actions` are not accepted in Core Ruleset conclusions.

Conclusion conditions use strings, including constant expressions such as
`when: "true"`. YAML group objects (`all`/`any`/`not`), sequences, booleans and null
are rejected here, at both strict and compatibility parser entry points. Combine
conditions with `&&`, `||`, `!` and parentheses. See the
[Core expression contract](expression.md#operator-precedence) for types,
precedence, escaping, operator limits and left-to-right boolean short-circuiting.

The first true conclusion selects its signal and skips the remaining conclusion
entries. This does not skip Rule executions: those have already completed.
Place the highest-priority business outcome first when conditions overlap.
For thresholds in descending order, `total_score >= 60` before `total_score > 50`
selects the first entry at score 60, even though both conditions are true.

### 3.2 Available context

| Reference | Available in a Core Ruleset conclusion? |
|---|---|
| `total_score` | Yes. The current Ruleset invocation's local accumulated score. |
| `event.<declared_path>` | Yes. Caller input with the declared type. |
| `exists(event.<declared_path>)` | Yes. Presence check for one declared event path. |
| `triggered_count`, `triggered_rules` | No. Compatibility VM values are not admitted as Core expression references. |
| `results.<resource_id>.*` | No. Call-result access belongs to permitted Pipeline conditions. |
| `features.*`, `service.*`, `vars.*`, `sys.*`, `list.*` and other runtime namespaces | No. These are outside this Core condition scope. |

Declared optional fields can be guarded, for example with
`exists(event.user.tier) && event.user.tier == "basic" && total_score >= 60`.
An absent optional field read returns `E_MISSING_INPUT`; an undeclared path is
rejected at compilation, even in a skipped boolean operand. Explicit null,
missing required fields and wrong input types fail input validation. See
[Rule input semantics](rule.md#22-rule-input-and-missing-fields).

Triggered Rule IDs may appear in execution evidence without being available as
`triggered_rules` in a Core expression. Count-based conclusions and membership
checks over that array remain compatibility examples. Core `contains` accepts
strings, not arrays.

### 3.3 Local signals and Pipeline results

| Signal | Local assessment communicated to the caller |
|---|---|
| `approve` | Positive assessment. |
| `decline` | Adverse assessment. |
| `review` | Review recommended. |
| `hold` | Deferral or additional verification recommended. |
| `pass` | No substantive assessment from this policy. |

These are result labels. `decline` does not block a request by itself, `hold` does
not initiate verification, and `pass` does not skip subsequent Pipeline steps.
The caller follows its explicit control flow; the selected Pipeline decision
chooses the final result, optional reason and action intents. Ruleset signals do
not implicitly override that decision, and the engine does not execute action intents.

A completed call exposes `results.<ruleset_id>.status == "completed"`, its local
`score`/`total_score`, and string `signal`. Step guards, Router routes and Pipeline
decisions may read those results only after the call is reached on every incoming
path. A reached, guard-skipped call exposes only `status: skipped`; check status
before reading its score or signal, using boolean short-circuiting. Reading an
unavailable skipped result returns `E_RESULT_UNAVAILABLE`. See
[Pipeline result scoping](pipeline.md#32-direct-call-results).

## 4. Related documentation

- [Rule](rule.md), [Pipeline](pipeline.md) and [CDL reference](overall.md): current executable contracts.
