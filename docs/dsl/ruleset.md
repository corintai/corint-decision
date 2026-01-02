# CORINT Risk Definition Language (RDL)
## Ruleset Specification (v0.2)

A **Ruleset** is a named collection of rules that can be reused, grouped, and executed as a unit within CORINT's Cognitive Risk Intelligence framework.
Rulesets enable modular design, separation of concerns, and cleaner pipeline logic.

---

## 1. Ruleset Structure

```yaml
ruleset:
  id: string
  name: string
  description: string
  extends: string                    # ✅ Parent ruleset ID (Implemented)
  rules:
    - <rule-id-1>
    - <rule-id-2>
    - <rule-id-3>
  conclusion:                        # Decision logic (produces approve/decline/review/hold/pass signals)
    - <conclusion-rules>
  metadata:                         # Optional metadata
    version: string
    author: string
```

**Key Terminology:**
- `conclusion` - Ruleset's decision logic that evaluates rule results and produces a signal
- `signal` - The decision output: `approve`, `decline`, `review`, `hold`, or `pass`
- `when` - Condition expression for each conclusion rule (uses comparison and logical operators)

---

## 2. `id`

A globally unique identifier for the ruleset.

Example:

```yaml
id: login_risk_rules
```

---

## 3. `name`

Human-readable name for the ruleset.

```yaml
name: Account Takeover Detection
```

---

## 4. `description`

A short explanation of the ruleset's purpose.

```yaml
description: Detect account takeover through multi-signal pattern analysis
```

---

## 5. `rules`

An ordered list of rule identifiers that belong to this ruleset.

The rules referenced here must exist in the system, typically defined in separate RDL rule files.

Example:

```yaml
rules:
  - high_risk_login
  - device_mismatch
  - ip_reputation_flag
```

Rules are executed **in the given order**.

---

## 5.5 Ruleset Inheritance (`extends`) (✅ Implemented)

**Ruleset inheritance allows child rulesets to extend parent rulesets, inheriting their rules and optionally overriding decision logic.**

This enables:
- **Code reuse** - Define common rules once in a base ruleset
- **Consistent baselines** - Maintain standard rule sets across variants
- **Easy customization** - Override decision thresholds per use case
- **Reduced duplication** - Eliminate redundant rule import

### 5.5.1 Basic Syntax

```yaml
version: "0.1"

import:
  rulesets:
    - library/rulesets/payment_base.yaml  # Import parent ruleset

---

ruleset:
  id: payment_high_value
  name: High-Value Payment Ruleset
  extends: payment_base  # ✨ Inherit from parent

  # Add additional rules on top of inherited ones
  rules:
    - amount_outlier

  # Override conclusion logic with stricter thresholds
  conclusion:
    - when: total_score >= 60
      signal: decline
      reason: "Risk score too high for large transaction"
```

### 5.5.2 Inheritance Behavior

When a ruleset extends a parent:

| Field | Behavior | Description |
|-------|----------|-------------|
| **`rules`** | **Merge + Auto-dedup** | Parent rules + child rules, duplicates automatically removed |
| **`conclusion`** | **Complete override** | Child replaces parent if defined, otherwise inherits parent's logic |
| **`name`** | **Override** | Child overrides if defined, otherwise inherits parent's name |
| **`description`** | **Override** | Child overrides if defined, otherwise inherits parent's description |
| **`metadata`** | **Override** | Child overrides if defined, otherwise inherits parent's metadata |

### 5.5.3 Creating Variants

Create multiple variants from the same base with different thresholds:

```yaml
# Stricter thresholds
ruleset:
  id: payment_high_value
  extends: payment_base
  conclusion:
    - when: total_score >= 60  # Stricter than standard (100)
      signal: decline

# More lenient thresholds
ruleset:
  id: payment_vip
  extends: payment_base
  conclusion:
    - when: total_score >= 150  # More lenient
      signal: decline
```

### 5.5.4 Error Detection

| Error Type | Example | Error Message |
|------------|---------|---------------|
| **Parent Not Found** | `extends: nonexistent_parent` | `ExtendsNotFound { child_id: "child", extends_id: "nonexistent_parent" }` |
| **Circular Inheritance** | `a extends b`, `b extends a` | `CircularExtends { child_id: "b", extends_id: "a" }` |

---

## 5.6 Ruleset Import and Dependencies

Rulesets use the `import` section to declare their rule dependencies explicitly. This enables modular design, compile-time dependency resolution, and clear dependency tracking. Rule IDs must be globally unique across the entire system.

### 5.6.1 Basic Import Syntax

Rulesets use multi-document YAML format with `---` separator:

```yaml
version: "0.2"

# First document: Import
import:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/account_takeover.yaml
    - library/rules/geography/suspicious_ip.yaml

---

# Second document: Ruleset definition
ruleset:
  id: fraud_detection_core
  name: Core Fraud Detection Ruleset
  description: Reusable fraud detection logic for transaction events

  rules:
    - fraud_farm_pattern
    - account_takeover_pattern
    - suspicious_ip_pattern

  conclusion:
    - when: total_score >= 100
      signal: decline
      reason: "High risk score"

    - when: total_score >= 50
      signal: review
      reason: "Medium risk - needs review"

    - default: true
      signal: approve
      reason: "No significant risk"
```

### 5.6.2 Import Best Practices

**Path Resolution:**
Import paths are resolved relative to the repository root:

```
repository/
├── library/
│   ├── rules/
│   │   └── fraud/
│   │       └── fraud_farm.yaml
│   └── rulesets/
│       └── fraud_detection_core.yaml  ← You are here
```

From `fraud_detection_core.yaml`:
```yaml
import:
  rules:
    - library/rules/fraud/fraud_farm.yaml  # ✅ Correct
```

**ID Naming Conventions:**
Ruleset IDs should follow: `<domain>_<purpose>_<variant?>`

Examples:
- `fraud_detection_core`
- `payment_standard`
- `payment_high_value`
- `account_takeover_detection`

**Metadata:**
Optional metadata for tracking and governance:

```yaml
ruleset:
  id: fraud_detection_core
  # ... rules and conclusion ...
  metadata:
    version: "1.0.0"
    last_updated: "2024-12-11"
    owner: "risk_team"
    tags:
      - fraud_detection
      - production_ready
```

### 5.6.3 Dependency Propagation

When a pipeline imports a ruleset, it automatically gets all the ruleset's rule dependencies:

```yaml
import:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml  # Automatically includes all rules

---

pipeline:
  id: fraud_detection_pipeline
  steps:
    - include:
        ruleset: fraud_detection_core  # All rules already loaded
```

---

## 6. `conclusion` (Direct Definition)

Conclusion logic evaluates the combined results of all rules and produces a decision signal.

### 6.1 Basic Structure and Sequential Execution

```yaml
conclusion:
  - when: <expression>
    signal: <signal-type>
    reason: <string>

  - when: <expression>
    signal: <signal-type>

  - default: true
    signal: <signal-type>
```

**Important: Sequential Execution and Short-Circuit Logic**

Conclusion rules are evaluated sequentially from top to bottom:

1. **Sequential Execution**: The system evaluates each `when` condition in order
2. **Short-Circuit Logic**: **When the first `when` condition is satisfied, the corresponding `signal` is executed immediately, and all subsequent rules are skipped.**
3. **Default Rule**: The `default: true` rule is only executed when all preceding `when` conditions are not satisfied

This means the order of conclusion rules is crucial! They should be arranged from **most specific to most general**.

**Example:**

```yaml
conclusion:
  # Rule 1: If total_score >= 150, execute decline, then stop
  - when: total_score >= 150
    signal: decline
    reason: "Critical risk score"

  # Rule 2: Only checked if rule 1 is not satisfied
  - when: total_score >= 100
    signal: decline
    reason: "High risk, needs blocking"

  # Rule 3: Only checked if rules 1 and 2 are not satisfied
  - when: total_score >= 50
    signal: review
    reason: "Medium risk, manual review"

  # Default rule: Only executed when all preceding rules are not satisfied
  - default: true
    signal: approve
    reason: "Low risk, approved"
```

**Execution Flow Examples:**

- If `total_score = 200`: Rule 1 satisfied → `decline` → **stop** (rules 2, 3, default skipped)
- If `total_score = 120`: Rule 1 not satisfied, Rule 2 satisfied → `decline` → **stop**
- If `total_score = 75`: Rules 1, 2 not satisfied, Rule 3 satisfied → `review` → **stop**
- If `total_score = 30`: Rules 1, 2, 3 not satisfied → execute default → `approve`

### 6.2 Available Context

Within conclusion conditions, you can access:

- `total_score` - Sum of all triggered rule scores
- `triggered_count` - Number of rules that triggered
- `triggered_rules` - Array of triggered rule IDs


### 6.3 Available Signals (✅ Implemented)

| Signal | Description | Use Case |
|--------|-------------|----------|
| `approve` | Approve the request | Transaction passes all checks |
| `decline` | Decline/block the request | Critical risk detected, deny transaction |
| `review` | Send for manual review | Needs human evaluation |
| `hold` | Temporarily suspend | Waiting for additional verification (2FA, KYC, etc.) |
| `pass` | Skip/no decision | Let downstream systems handle decision |

**Note:** These are the final decision signals from rulesets. Pipelines can route based on these signals using `results.<ruleset_id>.signal`.

---

## 7. Common Conclusion Patterns

### 7.1 Score-Based Conclusions

Decision signals based on total score:

```yaml
conclusion:
  # Very high risk - decline
  - when: total_score >= 150
    signal: decline
    reason: "Critical risk score"

  # High risk - decline
  - when: total_score >= 100
    signal: decline
    reason: "High risk, needs blocking"

  # Medium risk - review
  - when: total_score >= 50
    signal: review
    reason: "Medium risk, manual review"

  # Low risk - approve
  - default: true
    signal: approve
    reason: "Low risk, approved"
```

### 7.2 Count-Based Conclusions

```yaml
conclusion:
  - when: triggered_count >= 3
    signal: decline
    reason: "Multiple risk indicators"

  - when: triggered_count >= 2
    signal: review
    reason: "Multiple signals, needs analysis"

  - default: true
    signal: approve
```

### 7.3 Rule Combinations

```yaml
conclusion:
  # Critical single rule - decline immediately
  - when: triggered_rules contains "blocked_user"
    signal: decline
    reason: "User is blocked"

  # Specific rule combination - high risk pattern
  - when:
      all:
        - triggered_rules contains "new_device"
        - triggered_rules contains "unusual_location"
    signal: review
    reason: "Device and location anomaly"

  # Fallback to score-based logic
  - when: total_score >= 80
    signal: review

  - default: true
    signal: approve
```

### 7.4 Context-Aware Conclusions

```yaml
conclusion:
  # Consider user tier
  - when:
      all:
        - total_score >= 60
        - event.user.tier == "basic"
    signal: decline
    reason: "Medium risk for basic user"

  # Consider transaction amount
  - when:
      all:
        - total_score >= 50
        - event.transaction.amount > 10000
    signal: review
    reason: "Medium risk + high value"

  - default: true
    signal: approve
```

---

## 8. Complete Examples

### 8.1 Account Takeover Detection

```yaml
version: "0.2"

ruleset:
  id: account_takeover_detection
  name: Account Takeover Detection
  description: Multi-signal account takeover pattern detection

  rules:
    - new_device_login
    - unusual_location
    - failed_login_spike
    - behavior_anomaly
    - password_change_attempt

  conclusion:
    # Critical combination pattern
    - when:
        all:
          - triggered_rules contains "password_change_attempt"
          - triggered_rules contains "new_device_login"
          - triggered_rules contains "unusual_location"
      signal: decline
      reason: "Critical takeover indicators"

    # Multiple signals
    - when: triggered_count >= 3
      signal: review
      reason: "Multiple suspicious indicators"

    # High score threshold
    - when: total_score >= 100
      signal: review
      reason: "High risk score"

    - default: true
      signal: approve
```

---

## 9. Best Practices

### 9.1 Conclusion Logic Order

Order conclusion rules from most specific to most general (see section 6.1 for detailed explanation):

```yaml
conclusion:
  - when: critical_condition          # 1. Critical rules first
    signal: decline
  - when: specific_pattern            # 2. Specific combinations
    signal: review
  - when: total_score >= 80           # 3. Score/count thresholds
    signal: review
  - default: true                     # 4. Default fallback
    signal: approve
```

### 9.2 Use Meaningful Reasons

Always provide clear reasons for audit and explainability:

```yaml
conclusion:
  - when: total_score >= 100
    signal: decline
    reason: "Risk score {total_score} exceeds threshold"  # ✅ Specific

  - when: triggered_count >= 3
    signal: review
    reason: "Multiple risk indicators: {triggered_rules}"  # ✅ Detailed
```

### 9.3 Consider Business Context

Adapt signals based on business context (see section 7.4 for examples):

```yaml
conclusion:
  # Different thresholds for different user tiers
  - when:
      all:
        - event.user.tier == "premium"
        - total_score < 80
    signal: approve

  - when:
      all:
        - event.user.tier == "basic"
        - total_score < 50
    signal: approve
```

---

## 10. Related Documentation

For comprehensive understanding of rulesets and the CORINT ecosystem:

### Core Concepts
- **[import.md](import.md)** - Complete module system and dependency management specification
- **[rule.md](rule.md)** - Individual rule specification and rule library creation
- **[pipeline.md](pipeline.md)** - Pipeline orchestration that uses rulesets

### Advanced Topics
- **[expression.md](expression.md)** - Expression language for conditions
- **[context.md](context.md)** - Context management and variable access
- **[feature.md](feature.md)** - Feature engineering for rule conditions

### Architecture
- **[overall.md](overall.md)** - High-level RDL overview
- **[ARCHITECTURE.md](../ARCHITECTURE.md)** - Three-layer decision architecture

---

## 11. Summary

A CORINT Ruleset groups multiple rules into a reusable logical unit that:
- Evaluates rule results and **produces decision signals** (`approve`, `decline`, `review`, `hold`, `pass`)
- Supports **inheritance** via `extends` for code reuse (✅ Implemented)
- Uses **import** to declare rule dependencies explicitly
- Integrates with CORINT Pipelines

**Three-Layer Model:**
- **Rules** detect and score individual risk patterns
- **Rulesets** evaluate rule results and produce decision signals
- **Pipelines** route events to rulesets and use ruleset results

Rulesets are the signal-producing layer of CORINT's Risk Definition Language (RDL).
