# CORINT Risk Definition Language (RDL)
## Ruleset Specification (v0.2)

A **Ruleset** is a named collection of rules that can be reused, grouped, and executed as a unit within CORINT's Cognitive Risk Intelligence framework.
Rulesets enable modular design, separation of concerns, and cleaner pipeline logic.

**Important:** Rulesets produce **decision signals** through their `conclusion` section. The available signals are: `approve`, `decline`, `review`, `hold`, and `pass`. This enables:
- **Reusability**: Same ruleset can be used in different pipelines
- **Flexibility**: Pipelines can use results from multiple rulesets
- **Clarity**: Clear separation between detection (rules) and decision logic (conclusion)

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
    owner: string
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

  # Conclusion produces decision signals
  conclusion:
    - when: triggered_rules contains "card_testing"
      signal: decline
      reason: "Card testing detected"

    - when: total_score >= 100
      signal: decline
      reason: "High risk score"

    - when: total_score >= 60
      signal: review
      reason: "Medium risk - requires review"

    - default: true
      signal: approve
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

  # Override with stricter conclusion logic
  conclusion:
    - when: triggered_rules contains "card_testing"
      signal: decline
      reason: "Card testing detected"

    - when: total_score >= 60  # Stricter than parent (was 100)
      signal: decline
      reason: "Risk score too high for large transaction"

    - when: triggered_count >= 2
      signal: review
      reason: "Multiple risk indicators"

    - default: true
      signal: approve
```

**Result After Inheritance Resolution**:
- **Total rules**: 6 (5 from parent + 1 new)
- **Conclusion logic**: Uses child's stricter thresholds
- **Zero duplication**: Parent rules defined once, inherited automatically
- **Signal output**: `decline`, `review`, or `approve` based on evaluation

### 5.5.4 Multiple Inheritance Variants

Create multiple variants of the same base ruleset with different thresholds:

```yaml
# payment_standard.yaml - Standard thresholds
ruleset:
  id: payment_standard
  extends: payment_base
  conclusion:
    - when: total_score >= 100
      signal: decline

# payment_high_value.yaml - Strict thresholds
ruleset:
  id: payment_high_value
  extends: payment_base
  rules:
    - amount_outlier  # Add extra rule
  conclusion:
    - when: total_score >= 60  # Stricter
      signal: decline

# payment_vip.yaml - Lenient thresholds
ruleset:
  id: payment_vip
  extends: payment_base
  conclusion:
    - when: total_score >= 150  # More lenient
      signal: decline
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

  # Conclusion produces decision signals
  conclusion:
    - when: total_score >= 100
      signal: decline
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

  # Conclusion produces decision signals
  conclusion:
    # Critical patterns - decline immediately
    - when: triggered_rules contains "fraud_farm_pattern"
      signal: decline
      reason: "Critical: Fraud farm detected"

    - when: triggered_rules contains "account_takeover_pattern"
      signal: decline
      reason: "Critical: Account takeover detected"

    # High score threshold
    - when: total_score >= 150
      signal: decline
      reason: "High risk score"

    # Multiple suspicious indicators
    - when: total_score >= 100
      signal: review
      reason: "Multiple fraud indicators"

    # Single indicator or moderate score
    - when: total_score >= 50
      signal: review
      reason: "Single fraud indicator detected"

    # Clean transaction
    - default: true
      signal: approve
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

  # Conclusion produces decision signals
  conclusion:
    # Standard threshold: 100 points
    - when: total_score >= 100
      signal: decline
      reason: "Risk score too high"

    - when: total_score >= 50
      signal: review
      reason: "Manual review required"

    - default: true
      signal: approve
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

  # Conclusion produces decision signals
  conclusion:
    # Critical patterns - decline immediately
    - when:
        all:
          - any:
              - triggered_rules contains "card_testing"
              - triggered_rules contains "new_account_risk"
      signal: decline
      reason: "Critical fraud pattern detected"

    # Stricter threshold: 60 points (vs 100 in standard)
    - when: total_score >= 60
      signal: decline
      reason: "Risk score too high for large transaction"

    # Multiple risk indicators
    - when: triggered_count >= 2
      signal: review
      reason: "Multiple risk indicators detected"

    # Single risk indicator
    - when: triggered_count >= 1
      signal: review
      reason: "Single risk indicator"

    # Clean high-value transaction
    - default: true
      signal: approve
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
    # ... conclusion logic (produces decision signals) ...

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

Conclusion logic evaluates the combined results of all rules and produces a decision signal.

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

Decision signals based on triggered rule count:

```yaml
conclusion:
  # Multiple indicators - decline
  - when: triggered_count >= 3
    signal: decline
    reason: "Multiple risk indicators"

  # Some indicators - review
  - when: triggered_count >= 2
    signal: review
    reason: "Multiple signals, needs analysis"

  # Single indicator - hold for verification
  - when: triggered_count == 1
    signal: hold
    reason: "Single indicator detected, needs verification"

  # No indicators - approve
  - default: true
    signal: approve
```

### 7.3 Short-Circuit (Early Termination)

Critical rules that trigger immediate decline:

```yaml
conclusion:
  # Critical rule triggered - decline
  - when: triggered_rules contains "blocked_user"
    signal: decline
    reason: "User is blocked"

  # High-risk rule triggered - decline
  - when: triggered_rules contains "critical_security_breach"
    signal: decline
    reason: "Security breach detected"

  # Otherwise continue with normal logic
  - when: total_score >= 80
    signal: review

  - default: true
    signal: approve
```

### 7.4 Specific Rule Combinations

Decision signals based on specific rule combinations:

```yaml
conclusion:
  # Classic takeover pattern: device + location + behavior
  - when:
      all:
        - triggered_rules contains "new_device"
        - triggered_rules contains "unusual_location"
        - triggered_rules contains "behavior_anomaly"
    signal: decline
    reason: "Classic account takeover pattern"

  # Device + location (suspicious but not definitive)
  - when:
      all:
        - triggered_rules contains "new_device"
        - triggered_rules contains "unusual_location"
    signal: review
    reason: "Device and location anomaly"

  # Single weak signal
  - when:
      all:
        - triggered_rules contains "new_device"
        - triggered_count == 1
    signal: hold
    reason: "Only new device, needs verification"

  - default: true
    signal: approve
```

### 7.5 Weighted Scoring with Multipliers (📋 Planned - Requires set_var)

Weighted scoring with synergy effects (currently use explicit conditions instead):

```yaml
# 📋 PLANNED SYNTAX (not yet implemented):
conclusion:
  # Calculate weighted score
  - set_var: weighted_score  # Not yet supported
    value: |
      base_score = total_score
      if (triggered_rules contains "new_device" &&
          triggered_rules contains "unusual_geo") {
        base_score = base_score * 1.5
      }
      return base_score

# ✅ CURRENT WORKAROUND (use explicit conditions):
conclusion:
  # Synergy patterns - higher threshold
  - when:
      all:
        - triggered_rules contains "new_device"
        - triggered_rules contains "unusual_geo"
        - total_score >= 80  # Lower threshold due to synergy
    signal: decline

  # Multiple indicators - adjusted threshold
  - when:
      all:
        - triggered_count >= 3
        - total_score >= 90  # Lower threshold for multiple indicators
    signal: decline

  # Standard thresholds
  - when: total_score >= 120
    signal: decline

  - when: total_score >= 80
    signal: review

  - when: total_score >= 50
    signal: hold

  - default: true
    signal: approve
```

### 7.6 Context-Aware Conclusions

Decision signals incorporating pipeline context data:

```yaml
conclusion:
  # Consider LLM analysis confidence
  - when:
      all:
        - total_score >= 70
        - context.llm_analysis.confidence > 0.8
    signal: decline
    reason: "High score + high AI confidence"

  # Consider user tier - stricter for basic users
  - when:
      all:
        - total_score >= 60
        - event.user.tier == "basic"
    signal: decline
    reason: "Medium risk for basic user"

  # More lenient for premium users
  - when:
      all:
        - total_score >= 60
        - event.user.tier == "premium"
    signal: review
    reason: "Medium risk acceptable for premium user"

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

### 7.7 Time-Based Conclusions (📋 Requires hour() function - Planned)

Time-based conclusion logic:

```yaml
conclusion:
  # Off-hours + medium risk = higher signal
  - when:
      all:
        - total_score >= 40
        - any:
            - hour(event.timestamp) < 6
            - hour(event.timestamp) > 22
    signal: review
    reason: "Medium risk during off-hours"

  # Business hours + medium risk = moderate signal
  - when:
      all:
        - total_score >= 40
        - hour(event.timestamp) >= 6
        - hour(event.timestamp) <= 22
    signal: hold
    reason: "Medium risk during business hours"

  - when: total_score >= 80
    signal: decline

  - default: true
    signal: approve
```

### 7.8 Hybrid: Rules + Score + Combination

Combining multiple conclusion approaches:

```yaml
conclusion:
  # Priority 1: Critical rules (short-circuit)
  - when: triggered_rules contains "blocked_user"
    signal: decline
    reason: "User blocked"

  # Priority 2: Dangerous combinations
  - when:
      all:
        - triggered_rules contains "new_device"
        - triggered_rules contains "unusual_geo"
        - triggered_rules contains "high_value_transaction"
    signal: decline
    reason: "High-risk combination"

  # Priority 3: High score threshold
  - when: total_score >= 120
    signal: decline
    reason: "High risk score"

  # Priority 4: Multiple signals
  - when:
      all:
        - triggered_count >= 3
        - context.llm_analysis.risk_score > 0.7
    signal: decline
    reason: "Multiple signals + AI confirmation"

  # Priority 5: Moderate risk
  - when: total_score >= 50
    signal: review
    reason: "Moderate risk"

  # Default
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
    - new_device_login        # score: 40
    - unusual_location        # score: 50
    - failed_login_spike      # score: 35
    - behavior_anomaly        # score: 60
    - password_change_attempt # score: 70

  # Conclusion produces decision signals
  conclusion:
    # Critical: password change from new device in unusual location
    - when:
        all:
          - triggered_rules contains "password_change_attempt"
          - triggered_rules contains "new_device_login"
          - triggered_rules contains "unusual_location"
      signal: decline
      reason: "Critical takeover indicators"

    # High risk: classic takeover pattern
    - when:
        all:
          - triggered_rules contains "new_device_login"
          - triggered_rules contains "unusual_location"
          - triggered_rules contains "behavior_anomaly"
      signal: decline
      reason: "Classic takeover pattern detected"

    # Medium risk: multiple signals
    - when: triggered_count >= 3
      signal: review
      reason: "Multiple suspicious indicators"

    # Moderate risk: high score
    - when: total_score >= 100
      signal: review
      reason: "High risk score"

    # Low risk
    - default: true
      signal: approve
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
    - when:
        all:
          - triggered_rules contains "high_value_transaction"
          - triggered_rules contains "beneficiary_risk"
          - event.transaction.amount > 50000
      signal: decline
      reason: "High-value transaction to risky beneficiary"

    # High risk: complex pattern
    - when:
        any:
          - total_score >= 100
          - triggered_count >= 3
      signal: decline
      reason: "Complex fraud pattern needs blocking"

    # Medium risk: moderate score
    - when: total_score >= 60
      signal: review
      reason: "Moderate risk transaction"

    # Low risk
    - default: true
      signal: approve
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
      signal: decline
      reason: "Previous loan default"

    # High risk: low credit + high debt
    - when:
        all:
          - triggered_rules contains "low_credit_score"
          - triggered_rules contains "high_debt_ratio"
      signal: decline
      reason: "Poor credit profile"

    # Medium risk: score threshold
    - when: total_score >= 100
      signal: review
      reason: "Manual underwriting required"

    # Low risk: borderline
    - when: total_score >= 60
      signal: hold
      reason: "Borderline case - needs verification"

    # Normal: good credit profile
    - default: true
      signal: approve
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
    - when:
        all:
          - event.user.tier == "vip"
          - total_score < 100
      signal: approve
      reason: "VIP user, acceptable risk"

    # High risk: always decline
    - when: total_score >= 120
      signal: decline
      reason: "High risk login"

    # Business hours + moderate risk (requires hour() function - see expression.md)
    - when:
        all:
          - total_score >= 60
          - hour(event.timestamp) >= 9
          - hour(event.timestamp) <= 18
      signal: review
      reason: "Moderate risk during business hours"

    # Off-hours + any risk: higher signal (requires hour() function - see expression.md)
    - when:
        all:
          - total_score >= 40
          - any:
              - hour(event.timestamp) < 6
              - hour(event.timestamp) > 22
      signal: review
      reason: "Risk detected during off-hours"

    # Default: approve
    - default: true
      signal: approve
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
- **Supports inheritance** via `extends` for code reuse (✅ Implemented)
- Evaluates rule combinations and context
- **Produces decision signals**: `approve`, `decline`, `review`, `hold`, or `pass`
- Integrates cleanly with CORINT Pipelines
- Improves modularity and maintainability
- **Uses imports to declare rule dependencies explicitly**

**Key Points (Three-Layer Model):**
- **Rules** detect and score individual risk patterns
- **Rulesets** evaluate rule results and produce decision signals
- **Pipelines** route events to rulesets and can use ruleset results

**Inheritance Features (✅ Implemented):**
- **Inheritance (`extends`)** - Child rulesets inherit rules from parent, reducing duplication by 27%+
- **Compile-time resolution** - Zero runtime overhead, all resolved during compilation

**Code Reuse Benefits:**
- Define common patterns once, reuse everywhere
- Easy customization through parameters and overrides
- Consistent baselines with flexible variants
- Reduced maintenance burden
- Type-safe with compile-time validation

Rulesets are the signal-producing layer of CORINT's Risk Definition Language (RDL).
