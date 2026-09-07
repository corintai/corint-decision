# CORINT Expression Language Reference

<!-- cdl-scope: compatibility-unverified -->
> Historical snippets on this page are unverified compatibility references. They are not Core support evidence.
> For the executable contract and supported examples, use [CDL reference](overall.md),
> [Pipeline](pipeline.md) and the [language scope](overall.md#language-scope).
> Core and compatibility behavior are identified separately below. Strict tools validate
> Core admission; they do not verify compatibility execution.

> **Purpose**: A reference for expression syntax and evaluator differences. Code blocks
> illustrate expressions or configuration fragments, not standalone resource bundles.

## Quick Reference

CORINT expressions are used in:

- **Registry, Rule and Ruleset `when` conditions** - Entry guards and matching
- **Ruleset `conclusion.when`** - Local signal selection
- **Pipeline entry guards, step guards and Router conditions** - Execution and routing
- **Pipeline `decision` logic** - Final decision determination
- **Feature expressions** - Mathematical computations

## Expression Contexts

| Context | Evaluator | Supported Operations |
|---------|-----------|---------------------|
| **Strict Core conditions** | Shared ExpressionParser → Core type/dataflow checks → ExpressionCompiler → VM | Numeric `+ - * / %`, unary minus, comparisons, boolean short-circuiting, typed literal-array membership, string matching and `exists(event.path)` |
| **Compatibility compiled conditions** | Shared ExpressionParser → compatibility compiler → VM | Pipeline guards, routes and final decisions share expression instructions and short-circuiting; admission and external namespaces still depend on the entry point |
| **Compatibility Feature expressions** | Shared ExpressionParser → ExpressionEvaluator | Numeric `+ - * / %`, unary minus, parentheses and the math functions listed below; no comparisons, boolean conditions or string operations |

Core conditions must produce a boolean. Feature expressions produce a finite number
or `null`; they do not inherit the Core condition contract.

---

## Field Access

Field names may start with `_`. Core `event` paths must be declared in the input
schema, even inside an operand that will be skipped by boolean short-circuiting.

### Namespace Access Pattern

```text
event.<declared_path>             # Core event input, including nested fields
total_score                      # Core local aggregate, only in the scopes below
results.<resource_id>.<field>     # Core direct-call results, subject to dataflow checks
features.<feature_name>           # Compatibility only: computed features
service.<step_id>.<field>         # Compatibility only: service-call results
vars.<variable_name>             # Compatibility only: bound values
sys.<field>                       # Compatibility only: system metadata
```

| Core condition position | Available references |
| --- | --- |
| Registry guard, Rule/Ruleset `when`, Pipeline entry guard | Declared `event` fields and `exists(event.path)` |
| Ruleset `conclusion.when` | Declared `event` fields, `exists(event.path)` and local `total_score` |
| Pipeline step guard, Router route, Pipeline decision | Declared `event` fields, `exists(event.path)`, local `total_score` and available direct-call `results` |

`results.<resource_id>` refers to a direct Rule, Ruleset or child Pipeline call
reached on every incoming path, not to an arbitrary resource or a step ID.
Rules expose `matched` instead of `signal`; reached calls expose `status`.
Guarded calls may be skipped, in which case score/signal fields are absent. Check
their status before reading those fields. Parent and child Pipeline result scopes
are isolated. A declared input such as `event.total_score` is separate from the
runtime's local `total_score` aggregate. See [Core references](context.md#strict-core-input-and-results) and
[Pipeline result scoping](pipeline.md#32-direct-call-results).

In compatibility service execution, the default output is `service.<step_id>` for
both internal and external services. The provider name does not determine that
path. An explicit `output` may target `service.<path>` or `vars.<path>`; read the
configured path. Reading a service result does not invoke the service. See
[Execution context](context.md) for the separate compatibility namespaces.

### Examples

```text
# Core: fields must be declared with the corresponding types
event.type == "transaction"
event.amount > 1000
event.user.id == "user123"

# Compatibility compiled conditions only; requires supplied feature values
features.transaction_sum_7d > 5000
features.transaction_count_24h > 10

# Core step guards, Router routes or Pipeline decisions only;
# these direct Ruleset calls must be completed and in scope
results.supabase_risk_assessment.signal == "decline"
results.fraud_detection.total_score > 80
```

---

## Literals

Core scalar literals are finite numbers, strings and booleans. Arrays are allowed
only as membership operands under the restrictions below. Core does not admit
`null` literals, nullable inputs or array inputs. Feature expressions accept numeric
literals only; a referenced numeric input may be `null` and propagate that value.

```text
# Numbers
42
3.14
-100
1e-3

# Strings
"hello world"
'test@example.com'

# Booleans
true
false

# Compatibility parser syntax only; not a Core or Feature arithmetic literal
null

# Core: only on the right of in / not in / not_in
["US", "UK", "CA"]
[1, 2, 3, 4, 5]
```

---

## Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `==` | Equal | `event.status == "active"` |
| `!=` | Not equal | `event.country != "US"` |
| `<` | Less than | `event.amount < 1000` |
| `>` | Greater than | `event.transaction_sum_7d > 5000` |
| `<=` | Less than or equal | `event.transaction_count_24h <= 10` |
| `>=` | Greater than or equal | `event.amount >= 500` |

In Core, equality compares scalars of the same type; ordering compares numbers.
There is no implicit string-to-number or boolean-to-number conversion. The example
`event` fields must be declared and supplied by the caller; these expressions do
not compute features. Feature arithmetic does not accept comparison operators.

---

## Logical Operators

Core string conditions support `&&`, `||` and unary `!`, for example
`exists(event.verified) && event.verified == true`. Operands must be boolean and
evaluate left-to-right with short-circuiting.

The following YAML blocks are condition fragments. Core `all` and `any` must be
nonempty; `not` takes exactly one item. Each group object has one of these keys.
Groups can nest. `Ruleset.conclusion.when` accepts only a string, so use
`&& / || / !` there instead of group objects. Feature arithmetic accepts neither form.

### AND Logic (`all`)

```yaml
when:
  all:
    - event.type == "transaction"
    - event.source == "supabase"
    - event.transaction_sum_7d > 5000
```

### OR Logic (`any`)

```yaml
when:
  any:
    - event.country in ["RU", "NG", "UA"]
    - event.risk_score > 80
    - event.vip_status == true
```

### NOT Logic (`not`)

```yaml
when:
  not:
    - event.verified == true
```

### Nested Logic

```yaml
when:
  all:
    - event.type == "transaction"
    - any:
        - event.amount > 10000
        - event.country in ["RU", "CN"]
    - not:
        - event.blocked == true
```

---

## Membership Operators

### Array Membership

In Core, the left operand is a number, string or boolean, and the right operand
is a literal array of that same scalar type. Mixed types, `null`, nested arrays and
nonfinite elements fail with `E_TYPE`; there is no coercion. Arrays have at most 1024
items; larger arrays fail with `E_EXPRESSION_LIMIT`, and the shared token limit also applies.
Arrays are not accepted as event inputs. Duplicate elements do not affect membership.
An empty array makes `in` false and `not in` true. `not_in` is an alias for `not in`.

```text
event.country in ["US", "UK", "CA"]
event.status not in ["blocked", "suspended"]
```

### List Membership

**Compatibility only:** external `list.<id>` lookups are outside Core. These are
syntax examples, not evidence that a particular compatibility entry point provides
the referenced lists. See [List](list.md).

```text
event.user_id in list.blocked_users
event.ip_address in list.blocked_ips
event.email not in list.vip_emails
```

---

## String Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `contains` | String contains substring | `event.email contains "@suspicious.com"` |
| `starts_with` | String starts with | `event.phone starts_with "+1"` |
| `ends_with` | String ends with | `event.email ends_with ".com"` |
| `regex` | Regular expression match | `event.transaction_id regex "^TX-[0-9]{8}$"` |

Core string operators require string operands. Literal matching is case-sensitive,
uses Unicode text without normalization, and accepts an empty needle. `regex`
requires a string literal pattern checked at compile time and uses Unicode-aware search
semantics; use anchors for a whole-string match. Inline flags such as `(?i)` opt into
case-insensitive matching. An empty pattern matches; look-around and backreferences
are rejected. Patterns are checked even in a boolean operand that would be skipped.
Malformed or over-limit patterns return `E_INVALID_REGEX`: the limits are 4096 pattern
bytes, 64 syntax nesting levels and 1 MiB compiled regex size. The shared bounded cache
holds at most 32 patterns with a 256 KiB DFA cache per pattern.
Both operands of literal string matching may be declared string fields. An empty
needle matches every string, including an empty string. These operators preserve
short-circuiting in all Core condition scopes; reading a missing optional field still
returns `E_MISSING_INPUT` and should be guarded with `exists`.
These operators are not available in Feature arithmetic.

---

## Arithmetic Operators

Both Core conditions and compatibility Feature expressions support these numeric
operators. In Core, use arithmetic within a boolean condition, such as
`event.amount / event.quantity > 100`, with declared numeric fields and a nonzero
divisor. Bare names such as `a` and `b` below are Feature dependencies; Core uses
declared `event` paths or the scoped numeric references described above.

| Operator | Operation | Example |
|----------|-----------|---------|
| `+` | Addition | `a + b` |
| `-` | Subtraction | `a - b` |
| `*` | Multiplication | `a * b` |
| `/` | Division | `a / b` |
| `%` | Remainder | `a % b` |
| unary `-` | Negation | `-a` |
| `( )` | Parentheses | `(a + b) * c` |

Core numbers use IEEE-754 binary64. Nonfinite arithmetic results fail with
`E_NUMBER_OVERFLOW`; division or remainder by zero fails with `E_DIVISION_BY_ZERO`.
Feature arithmetic has the distinct null/error behavior described below.

### Feature Expression Examples

These are compatibility Feature configuration fragments. Their dependencies must
be registered or supplied through that entry point; they are not Core resources.

```yaml
features:
  - name: transaction_rate
    type: expression
    expression: "txn_count_24h / txn_count_7d"

  - name: amount_ratio
    type: expression
    expression: "event.amount / avg_transaction_amount"
```

### Feature Functions and Inputs

| Function | Arguments | Operation |
| --- | --- | --- |
| `min(a, b)`, `max(a, b)` | Exactly two numbers | Minimum or maximum |
| `abs(x)`, `sqrt(x)` | Exactly one number | Absolute value or square root |
| `ceil(x)`, `floor(x)`, `round(x)` | Exactly one number | Round up, down, or to the nearest integer; `round` ties away from zero |

Bare feature names and `features.name` create dependencies, which are extracted
automatically from the parsed expression. `event.nested.field` reads request data
without creating a Feature dependency. Evaluation resolves these references from
values; it is not textual substitution.

Operands must be finite numbers or `null`. Null propagates through arithmetic and
math functions; division or remainder by zero returns `null`. Missing inputs,
wrong types and nonfinite results (including `sqrt(-1)`) are errors. Unsupported
paths, operators, functions and function arities are rejected during registration.
These math functions are outside Core; Core's only function is `exists(event.path)`.
See [Feature arithmetic](feature.md#expression).

The ratio examples preserve their denominators, so a zero denominator produces
`null`. Adding `0.0001` changes the formula and still gives zero when the original
denominator is `-0.0001`. It is not equivalent to `max(x, 1)`; use `max` only when
clamping the denominator to at least one is an intended part of the calculation.

---

## Operator Precedence

For compiled conditions, highest to lowest: parentheses; unary `!` / `-`;
`* / %`; `+ -`; relational and membership/string operators;
`== !=`; `&&`; `||`. Equal-precedence binary operators associate left-to-right.
`&&` and `||` short-circuit, including when combined with YAML boolean groups.
For example, `true || false && false` is true. `conclusion.when` accepts only
string expressions; use these operators instead of YAML group objects there.

Strings are consumed as one token: Unicode and operator characters inside quotes
remain literal content. Both quote styles accept `\\`, `\"`, `\'`, `\/`, `\n`,
`\r`, `\t`, `\b`, `\f`, and `\uXXXX` (including surrogate pairs). Unknown escapes,
unterminated strings, malformed tokens and more than 128 recursive parser levels
fail with `E_INVALID_EXPRESSION`. Core expressions are limited to 4096 tokens and
128 actual AST levels; parser recursion is separately limited to 128 levels and
counts grouping, unary operators and precedence descent. Decimal/scientific numbers and unary negation are supported,
including `1e-3`, `event.amount > -1` and `-event.amount`.

YAML quoting and expression escaping are separate layers. Quote the entire YAML
scalar when required by YAML syntax; expression string escapes are processed after
YAML decoding.

The [literal types](#literals) and [field-access rules](#field-access)
define the accepted values and namespaces for strict entry points. Recognizing
compatibility syntax does not enable it in Core. Feature `ExpressionEvaluator`
is a separate evaluator and does not inherit this compiled-condition contract.

---

## Condition Fragments

These snippets must be placed inside the indicated resource and supplied with an
input schema and referenced resources. They are not standalone documents. Use the
executable examples in [Rule](rule.md#4-language-examples), [Ruleset](ruleset.md)
and [Pipeline](pipeline.md) for complete bundles.

### Rule Condition (Core)

Place this `when` block inside a Rule with its required `id`, `name` and `score`.
Declare both transaction fields as numeric inputs; the caller supplies their values.

```yaml
when:
  all:
    - event.transaction_sum_7d > 5000
    - event.transaction_count_24h > 10
```

### Pipeline Decision (Core)

Place this decision in a Pipeline with `id`, `name`, `entry` and `steps`. Every path
to the decision must complete the direct `supabase_risk_assessment` Ruleset call.
The final `default: true` row covers signals other than `decline` and `review`.

```yaml
decision:
  - when: results.supabase_risk_assessment.signal == "decline"
    result: decline
    reason: "Risk assessment failed"
  - when: results.supabase_risk_assessment.signal == "review"
    result: review
    actions: ["KYC"]
  - default: true
    result: approve
```

### Complex Condition (Compatibility Only)

This Rule `when` fragment refers to Feature and external list values, which Core
rejects. A compatibility host must supply those capabilities and a complete Rule
definition (`id`, `name`, `score`); parser acceptance alone does not verify execution.

```yaml
when:
  all:
    - event.type == "transaction"
    - event.amount >= 500
    - any:
        - event.country in ["RU", "NG", "CN"]
        - features.risk_score > 70
    - not:
        - event.user_id in list.vip_users
```

---

## Error Handling

### Missing Fields

Core rejects missing required inputs with `E_INPUT_SCHEMA`. Reading an absent
optional field reports `E_MISSING_INPUT`; use `exists(event.path)` to guard the read.
For a declared optional boolean `verified`, this condition is false when it is absent:

```yaml
when: "exists(event.verified) && event.verified == true"
```

At permissive compatibility field-loading entry points, missing fields may resolve
to `null`. Feature arithmetic instead errors on missing references; an explicitly
supplied `null` value propagates. Neither behavior changes Core admission.

### Type Mismatches

Core rejects operand type mismatches during compilation (`E_TYPE`) or input
validation (`E_INPUT_SCHEMA`). For this Core fragment, declare `amount` as a number;
a supplied string is invalid input, not a condition that evaluates to false:

```yaml
when: "event.amount > 1000"
```

Some compatibility comparisons return false for mismatched types. Feature
arithmetic rejects nonnumeric, non-null operands without coercion.

### Division by Zero

Feature arithmetic returns `null` for division or remainder by zero. Core returns
`E_DIVISION_BY_ZERO`. To make a zero denominator yield a false condition, explicitly
guard the division using short-circuiting. Here all three fields are required numbers:

```yaml
when: "event.denominator != 0 && event.numerator / event.denominator > event.limit"
```

---

## BNF Grammar

### Compiled Condition Grammar

The precedence table above is normative for the shared lexer/parser. The outline
below is EBNF; semantic validation further limits types and references in Core.
YAML `all` / `any` / `not` groups wrap expressions and are not string tokens.

```text
expression = or
or         = and { "||" and }
and        = equality { "&&" equality }
equality   = relation { ("==" | "!=") relation }
relation   = sum { ("<" | "<=" | ">" | ">=" | matching-op) sum }
sum        = product { ("+" | "-") product }
product    = unary { ("*" | "/" | "%") unary }
unary      = ("!" | "-") unary | primary
primary    = literal | field | "(" expression ")" | function-call
number     = digits ["." [digits]] [("e" | "E") ["+" | "-"] digits]
matching-op = "in" | "not in" | "not_in" | "contains"
            | "starts_with" | "ends_with" | "regex"
```

String literals follow the escaping rules above. Core admits number/string/boolean literals,
declared fields, and literal arrays only on the right of `in` / `not in` / `not_in`.
The membership/string operators listed above are admitted under the strict
[Core type and pattern limits](#string-operators).
`contains` / `starts_with` / `ends_with` require strings; `regex` requires a constant
pattern checked at compile time. External list/Feature/Service references remain outside
Core. The only function admitted by Core is `exists(event.declared_path)`.
Compatibility parsing additionally recognizes null, arrays in other positions and function
calls; their recognition alone does not imply compiler/runtime support. Resource
IDs and input schema field names remain subject to their respective schema rules.

### Feature Expression Grammar

```bnf
<feature-expression> ::= <arithmetic-expr>

<arithmetic-expr> ::= <term>
                    | <arithmetic-expr> "+" <term>
                    | <arithmetic-expr> "-" <term>

<term> ::= <factor>
         | <term> "*" <factor>
         | <term> "/" <factor>
         | <term> "%" <factor>

<factor> ::= <number>
           | <feature-name>
           | "features." <feature-name>
           | "event." <field-path>
           | "-" <factor>
           | <math-function-call>
           | "(" <arithmetic-expr> ")"

<feature-name> ::= <identifier>
```

Function names, arities, null and error behavior are specified in
[Feature arithmetic](feature.md#expression).

### Syntax Examples

**Core expression fragments** (declared inputs and result scoping still apply):

```text
event.type == "transaction"
exists(event.verified) && event.verified == true
event.amount >= 500
results.supabase_risk_assessment.signal == "decline"
event.country in ["US", "UK", "CA"]
event.email contains "@suspicious.com"
```

**Compatibility Feature expressions** (numeric dependencies must be supplied):

```text
txn_count_24h / txn_count_7d
(a + b) * c
max(features.retry_count, 1)
abs(event.amount) % 10
```

**Invalid patterns:**

```text
Event.type                 # Core reference error: Event is not the event namespace
event.user..id              # ❌ Double dots not allowed
.event.type                 # ❌ Cannot start with dot
event.type.                 # ❌ Cannot end with dot
```

`Event.type` is accepted as a path by the shared parser but rejected as a Core
reference. The other three examples are parser errors.

---

## Summary

### Strict Core Conditions

- Declared `event` inputs and `exists(event.path)`; local `total_score` and direct-call `results` only in the scopes listed above.
- Numeric `+ - * / %`, unary minus and comparisons of the permitted types.
- Boolean `&& / || / !`, parentheses, nonempty YAML `all/any` and one-item `not`; Ruleset conclusion conditions are string-only.
- Typed literal-array `in / not in / not_in` and string `contains / starts_with / ends_with / regex` within the Core limits.
- Finite number, string and boolean literals. Arrays are membership operands only; `null` and external namespaces are outside Core.

### Compatibility Feature Expressions

- Numeric `+ - * / %`, unary minus, parentheses and the listed math functions.
- Bare feature names or `features.name` with automatic dependency extraction; `event.nested.field` reads numeric request values.
- Explicit null propagation and null on division/remainder by zero; missing values, wrong types and nonfinite results are errors.

Compatibility compiled conditions have a separate entry-point-dependent scope.
External Feature, list, service, variable and system references are not Core
capabilities, and are not all accepted by Feature arithmetic.

---

**For detailed documentation, see:**

- [rule.md](rule.md) - Rule definitions
- [ruleset.md](ruleset.md) - Local aggregation and conclusion conditions
- [pipeline.md](pipeline.md) - Pipeline configuration
- [feature.md](feature.md) - Feature engineering
- [context.md](context.md) - Context and variable management

## Resource limits

The shared text parser accepts at most 4,096 tokens and an AST depth of 128, including flat binary chains. The VM validates jump targets and limits each program execution (including its decision block) to 1,000,000 instructions. Exceeding a limit returns an error.
