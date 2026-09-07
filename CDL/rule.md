# Rule — CDL Core Risk draft-1

This reference applies to experimental profile `cdl-core-risk-draft-1`, language
version `"0.1"`. See [CDL reference](overall.md), the [resource schema](schema/core.json)
and the [language scope](overall.md#language-scope).

A Rule evaluates deterministic conditions and contributes a fixed integer score
when it matches. It does not invoke an LLM or a service, select a final decision,
or emit actions. Rulesets aggregate rule scores; the selected [Pipeline decision](pipeline.md)
chooses the final result and action intents.

Writing `version: "0.1"` does not enable strict Core at a compatibility entry point.

## 1. Document and fields

Each source contains one YAML document with explicit string `version: "0.1"` and
one `rule` object. Unknown fields, duplicate keys and invalid types are rejected.

| Field | Required | Core contract |
|---|---|---|
| `id` | Yes | ASCII identifier matching `^[A-Za-z_][A-Za-z0-9_]*$`; unique across resources in the supplied bundle. `end` is reserved. |
| `name` | Yes | Human-readable string containing at least one non-whitespace character. |
| `description` | No | Descriptive string, with no execution semantics. For example, “Detect payments above the review threshold.” |
| `when` | Yes | Boolean expression string or a logical group, as defined below. |
| `score` | Yes | Integer value from `-2147483648` to `2147483647` (i32), including zero and negative values. |

`metadata` and `params` are outside this Core shape and are rejected. `score: 80.5`
is invalid; integral spellings such as `60.0` normalize to `60` after the range
check. Prefer integer literals in source files.

## 2. Conditions and input scope

### 2.1 Expression strings and logical groups

A simple condition is `when: event.amount > 1000`. Constant conditions must also
be strings: use `when: "true"`, not the YAML boolean `when: true`.

| Form | Meaning | Structure |
|---|---|---|
| Expression string | Must evaluate to a boolean | Supports comparisons, arithmetic, `&&`, `\|\|`, `!` and parentheses. |
| `all` | Every child must be true | Nonempty sequence of conditions. |
| `any` | At least one child must be true | Nonempty sequence of conditions. |
| `not` | Negates its child | Sequence containing exactly one condition. |

Each group object has exactly one key: `all`, `any` or `not`. To negate several
conditions, put an explicit `all` or `any` group inside the single `not` item.
Empty groups and sibling operators in one group object are invalid. Unknown group
keys return `E_UNKNOWN_FIELD`.

Conditions evaluate left-to-right and short-circuit, including nested groups.
Skipped operands are not evaluated, even with Trace enabled. Put presence guards
before optional field reads; within those dependencies, cheaper conditions can
come first in `all` and more likely matches can come first in `any`.

Nesting is subject to compiler/parser limits. String expressions are bounded to
4096 tokens and 128 actual AST levels; parser recursion is separately bounded to
128 levels and includes precedence descent. These limits do not promise 128
parentheses or an unlimited number of nested YAML groups. See the
[expression contract](expression.md#operator-precedence) for precedence,
escaping and exact limits. Quote the entire expression when YAML syntax requires
it; YAML escaping and expression escaping are separate layers.

### 2.2 Rule input and missing fields

Rule conditions may read only declared `event.<path>` fields, or check their
presence with `exists(event.<path>)`. A field's type and required/optional status
come from the caller-supplied [input schema](schema/input.json). Field names in
that schema omit the `event.` prefix; conditions include it.

| Reference | Allowed in a Core Rule condition? |
|---|---|
| `event.amount`, declared nested paths | Yes, with the declared scalar type. Closed objects may contain declared child fields. |
| `exists(event.amount)` | Yes, for one declared event path. |
| `total_score`, `results.<id>.*` | No. Aggregate/result reads belong to the permitted Ruleset/Pipeline scopes, not Rule conditions. |
| `features.*`, `service.*`, `vars.*`, `sys.*`, `env.*`, `llm.*`, `list.*` | No. Runtime namespaces and external lookups are outside this Rule contract. |
| `api.*` | No. This former namespace has been removed; compatibility service results use `service.<step_id>`. |

Optional fields may be absent. Reading an absent optional field returns
`E_MISSING_INPUT`; guard it with, for example,
`exists(event.payment.amount) && event.payment.amount > 1000`, with that path
explicitly declared in the schema. An undeclared path is a compile-time error,
even inside `exists` or a short-circuited operand.

Explicit null, missing required fields, wrong types and undeclared input fields
fail input validation with `E_INPUT_SCHEMA`. Optional does not mean nullable;
missing values are not converted to zero or false. Core input types are finite
numbers, strings, booleans and closed objects; input arrays are not enabled.
Infix `exists` / `missing` and null comparisons are not presence checks. See
[Core input rules](context.md#strict-core-input-and-results) for nested input and error semantics.

### 2.3 Operators and type constraints

All entries below apply to strict Core. There is no implicit type coercion.

| Operator | Required operands / behavior |
|---|---|
| `==`, `!=` | Two scalars of the same type: number, string or boolean. |
| `<`, `>`, `<=`, `>=` | Two numbers. |
| `+`, `-`, `*`, `/`, `%`; unary `-` | Numeric arithmetic. Divide/modulo by zero and nonfinite results return errors. |
| `&&`, `\|\|`, `!` | Boolean operands, with short-circuit evaluation. |
| `in`, `not in` (`not_in` alias) | A scalar and a literal array of the same scalar type, e.g. `event.country in ["US", "CA"]`. |
| `contains`, `starts_with`, `ends_with` | Two strings. Array containment is not supported in Core. |
| `regex` | A string and a string-literal pattern; dynamic patterns are rejected. |

Membership arrays permit up to 1024 items, subject to the shared token limit.
Mixed types, null and nested arrays are invalid. Membership in an empty array is
false; non-membership is true. Duplicates do not change the result. `in list` is
not a separate operator: historical `event.user_id in list.vip_users` uses an
external list operand, which strict Core rejects.

Literal string matching is case-sensitive Unicode text matching without
normalization. An empty needle matches every string. Regex uses Rust regex search
semantics: add `^` and `$` for a whole-string match or `(?i)` for case-insensitive
matching. Look-around and backreferences are rejected. Invalid patterns fail
before execution, including patterns in unreachable operands. For pattern limits
and matching rules, see [membership and string matching](expression.md#string-operators).

## 3. Score and invocation results

A matching Rule adds its configured score once per invocation. A miss contributes
zero. A zero-score match still records a triggered rule and, for a direct Rule
call, `matched: true`; a miss has `matched: false`. A guard-skipped call is a
separate state with `status: skipped` and no score or matched value.

Negative scores reduce the current invocation's contribution. Totals may be
negative; the engine does not automatically clamp or normalize them. Checked i32
accumulation returns `E_SCORE_OVERFLOW` on overflow.

A Ruleset accumulates its ordered rule invocations into a local score. A Pipeline
adds each direct Rule/Ruleset/sub-Pipeline call's returned score once. It does not
add child-internal rule scores again. Each call starts from zero; the same Rule
can execute again in another invocation. See [Pipeline call scoping](pipeline.md#32-direct-call-results).

Direct Rule calls expose `status`, `score`/`total_score` and `matched` to permitted
later Pipeline conditions. They do not produce a signal or actions. Ruleset
conclusions produce local signals; the parent Pipeline explicitly selects the
final decision and action intents.

## 4. Related documentation

- [CDL reference](overall.md), [Ruleset](ruleset.md) and [Pipeline](pipeline.md): current executable contracts.
- [Expressions](expression.md), [features](feature.md) and [imports](import.md): separately scoped references.
