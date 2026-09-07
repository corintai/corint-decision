# CORINT Expression Language Reference

<!-- cdl-scope: compatibility-unverified -->
> This page is an unverified compatibility reference. Its snippets are not Core support evidence.
> For the executable contract and supported examples, use [CDL Core](cdl-core.md),
> [Pipeline](pipeline.md) and the [capability inventory](schema/capabilities.json).
> Described behavior may be incomplete in compatibility entry points; validate through the strict tools before delivery.


> **Purpose**: This document provides a concise reference for LLM agents working with CORINT's expression language. For detailed documentation, see other DSL guides.

## Quick Reference

CORINT expressions are used in:
- **Rule `when` conditions** - Pattern matching
- **Pipeline `when` blocks** - Event routing
- **Pipeline `decision` logic** - Final decision determination
- **Feature expressions** - Mathematical computations

## Expression Contexts

| Context | Evaluator | Supported Operations |
|---------|-----------|---------------------|
| **Compiled Rule/Pipeline/Registry conditions** | Shared ExpressionParser → ExpressionCompiler → VM | Core supports checked arithmetic, comparisons, boolean short-circuiting, typed literal-array membership and string matching; external namespaces remain compatibility-only |
| **Feature Expressions** | ExpressionEvaluator | Basic arithmetic (+, -, *, /, parentheses) only |

---

## Field Access

Field names may start with `_`; in strict Core they must also be declared in the input schema.

### Namespace Access Pattern

```yaml
event.<field>                    # Event data
event.<nested.field>             # Nested fields
features.<feature_name>          # Computed features
results.<ruleset_id>.<field>     # Ruleset results
api.<api_name>.<field>           # External API results
service.<service_name>.<field>   # Internal service results
vars.<variable_name>             # Variables
sys.<field>                      # System metadata
```

### Examples

```yaml
# Event fields
event.type == "transaction"
event.amount > 1000
event.user.id == "user123"

# Features (from supabase_feature_ruleset.yaml)
features.transaction_sum_7d > 5000
features.transaction_count_24h > 10

# Results
results.supabase_risk_assessment.signal == "decline"
results.fraud_detection.total_score > 80
```

---

## Literals

```yaml
# Numbers
42
3.14
-100

# Strings
"hello world"
'test@example.com'

# Booleans
true
false

# Null
null

# Arrays
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
| `>` | Greater than | `features.transaction_sum_7d > 5000` |
| `<=` | Less than or equal | `features.transaction_count_24h <= 10` |
| `>=` | Greater than or equal | `event.amount >= 500` |

---

## Logical Operators

### AND Logic (`all`)

```yaml
when:
  all:
    - event.type == "transaction"
    - event.source == "supabase"
    - features.transaction_sum_7d > 5000
```

### OR Logic (`any`)

```yaml
when:
  any:
    - event.country in ["RU", "NG", "UA"]
    - features.risk_score > 80
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
        - event.user_id in list.blocked_users
```

---

## Membership Operators

### Array Membership

```yaml
event.country in ["US", "UK", "CA"]
event.status not in ["blocked", "suspended"]
```

### List Membership

```yaml
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

---

## Arithmetic Operators (Feature Expressions Only)

| Operator | Operation | Example |
|----------|-----------|---------|
| `+` | Addition | `a + b` |
| `-` | Subtraction | `a - b` |
| `*` | Multiplication | `a * b` |
| `/` | Division | `a / b` |
| `( )` | Parentheses | `(a + b) * c` |

**Note:** Arithmetic is primarily used in feature expressions, not rule conditions.

### Feature Expression Examples

```yaml
features:
  - name: transaction_rate
    type: expression
    expression: "txn_count_24h / (txn_count_7d + 0.0001)"
    
  - name: amount_ratio
    type: expression
    expression: "event.amount / (avg_transaction_amount + 0.0001)"
```

**Limitations:**
- No function calls (use workarounds like `(x + 0.0001)` instead of `max(x, 1)`)
- No modulo `%` operator
- Only basic arithmetic: `+`, `-`, `*`, `/`, parentheses

---

## Operator Precedence

For compiled conditions, highest to lowest: parentheses; unary `!` / `-`;
`* / %`; `+ -`; relational and compatibility membership/string operators;
`== !=`; `&&`; `||`. Equal-precedence binary operators associate left-to-right.
`&&` and `||` short-circuit, including when combined with YAML boolean groups.
For example, `true || false && false` is true. `conclusion.when` accepts only
string expressions; use these operators instead of YAML group objects there.

Strings are consumed as one token: Unicode and operator characters inside quotes
remain literal content. Both quote styles accept `\\`, `\"`, `\'`, `\/`, `\n`,
`\r`, `\t`, `\b`, `\f`, and `\uXXXX` (including surrogate pairs). Unknown escapes,
unterminated strings, malformed tokens and more than 128 recursive parser levels
fail explicitly. Decimal/scientific numbers and unary negation are supported,
including `1e-3`, `event.amount > -1` and `-event.amount`.

The [Core expression contract](cdl-core.md#3-expressions-types-and-dataflow)
defines the accepted types and namespaces for strict entry points. Recognizing
compatibility syntax does not enable it in Core. Feature `ExpressionEvaluator`
is a separate evaluator and does not inherit this compiled-condition contract.

---

## Complete Examples

### Rule Condition 

```yaml
rule:
  id: high_transaction_volume
  when:
    all:
      - features.transaction_sum_7d > 5000
      - features.transaction_count_24h > 10
  score: 50
```

### Pipeline Decision

```yaml
pipeline:
  decision:
    - when: results.supabase_risk_assessment.signal == "decline"
      result: decline
      reason: "Risk assessment failed"


    - when: results.supabase_risk_assessment.signal == "review"
      result: review
      actions: ["KYC"]
```

### Complex Condition

```yaml
rule:
  id: high_risk_transaction
  when:
    all:
      - event.type == "transaction"
      - event.amount >= 500
      - any:
          - event.country in ["RU", "NG", "CN"]
          - features.risk_score > 70
      - not:
          - event.user_id in list.vip_users
  score: 100
```

---

## Error Handling

### Missing Fields

At permissive compatibility field-loading entry points, missing fields can evaluate to `null`. Strict Core rejects missing required inputs and reports `E_MISSING_INPUT` when a missing optional field is read; use `exists(event.path)` before reading it:

```yaml
# If event.verified doesn't exist, returns false
- event.verified == true
```

### Type Mismatches

Compatibility comparisons can return `false` for mismatched types. Strict Core rejects type mismatches during compilation or input validation:

```yaml
# If event.amount is a string, returns false
- event.amount > 1000
```

### Division by Zero

The compatibility Feature expression evaluator can return `null` for division by zero. Compiled Core arithmetic instead returns `E_DIVISION_BY_ZERO`:

```yaml
# Historical approximation; changes the calculation and is not Core error handling
expression: "numerator / (denominator + 0.0001)"
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
relation   = sum { ("<" | "<=" | ">" | ">=" | compatibility-op) sum }
sum        = product { ("+" | "-") product }
product    = unary { ("*" | "/" | "%") unary }
unary      = ("!" | "-") unary | primary
primary    = literal | field | "(" expression ")" | function-call
number     = digits ["." [digits]] [("e" | "E") ["+" | "-"] digits]
compatibility-op = "in" | "not in" | "not_in" | "contains"
                 | "starts_with" | "ends_with" | "regex"
```

String literals follow the escaping rules above. Core admits scalar literals,
declared fields, and literal arrays only on the right of `in` / `not in` / `not_in`.
The membership/string operators listed above are now admitted under the strict
[Core type and pattern limits](cdl-core.md#membership-and-string-matching).
`contains` / `starts_with` / `ends_with` require strings; `regex` requires a constant
pattern checked at compile time. External list/Feature/API references remain outside
Core. The only function admitted by Core is `exists(event.declared_path)`.
Compatibility parsing additionally recognizes null, literal arrays and function
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

<factor> ::= <number>
           | <feature-name>
           | "(" <arithmetic-expr> ")"

<feature-name> ::= <identifier>
```

### Syntax Examples

**Valid expressions:**
```yaml
event.type == "transaction"
features.transaction_sum_7d > 5000
event.amount >= 500
results.supabase_risk_assessment.signal == "decline"
event.country in ["US", "UK", "CA"]
event.email contains "@suspicious.com"
```

**Valid feature expressions:**
```yaml
txn_count_24h / (txn_count_7d + 0.0001)
(a + b) * c
numerator / (denominator + 0.0001)
```

**Invalid patterns:**
```yaml
Event.type                   # ❌ Namespace must be lowercase
event.user..id              # ❌ Double dots not allowed
.event.type                 # ❌ Cannot start with dot
event.type.                 # ❌ Cannot end with dot
```

---

## Summary

### Supported in Rules/Pipelines

- Field access (event, features, results, api, service, vars, sys)
- Comparison operators (==, !=, <, >, <=, >=)
- Logical operators (all/any/not)
- Membership operators (in, not in, in list)
- String operators (contains, starts_with, ends_with, regex)
- Literals (numbers, strings, booleans, null, arrays)

### Supported in Feature Expressions

- Basic arithmetic (+, -, *, /, parentheses)
- Feature name substitution
- Automatic dependency extraction

---

**For detailed documentation, see:**
- [rule.md](rule.md) - Rule definitions
- [pipeline.md](pipeline.md) - Pipeline configuration
- [feature.md](feature.md) - Feature engineering
- [context.md](context.md) - Context and variable management

## Resource limits

The shared text parser accepts at most 4,096 tokens and an AST depth of 128, including flat binary chains. The VM validates jump targets and limits each program execution (including its decision block) to 1,000,000 instructions. Exceeding a limit returns an error.
