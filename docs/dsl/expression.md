# CORINT Expression Language Reference (v0.2)

The **Expression Language** powers CORINT's condition evaluation system for rules and pipelines.

> **⚠️ Important:** This document clearly marks **✅ Implemented** vs **📋 Planned** features.

---

## 1. Overview

### 1.1 Expression Usage

Expressions are used in:
- **Rule `when` conditions** - Match events to rules
- **Pipeline `when` blocks** - Route events to pipelines
- **Decision logic** - Final decision determination
- **Feature expressions** - Mathematical computations (limited scope)

### 1.2 Expression Contexts

CORINT has **two separate expression evaluators** with different capabilities:

| Context | Evaluator | Purpose | Supported Operations |
|---------|-----------|---------|---------------------|
| **Rules/Pipelines** | WhenEvaluator | Condition matching | ✅ Full comparison, logical, list operations |
| **Feature Expressions** | ExpressionEvaluator | Math computations | ✅ Basic arithmetic (+, -, *, /, parentheses) only |

---

## 2. Field Access (✅ Implemented)

### 2.1 Event Field Access

Access event fields using dot notation:

```yaml
# In rules and pipelines
when:
  all:
    - event.type == "transaction"
    - event.amount > 1000
    - event.user_id == "user123"
```

**Nested field access:**
```yaml
- event.user.profile.age > 18
- event.metadata.device_type == "mobile"
```

### 2.2 Feature Access

Access computed features using the `features.` prefix:

```yaml
when:
  all:
    - features.transaction_sum_7d > 5000
    - features.login_count_24h > 10
    - features.unique_devices_7d < 3
```

### 2.3 Result Access

Access results from previous steps:

```yaml
when:
  all:
    - results.user_risk_ruleset.signal == "review"
    - results.device_check.score > 50
```

---

## 3. Literals (✅ Implemented)

### 3.1 Supported Literal Types

```yaml
# Numbers
42
3.14
-100

# Strings (double or single quotes)
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

## 4. Comparison Operators (✅ Implemented)

| Operator | Description | Example |
|----------|-------------|---------|
| `==` | Equal | `event.status == "active"` |
| `!=` | Not equal | `event.country != "US"` |
| `<` | Less than | `event.amount < 1000` |
| `>` | Greater than | `event.score > 80` |
| `<=` | Less than or equal | `features.failed_login_count_24h <= 3` |
| `>=` | Greater than or equal | `event.balance >= 100` |

**Example:**
```yaml
rule:
  id: high_value_transaction
  when:
    all:
      - event.type == "transaction"
      - event.amount >= 1000
      - event.verified == true
  score: 50
```

---

## 5. Logical Operators (✅ Implemented)

### 5.1 AND Logic (all)

```yaml
when:
  all:
    - event.type == "login"
    - event.country == "US"
    - features.login_count_24h < 10
```

### 5.2 OR Logic (any)

```yaml
when:
  any:
    - event.country in ["RU", "NG", "UA"]
    - features.risk_score > 80
    - event.vip_status == true
```

### 5.3 NOT Logic (not)

```yaml
when:
  not:
    - event.verified == true
```

### 5.4 Nested Logic

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

## 6. Membership Operators (✅ Implemented)

### 6.1 Array Membership (`in` / `not in`)

Check if value is in an array:

```yaml
when:
  all:
    - event.country in ["US", "UK", "CA"]
    - event.status not in ["blocked", "suspended"]
```

### 6.2 List Membership (`in list` / `not in list`)

Check membership in configured custom lists:

```yaml
when:
  all:
    - event.user_id in list.blocked_users
    - event.ip_address in list.blocked_ips
    - event.email not in list.vip_emails
```

**List configuration** is required in `lists/` directory. See [list.md](list.md) for details.

---

## 7. String Operators (✅ Implemented)

| Operator | Description | Example |
|----------|-------------|---------|
| `contains` | String contains substring | `event.email contains "@suspicious.com"` |
| `starts_with` | String starts with | `event.phone starts_with "+1"` |
| `ends_with` | String ends with | `event.email ends_with ".com"` |
| `regex` | Regular expression match | `event.transaction_id regex "^TX-[0-9]{8}$"` |

**Example:**
```yaml
rule:
  id: suspicious_email
  when:
    any:
      - event.email contains "test"
      - event.email ends_with "@temporary.com"
      - event.email regex "^[0-9]+@"
  score: 70
```

---

## 8. Arithmetic Operators (✅ Implemented in Feature Expressions)

> **Note:** Arithmetic is primarily used in **Feature Expressions**, not rule conditions.

### 8.1 Supported Operators

| Operator | Operation | Example |
|----------|-----------|---------|
| `+` | Addition | `a + b` |
| `-` | Subtraction | `a - b` |
| `*` | Multiplication | `a * b` |
| `/` | Division | `a / b` |
| `( )` | Parentheses | `(a + b) * c` |

### 8.2 Feature Expression Examples

```yaml
# Feature definitions with expressions
features:
  - name: transaction_rate
    type: expression
    expression: "txn_count_24h / (txn_count_7d + 0.0001)"
    # Dependencies auto-extracted

  - name: amount_ratio
    type: expression
    expression: "event.amount / (avg_transaction_amount + 0.0001)"

  - name: velocity_spike
    type: expression
    expression: "txn_count_1h / (txn_count_24h + 0.0001)"
```

**Limitations:**
- No function calls (use workarounds like `(x + 0.0001)` instead of `max(x, 1)`)
- No modulo `%` operator in feature expressions
- Only basic arithmetic: `+`, `-`, `*`, `/`, parentheses

---

## 9. Operator Precedence

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

## 10. Planned Features (📋 Not Yet Implemented)

The following features are documented for future implementation:

### 10.1 Mathematical Functions (📋 Planned)

```yaml
# NOT YET SUPPORTED
- abs(event.amount_change) > 10000
- max(event.amounts) > 50000
- min(event.credit_scores) < 300
- round(event.fee_rate, 2) == 0.03
```

**Current Workaround:** Use feature expressions with basic arithmetic.

### 10.2 String Functions (📋 Planned)

```yaml
# NOT YET SUPPORTED
- lower(event.email) == "admin@company.com"
- upper(event.country) in ["US", "UK"]
- length(event.password) >= 8
```

**Current Workaround:** Pre-process data or use external validation.

### 10.3 Date/Time Functions (📋 Planned)

```yaml
# NOT YET SUPPORTED
- days_between(event.last_login, now()) > 90
- hour(event.timestamp) >= 22
- age_in_years(event.birth_date) >= 18
```

**Current Workaround:** Calculate in application layer before sending event.

### 10.4 Array Functions (📋 Planned)

```yaml
# NOT YET SUPPORTED
- length(event.addresses) > 1
- first(event.login_history).timestamp > "2024-01-01"
```

**Current Workaround:** Use aggregation features with count/distinct methods.

### 10.5 Ternary Expressions (📋 Planned)

```yaml
# NOT YET SUPPORTED
score: event.is_premium ? 0 : 50
```

**Current Workaround:** Use multiple rules with different conditions.

### 10.6 Null-Safe Navigation (📋 Planned)

```yaml
# NOT YET SUPPORTED
- event.user.profile?.age > 18
- event.metadata?.risk_level == "high"
```

**Current Workaround:** Ensure complete event data structure.

### 10.7 Default Values (📋 Planned)

```yaml
# NOT YET SUPPORTED
- (event.risk_score ?? 50) > 80
```

**Current Workaround:** Handle defaults in application layer.

---

## 11. Syntax Examples

### 11.1 Rule Condition Example

```yaml
rule:
  id: high_risk_transaction
  name: High Risk Transaction Detection
  when:
    all:
      - event.type == "transaction"
      - event.amount >= 5000
      - any:
          - event.country in ["RU", "NG", "CN"]
          - features.risk_score > 70
      - not:
          - event.user_id in list.vip_users
  score: 100
```

### 11.2 Pipeline Matching Example

```yaml
pipeline:
  id: transaction_pipeline
  name: Transaction Processing
  when:
    all:
      - event.type == "transaction"
      - event.amount > 0

  steps:
    - step:
        id: risk_check
        type: ruleset
        ruleset: transaction_risk_ruleset

  decision:
    - when: results.transaction_risk_ruleset.signal == "decline"
      result: decline
      reason: "High risk detected"
      terminate: true
```

### 11.3 Feature Expression Example

```yaml
features:
  # Aggregation features
  - name: failed_login_count_24h
    type: aggregation
    method: count
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "{event.user_id}"
    window: 24h
    when: type == "login" AND status == "failed"

  - name: total_login_count_24h
    type: aggregation
    method: count
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "{event.user_id}"
    window: 24h
    when: type == "login"

  # Expression feature - failure rate
  - name: login_failure_rate
    type: expression
    expression: "failed_login_count_24h / (total_login_count_24h + 0.0001)"
    # Dependencies auto-extracted from expression
```

---

## 12. Best Practices

### 12.1 Readability

✅ **Good:**
```yaml
when:
  all:
    - event.type == "transaction"
    - event.amount > 1000
    - features.risk_score < 50
```

❌ **Bad:**
```yaml
when:
  all:
    - event.type=="transaction"&&event.amount>1000
```

### 12.2 Performance

✅ **Good:** Put simple checks first
```yaml
when:
  all:
    - event.type == "transaction"      # Fast: simple field check
    - event.amount > 1000               # Fast: simple comparison
    - features.complex_score > 80       # Slower: computed feature
```

❌ **Bad:** Expensive checks first
```yaml
when:
  all:
    - features.complex_score > 80       # Always evaluated
    - event.type == "transaction"
```

### 12.3 List Membership

✅ **Good:** Use configured lists
```yaml
when:
  all:
    - event.user_id in list.blocked_users
```

❌ **Bad:** Hardcode large arrays
```yaml
when:
  all:
    - event.country in ["RU", "NG", "UA", "CN", "BR", ...]  # Hard to maintain
```

---

## 13. Error Handling

### 13.1 Missing Fields

If a field is missing, it evaluates to `null`:

```yaml
# If event.verified doesn't exist, this returns false
- event.verified == true
```

### 13.2 Type Mismatches

Type mismatches result in `false`:

```yaml
# If event.amount is a string, this returns false
- event.amount > 1000
```

### 13.3 Division by Zero

In feature expressions, division by zero returns `null`:

```yaml
# If denominator is 0, result is null (treated as 0 in rules)
expression: "numerator / denominator"
```

**Recommended workaround:**
```yaml
expression: "numerator / (denominator + 0.0001)"
```

---

## 14. Summary

### ✅ Currently Supported

**In Rules/Pipelines:**
- Field access (event, features, results)
- Comparison operators (==, !=, <, >, <=, >=)
- Logical operators (all/any/not)
- Membership operators (in, not in, in list)
- String operators (contains, starts_with, ends_with, regex)
- Literals (numbers, strings, booleans, null, arrays)

**In Feature Expressions:**
- Basic arithmetic (+, -, *, /, parentheses)
- Feature name substitution
- Automatic dependency extraction

### 📋 Planned for Future

- Mathematical functions (abs, max, min, round, etc.)
- String functions (lower, upper, length, trim)
- Date/time functions
- Array functions
- Ternary expressions
- Null-safe navigation
- Default value operators

---

## 15. Related Documentation

- [feature.md](feature.md) - Feature engineering and aggregations
- [rule.md](rule.md) - Rule definitions
- [pipeline.md](pipeline.md) - Pipeline configuration
- [list.md](list.md) - Custom list configuration
- [context.md](context.md) - Context and variable management
