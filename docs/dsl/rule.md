# CORINT Risk Definition Language (RDL)
## Rule Specification (v0.1)

A **Rule** is the smallest executable logic unit within CORINT's Cognitive Risk Intelligence framework.
Rules define deterministic conditions used to evaluate risk events and generate risk scores.

---

## 1. Rule Structure

### 1.1 Implemented Fields (✅)

```yaml
rule:
  id: string                 # ✅ Required: Unique identifier
  name: string               # ✅ Required: Human-readable name
  description: string        # ✅ Optional: Rule description
  when: <condition-block>    # ✅ Required: Condition logic
  score: number              # ✅ Required: Risk score (supports negative values)
  metadata:                  # ✅ Optional: Arbitrary metadata
    <key>: <value>
```
---

## 2. `id`

A globally unique identifier for the rule.

Example:

```yaml
id: high_risk_login
```

---

## 3. `name`

Human-readable name for the rule.

```yaml
name: High-Risk Login Detection
```

---

## 4. `description`

A short explanation of the rule’s purpose.

```yaml
description: Detect risky login behavior using rules and LLM reasoning.
```

---

## 5. `when` Block

The core of the rule. Describes conditions that must be satisfied for the rule to trigger.

The `when` clause supports boolean expressions, logical operators (`all`/`any`/`not`), and arbitrary nesting for complex condition evaluation.

### 5.1 Basic Syntax (✅ Implemented)

**1. Simple Boolean Expression:**
A single boolean expression that evaluates to true or false.

```yaml
when: event.amount < 100
```

**2. Logical Operators:**
- `all`: All conditions must be true (AND logic)
- `any`: At least one condition must be true (OR logic)
- `not`: Negate the condition (NOT logic)

**3. Nesting:**
Logical operators can be nested arbitrarily to create complex conditions.

### 5.2 Logical Operators (✅ Implemented)

**all** - All conditions must be true (AND logic):
```yaml
when:
  all:
    - event.amount > 1000
    - event.country == "US"
    - features.txn_count_24h < 10
    - not:
        - features.risk_tags contains "proxy"
```

**any** - At least one condition must be true (OR logic):
```yaml
when:
  any:
    - event.country in ["RU", "NG", "CN"]
    - features.risk_score > 80
    - event.amount > 10000
```

**not** - Negation:
```yaml
when:
  not:
    - event.verified == true
```

**Complex Nested Logic:**
```yaml
when:
  any:
    # Category A: High Amount + High-Risk Country + Non-Whitelist
    - all:
        - event.amount >= 3000
        - event.country in ["NG", "PK", "UA", "RU"]
        - not:
            - event.user_id in list.vip_users
    
    # Category B: Device/IP Anomaly + High Failure Frequency
    - all:
        - any:
            - event.device.is_emulator == true
            - api.network_check.is_proxy == true
            - api.network_check.is_tor == true
        - features.login_fail_count_1h >= 3
```

**Important:** 
- Rules use the `event.` prefix to access event fields (unlike feature definitions)
- When accessing calculated features, always use the `features.` namespace prefix:
  - ✅ `features.transaction_sum_7d > 5000` - Correct
  - ❌ `transaction_sum_7d > 5000` - Incorrect (will not work)

### 5.3 Supported Operators (✅ Implemented)

| Operator | Meaning | Example |
|----------|---------|---------|
| `==` | equal | `event.status == "active"` |
| `!=` | not equal | `event.country != "US"` |
| `<, >, <=, >=` | numeric comparison | `event.amount > 1000` |
| `in` | membership in array | `event.country in ["RU", "NG"]` |
| `in list` | membership in custom list | `event.user_id in list.blocked_users` |
| `not in` | not in array | `event.status not in ["blocked", "suspended"]` |
| `not in list` | not in custom list | `event.email not in list.vip_emails` |
| `contains` | string/array contains | `event.email contains "@suspicious.com"`, `event.user.tags contains "vip"` |
| `starts_with` | string starts with | `event.phone starts_with "+1"` |
| `ends_with` | string ends with | `event.email ends_with ".com"` |
| `regex` | regular expression match | `event.id regex "^TX-[0-9]{8}$"` |

**Operator Examples:**
```yaml
event.amount == 100              # Equality
event.amount != 0               # Inequality
event.amount > 1000             # Greater than
event.amount >= 500             # Greater than or equal
event.amount < 100              # Less than
event.amount <= 50              # Less than or equal
event.country in ["US", "CA", "UK"]  # Membership check
event.user_id in list.blocked_users   # Custom list membership
event.status not in ["blocked", "suspended"]  # Not in array
event.email not in list.vip_emails    # Not in custom list
event.user.tags contains "vip"        # String/array contains
features.risk_tags contains "proxy"   # Array contains
```

> **Note:** `exists` and `missing` operators are NOT currently implemented. Check for null/non-null values instead: `event.field == null` or `event.field != null`

### 5.4 Context Variables (✅ Implemented)

Common context variable prefixes for rule conditions:

| Prefix | Purpose | Example |
|--------|---------|---------|
| `event.*` | Event data | `event.amount`, `event.user_id`, `event.type` |
| `features.*` | Computed features | `features.transaction_sum_7d`, `features.login_count_24h` |
| `api.*` | External API results | `api.device_fingerprint.risk_score`, `api.ip_geolocation.country` |
| `service.*` | Internal service results | `service.user_profile.vip_status` |
| `vars.*` | Pipeline variables | `vars.high_risk_threshold` |
| `sys.*` | System variables | `sys.hour`, `sys.timestamp` |
| `results.*` | Ruleset results | `results.fraud_detection.signal` |
| `list.*` | Custom lists | `list.blocked_users`, `list.vip_emails` |

**Note:** In rules, always use the appropriate namespace prefix. For example:
- ✅ `event.amount` - Correct
- ✅ `features.txn_count_24h` - Correct
- ❌ `amount` - Incorrect (missing prefix)

### 5.5 Common Patterns (✅ Implemented)

**Pattern 1: Whitelist Check**
```yaml
when:
  all:
    - event.amount > 1000
    - not:
        - event.user_id in list.vip_users
```

**Pattern 2: Blacklist Check**
```yaml
when:
  any:
    - event.user_id in list.blocked_users
    - event.user.tags contains "fraud"
```

**Pattern 3: Risk Score Range**
```yaml
when:
  all:
    - features.total_score >= 50
    - features.total_score < 80
```

**Pattern 4: Geographic Restriction**
```yaml
when:
  all:
    - event.country not in ["US", "CA", "UK"]
    - event.amount > 1000
```

**Pattern 5: Multiple Risk Indicators**
```yaml
when:
  any:
    - all:
        - event.device.is_emulator == true
        - features.login_fail_count_1h >= 3
    - all:
        - api.network_check.is_proxy == true
        - event.amount > 5000
```

### 5.6 Best Practices

**1. Readability:**
- Use comments to explain complex condition groups
- Group related conditions together
- Use meaningful variable names

**2. Performance:**
- Place cheaper conditions first in `all` clauses
- Place more likely conditions first in `any` clauses
- Avoid deeply nested conditions when possible

**3. Maintainability:**
- Keep conditions focused and specific
- Document the business logic behind complex conditions
- Use consistent naming conventions

**4. Testing:**
- Test each condition branch independently
- Test edge cases (boundary values, null values)
- Test nested conditions at each level

---

## 6. `score`

The numeric score to be added when the rule is triggered.

```yaml
score: +80
```

Scores are typically added together across multiple rules and later aggregated in a pipeline step.
### 6.1 Negative Scores

The `score` field supports both positive and **negative numbers**.  
Negative scores are typically used to lower the total risk score, grant trust credits, or offset other rules that increase risk.

For example:

```yaml
score: -40
```

When this rule is triggered, **40 points will be subtracted** from the total risk score.  
Negative scores are useful for modeling low-risk behavior, whitelist conditions, or strong authentication that should reduce the assessed risk.

**Note:** The aggregate risk score may become negative depending on your scoring logic; it is recommended to validate or normalize the total score as appropriate for your use case.

---

## 7. Complete Examples

```yaml
version: "0.1"

rule:
  id: high_risk_login
  name: High-Risk Login Detection
  description: Detect risky login behavior based on device, location, and history.

  when:
    all:
      - event.type == "login"                       # Correct syntax with ==
      - event.device.is_new == true                 # Use event. prefix
      - event.country in ["RU", "UA", "NG"]         # Correct syntax
      - features.login_failed_count_24h > 3         # Access feature with features. prefix

  score: 80                                          # No need for + prefix
```

---

## 8. Summary

### 8.1 Implemented Features (✅)

A CORINT Rule currently supports:

**Core Fields:**
- ✅ `id` - Unique identifier
- ✅ `name` - Human-readable name
- ✅ `description` - Optional description
- ✅ `when` - Condition logic (all/any/not)
- ✅ `score` - Risk score (supports negative values)
- ✅ `metadata` - Arbitrary metadata

**Condition Logic:**
- ✅ Logical operators: `all` (AND), `any` (OR), `not` (NOT)
- ✅ Nested condition groups
- ✅ Event field access with `event.` prefix
- ✅ Feature access with `features.` prefix
- ✅ Comparison operators: `==`, `!=`, `<`, `>`, `<=`, `>=`
- ✅ Membership operators: `in`, `not in`, `in list`, `not in list`
- ✅ String operators: `contains`, `starts_with`, `ends_with`, `regex`

**Integration:**
- ✅ Forms the basis of reusable Rulesets
- ✅ Integrates seamlessly into Pipelines
- ✅ **Does not define actions** (actions defined in Ruleset)
---

## 8.2. Related Documentation

- `import.md` - Module system and code reuse (NEW)
- `ruleset.md` - Ruleset and decision logic
- `expression.md` - Expression language for conditions
- `feature.md` - Feature engineering for rule conditions
- `../repository/README.md` - Rule library usage guide
