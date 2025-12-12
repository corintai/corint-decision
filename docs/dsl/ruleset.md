# CORINT Risk Definition Language (RDL)
## Ruleset Specification (v0.1)

A **Ruleset** is a named collection of rules that can be reused, grouped, and executed as a unit within CORINT’s Cognitive Risk Intelligence framework.  
Rulesets enable modular design, separation of concerns, and cleaner pipeline logic.

---

## 1. Ruleset Structure

```yaml
ruleset:
  id: string
  name: string
  description: string
  extends: string                    # ✨ NEW: Parent ruleset ID (Phase 3)
  rules:
    - <rule-id-1>
    - <rule-id-2>
    - <rule-id-3>
  decision_logic:                   # Option 1: Define directly
    - <decision-rules>
  decision_template:                # Option 2: Use template (Phase 3)
    template: <template-id>
    params:
      <param-key>: <param-value>
  metadata:                         # Optional metadata
    version: string
    owner: string
```

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

  # Override decision logic with stricter thresholds
  decision_logic:
    - condition: total_score >= 60
      action: deny
      reason: "Risk score too high for large transaction"
```

### 5.5.2 Inheritance Behavior

When a ruleset extends a parent:

| Field | Behavior | Description |
|-------|----------|-------------|
| **`rules`** | **Merge + Auto-dedup** | Parent rules + child rules, duplicates automatically removed |
| **`decision_logic`** | **Complete override** | Child replaces parent if defined, otherwise inherits parent's logic |
| **`name`** | **Override** | Child overrides if defined, otherwise inherits parent's name |
| **`description`** | **Override** | Child overrides if defined, otherwise inherits parent's description |
| **`metadata`** | **Override** | Child overrides if defined, otherwise inherits parent's metadata |

### 5.5.3 Complete Example: Payment Risk Hierarchy

**Parent Ruleset** (`payment_base.yaml`):
```yaml
version: "0.1"

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

  decision_logic:
    - condition: triggered_rules contains "card_testing"
      action: deny
      reason: "Card testing detected"
      terminate: true

    - condition: total_score >= 100
      action: deny
      reason: "High risk score"

    - condition: total_score >= 60
      action: review
      reason: "Medium risk - requires review"

    - default: true
      action: approve
```

**Child Ruleset** (`payment_high_value.yaml`):
```yaml
version: "0.1"

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

  # Override with stricter decision logic
  decision_logic:
    - condition: triggered_rules contains "card_testing"
      action: deny
      reason: "Card testing detected"
      terminate: true

    - condition: total_score >= 60  # Stricter than parent (was 100)
      action: deny
      reason: "Risk score too high for large transaction"

    - condition: triggered_count >= 2
      action: review
      reason: "Multiple risk indicators"

    - default: true
      action: approve
```

**Result After Inheritance Resolution**:
- **Total rules**: 6 (5 from parent + 1 new)
- **Decision logic**: Uses child's stricter thresholds
- **Zero duplication**: Parent rules defined once, inherited automatically

### 5.5.4 Multiple Inheritance Variants

Create multiple variants of the same base ruleset with different thresholds:

```yaml
# payment_standard.yaml - Standard thresholds
ruleset:
  id: payment_standard
  extends: payment_base
  decision_logic:
    - condition: total_score >= 100
      action: deny

# payment_high_value.yaml - Strict thresholds
ruleset:
  id: payment_high_value
  extends: payment_base
  rules:
    - amount_outlier  # Add extra rule
  decision_logic:
    - condition: total_score >= 60  # Stricter
      action: deny

# payment_vip.yaml - Lenient thresholds
ruleset:
  id: payment_vip
  extends: payment_base
  decision_logic:
    - condition: total_score >= 150  # More lenient
      action: deny
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
version: "0.1"

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

  decision_logic:
    - condition: total_score >= 100
      action: deny
```

### 5.5.2 Complete Example with Imports

Here's a production-grade ruleset that imports all its rule dependencies:

```yaml
version: "0.1"

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

  decision_logic:
    # Critical patterns - immediate deny
    - condition: triggered_rules contains "fraud_farm_pattern"
      action: deny
      reason: "Critical: Fraud farm detected"
      terminate: true

    - condition: triggered_rules contains "account_takeover_pattern"
      action: deny
      reason: "Critical: Account takeover detected"
      terminate: true

    # High score threshold
    - condition: total_score >= 150
      action: deny
      reason: "High risk score"
      terminate: true

    # Multiple suspicious indicators
    - condition: total_score >= 100
      action: review
      reason: "Multiple fraud indicators"
      terminate: true

    # Single indicator or moderate score
    - condition: total_score >= 50
      action: challenge
      reason: "Single fraud indicator detected"
      terminate: true

    # Clean transaction
    - default: true
      action: approve
      reason: "No significant fraud indicators"

  metadata:
    version: "1.0.0"
    last_updated: "2024-12-11"
    owner: "risk_team"
```

### 5.5.3 Multiple Rulesets with Different Thresholds

You can create multiple rulesets that import the same rules but with different decision logic:

**Standard Risk Ruleset:**
```yaml
version: "0.1"

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

  decision_logic:
    # Standard threshold: 100 points
    - condition: total_score >= 100
      action: deny
      reason: "Risk score too high"

    - condition: total_score >= 50
      action: review
      reason: "Manual review required"

    - default: true
      action: approve
```

**High-Value Transaction Ruleset** (stricter thresholds):
```yaml
version: "0.1"

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

  decision_logic:
    # Critical patterns - immediate deny
    - condition: |
        triggered_rules contains "card_testing" ||
        triggered_rules contains "new_account_risk"
      action: deny
      reason: "Critical fraud pattern detected"
      terminate: true

    # Stricter threshold: 60 points (vs 100 in standard)
    - condition: total_score >= 60
      action: deny
      reason: "Risk score too high for large transaction"
      terminate: true

    # Multiple risk indicators
    - condition: triggered_count >= 2
      action: review
      reason: "Multiple risk indicators detected"
      terminate: true

    # Single risk indicator - require 3DS
    - condition: triggered_count >= 1
      action: challenge
      reason: "Require 3DS authentication"
      terminate: true

    # Clean high-value transaction
    - default: true
      action: approve
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

  decision_logic:
    # ... decision logic ...

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

## 6. Decision Logic Templates (`decision_template`) **[Phase 3]**

**Decision Logic Templates provide reusable, parameterized decision logic patterns that can be shared across multiple rulesets.**

Instead of defining `decision_logic` directly in each ruleset, you can reference a template and customize it with parameters.

### 6.1 Why Use Templates?

**Problem:**
```yaml
# payment_standard.yaml - 50 lines
decision_logic:
  - condition: total_score >= 100
    action: deny
  - condition: total_score >= 60
    action: review
  - default: true
    action: approve

# payment_high_value.yaml - 50 lines (DUPLICATE with different thresholds!)
decision_logic:
  - condition: total_score >= 150  # Different threshold
    action: deny
  - condition: total_score >= 80   # Different threshold
    action: review
  - default: true
    action: approve
```

**Solution with Templates:**
```yaml
# Define template once
template:
  id: score_based_decision
  params:
    critical_threshold: 200
    high_threshold: 100
  decision_logic: [5 rules]

# Use in multiple rulesets with different params
ruleset:
  decision_template:
    template: score_based_decision
    params:
      critical_threshold: 150  # Override!
```

### 6.2 Template Definition

Templates are defined in separate YAML files:

```yaml
version: "0.1"

---

template:
  id: score_based_decision
  name: Score-Based Decision Template
  description: |
    Standard score threshold decision logic.
    Uses total_score from rule evaluation to make decisions.

  # Default parameter values
  params:
    critical_threshold: 200
    high_threshold: 100
    medium_threshold: 60
    low_threshold: 30

  # Decision logic with parameter placeholders
  decision_logic:
    - condition: total_score >= 200
      action: deny
      reason: "Critical risk detected (score: {total_score})"
      terminate: true

    - condition: total_score >= 100
      action: deny
      reason: "High risk detected (score: {total_score})"
      terminate: true

    - condition: total_score >= 60
      action: review
      reason: "Medium risk - requires review (score: {total_score})"
      terminate: true

    - condition: total_score >= 30
      action: review
      reason: "Low risk monitoring"
      terminate: false

    - default: true
      action: approve
      reason: "Transaction approved - low risk"
```

### 6.3 Using Templates in Rulesets

Import and reference templates with custom parameters:

```yaml
version: "0.1"

imports:
  rules:
    - library/rules/payment/card_testing.yaml
    - library/rules/payment/velocity_check.yaml
  templates:
    - library/templates/score_based_decision.yaml  # ✨ Import template

---

ruleset:
  id: payment_with_template
  name: Payment Ruleset (Using Template)

  rules:
    - card_testing
    - velocity_check

  # Use template instead of defining decision_logic
  decision_template:
    template: score_based_decision
    params:
      critical_threshold: 150  # Override default (200 -> 150)
      high_threshold: 80       # Override default (100 -> 80)
      # medium_threshold: 60   # Use template default
      # low_threshold: 30      # Use template default
```

### 6.4 Template Resolution Process

Templates are resolved at **compile-time** (zero runtime overhead):

1. **Parse**: Ruleset references template via `decision_template` field
2. **Load**: Compiler loads template from imports
3. **Merge Parameters**: Template defaults + ruleset overrides
4. **Instantiate**: Apply merged parameters to decision logic
5. **Replace**: Populate ruleset's `decision_logic` with resolved template
6. **Clear**: Remove `decision_template` reference (fully resolved)

**After compilation:**
- Ruleset contains complete `decision_logic` (5 rules from template)
- Parameters are merged (critical=150, high=80, medium=60, low=30)
- `decision_template` field is cleared
- Zero runtime performance impact

### 6.5 Template Types

**1. Score-Based Template**

Uses `total_score` for threshold-based decisions:

```yaml
template:
  id: score_based_decision
  params:
    critical_threshold: 200
    high_threshold: 100
  decision_logic:
    - condition: total_score >= params.critical_threshold
      action: deny
```

**2. Pattern-Based Template**

Uses `triggered_rules` for pattern matching:

```yaml
template:
  id: pattern_based_decision
  decision_logic:
    - condition: triggered_rules contains "card_testing"
      action: deny
      reason: "Critical pattern: Card testing detected"
      terminate: true

    - condition: triggered_rules contains "fraud_farm"
      action: deny
      reason: "Critical pattern: Fraud farm detected"
      terminate: true
```

**3. Hybrid Template**

Combines score and pattern matching:

```yaml
template:
  id: hybrid_decision
  params:
    deny_score: 150
    review_score: 80
  decision_logic:
    # Pattern-based denial (highest priority)
    - condition: triggered_rules contains "card_testing"
      action: deny
      terminate: true

    # Score-based denial
    - condition: total_score >= params.deny_score
      action: deny
      terminate: true

    # Combined: Score + pattern
    - condition: |
        total_score >= params.review_score AND
        triggered_rules contains "velocity_abuse"
      action: review
      terminate: true
```

### 6.6 Multiple Rulesets Sharing Templates

Different rulesets can use the same template with different parameters:

```yaml
# payment_standard.yaml
ruleset:
  id: payment_standard
  decision_template:
    template: score_based_decision
    # Use all defaults

# payment_high_value.yaml
ruleset:
  id: payment_high_value
  decision_template:
    template: score_based_decision
    params:
      critical_threshold: 150  # Stricter
      high_threshold: 80       # Stricter

# payment_vip.yaml
ruleset:
  id: payment_vip
  decision_template:
    template: score_based_decision
    params:
      critical_threshold: 250  # More lenient
      high_threshold: 150      # More lenient
```

### 6.7 Standard Template Library

CORINT provides standard templates out of the box:

| Template | Use Case | Parameters |
|----------|----------|------------|
| `score_based_decision` | Threshold-based decisions | `critical_threshold`, `high_threshold`, `medium_threshold`, `low_threshold` |
| `pattern_based_decision` | Rule pattern matching | None (uses predefined critical patterns) |
| `hybrid_decision` | Combined score + pattern | `deny_score`, `review_score`, `critical_pattern`, `high_risk_pattern` |

### 6.8 Benefits

1. **Reduced Duplication** - Define decision logic once, reuse everywhere (70%+ code reduction)
2. **Easy Customization** - Override only the parameters you need
3. **Consistency** - All rulesets using the same template follow the same pattern
4. **Maintainability** - Update template once, all users get the change
5. **Type Safety** - Compile-time validation of template references
6. **Zero Runtime Overhead** - All resolved at compile-time

**Code Savings Example:**
- **Without templates**: 3 rulesets × 50 lines decision logic = 150 lines
- **With templates**: 1 template (50 lines) + 3 rulesets (10 lines each) = 80 lines
- **Savings**: 70 lines (47% reduction)

### 6.9 Combining Templates and Inheritance

You can use both `extends` and `decision_template`:

```yaml
ruleset:
  id: payment_high_value
  extends: payment_base          # Inherit rules from parent
  decision_template:              # Use template for decision logic
    template: score_based_decision
    params:
      critical_threshold: 150
```

---

## 7. `decision_logic` (Direct Definition)

**This is where actions are defined when NOT using templates.**

Decision logic evaluates the combined results of all rules and determines the final action.

### 7.1 Basic Structure

```yaml
decision_logic:
  - condition: <expression>
    action: <action-type>
    reason: <string>
  
  - condition: <expression>
    action: <action-type>
    
  - default: true
    action: <action-type>
```

### 7.2 Available Context

Within decision_logic conditions, you can access:

- `total_score` - Sum of all triggered rule scores
- `triggered_count` - Number of rules that triggered
- `triggered_rules` - Array of triggered rule IDs
- `context.*` - Any pipeline context data

### 7.3 Built-in Actions

| Action | Description | Use Case |
|--------|-------------|----------|
| `approve` | Automatically approve | Low risk |
| `deny` | Automatically reject | High risk |
| `review` | Send to human review | Needs judgment |
| `infer` | Send to AI analysis (async) | Complex patterns |

---

## 8. Common Decision Patterns

### 8.1 Score-Based Decisions

Decisions based on total score:

```yaml
decision_logic:
  # Critical risk
  - condition: total_score >= 150
    action: deny
    reason: "Critical risk score"
    
  # High risk
  - condition: total_score >= 100
    action: infer
    infer:
      data_snapshot:
        - event.*
        - context.*
    reason: "High risk, needs AI analysis"
    
  # Medium risk  
  - condition: total_score >= 50
    action: review
    reason: "Medium risk, manual review"
    
  # Low risk
  - default: true
    action: approve
    reason: "Low risk"
```

### 8.2 Count-Based Decisions

Decisions based on triggered rule count:

```yaml
decision_logic:
  # Multiple indicators
  - condition: triggered_count >= 3
    action: deny
    reason: "Multiple risk indicators"

  # Some indicators
  - condition: triggered_count >= 2
    action: infer
    infer:
      data_snapshot:
        - event.*
        - context.*
    reason: "Multiple signals, needs analysis"

  # Single indicator
  - condition: triggered_count == 1
    action: review
    reason: "Single indicator detected"

  # No indicators
  - default: true
    action: approve
```

### 8.3 Short-Circuit (Early Termination)

Terminate immediately when specific rule triggers:

```yaml
decision_logic:
  # Critical rule triggered - immediate deny
  - condition: triggered_rules contains "blocked_user"
    action: deny
    reason: "User is blocked"
    terminate: true  # Stop here, don't evaluate further
    
  # High-risk rule triggered - immediate review
  - condition: triggered_rules contains "critical_security_breach"
    action: infer
    infer:
      data_snapshot:
        - event.*
        - context.*
    reason: "Security breach detected"
    terminate: true
    
  # Otherwise continue with normal logic
  - condition: total_score >= 80
    action: review
    
  - default: true
    action: approve
```

### 8.4 Specific Rule Combinations

Decisions based on specific rule combinations:

```yaml
decision_logic:
  # Classic takeover pattern: device + location + behavior
  - condition: |
      triggered_rules contains "new_device" &&
      triggered_rules contains "unusual_location" &&
      triggered_rules contains "behavior_anomaly"
    action: deny
    reason: "Classic account takeover pattern"
    
  # Device + location (suspicious but not definitive)
  - condition: |
      triggered_rules contains "new_device" &&
      triggered_rules contains "unusual_location"
    action: infer
    infer:
      data_snapshot:
        - event.*
        - context.*
    reason: "Device and location anomaly"
    
  # Single weak signal
  - condition: |
      triggered_rules contains "new_device" &&
      triggered_count == 1
    action: approve
    reason: "Only new device, acceptable"
    
  - default: true
    action: approve
```

### 8.5 Weighted Scoring with Multipliers

Weighted scoring with synergy effects:

```yaml
decision_logic:
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
      
  # Decisions based on weighted score
  - condition: vars.weighted_score >= 120
    action: deny
    
  - condition: vars.weighted_score >= 80
    action: infer
    infer:
      data_snapshot:
        - event.*
        - context.*
        
  - condition: vars.weighted_score >= 50
    action: review
    
  - default: true
    action: approve
```

### 8.6 Context-Aware Decisions

Decisions incorporating pipeline context data:

```yaml
decision_logic:
  # Consider LLM analysis confidence
  - condition: |
      total_score >= 70 &&
      context.llm_analysis.confidence > 0.8
    action: deny
    reason: "High score + high AI confidence"
    
  # Consider user tier
  - condition: |
      total_score >= 60 &&
      event.user.tier == "basic"
    action: review
    reason: "Medium risk for basic user"
    
  - condition: |
      total_score >= 60 &&
      event.user.tier == "premium"
    action: approve
    reason: "Medium risk acceptable for premium user"
    
  # Consider transaction amount
  - condition: |
      total_score >= 50 &&
      event.transaction.amount > 10000
    action: infer
    infer:
      data_snapshot:
        - event.*
        - context.*
    reason: "Medium risk + high value"
    
  - default: true
    action: approve
```

### 8.7 Time-Based Decisions

Time-based decision logic:

```yaml
decision_logic:
  # Off-hours + medium risk = more cautious
  - condition: |
      total_score >= 40 &&
      (hour(event.timestamp) < 6 || hour(event.timestamp) > 22)
    action: review
    reason: "Medium risk during off-hours"
    
  # Business hours + medium risk = AI analysis
  - condition: |
      total_score >= 40 &&
      hour(event.timestamp) >= 6 && hour(event.timestamp) <= 22
    action: infer
    infer:
      data_snapshot:
        - event.*
        - context.*
    reason: "Medium risk during business hours"
    
  - condition: total_score >= 80
    action: deny
    
  - default: true
    action: approve
```

### 8.8 Hybrid: Rules + Score + Combination

Combining multiple decision approaches:

```yaml
decision_logic:
  # Priority 1: Critical rules (short-circuit)
  - condition: triggered_rules contains "blocked_user"
    action: deny
    reason: "User blocked"
    terminate: true
    
  # Priority 2: Dangerous combinations
  - condition: |
      triggered_rules contains "new_device" &&
      triggered_rules contains "unusual_geo" &&
      triggered_rules contains "high_value_transaction"
    action: deny
    reason: "High-risk combination"
    
  # Priority 3: High score threshold
  - condition: total_score >= 120
    action: infer
    infer:
      data_snapshot:
        - event.*
        - context.*
    reason: "High risk score"
    
  # Priority 4: Multiple signals
  - condition: |
      triggered_count >= 3 &&
      context.llm_analysis.risk_score > 0.7
    action: review
    reason: "Multiple signals + AI confirmation"
    
  # Priority 5: Moderate risk
  - condition: total_score >= 50
    action: review
    reason: "Moderate risk"
    
  # Default: approve
  - default: true
    action: approve
```

---

## 9. Complete Examples

### 9.1 Account Takeover Detection

```yaml
version: "0.1"

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
    
  decision_logic:
    # Critical: password change from new device in unusual location
    - condition: |
        triggered_rules contains "password_change_attempt" &&
        triggered_rules contains "new_device_login" &&
        triggered_rules contains "unusual_location"
      action: deny
      reason: "Critical takeover indicators"
      terminate: true
      
    # High risk: classic takeover pattern
    - condition: |
        triggered_rules contains "new_device_login" &&
        triggered_rules contains "unusual_location" &&
        triggered_rules contains "behavior_anomaly"
      action: infer
      infer:
        data_snapshot:
          - event.*
          - context.*
      reason: "Classic takeover pattern detected"
      
    # Medium risk: multiple signals
    - condition: triggered_count >= 3
      action: review
      reason: "Multiple suspicious indicators"
      
    # Moderate risk: high score
    - condition: total_score >= 100
      action: infer
      infer:
        data_snapshot:
          - event.*
          - context.account_takeover_detection.*
      reason: "High risk score"
      
    # Low risk
    - default: true
      action: approve
```

### 9.2 Transaction Fraud Detection

```yaml
version: "0.1"

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
    
  decision_logic:
    # Immediate deny: high value + high risk beneficiary
    - condition: |
        triggered_rules contains "high_value_transaction" &&
        triggered_rules contains "beneficiary_risk" &&
        event.transaction.amount > 50000
      action: deny
      reason: "High-value transaction to risky beneficiary"
      
    # AI analysis: complex pattern
    - condition: |
        total_score >= 100 ||
        triggered_count >= 3
      action: infer
      infer:
        data_snapshot:
          - event.transaction
          - event.user
          - context.transaction_fraud_detection.triggered_rules
          - context.transaction_fraud_detection.total_score
      reason: "Complex fraud pattern needs analysis"
      
    # Human review: moderate risk
    - condition: total_score >= 60
      action: review
      reason: "Moderate risk transaction"
      
    # Approve: low risk
    - default: true
      action: approve
```

### 9.3 Credit Application Risk

```yaml
version: "0.1"

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
    
  decision_logic:
    # Automatic deny: previous default
    - condition: triggered_rules contains "previous_default"
      action: deny
      reason: "Previous loan default"
      terminate: true
      
    # High risk: low credit + high debt
    - condition: |
        triggered_rules contains "low_credit_score" &&
        triggered_rules contains "high_debt_ratio"
      action: infer
      infer:
        data_snapshot:
          - event.applicant
          - event.application
          - context.credit_application_risk.*
      reason: "Poor credit profile"
      
    # Moderate risk: score threshold
    - condition: total_score >= 100
      action: review
      reason: "Manual underwriting required"
      
    # Low-moderate risk: AI assistance
    - condition: total_score >= 60
      action: infer
      infer:
        data_snapshot:
          - event.applicant
          - context.credit_application_risk.*
      reason: "Borderline case"
      
    # Approve: good credit profile
    - default: true
      action: approve
```

### 9.4 Login Risk with Context

```yaml
version: "0.1"

ruleset:
  id: contextual_login_risk
  name: Contextual Login Risk Detection  
  description: Login risk with user tier and time context
  
  rules:
    - suspicious_device        # score: 50
    - geo_anomaly             # score: 60
    - failed_attempts         # score: 40
    - unusual_time            # score: 30
    
  decision_logic:
    # VIP users: more lenient
    - condition: |
        event.user.tier == "vip" &&
        total_score < 100
      action: approve
      reason: "VIP user, acceptable risk"
      
    # Business hours + moderate risk: AI check
    - condition: |
        total_score >= 60 &&
        hour(event.timestamp) >= 9 &&
        hour(event.timestamp) <= 18
      action: infer
      infer:
        data_snapshot:
          - event.*
          - context.*
      reason: "Moderate risk during business hours"
      
    # Off-hours + any risk: review
    - condition: |
        total_score >= 40 &&
        (hour(event.timestamp) < 6 || hour(event.timestamp) > 22)
      action: review
      reason: "Risk detected during off-hours"
      
    # High risk: always deny
    - condition: total_score >= 120
      action: deny
      reason: "High risk login"
      
    # Default: approve
    - default: true
      action: approve
```

---

## 10. Best Practices

### 10.1 Decision Logic Order

Order decision rules from most specific to most general:

```yaml
decision_logic:
  # 1. Critical rules (short-circuit)
  - condition: critical_condition
    action: deny
    terminate: true
    
  # 2. Specific combinations
  - condition: specific_pattern
    action: infer
    
  # 3. Count-based
  - condition: triggered_count >= 3
    action: review
    
  # 4. Score-based
  - condition: total_score >= 80
    action: review
    
  # 5. Default
  - default: true
    action: approve
```

### 10.2 Use Meaningful Reasons

Always provide clear reasons for audit and explainability:

```yaml
decision_logic:
  - condition: total_score >= 100
    action: deny
    reason: "Risk score {total_score} exceeds threshold"  # Good: specific
    
  - condition: triggered_count >= 3
    action: review
    reason: "Multiple risk indicators: {triggered_rules}"  # Good: detailed
```

### 10.3 Consider Business Context

Adapt decisions based on business context:

```yaml
decision_logic:
  # Different thresholds for different user tiers
  - condition: |
      event.user.tier == "premium" &&
      total_score < 80
    action: approve
    
  - condition: |
      event.user.tier == "basic" &&
      total_score < 50
    action: approve
```

---

## 11. Related Documentation

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

## 12. Summary

A CORINT Ruleset:

- Groups multiple rules into a reusable logical unit
- **Defines decision logic** through `decision_logic` or `decision_template` **[Phase 3]**
- **Supports inheritance** via `extends` for code reuse **[Phase 3]**
- Evaluates rule combinations and context
- **Produces final actions** (approve/deny/review/infer)
- Integrates cleanly with CORINT Pipelines
- Improves modularity and maintainability
- **Uses imports to declare rule dependencies explicitly**

**Key Points:**
- Rules detect and score
- Rulesets decide and act
- Pipelines orchestrate flow
- Imports enable modular, reusable rule libraries

**Phase 3 Features:**
- **Inheritance (`extends`)** - Child rulesets inherit rules from parent, reducing duplication by 27%+
- **Decision Logic Templates** - Reusable decision patterns with parameters, reducing duplication by 70%+
- **Combine both** - Use inheritance for rules and templates for decision logic
- **Compile-time resolution** - Zero runtime overhead, all resolved during compilation

**Code Reuse Benefits:**
- Define common patterns once, reuse everywhere
- Easy customization through parameters and overrides
- Consistent baselines with flexible variants
- Reduced maintenance burden
- Type-safe with compile-time validation

Rulesets are the decision-making foundation of CORINT's Risk Definition Language (RDL).
