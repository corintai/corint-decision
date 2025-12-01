# CORINT Risk Definition Language (RDL)
## Rule Specification (v0.1)

A **Rule** is the smallest executable logic unit within CORINT’s Cognitive Risk Intelligence framework.  
Rules define deterministic or AI-augmented conditions used to evaluate risk events and generate risk scores or actions.

---

## 1. Rule Structure

```yaml
rule:
  id: string
  name: string
  description: string
  when: <condition-block>
  score: number
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

The core of the rule.  
Describes conditions that must be satisfied for the rule to trigger.

Structure:

```yaml
when:
  <event-filter>
  conditions:
    - <expression>
    - <expression>
    - ...
```

### 5.1 Event Filter

A single key-value pair that determines if the rule applies to a given event type.

```yaml
event.type: login
```

### 5.2 Conditions

A list of logical expressions evaluated **AND** by default.

Example:

```yaml
conditions:
  - geo.country in ["RU", "NG"]
  - device.is_new == true
  - user.login_failed_count > 3
```

#### Supported Operators

| Operator | Meaning |
|----------|---------|
| `==` | equal |
| `!=` | not equal |
| `<, >, <=, >=` | numeric comparison |
| `in` | membership in array/list |
| `regex` | regular-expression match |
| `exists` | field exists |
| `missing` | field is missing |

---

## 6. LLM-Based Conditions

RDL allows rules to incorporate LLM-powered cognitive reasoning.

### 6.1 Text Reasoning

```yaml
- LLM.reason(event) contains "suspicious"
```

### 6.2 Tag-Based Reasoning

```yaml
- LLM.tags contains "device_mismatch"
```

### 6.3 Score-Based Reasoning

```yaml
- LLM.score > 0.7
```

### 6.4 Structured JSON Output

```yaml
- LLM.output.risk_score > 0.3
```

---

## 7. External API Conditions

Rules may reference third-party risk signals:

```yaml
- external_api.Chainalysis.risk_score > 80
```

Other examples:

- Device fingerprint provider  
- IP reputation lookup  
- Email risk scoring  
- Web3 on-chain intelligence  

---

## 8. `score`

The numeric score to be added when the rule is triggered.

```yaml
score: +80
```

Scores are typically added together across multiple rules and later aggregated in a pipeline step.
### 8.1 Negative Scores

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

## 9. Dynamic Thresholds

### 9.1 Overview

Static thresholds may not adapt well to changing patterns. Dynamic thresholds allow rules to automatically adjust based on historical data.

### 9.2 Dynamic Threshold Definition

```yaml
rule:
  id: adaptive_velocity_check
  name: Adaptive Transaction Velocity Check
  description: Detect unusual transaction velocity using adaptive thresholds

  when:
    event.type: transaction
    conditions:
      - event.velocity > dynamic_threshold.value

  # Dynamic threshold configuration
  dynamic_threshold:
    id: user_velocity_threshold

    # Data source
    source:
      metric: user.transaction_velocity
      entity: user.id

    # Calculation method
    method: percentile              # percentile | stddev | moving_average

    percentile:
      value: 95                     # 95th percentile
      window: 30d                   # Historical window
      min_samples: 100              # Minimum data points

    # Bounds
    bounds:
      min: 5                        # Never below 5
      max: 100                      # Never above 100

    # Refresh frequency
    refresh:
      interval: 1h                  # Recalculate hourly
      cache_ttl: 3600               # Cache for 1 hour

    # Fallback if insufficient data
    fallback:
      value: 20
      reason: "Insufficient historical data"

  score: 60
```

### 9.3 Threshold Methods

#### Percentile-Based

```yaml
dynamic_threshold:
  method: percentile
  percentile:
    value: 95                       # Use 95th percentile as threshold
    window: 30d
    min_samples: 50
```

#### Standard Deviation-Based

```yaml
dynamic_threshold:
  method: stddev
  stddev:
    multiplier: 2                   # Mean + 2 * stddev
    window: 30d
    min_samples: 50
```

#### Moving Average-Based

```yaml
dynamic_threshold:
  method: moving_average
  moving_average:
    type: exponential               # simple | exponential | weighted
    window: 7d
    multiplier: 1.5                 # 1.5x the moving average
```

### 9.4 Entity-Specific Thresholds

```yaml
dynamic_threshold:
  # Per-user threshold
  entity_level: user
  entity_key: user.id

  # Global fallback if user has no history
  global_fallback:
    enabled: true
    method: percentile
    percentile: 95
```

### 9.5 Time-Aware Thresholds

```yaml
dynamic_threshold:
  method: percentile

  # Different thresholds for different times
  time_segmentation:
    enabled: true
    segments:
      - name: business_hours
        condition: hour(event.timestamp) >= 9 && hour(event.timestamp) <= 17
        percentile: 95

      - name: off_hours
        condition: hour(event.timestamp) < 9 || hour(event.timestamp) > 17
        percentile: 90               # More sensitive during off-hours

      - name: weekend
        condition: day_of_week(event.timestamp) in ["saturday", "sunday"]
        percentile: 90
```

### 9.6 Complete Dynamic Threshold Example

```yaml
rule:
  id: adaptive_amount_anomaly
  name: Adaptive Transaction Amount Anomaly
  description: Detect unusual transaction amounts based on user history

  when:
    event.type: transaction
    conditions:
      # Amount exceeds user's dynamic threshold
      - event.transaction.amount > dynamic_threshold.high_amount.value

      # Or amount is unusually low (potential testing)
      - any:
          - event.transaction.amount > dynamic_threshold.high_amount.value
          - event.transaction.amount < dynamic_threshold.low_amount.value

  dynamic_thresholds:
    high_amount:
      source:
        metric: transaction.amount
        entity: user.id
        filter: transaction.status == "completed"
      method: percentile
      percentile:
        value: 95
        window: 90d
        min_samples: 10
      bounds:
        min: 100
        max: 1000000
      fallback:
        value: 10000

    low_amount:
      source:
        metric: transaction.amount
        entity: user.id
      method: percentile
      percentile:
        value: 5
        window: 90d
        min_samples: 10
      bounds:
        min: 0.01
        max: 100
      fallback:
        value: 1

  score: 50
```

---

## 10. Rule Dependencies and Conflict Management

### 10.1 Overview

As rule sets grow, managing dependencies and conflicts becomes critical. CORINT provides mechanisms to:
- Define rule execution order
- Specify dependencies between rules
- Detect and handle conflicts
- Set rule priorities

### 10.2 Rule Priority

```yaml
rule:
  id: critical_blocklist_check
  name: Blocklist Check

  # Priority: higher number = higher priority
  # Range: 0-1000, default: 100
  priority: 900

  when:
    conditions:
      - user.id in blocklist

  score: 1000
```

### 10.3 Rule Dependencies

```yaml
rule:
  id: complex_fraud_pattern
  name: Complex Fraud Pattern Detection

  # This rule depends on other rules running first
  depends_on:
    - rule: device_fingerprint_check
      required: true              # Must run before this rule

    - rule: velocity_check
      required: true

    - rule: geo_analysis
      required: false             # Optional dependency

  # Access dependency results in conditions
  when:
    event.type: transaction
    conditions:
      # Use results from dependency rules
      - context.rules.device_fingerprint_check.triggered == true
      - context.rules.velocity_check.score > 30

  score: 80
```

### 10.4 Dependency Graph

```yaml
rule:
  id: rule_c
  depends_on:
    - rule: rule_a
    - rule: rule_b

# Execution order: rule_a → rule_b → rule_c (based on dependencies)
# If rule_a and rule_b have no dependencies, they can run in parallel
```

### 10.5 Conflict Detection

```yaml
rule:
  id: high_value_approved_user
  name: High Value Approved User

  # Declare potential conflicts
  conflicts_with:
    - rule: high_value_new_user
      resolution: priority        # priority | first_match | both
      reason: "Same transaction cannot be both approved and new user"

    - rule: blocked_user_check
      resolution: priority
      reason: "Approved user should not be on blocklist"

  when:
    event.type: transaction
    conditions:
      - event.transaction.amount > 10000
      - user.status == "approved"

  score: -30                      # Reduce risk for approved users
```

### 10.6 Conflict Resolution Strategies

```yaml
conflict_resolution:
  # Global conflict resolution strategy
  default_strategy: priority       # priority | first_match | all | manual

  strategies:
    priority:
      description: "Higher priority rule wins"

    first_match:
      description: "First triggered rule wins"

    all:
      description: "All conflicting rules can trigger"
      score_aggregation: sum

    manual:
      description: "Require manual resolution"
      escalate_to: risk_analyst
```

### 10.7 Rule Groups

```yaml
rule:
  id: geo_risk_high
  name: High Risk Geography

  # Rule group for mutual exclusivity
  group: geo_risk_level
  group_priority: 3               # Highest in group

  when:
    conditions:
      - geo.country in ["NK", "IR", "SY"]
  score: 100

---

rule:
  id: geo_risk_medium
  name: Medium Risk Geography

  group: geo_risk_level
  group_priority: 2

  when:
    conditions:
      - geo.country in ["RU", "CN", "NG"]
  score: 50

---

rule:
  id: geo_risk_low
  name: Low Risk Geography

  group: geo_risk_level
  group_priority: 1               # Lowest in group

  when:
    conditions:
      - geo.country in ["BR", "IN", "MX"]
  score: 20

# Only one rule in group can trigger (highest priority match)
```

### 10.8 Conditional Dependencies

```yaml
rule:
  id: enhanced_verification
  name: Enhanced Verification Required

  depends_on:
    - rule: basic_verification
      required: true

    # Conditional dependency
    - rule: llm_deep_analysis
      required_if: context.rules.basic_verification.score > 50
      timeout: 5000ms
      on_timeout: skip

  when:
    conditions:
      - context.rules.basic_verification.triggered == true
      - context.rules.basic_verification.score > 30

  score: 40
```

### 10.9 Dependency Validation

```yaml
# System validates at compile time:
dependency_validation:
  checks:
    - circular_dependency_detection
    - missing_dependency_detection
    - conflict_consistency_check
    - priority_conflict_detection

  on_error:
    circular_dependency: fail
    missing_dependency: fail
    conflict_inconsistency: warn
    priority_conflict: warn
```

### 10.10 Complete Example with Dependencies

```yaml
version: "0.1"

rule:
  id: sophisticated_fraud_detection
  name: Sophisticated Fraud Detection
  description: Multi-layer fraud detection with dependencies

  # High priority
  priority: 500

  # Dependencies
  depends_on:
    - rule: device_risk_check
      required: true
    - rule: velocity_check
      required: true
    - rule: behavioral_analysis
      required: false
      on_missing: continue

  # Conflicts
  conflicts_with:
    - rule: trusted_user_bypass
      resolution: priority

  # Group
  group: fraud_detection_tier
  group_priority: 3

  when:
    event.type: transaction
    conditions:
      # Use dependency results
      - all:
          - context.rules.device_risk_check.triggered == true
          - context.rules.velocity_check.score >= 40
      - any:
          - context.rules.behavioral_analysis.score > 60
          - event.transaction.amount > 50000

  # Dynamic threshold
  dynamic_threshold:
    source:
      metric: transaction.amount
      entity: user.id
    method: percentile
    percentile:
      value: 99
      window: 90d

  score: 85

  metadata:
    version: "1.2.0"
    author: "fraud-team"
    last_updated: "2024-02-01"
```

---

## 11. Rule Metadata

### 11.1 Metadata Fields

```yaml
rule:
  id: high_risk_login

  metadata:
    # Version tracking
    version: "1.2.0"

    # Ownership
    author: "security-team"
    owner: "risk-ops"

    # Timestamps
    created_at: "2024-01-01T00:00:00Z"
    updated_at: "2024-02-01T10:30:00Z"

    # Documentation
    documentation_url: "https://wiki.company.com/rules/high_risk_login"

    # Tags for organization
    tags:
      - authentication
      - high_priority
      - login

    # Compliance
    compliance:
      - PCI-DSS
      - SOC2

    # Change history
    changelog:
      - version: "1.2.0"
        date: "2024-02-01"
        author: "alice@company.com"
        changes:
          - "Added LLM condition"
          - "Adjusted score from 70 to 80"
      - version: "1.1.0"
        date: "2024-01-15"
        changes:
          - "Added geo condition"
```

---

## 12. Decision Making

**Rules do not define actions.**

Actions are defined at the Ruleset level through `decision_logic`.
Rules only detect risk factors and contribute scores.

See `ruleset.md` for decision-making configuration.

---

## 13. Complete Examples

### 13.1 Login Risk Example

```yaml
version: "0.1"

rule:
  id: high_risk_login
  name: High-Risk Login Detection
  description: Detect risky login behavior using rules + LLM reasoning.

  when:
    event.type: login
    conditions:
      - device.is_new == true
      - geo.country in ["RU", "UA", "NG"]
      - user.login_failed_count > 3
      - LLM.reason(event) contains "suspicious"
      - LLM.score > 0.7

  score: +80
```

---

### 13.2 Loan Application Consistency Rule

```yaml
version: "0.1"

rule:
  id: loan_inconsistency
  name: Loan Application Inconsistency
  description: Detect inconsistencies between declared user info and LLM inference.

  when:
    event.type: loan_application
    conditions:
      - applicant.income < 3000
      - applicant.request_amount > applicant.income * 3
      - LLM.output.employment_stability < 0.3

  score: +120
```

---

## 14. Summary

A CORINT Rule:

- Defines an atomic risk detection  
- Combines structured logic + LLM reasoning  
- Produces a score when triggered
- **Does not define action** (actions defined in Ruleset)
- Supports **dynamic thresholds** for adaptive detection
- Manages **dependencies** and **conflicts** with other rules
- Forms the basis of reusable Rulesets
- Integrates seamlessly into Pipelines

This document establishes the authoritative specification of RDL Rules for CORINT v0.1.

---

## 15. Related Documentation

- `ruleset.md` - Ruleset and decision logic
- `expression.md` - Expression language for conditions
- `feature.md` - Feature engineering for rule conditions
- `versioning.md` - Rule versioning and deployment
- `test.md` - Testing rules
