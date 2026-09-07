# CDL Registry

## Strict Core Registry

This reference defines Registry selection for execution profile **`cdl-core-risk-draft-1`**,
language version **`"0.1"`**. A Registry is the ordered entry-point selection policy for a
complete resource collection. The [common resource constraints](overall.md#document-and-resource-constraints) and
[resource Schema](schema/core.json) apply. Selecting a Pipeline does not load its source file.

### 1. Document and fields

Each source contains one YAML document with explicit string `version: "0.1"` and one resource.
The complete collection must contain exactly one Registry and every referenced resource.

| Field | Type | Requirement |
|---|---|---|
| `version` | String | Required; exactly `"0.1"`. |
| `registry` | Array | Required; at least one entry. Array order defines matching priority. |
| `registry[].pipeline` | String | Required; ID of a Pipeline in the supplied collection. |
| `registry[].when` | Expression string or condition group | Required; must evaluate to a Boolean. |

A Registry has no separate `id`, `name`, `priority` or `default` field. Unknown fields, duplicate
YAML keys, missing fields and invalid types are rejected. Pipeline references use resource IDs,
not filenames; their spelling, case and reserved-name rules follow the common resource constraints.

Multiple entries may reference the **same Pipeline ID** with different conditions. This does not
create duplicate Pipeline resources or cause repeated execution: the first matching entry wins.
A second Registry source or two resource definitions with the same ID are invalid.

### 2. Conditions and scope

Conditions use the [Rule condition structure](rule.md#2-conditions-and-input-scope) and
[expression semantics](expression.md). Use a Boolean expression such as `event.amount > 1000`;
constant conditions must be strings, for example `when: "true"` or `when: "false"`, not YAML
Boolean values. An object contains exactly one of `all`, `any` or `not`: `all` and `any` require
nonempty arrays, while `not` requires exactly one child condition. Groups can be nested.

Registry conditions may read declared `event.<path>` inputs and use `exists(event.<path>)`.
Bare fields such as `amount`, `total_score`, resource results and runtime namespaces are outside
this scope. The request's input schema is validated before Registry matching, including required
fields that a particular route would not read. See [input and result scope](context.md#strict-core-input-and-results).

`all`, `any` and Boolean expression operators use their normal short-circuit rules. A condition
that is not evaluated does not produce an execution error. Its syntax, references and types must
still pass compilation, as must the conditions and targets of later Registry entries.

### 3. Selection and errors

1. Evaluate Registry entries from top to bottom.
2. A false condition advances to the next entry. An evaluation error stops the request.
3. The first true condition selects exactly one Pipeline. Later entries are not evaluated.
4. Evaluate the selected Pipeline's optional `when` guard. If absent, proceed into its steps.
   If false, return `E_PIPELINE_SKIPPED`; do not execute its steps or decision, and do not resume
   Registry matching.
5. Execute that Pipeline's steps and final `decision` according to the [Pipeline contract](pipeline.md).

Order is the only priority rule: the engine does not sort entries by specificity. Put a more
specific route before any broader condition that would also match it. An explicit final entry
with `when: "true"` supplies a fallback. It is optional and is an ordinary Registry entry;
no fallback Pipeline or business decision is injected.

| Situation | Result |
|---|---|
| Referenced Pipeline is missing or the ID names another resource kind | `E_UNRESOLVED_REF` before execution, even for an unreachable entry. |
| Condition has invalid syntax, references or types | Compilation fails; no entry is silently dropped. |
| Request violates the input schema | `E_INPUT_SCHEMA` before selection. |
| No entry matches | `E_NO_PIPELINE_MATCH`; no business decision is produced. |
| Selected Pipeline's guard is false | `E_PIPELINE_SKIPPED`; a later fallback is not tried. |
| A condition or selected Pipeline fails during evaluation | The error propagates; it is not converted to false, fallback or approval. |

The same rules apply with execution observation enabled or disabled.
