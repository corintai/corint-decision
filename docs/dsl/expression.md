# CORINT Expression Language Reference

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
| **Rules/Pipelines** | WhenEvaluator | ✅ Comparison, logical, membership, string operations |
| **Feature Expressions** | ExpressionEvaluator | ✅ Basic arithmetic (+, -, *, /, parentheses) only |

---

## Field Access

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
env.<config_key>                 # Environment config
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

**Rule/Pipeline Conditions:**
1. Field access, literals
2. Comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`)
3. `in`, `contains`, `regex`, etc.
4. Logical `all` (AND)
5. Logical `any` (OR)
6. Logical `not`

**Feature Expressions:**
1. Parentheses `( )`
2. Multiplication `*`, Division `/`
3. Addition `+`, Subtraction `-`

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
      reason: "{results.supabase_risk_assessment.reason}"
      
    
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

Missing fields evaluate to `null`:

```yaml
# If event.verified doesn't exist, returns false
- event.verified == true
```

### Type Mismatches

Type mismatches result in `false`:

```yaml
# If event.amount is a string, returns false
- event.amount > 1000
```

### Division by Zero

In feature expressions, division by zero returns `null`:

```yaml
# Recommended workaround
expression: "numerator / (denominator + 0.0001)"
```

---

## BNF Grammar

### Expression Grammar

```bnf
<expression> ::= <comparison>
               | <logical-group>
               | <membership>
               | <string-operation>

<comparison> ::= <operand> <comparison-op> <operand>

<comparison-op> ::= "=="
                  | "!="
                  | "<"
                  | ">"
                  | "<="
                  | ">="

<operand> ::= <field-access>
            | <literal>

<field-access> ::= <namespace> "." <field-path>

<namespace> ::= "event"
              | "features"
              | "results"
              | "api"
              | "service"
              | "vars"
              | "sys"
              | "env"

<field-path> ::= <identifier>
               | <identifier> "." <field-path>

<identifier> ::= <letter> <identifier-rest>*

<identifier-rest> ::= <letter>
                    | <digit>
                    | "_"

<letter> ::= "a".."z" | "A".."Z"

<digit> ::= "0".."9"

<logical-group> ::= "all:" <condition-list>
                  | "any:" <condition-list>
                  | "not:" <condition-list>

<condition-list> ::= "-" <expression>
                   | "-" <expression> <condition-list>

<membership> ::= <operand> "in" <array>
               | <operand> "not" "in" <array>
               | <operand> "in" "list." <identifier>
               | <operand> "not" "in" "list." <identifier>

<array> ::= "[" <array-elements> "]"
          | "[" "]"

<array-elements> ::= <literal>
                   | <literal> "," <array-elements>

<string-operation> ::= <operand> <string-op> <string-literal>

<string-op> ::= "contains"
              | "starts_with"
              | "ends_with"
              | "regex"

<string-literal> ::= <quoted-string>

<quoted-string> ::= '"' <string-chars> '"'
                  | "'" <string-chars> "'"

<string-chars> ::= <char>*
                 | <escaped-char> <string-chars>

<escaped-char> ::= "\\" <char>

<literal> ::= <number>
            | <string-literal>
            | <boolean>
            | "null"

<number> ::= <integer>
           | <float>

<integer> ::= <digit>+
            | "-" <digit>+

<float> ::= <digit>+ "." <digit>+
         | "-" <digit>+ "." <digit>+

<boolean> ::= "true"
            | "false"
```

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
event._private              # ❌ Field cannot start with _
event.user..id              # ❌ Double dots not allowed
.event.type                 # ❌ Cannot start with dot
event.type.                 # ❌ Cannot end with dot
```

---

## Summary

### ✅ Supported in Rules/Pipelines

- Field access (event, features, results, api, service, vars, sys, env)
- Comparison operators (==, !=, <, >, <=, >=)
- Logical operators (all/any/not)
- Membership operators (in, not in, in list)
- String operators (contains, starts_with, ends_with, regex)
- Literals (numbers, strings, booleans, null, arrays)

### ✅ Supported in Feature Expressions

- Basic arithmetic (+, -, *, /, parentheses)
- Feature name substitution
- Automatic dependency extraction

---

**For detailed documentation, see:**
- [rule.md](rule.md) - Rule definitions
- [pipeline.md](pipeline.md) - Pipeline configuration
- [feature.md](feature.md) - Feature engineering
- [context.md](context.md) - Context and variable management
