# CORINT Risk Definition Language (RDL)
## Rule Specification (v0.1)

A **Rule** is the smallest executable logic unit within CORINT's Cognitive Risk Intelligence framework.
Rules define deterministic conditions used to evaluate risk events and generate risk scores.

> **⚠️ Important:** This document clearly marks **✅ Implemented** vs **📋 Planned** features.

---

## 1. Rule Structure

### 1.1 Implemented Fields (✅)

```yaml
rule:
  id: string                 # ✅ Required: Unique identifier
  name: string               # ✅ Required: Human-readable name
  description: string        # ✅ Optional: Rule description
  params:                    # ✅ Parsed (Phase 3): Parameters (runtime substitution not yet implemented)
    <param-key>: <value>
  when: <condition-block>    # ✅ Required: Condition logic
  score: number              # ✅ Required: Risk score (supports negative values)
  metadata:                  # ✅ Optional: Arbitrary metadata
    <key>: <value>
```

### 1.2 Planned Fields (📋 Not Yet Implemented)

The following fields are documented for future implementation but are **NOT** currently supported:

```yaml
rule:
  priority: number           # 📋 NOT IMPLEMENTED
  depends_on: [...]          # 📋 NOT IMPLEMENTED
  conflicts_with: [...]      # 📋 NOT IMPLEMENTED
  group: string              # 📋 NOT IMPLEMENTED
  group_priority: number     # 📋 NOT IMPLEMENTED
  dynamic_threshold: {...}   # 📋 NOT IMPLEMENTED
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
  when:
    all:
    - <expression>
    - <expression>
    - ...
```

### 5.1 Condition Syntax (✅ Implemented)

CORINT supports structured condition logic using `all`, `any`, and `not`:

```yaml
when:
  all:                                      # All conditions must be true (AND)
    - event.type == "login"                 # Event field access
    - event.country in ["RU", "NG"]         # Membership check
    - event.device.is_new == true           # Nested field access
    - features.login_failed_count_24h > 3   # Feature access with features. prefix
```

**Important:** Unlike feature definitions, rules use the `event.` prefix to access event fields.

### 5.2 Logical Operators (✅ Implemented)

**all** - All conditions must be true (AND logic):
```yaml
when:
  all:
    - event.amount > 1000
    - event.country == "US"
    - features.txn_count_24h < 10
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

**Nested logic**:
```yaml
when:
  all:
    - event.type == "transaction"
    - any:
        - event.amount > 10000
        - event.country in ["RU", "CN"]
    - not:
        - event.user_id in list.vip_users
```

**Note**: When accessing calculated features, always use the `features.` namespace prefix:
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
| `contains` | string contains substring | `event.email contains "@suspicious.com"` |
| `starts_with` | string starts with | `event.phone starts_with "+1"` |
| `ends_with` | string ends with | `event.email ends_with ".com"` |
| `regex` | regular expression match | `event.id regex "^TX-[0-9]{8}$"` |

> **Note:** `exists` and `missing` operators are NOT currently implemented. Check for null/non-null values instead: `event.field == null` or `event.field != null`

---

## 6. LLM-Based Conditions (📋 NOT IMPLEMENTED)

> **⚠️ WARNING:** This section documents planned features that are **NOT currently implemented**.
> Do NOT use LLM operators in production rules - they will fail at runtime.

The following LLM-based condition syntax is planned for future releases but not yet available:

### 6.1 Text Reasoning (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED
- LLM.reason(event) contains "suspicious"
```

### 6.2 Tag-Based Reasoning (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED
- LLM.tags contains "device_mismatch"
```

### 6.3 Score-Based Reasoning (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED
- LLM.score > 0.7
```

### 6.4 Structured JSON Output (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED
- LLM.output.risk_score > 0.3
```

**Current Workaround:** Implement LLM reasoning in your application layer before sending events to CORINT.

---

## 7. External API Conditions (📋 NOT IMPLEMENTED)

> **⚠️ WARNING:** This section documents planned features that are **NOT currently implemented**.

The following external API condition syntax is planned but not yet available:

```yaml
# ⚠️ NOT YET IMPLEMENTED
- external_api.Chainalysis.risk_score > 80
- external_api.DeviceFingerprint.is_suspicious == true
```

**Planned integrations:**
- Device fingerprint providers
- IP reputation lookups
- Email risk scoring
- Web3 on-chain intelligence

**Current Workaround:** Fetch external API data in your application layer and include it in the event data sent to CORINT.

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

## 9. Parameterized Rules (`params`) **[Phase 3]**

**Parameterized rules allow you to define default parameter values that can be customized per use case without duplicating rule logic.**

This enables:
- **Reusable rule templates** - Define rules once with configurable parameters
- **Easy customization** - Override parameter values per deployment
- **Reduced duplication** - Same rule logic, different thresholds
- **Type-safe configuration** - Parameters validated at compile-time

### 9.1 Basic Syntax

```yaml
rule:
  id: velocity_pattern
  name: Velocity Pattern Detection
  description: Detects velocity abuse with configurable thresholds

  # Default parameter values
  params:
    time_window_minutes: 60
    max_transactions: 10
    max_amount: 5000
    severity: "medium"

  when:
    all:
      - event.type == "transaction"
      # Reference params in conditions (future support)
      - transaction_count_last_hour > 10
      - total_amount_last_hour > 5000

  score: 60
```

### 9.2 Parameter Types

Parameters support JSON-compatible types:

```yaml
params:
  # Numbers
  threshold: 100
  multiplier: 1.5

  # Strings
  severity: "high"
  category: "fraud"

  # Booleans
  enabled: true
  strict_mode: false

  # Arrays
  allowed_countries: ["US", "CA", "UK"]
  risk_levels: [1, 2, 3]

  # Objects
  thresholds:
    low: 30
    medium: 60
    high: 100
```

### 9.3 Use Cases

**1. Regional Variations**

Different thresholds for different regions:

```yaml
# Base rule with params
rule:
  id: transaction_velocity_check
  params:
    max_transactions_per_hour: 10
    max_amount_per_hour: 5000
  when:
    # Conditions using thresholds
```

Deploy with region-specific overrides:
- US deployment: `max_transactions_per_hour: 15`
- EU deployment: `max_transactions_per_hour: 10` (stricter)
- APAC deployment: `max_transactions_per_hour: 20` (more lenient)

**2. A/B Testing**

Test different thresholds without changing rule logic:

```yaml
rule:
  id: fraud_score_threshold
  params:
    deny_threshold: 100
    review_threshold: 60
  when:
    # Rule logic
```

Run A/B tests:
- Control group: Default parameters
- Test group A: `deny_threshold: 120` (more lenient)
- Test group B: `deny_threshold: 80` (stricter)

**3. Environment-Specific Configuration**

Different settings for dev/staging/production:

```yaml
rule:
  id: rate_limit_check
  params:
    rate_limit: 100
    burst_size: 10
```

Override per environment:
- Development: `rate_limit: 1000` (relaxed for testing)
- Staging: `rate_limit: 100` (production-like)
- Production: `rate_limit: 50` (strict)

### 9.4 Status and Future Work

**Current Status (Phase 3):**
- ✅ AST support for params field
- ✅ Parser support for reading params from YAML
- ✅ Parameters stored in rule definition

**Future Implementation:**
- ⏳ Parameter substitution in rule conditions
- ⏳ Runtime parameter override mechanism
- ⏳ Parameter validation and type checking
- ⏳ Parameter inheritance in rule templates

**Note:** While the params field is fully parsed and stored in Phase 3, runtime parameter substitution and rule instantiation will be implemented in future phases when specific use cases require it.

### 9.5 Best Practices

1. **Use Descriptive Names**
   ```yaml
   # Good
   params:
     max_transactions_per_hour: 10

   # Avoid
   params:
     limit: 10
   ```

2. **Provide Sensible Defaults**
   ```yaml
   params:
     threshold: 100  # Good default for most cases
   ```

3. **Document Parameters**
   ```yaml
   rule:
     id: velocity_check
     description: |
       Velocity check with configurable thresholds.
       Parameters:
       - time_window_minutes: Time window for counting (default: 60)
       - max_transactions: Maximum transactions allowed (default: 10)
     params:
       time_window_minutes: 60
       max_transactions: 10
   ```

4. **Group Related Parameters**
   ```yaml
   params:
     thresholds:
       low: 30
       medium: 60
       high: 100
     limits:
       daily: 1000
       hourly: 100
   ```

---

## 10. Dynamic Thresholds (📋 NOT IMPLEMENTED)

> **⚠️ WARNING:** This entire section documents planned features that are **NOT currently implemented**.
> The `dynamic_threshold` field does NOT exist in the Rule struct and will cause parse errors.

### 10.1 Overview

Static thresholds may not adapt well to changing patterns. Dynamic thresholds (when implemented) will allow rules to automatically adjust based on historical data.

### 10.2 Dynamic Threshold Definition (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED - This will cause parse errors
rule:
  id: adaptive_velocity_check
  name: Adaptive Transaction Velocity Check
  description: Detect unusual transaction velocity using adaptive thresholds

  when:
    all:
      - event.type == "transaction"
      - event.velocity > dynamic_threshold.value  # NOT SUPPORTED

  # Dynamic threshold configuration (NOT SUPPORTED)
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
    all:
      - event.type == "transaction"
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

**Current Workaround:** Use feature engineering to compute percentile-based thresholds as features, then reference them in rules:
```yaml
# Compute threshold as a feature
- name: user_txn_p95
  type: aggregation
  method: percentile
  percentile: 95
  # ...

# Use in rule condition
rule:
  when:
    all:
      - event.amount > features.user_txn_p95
```

---

## 11. Rule Dependencies and Conflict Management (📋 NOT IMPLEMENTED)

> **⚠️ WARNING:** This entire section documents planned features that are **NOT currently implemented**.
> Fields like `priority`, `depends_on`, `conflicts_with`, and `group` do NOT exist in the Rule struct.

### 10.1 Overview (Planned)

When implemented, CORINT will provide mechanisms to:
- Define rule execution order
- Specify dependencies between rules
- Detect and handle conflicts
- Set rule priorities

### 10.2 Rule Priority (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED - priority field does not exist
rule:
  id: critical_blocklist_check
  name: Blocklist Check

  # Priority: higher number = higher priority (NOT SUPPORTED)
  priority: 900  # WILL CAUSE PARSE ERROR OR BE IGNORED

  when:
    all:
      - event.user_id in list.blocklist  # Use "in list" operator instead

  score: 1000
```

### 10.3 Rule Dependencies (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED - depends_on field does not exist
rule:
  id: complex_fraud_pattern
  name: Complex Fraud Pattern Detection

  # This rule depends on other rules running first (NOT SUPPORTED)
  depends_on:                            # WILL CAUSE PARSE ERROR OR BE IGNORED
    - rule: device_fingerprint_check
      required: true

  # Access dependency results in conditions (NOT SUPPORTED)
  when:
    all:
      - event.type == "transaction"
      # context.rules.* does NOT exist
      - context.rules.device_fingerprint_check.triggered == true  # NOT SUPPORTED

  score: 80
```

**Current Workaround:** Rules are currently evaluated independently. Rulesets aggregate all triggered rules' scores.

### 10.4 Dependency Graph (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED
rule:
  id: rule_c
  depends_on:                    # NOT SUPPORTED
    - rule: rule_a
    - rule: rule_b
```

### 10.5 Conflict Detection (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED
rule:
  id: high_value_approved_user
  name: High Value Approved User

  # Declare potential conflicts (NOT SUPPORTED)
  conflicts_with:                # NOT SUPPORTED
    - rule: high_value_new_user
      resolution: priority        # priority | first_match | both
      reason: "Same transaction cannot be both approved and new user"

    - rule: blocked_user_check
      resolution: priority
      reason: "Approved user should not be on blocklist"

  when:
    all:
      - event.type == "transaction"
      - event.transaction.amount > 10000
      - user.status == "approved"

  score: -30                      # Reduce risk for approved users
```

### 10.6 Conflict Resolution Strategies (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED
conflict_resolution:              # NOT SUPPORTED
  default_strategy: priority
  # ...
```

### 10.7 Rule Groups (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED - group fields do not exist
rule:
  id: geo_risk_high
  name: High Risk Geography

  # Rule group for mutual exclusivity (NOT SUPPORTED)
  group: geo_risk_level           # NOT SUPPORTED
  group_priority: 3               # NOT SUPPORTED

  when:
    all:
      - event.country in ["NK", "IR", "SY"]  # Use event. prefix
  score: 100

---

rule:
  id: geo_risk_medium
  name: Medium Risk Geography

  group: geo_risk_level           # NOT SUPPORTED
  group_priority: 2               # NOT SUPPORTED

  when:
    all:
      - event.country in ["RU", "CN", "NG"]  # Correct syntax
  score: 50

---

rule:
  id: geo_risk_low
  name: Low Risk Geography

  group: geo_risk_level           # NOT SUPPORTED
  group_priority: 1               # NOT SUPPORTED

  when:
    all:
      - event.country in ["BR", "IN", "MX"]  # Correct syntax
  score: 20
```

### 10.8 Conditional Dependencies (📋 Planned)

```yaml
# ⚠️ NOT YET IMPLEMENTED
rule:
  id: enhanced_verification
  name: Enhanced Verification Required

  depends_on:                     # NOT SUPPORTED
    - rule: basic_verification
      required: true

    # Conditional dependency
    - rule: llm_deep_analysis
      required_if: context.rules.basic_verification.score > 50
      timeout: 5000ms
      on_timeout: skip

  when:
    all:
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
    all:
      - event.type == "transaction"
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
    updated: "2024-02-01"
```

---

## 12. Rule Metadata

### 11.1 Metadata Fields

```yaml
rule:
  id: high_risk_login

  metadata:
    # === Recommended Fields ===
    version: "1.2.0"
    author: "security-team"
    updated: "2024-02-01"

    # === Custom Fields ===
    # Ownership
    owner: "risk-ops"

    # Timestamps
    created: "2024-01-01"

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

## 13. Decision Making

**Rules do not define actions.**

Actions are defined at the Ruleset level through `decision_logic`.
Rules only detect risk factors and contribute scores.

See `ruleset.md` for decision-making configuration.

---

## 14. Complete Examples

### 13.1 Login Risk Example (✅ Correct Syntax)

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

> **Note:** The original example showed LLM conditions which are NOT implemented. Use feature engineering instead.

---

### 13.2 Transaction Risk Example (✅ Correct Syntax)

```yaml
version: "0.1"

rule:
  id: high_value_new_user
  name: High Value Transaction from New User
  description: Detect high-value transactions from users with limited history.

  when:
    all:
      - event.type == "transaction"
      - event.amount > 5000
      - features.user_total_transactions < 5        # Low transaction count

  score: 120
```

---

## 15. Summary

### 15.1 Implemented Features (✅)

A CORINT Rule currently supports:

**Core Fields:**
- ✅ `id` - Unique identifier
- ✅ `name` - Human-readable name
- ✅ `description` - Optional description
- ✅ `when` - Condition logic (all/any/not)
- ✅ `score` - Risk score (supports negative values)
- ✅ `metadata` - Arbitrary metadata
- ✅ `params` - Parameters (parsed, runtime substitution not yet implemented)

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

### 15.2 Planned Features (📋)

The following are documented for future implementation:

**NOT YET IMPLEMENTED:**
- 📋 LLM-based conditions (`LLM.reason()`, `LLM.score`, etc.)
- 📋 External API conditions (`external_api.*`)
- 📋 Dynamic thresholds (`dynamic_threshold` field)
- 📋 Rule dependencies (`depends_on` field)
- 📋 Rule priorities (`priority` field)
- 📋 Conflict management (`conflicts_with` field)
- 📋 Rule groups (`group`, `group_priority` fields)
- 📋 Parameter substitution in conditions (params are parsed but not substituted)
- 📋 `exists` and `missing` operators

### 15.3 Current Capabilities

**What you can do today:**
- Define deterministic risk detection rules
- Access event fields and computed features
- Use complex boolean logic with nesting
- Assign positive or negative risk scores
- Organize rules into rulesets
- Add metadata for governance

**What requires workarounds:**
- LLM reasoning → Implement in application layer before sending events
- External APIs → Fetch data and include in event payload
- Dynamic thresholds → Use percentile features with feature engineering
- Rule dependencies → Use ruleset-level score aggregation

This document establishes the authoritative specification of RDL Rules for CORINT v0.1.

---

## 15. Rule Library and Reusability

### 15.1 Creating Reusable Rule Files

Rules can be defined in separate files for reuse across multiple rulesets and pipelines:

```yaml
# library/rules/fraud/fraud_farm.yaml
version: "0.1"

rule:
  id: fraud_farm_pattern
  name: Fraud Farm Detection
  description: Detect organized fraud farms with high IP/device association

  when:
    all:
      - ip_device_count > 10
      - ip_user_count > 5

  score: 100

  metadata:
    version: "1.0.0"
    author: "fraud-team"
    updated: "2024-12-11"
    category: fraud
    severity: critical
    tags: [organized_fraud, bot_networks]
```

### 15.2 Rule Metadata for Library

When creating rules for a library, include comprehensive metadata:

```yaml
metadata:
  # === Recommended Fields ===
  version: "1.0.0"              # Semantic versioning (MAJOR.MINOR.PATCH)
  author: "Team Name"
  updated: "2024-12-11"

  # === Custom Fields (Recommended for Library Rules) ===
  category: fraud | payment | geography | account | device
  severity: critical | high | medium | low
  tags: [tag1, tag2, tag3]
  description_detail: "Detailed explanation"
  detection: "Condition summary"
  features:
    - feature_name: "Feature description"
    - another_feature: "Another description"
  common_in: [attack_type1, attack_type2]
  changelog:
    - version: "1.0.0"
      date: "2024-12-11"
      changes: "Initial version"
```

### 15.3 ID Naming Conventions

**Rule ID Format**: `<category>_<specific_pattern>`

| Category | Prefix | Example |
|----------|--------|---------|
| Fraud | `fraud_` | `fraud_farm_pattern`, `fraud_velocity_abuse` |
| Payment | `payment_` | `payment_card_testing`, `payment_high_value` |
| Geography | `geo_` | `geo_suspicious_country`, `geo_impossible_travel` |
| Account | `account_` | `account_takeover_pattern`, `account_new_user_risk` |
| Device | `device_` | `device_fingerprint_mismatch` |

**Naming Principles**:
- ✅ Use `snake_case` (lowercase + underscores)
- ✅ Include category prefix to avoid conflicts
- ✅ Be descriptive and clear
- ✅ Use `_pattern` or `_check` suffix
- ❌ Avoid generic names like `rule1`, `check`, `test`
- ❌ Keep under 50 characters

### 15.4 Using Rules from Library

Rules are imported and referenced by rulesets (not by pipelines directly):

```yaml
# library/rulesets/fraud_detection_core.yaml
version: "0.1"

import:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/account_takeover.yaml

---

ruleset:
  id: fraud_detection_core
  rules:
    - fraud_farm_pattern          # Reference by ID
    - account_takeover_pattern    # Reference by ID
```

### 15.5 Rule Testing

Create test files alongside rule definitions:

```yaml
# library/rules/fraud/fraud_farm.test.yaml
tests:
  - name: "Fraud farm detected - high device count"
    input:
      ip_device_count: 15
      ip_user_count: 8
    expected:
      triggered: true
      score: 100

  - name: "Normal traffic - below threshold"
    input:
      ip_device_count: 2
      ip_user_count: 1
    expected:
      triggered: false
      score: 0

  - name: "Edge case - only device count high"
    input:
      ip_device_count: 15
      ip_user_count: 2
    expected:
      triggered: false
      score: 0
```

### 15.6 Benefits of Rule Libraries

✅ **Reusability**: Define once, use in multiple rulesets
✅ **Consistency**: Same logic across all pipelines
✅ **Maintainability**: Update in one place
✅ **Testability**: Test rules independently
✅ **Collaboration**: Team members work on separate rules
✅ **Versioning**: Track changes with metadata

(See `import.md` for complete module system specification.)

---

## 16. Related Documentation

- `import.md` - Module system and code reuse (NEW)
- `ruleset.md` - Ruleset and decision logic
- `expression.md` - Expression language for conditions
- `feature.md` - Feature engineering for rule conditions
- `versioning.md` - Rule versioning and deployment
- `../repository/README.md` - Rule library usage guide
