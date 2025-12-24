# CORINT Risk Definition Language (RDL)
## Ruleset Specification (v0.2)

A **Ruleset** is a named collection of rules that can be reused, grouped, and executed as a unit within CORINT's Cognitive Risk Intelligence framework.
Rulesets enable modular design, separation of concerns, and cleaner pipeline logic.

**Important:** Rulesets produce **conclusions** (signals), NOT final decisions. Final decisions are made at the **Pipeline level** in the `decision:` section. This separation allows:
- **Reusability**: Same ruleset can be used with different decision thresholds
- **Flexibility**: Pipeline can combine signals from multiple rulesets
- **Clarity**: Clear separation between detection/conclusion and final action

---

## 1. Ruleset Structure

```yaml
ruleset:
  id: string
  name: string
  description: string
  extends: string                    # ✨ Parent ruleset ID (Phase 3)
  rules:
    - <rule-id-1>
    - <rule-id-2>
    - <rule-id-3>
  conclusion:                        # Conclusion logic (produces signals, NOT final decisions)
    - <conclusion-rules>
  metadata:                         # Optional metadata
    version: string
    owner: string
```

**Key Terminology:**
- `conclusion` - Ruleset's assessment logic that produces **signals** (not final decisions)
- `signal` - The output type (e.g., `high_risk`, `suspicious`, `normal`)
- `when` - Condition for each conclusion rule
- Final `decision` (approve/decline/review/hold/pass) is made at **Pipeline level**

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

## 5.5 Ruleset Inheritance (`extends`) **[Phase 3]**

**Ruleset inheritance allows child rulesets to extend parent rulesets, inheriting their rules and optionally overriding decision logic.**

This enables:
- **Code reuse** - Define common rules once in a base ruleset
- **Consistent baselines** - Maintain standard rule sets across variants
- **Easy customization** - Override decision thresholds per use case
- **Reduced duplication** - Eliminate redundant rule imports

### 5.5.1 Basic Inheritance Syntax

```yaml
version: "0.1"

imports:
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
      signal: high_risk
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

### 5.5.3 Complete Example: Payment Risk Hierarchy

**Parent Ruleset** (`payment_base.yaml`):
```yaml
version: "0.2"

imports:
  rules:
    - library/rules/geography/suspicious_ip.yaml
    - library/rules/payment/card_testing.yaml
    - library/rules/payment/velocity_check.yaml
    - library/rules/payment/new_account_risk.yaml
    - library/rules/payment/suspicious_email.yaml

---

ruleset:
  id: payment_base
  name: Base Payment Risk Ruleset
  description: Common payment risk rules for all transaction types

  rules:
    - suspicious_ip
    - card_testing
    - velocity_check
    - new_account_risk
    - suspicious_email

  # Conclusion produces signals, NOT final decisions
  conclusion:
    - when: triggered_rules contains "card_testing"
      signal: critical_risk
      reason: "Card testing detected"
      terminate: true

    - when: total_score >= 100
      signal: high_risk
      reason: "High risk score"

    - when: total_score >= 60
      signal: medium_risk
      reason: "Medium risk - requires review"

    - default: true
      signal: low_risk
```

**Child Ruleset** (`payment_high_value.yaml`):
```yaml
version: "0.2"

imports:
  rulesets:
    - library/rulesets/payment_base.yaml  # Import parent
  rules:
    - library/rules/fraud/amount_outlier.yaml  # Additional rule

---

ruleset:
  id: payment_high_value
  name: High-Value Payment Risk Ruleset
  description: Stricter thresholds for high-value transactions (> $1000)
  extends: payment_base  # ✨ Inherit from parent

  # Add one more rule on top of inherited 5 rules
  rules:
    - amount_outlier

  # Override with stricter conclusion logic (produces signals, not final decisions)
  conclusion:
    - when: triggered_rules contains "card_testing"
      signal: critical_risk
      reason: "Card testing detected"
      terminate: true

    - when: total_score >= 60  # Stricter than parent (was 100)
      signal: high_risk
      reason: "Risk score too high for large transaction"

    - when: triggered_count >= 2
      signal: medium_risk
      reason: "Multiple risk indicators"

    - default: true
      signal: low_risk
```

**Result After Inheritance Resolution**:
- **Total rules**: 6 (5 from parent + 1 new)
- **Conclusion logic**: Uses child's stricter thresholds
- **Zero duplication**: Parent rules defined once, inherited automatically
- **Signal output**: `critical_risk`, `high_risk`, `medium_risk`, or `low_risk`
- **Final decision**: Made at Pipeline level based on signal

### 5.5.4 Multiple Inheritance Variants

Create multiple variants of the same base ruleset with different thresholds:

```yaml
# payment_standard.yaml - Standard thresholds
ruleset:
  id: payment_standard
  extends: payment_base
  conclusion:
    - when: total_score >= 100
      signal: high_risk

# payment_high_value.yaml - Strict thresholds
ruleset:
  id: payment_high_value
  extends: payment_base
  rules:
    - amount_outlier  # Add extra rule
  conclusion:
    - when: total_score >= 60  # Stricter
      signal: high_risk

# payment_vip.yaml - Lenient thresholds
ruleset:
  id: payment_vip
  extends: payment_base
  conclusion:
    - when: total_score >= 150  # More lenient
      signal: high_risk
```

### 5.5.5 Error Detection

The compiler validates inheritance chains and reports errors:

**Parent Not Found:**
```yaml
ruleset:
  id: child
  extends: nonexistent_parent  # ❌ Error
```
Error: `ExtendsNotFound { child_id: "child", extends_id: "nonexistent_parent" }`

**Circular Inheritance:**
```yaml
# ruleset_a.yaml
ruleset:
  id: a
  extends: b

# ruleset_b.yaml
ruleset:
  id: b
  extends: a  # ❌ Circular!
```
Error: `CircularExtends { child_id: "b", extends_id: "a" }`

### 5.5.6 Benefits

1. **Reduced Code Duplication** - Common rules defined once
2. **Consistent Baselines** - All variants start from same foundation
3. **Easy Maintenance** - Update parent once, all children inherit changes
4. **Clear Relationships** - Explicit parent-child hierarchy
5. **Flexible Customization** - Override what you need, inherit the rest
6. **Compile-Time Validation** - Catch errors early

**Code Savings Example:**
- **Without extends**: 3 rulesets × 50 lines each = 150 lines
- **With extends**: 1 base (50 lines) + 3 children (20 lines each) = 110 lines
- **Savings**: 40 lines (27% reduction)

---

## 5.6 Ruleset Imports and Dependencies

**Rulesets use the `imports` section to declare their rule dependencies explicitly.**

This enables:
- **Modular design** - Rules are defined once and reused across multiple rulesets
- **Compile-time dependency resolution** - All dependencies are resolved during compilation
- **Clear dependency tracking** - Explicit declaration of what rules a ruleset needs
- **Global ID uniqueness** - Rule IDs must be globally unique across the entire system

### 5.5.1 Basic Import Syntax

Rulesets use multi-document YAML format with `---` separator:

```yaml
version: "0.2"

# First document: Imports
imports:
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

  # Reference imported rules by their IDs
  rules:
    - fraud_farm_pattern
    - account_takeover_pattern
    - suspicious_ip_pattern

  # Conclusion produces signals, NOT final decisions
  conclusion:
    - when: total_score >= 100
      signal: high_risk
```

### 5.5.2 Complete Example with Imports

Here's a production-grade ruleset that imports all its rule dependencies:

```yaml
version: "0.2"

imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/account_takeover.yaml
    - library/rules/fraud/velocity_abuse.yaml
    - library/rules/fraud/amount_outlier.yaml
    - library/rules/geography/suspicious_geography.yaml
    - library/rules/fraud/new_user_fraud.yaml

---

ruleset:
  id: fraud_detection_core
  name: Core Fraud Detection Ruleset
  description: Comprehensive fraud detection for transaction events

  # Reference all imported rules
  rules:
    - fraud_farm_pattern        # 100 points
    - account_takeover_pattern  # 85 points
    - velocity_abuse_pattern    # 70 points
    - amount_outlier_pattern    # 75 points
    - suspicious_geography_pattern  # 60 points
    - new_user_fraud_pattern    # 50 points

  # Conclusion produces signals, NOT final decisions
  conclusion:
    # Critical patterns - emit critical_risk signal
    - when: triggered_rules contains "fraud_farm_pattern"
      signal: critical_risk
      reason: "Critical: Fraud farm detected"
      terminate: true

    - when: triggered_rules contains "account_takeover_pattern"
      signal: critical_risk
      reason: "Critical: Account takeover detected"
      terminate: true

    # High score threshold
    - when: total_score >= 150
      signal: high_risk
      reason: "High risk score"
      terminate: true

    # Multiple suspicious indicators
    - when: total_score >= 100
      signal: medium_risk
      reason: "Multiple fraud indicators"

    # Single indicator or moderate score
    - when: total_score >= 50
      signal: low_risk
      reason: "Single fraud indicator detected"

    # Clean transaction
    - default: true
      signal: normal
      reason: "No significant fraud indicators"

  metadata:
    version: "1.0.0"
    last_updated: "2024-12-11"
    owner: "risk_team"
```

### 5.5.3 Multiple Rulesets with Different Thresholds

You can create multiple rulesets that import the same rules but with different conclusion logic:

**Standard Risk Ruleset:**
```yaml
version: "0.2"

imports:
  rules:
    - library/rules/payment/card_testing.yaml
    - library/rules/payment/velocity_check.yaml
    - library/rules/geography/suspicious_ip.yaml

---

ruleset:
  id: payment_standard
  name: Standard Payment Risk Ruleset
  description: Standard thresholds for normal transactions

  rules:
    - card_testing
    - velocity_check
    - suspicious_ip

  # Conclusion produces signals, NOT final decisions
  conclusion:
    # Standard threshold: 100 points
    - when: total_score >= 100
      signal: high_risk
      reason: "Risk score too high"

    - when: total_score >= 50
      signal: medium_risk
      reason: "Manual review required"

    - default: true
      signal: low_risk
```

**High-Value Transaction Ruleset** (stricter thresholds):
```yaml
version: "0.2"

imports:
  rules:
    # Import the SAME rules as payment_standard
    - library/rules/payment/card_testing.yaml
    - library/rules/payment/velocity_check.yaml
    - library/rules/geography/suspicious_ip.yaml
    - library/rules/payment/new_account_risk.yaml
    - library/rules/payment/suspicious_email.yaml

---

ruleset:
  id: payment_high_value
  name: High-Value Payment Risk Ruleset
  description: Stricter thresholds for high-value transactions (> $1000)

  rules:
    - card_testing
    - velocity_check
    - suspicious_ip
    - new_account_risk
    - suspicious_email

  # Conclusion produces signals, NOT final decisions
  conclusion:
    # Critical patterns - emit critical_risk signal
    - when: |
        triggered_rules contains "card_testing" ||
        triggered_rules contains "new_account_risk"
      signal: critical_risk
      reason: "Critical fraud pattern detected"
      terminate: true

    # Stricter threshold: 60 points (vs 100 in standard)
    - when: total_score >= 60
      signal: high_risk
      reason: "Risk score too high for large transaction"
      terminate: true

    # Multiple risk indicators
    - when: triggered_count >= 2
      signal: medium_risk
      reason: "Multiple risk indicators detected"

    # Single risk indicator
    - when: triggered_count >= 1
      signal: low_risk
      reason: "Single risk indicator"

    # Clean high-value transaction
    - default: true
      signal: normal
      reason: "Clean high-value transaction"
```

### 5.5.4 Import Path Resolution

Import paths are resolved relative to the repository root:

```
repository/
├── library/
│   ├── rules/
│   │   ├── fraud/
│   │   │   ├── fraud_farm.yaml
│   │   │   └── account_takeover.yaml
│   │   └── payment/
│   │       └── card_testing.yaml
│   └── rulesets/
│       └── fraud_detection_core.yaml  ← You are here
└── pipelines/
    └── fraud_detection.yaml
```

From `fraud_detection_core.yaml`, you import rules using paths relative to repository root:
```yaml
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml        # ✅ Correct
    - library/rules/fraud/account_takeover.yaml  # ✅ Correct
```

### 5.5.5 Dependency Propagation

**Important:** When a pipeline imports a ruleset, it automatically gets all the ruleset's rule dependencies:

```yaml
# Pipeline only needs to import the ruleset
version: "0.1"

imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml  # This brings in all 6 rules automatically

---

pipeline:
  id: fraud_detection_pipeline

  steps:
    - include:
        ruleset: fraud_detection_core  # All rules are already loaded
```

The compiler automatically resolves the transitive dependencies:
```
Pipeline imports: fraud_detection_core
  ↓
fraud_detection_core imports:
  - fraud_farm.yaml
  - account_takeover.yaml
  - velocity_abuse.yaml
  - amount_outlier.yaml
  - suspicious_geography.yaml
  - new_user_fraud.yaml
  ↓
All 6 rules are available to the pipeline
```

### 5.5.6 ID Naming Conventions

**Ruleset IDs** should follow the pattern: `<domain>_<purpose>_<variant?>`

| Domain | Purpose | Variant | Example |
|--------|---------|---------|---------|
| `fraud_detection` | Core detection | - | `fraud_detection_core` |
| `payment` | Standard risk | - | `payment_standard` |
| `payment` | High value | `high_value` | `payment_high_value` |
| `account_takeover` | Detection | - | `account_takeover_detection` |
| `credit` | Application risk | - | `credit_application_risk` |
| `kyc` | Enhanced due diligence | `enhanced` | `kyc_enhanced_edd` |

**Benefits:**
- **Clarity** - Purpose is immediately clear
- **Organization** - Easy to find related rulesets
- **Versioning** - Variants can represent different risk tolerances

### 5.5.7 Ruleset Metadata

Include metadata for better tracking and governance:

```yaml
ruleset:
  id: fraud_detection_core
  name: Core Fraud Detection Ruleset
  description: Comprehensive fraud detection for transaction events

  rules:
    - fraud_farm_pattern
    - account_takeover_pattern

  conclusion:
    # ... conclusion logic (produces signals) ...

  metadata:
    version: "1.0.0"
    last_updated: "2024-12-11"
    owner: "risk_team"
    contact: "risk-team@example.com"
    change_log:
      - version: "1.0.0"
        date: "2024-12-11"
        changes: "Initial release with 6 fraud detection rules"
    tags:
      - fraud_detection
      - transaction_risk
      - production_ready
```

### 5.5.8 Benefits of Import-Based Rulesets

1. **Reusability** - Rules defined once, used in multiple rulesets
2. **Maintainability** - Update a rule in one place, all rulesets get the change
3. **Modularity** - Mix and match rules for different use cases
4. **Clarity** - Explicit dependencies make the system easier to understand
5. **Flexibility** - Same rules, different thresholds for different scenarios
6. **Type Safety** - Compiler validates all rule IDs at compile time

---

## 6. `conclusion` (Direct Definition)

Conclusion logic evaluates the combined results of all rules and produces signals. **Final decisions are made at the Pipeline level.**

### 6.1 Basic Structure

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

### 6.2 Available Context

Within conclusion conditions, you can access:

- `total_score` - Sum of all triggered rule scores
- `triggered_count` - Number of rules that triggered
- `triggered_rules` - Array of triggered rule IDs
- `context.*` - Any pipeline context data

### 6.3 Built-in Signals

| Signal | Description | Use Case |
|--------|-------------|----------|
| `critical_risk` | Critical risk detected | Immediate blocking patterns |
| `high_risk` | High risk level | Above threshold |
| `medium_risk` | Medium risk level | Needs review |
| `low_risk` | Low risk level | Minor concerns |
| `normal` | No significant risk | Clean transaction |

**Note:** Final decisions (`approve`, `decline`, `review`, `hold`, `pass`) are made at **Pipeline level** based on signals.

---

## 7. Common Conclusion Patterns

### 7.1 Score-Based Conclusions

Signals based on total score:

```yaml
conclusion:
  # Critical risk
  - when: total_score >= 150
    signal: critical_risk
    reason: "Critical risk score"

  # High risk
  - when: total_score >= 100
    signal: high_risk
    reason: "High risk, needs analysis"

  # Medium risk
  - when: total_score >= 50
    signal: medium_risk
    reason: "Medium risk, manual review"

  # Low risk
  - default: true
    signal: low_risk
    reason: "Low risk"
```

### 7.2 Count-Based Conclusions

Signals based on triggered rule count:

```yaml
conclusion:
  # Multiple indicators
  - when: triggered_count >= 3
    signal: critical_risk
    reason: "Multiple risk indicators"

  # Some indicators
  - when: triggered_count >= 2
    signal: high_risk
    reason: "Multiple signals, needs analysis"

  # Single indicator
  - when: triggered_count == 1
    signal: medium_risk
    reason: "Single indicator detected"

  # No indicators
  - default: true
    signal: normal
```

### 7.3 Short-Circuit (Early Termination)

Terminate immediately when specific rule triggers:

```yaml
conclusion:
  # Critical rule triggered - emit critical_risk and stop
  - when: triggered_rules contains "blocked_user"
    signal: critical_risk
    reason: "User is blocked"
    terminate: true  # Stop here, don't evaluate further

  # High-risk rule triggered
  - when: triggered_rules contains "critical_security_breach"
    signal: critical_risk
    reason: "Security breach detected"
    terminate: true

  # Otherwise continue with normal logic
  - when: total_score >= 80
    signal: high_risk

  - default: true
    signal: normal
```

### 7.4 Specific Rule Combinations

Signals based on specific rule combinations:

```yaml
conclusion:
  # Classic takeover pattern: device + location + behavior
  - when: |
      triggered_rules contains "new_device" &&
      triggered_rules contains "unusual_location" &&
      triggered_rules contains "behavior_anomaly"
    signal: critical_risk
    reason: "Classic account takeover pattern"

  # Device + location (suspicious but not definitive)
  - when: |
      triggered_rules contains "new_device" &&
      triggered_rules contains "unusual_location"
    signal: high_risk
    reason: "Device and location anomaly"

  # Single weak signal
  - when: |
      triggered_rules contains "new_device" &&
      triggered_count == 1
    signal: low_risk
    reason: "Only new device, acceptable"

  - default: true
    signal: normal
```

### 7.5 Weighted Scoring with Multipliers

Weighted scoring with synergy effects:

```yaml
conclusion:
  # Calculate weighted score
  - set_var: weighted_score
    value: |
      base_score = total_score

      # Synergy: if certain rules trigger together, multiply
      if (triggered_rules contains "new_device" &&
          triggered_rules contains "unusual_geo") {
        base_score = base_score * 1.5
      }

      if (triggered_count >= 3) {
        base_score = base_score * 1.3
      }

      return base_score

  # Signals based on weighted score
  - when: vars.weighted_score >= 120
    signal: critical_risk

  - when: vars.weighted_score >= 80
    signal: high_risk

  - when: vars.weighted_score >= 50
    signal: medium_risk

  - default: true
    signal: normal
```

### 7.6 Context-Aware Conclusions

Signals incorporating pipeline context data:

```yaml
conclusion:
  # Consider LLM analysis confidence
  - when: |
      total_score >= 70 &&
      context.llm_analysis.confidence > 0.8
    signal: critical_risk
    reason: "High score + high AI confidence"

  # Consider user tier
  - when: |
      total_score >= 60 &&
      event.user.tier == "basic"
    signal: high_risk
    reason: "Medium risk for basic user"

  - when: |
      total_score >= 60 &&
      event.user.tier == "premium"
    signal: medium_risk
    reason: "Medium risk acceptable for premium user"

  # Consider transaction amount
  - when: |
      total_score >= 50 &&
      event.transaction.amount > 10000
    signal: high_risk
    reason: "Medium risk + high value"

  - default: true
    signal: normal
```

### 7.7 Time-Based Conclusions

Time-based conclusion logic:

```yaml
conclusion:
  # Off-hours + medium risk = higher signal
  - when: |
      total_score >= 40 &&
      (hour(event.timestamp) < 6 || hour(event.timestamp) > 22)
    signal: high_risk
    reason: "Medium risk during off-hours"

  # Business hours + medium risk = moderate signal
  - when: |
      total_score >= 40 &&
      hour(event.timestamp) >= 6 && hour(event.timestamp) <= 22
    signal: medium_risk
    reason: "Medium risk during business hours"

  - when: total_score >= 80
    signal: critical_risk

  - default: true
    signal: normal
```

### 7.8 Hybrid: Rules + Score + Combination

Combining multiple conclusion approaches:

```yaml
conclusion:
  # Priority 1: Critical rules (short-circuit)
  - when: triggered_rules contains "blocked_user"
    signal: critical_risk
    reason: "User blocked"
    terminate: true

  # Priority 2: Dangerous combinations
  - when: |
      triggered_rules contains "new_device" &&
      triggered_rules contains "unusual_geo" &&
      triggered_rules contains "high_value_transaction"
    signal: critical_risk
    reason: "High-risk combination"

  # Priority 3: High score threshold
  - when: total_score >= 120
    signal: high_risk
    reason: "High risk score"

  # Priority 4: Multiple signals
  - when: |
      triggered_count >= 3 &&
      context.llm_analysis.risk_score > 0.7
    signal: high_risk
    reason: "Multiple signals + AI confirmation"

  # Priority 5: Moderate risk
  - when: total_score >= 50
    signal: medium_risk
    reason: "Moderate risk"

  # Default
  - default: true
    signal: normal
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
    - new_device_login        # score: 40
    - unusual_location        # score: 50
    - failed_login_spike      # score: 35
    - behavior_anomaly        # score: 60
    - password_change_attempt # score: 70

  # Conclusion produces signals, final decisions at Pipeline level
  conclusion:
    # Critical: password change from new device in unusual location
    - when: |
        triggered_rules contains "password_change_attempt" &&
        triggered_rules contains "new_device_login" &&
        triggered_rules contains "unusual_location"
      signal: critical_risk
      reason: "Critical takeover indicators"
      terminate: true

    # High risk: classic takeover pattern
    - when: |
        triggered_rules contains "new_device_login" &&
        triggered_rules contains "unusual_location" &&
        triggered_rules contains "behavior_anomaly"
      signal: high_risk
      reason: "Classic takeover pattern detected"

    # Medium risk: multiple signals
    - when: triggered_count >= 3
      signal: medium_risk
      reason: "Multiple suspicious indicators"

    # Moderate risk: high score
    - when: total_score >= 100
      signal: medium_risk
      reason: "High risk score"

    # Low risk
    - default: true
      signal: normal
```

### 8.2 Transaction Fraud Detection

```yaml
version: "0.2"

ruleset:
  id: transaction_fraud_detection
  name: Transaction Fraud Detection
  description: Real-time transaction fraud detection

  rules:
    - high_value_transaction    # score: 60
    - velocity_spike           # score: 50
    - unusual_merchant         # score: 40
    - beneficiary_risk         # score: 70
    - amount_anomaly           # score: 45

  conclusion:
    # Critical: high value + high risk beneficiary
    - when: |
        triggered_rules contains "high_value_transaction" &&
        triggered_rules contains "beneficiary_risk" &&
        event.transaction.amount > 50000
      signal: critical_risk
      reason: "High-value transaction to risky beneficiary"

    # High risk: complex pattern
    - when: |
        total_score >= 100 ||
        triggered_count >= 3
      signal: high_risk
      reason: "Complex fraud pattern needs analysis"

    # Medium risk: moderate score
    - when: total_score >= 60
      signal: medium_risk
      reason: "Moderate risk transaction"

    # Low risk
    - default: true
      signal: normal
```

### 8.3 Credit Application Risk

```yaml
version: "0.2"

ruleset:
  id: credit_application_risk
  name: Credit Application Risk Assessment
  description: Evaluate credit application risk

  rules:
    - low_credit_score          # score: 80
    - high_debt_ratio          # score: 60
    - employment_unstable      # score: 50
    - income_inconsistent      # score: 40
    - previous_default         # score: 100

  conclusion:
    # Critical: previous default
    - when: triggered_rules contains "previous_default"
      signal: critical_risk
      reason: "Previous loan default"
      terminate: true

    # High risk: low credit + high debt
    - when: |
        triggered_rules contains "low_credit_score" &&
        triggered_rules contains "high_debt_ratio"
      signal: high_risk
      reason: "Poor credit profile"

    # Medium risk: score threshold
    - when: total_score >= 100
      signal: medium_risk
      reason: "Manual underwriting required"

    # Low risk: borderline
    - when: total_score >= 60
      signal: low_risk
      reason: "Borderline case"

    # Normal: good credit profile
    - default: true
      signal: normal
```

### 8.4 Login Risk with Context

```yaml
version: "0.2"

ruleset:
  id: contextual_login_risk
  name: Contextual Login Risk Detection
  description: Login risk with user tier and time context

  rules:
    - suspicious_device        # score: 50
    - geo_anomaly             # score: 60
    - failed_attempts         # score: 40
    - unusual_time            # score: 30

  conclusion:
    # VIP users: more lenient signal
    - when: |
        event.user.tier == "vip" &&
        total_score < 100
      signal: low_risk
      reason: "VIP user, acceptable risk"

    # High risk: always critical
    - when: total_score >= 120
      signal: critical_risk
      reason: "High risk login"

    # Business hours + moderate risk
    - when: |
        total_score >= 60 &&
        hour(event.timestamp) >= 9 &&
        hour(event.timestamp) <= 18
      signal: medium_risk
      reason: "Moderate risk during business hours"

    # Off-hours + any risk: higher signal
    - when: |
        total_score >= 40 &&
        (hour(event.timestamp) < 6 || hour(event.timestamp) > 22)
      signal: high_risk
      reason: "Risk detected during off-hours"

    # Default: normal
    - default: true
      signal: normal
```

---

## 9. Best Practices

### 9.1 Conclusion Logic Order

Order conclusion rules from most specific to most general:

```yaml
conclusion:
  # 1. Critical rules (short-circuit)
  - when: critical_condition
    signal: critical_risk
    terminate: true

  # 2. Specific combinations
  - when: specific_pattern
    signal: high_risk

  # 3. Count-based
  - when: triggered_count >= 3
    signal: medium_risk

  # 4. Score-based
  - when: total_score >= 80
    signal: medium_risk

  # 5. Default
  - default: true
    signal: normal
```

### 9.2 Use Meaningful Reasons

Always provide clear reasons for audit and explainability:

```yaml
conclusion:
  - when: total_score >= 100
    signal: high_risk
    reason: "Risk score {total_score} exceeds threshold"  # Good: specific

  - when: triggered_count >= 3
    signal: medium_risk
    reason: "Multiple risk indicators: {triggered_rules}"  # Good: detailed
```

### 9.3 Consider Business Context

Adapt signals based on business context:

```yaml
conclusion:
  # Different thresholds for different user tiers
  - when: |
      event.user.tier == "premium" &&
      total_score < 80
    signal: low_risk

  - when: |
      event.user.tier == "basic" &&
      total_score < 50
    signal: low_risk
```

---

## 10. Related Documentation

For comprehensive understanding of rulesets and the CORINT ecosystem:

### Core Concepts
- **[imports.md](imports.md)** - Complete module system and dependency management specification
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

A CORINT Ruleset:

- Groups multiple rules into a reusable logical unit
- **Defines conclusion logic** through `conclusion` section
- **Supports inheritance** via `extends` for code reuse **[Phase 3]**
- Evaluates rule combinations and context
- **Produces signals** (`critical_risk`, `high_risk`, `medium_risk`, `low_risk`, `normal`)
- **Does NOT make final decisions** - Final decisions (approve/decline/review/hold/pass) are made at **Pipeline level**
- Integrates cleanly with CORINT Pipelines
- Improves modularity and maintainability
- **Uses imports to declare rule dependencies explicitly**

**Key Points (Three-Layer Model):**
- **Rules** detect and score individual risk patterns
- **Rulesets** produce signals/conclusions based on rule results
- **Pipelines** make final decisions based on signals

**Phase 3 Features:**
- **Inheritance (`extends`)** - Child rulesets inherit rules from parent, reducing duplication by 27%+
- **Compile-time resolution** - Zero runtime overhead, all resolved during compilation

**Code Reuse Benefits:**
- Define common patterns once, reuse everywhere
- Easy customization through parameters and overrides
- Consistent baselines with flexible variants
- Reduced maintenance burden
- Type-safe with compile-time validation

Rulesets are the signal-producing layer of CORINT's Risk Definition Language (RDL).
