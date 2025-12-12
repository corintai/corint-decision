# RDL Imports and Module System
## Complete Specification (v0.1)

## 1. Overview

RDL provides a powerful module system for code reuse and maintainability. The `imports` mechanism allows you to:

- Create reusable rule libraries
- Build composable rulesets
- Reduce code duplication
- Maintain consistency across pipelines
- Enable team collaboration
- Support versioning and updates

---

## 2. Basic Syntax

### 2.1 Import Declaration

Imports are declared at the top of an RDL file using YAML multi-document format:

```yaml
version: "0.1"

# Import section (first document)
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/payment/card_testing.yaml
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

# Definition section (second document)
ruleset:
  id: my_ruleset
  rules:
    - fraud_farm_pattern    # Reference imported rule
    - card_testing          # Reference imported rule
```

### 2.2 Import Types

| Import Type | Purpose | Example |
|------------|---------|---------|
| `rules` | Import individual rule definitions | `library/rules/fraud/fraud_farm.yaml` |
| `rulesets` | Import ruleset definitions | `library/rulesets/fraud_detection_core.yaml` |
| `templates` | Import decision logic templates | `library/templates/score_based_decision.yaml` |

---

## 3. Dependency Hierarchy

### 3.1 Three-Layer Structure

```
┌─────────────┐
│  Pipeline   │  ← imports rulesets
└──────┬──────┘
       │
┌──────▼──────┐
│  Ruleset    │  ← imports rules
└──────┬──────┘
       │
┌──────▼──────┐
│    Rule     │  ← no imports (leaf)
└─────────────┘
```

### 3.2 Layer Responsibilities

**Rules (Layer 1 - Detectors)**
- No dependencies
- Self-contained detection logic
- No imports needed

```yaml
version: "0.1"

rule:
  id: fraud_farm_pattern
  name: Fraud Farm Detection
  when:
    conditions:
      - ip_device_count > 10
  score: 100
```

**Rulesets (Layer 2 - Decision Makers)**
- Import rules they use
- Combine rules with decision logic
- Explicit dependency declaration

```yaml
version: "0.1"

imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/account_takeover.yaml

---

ruleset:
  id: fraud_detection_core
  rules:
    - fraud_farm_pattern
    - account_takeover_pattern
  decision_logic:
    - condition: total_score >= 100
      action: deny
```

**Pipelines (Layer 3 - Orchestrators)**
- Import rulesets they use
- Rule dependencies auto-propagate
- Focus on business flow

```yaml
version: "0.1"

imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

pipeline:
  id: fraud_detection_pipeline
  when:
    event.type: transaction
  steps:
    - include:
        ruleset: fraud_detection_core
```

---

## 4. Dependency Propagation

### 4.1 Automatic Dependency Loading

When a pipeline imports a ruleset, the compiler automatically loads all the rules that the ruleset depends on:

```yaml
# Pipeline imports ruleset
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

# Compiler automatically loads:
# 1. fraud_detection_core.yaml
# 2. All rules imported by fraud_detection_core.yaml:
#    - library/rules/fraud/fraud_farm.yaml
#    - library/rules/fraud/account_takeover.yaml
#    - library/rules/fraud/velocity_abuse.yaml
#    - ... (all dependencies)
```

### 4.2 Deduplication

If the same rule is imported multiple times (e.g., by different rulesets), the compiler automatically deduplicates:

```yaml
# Both rulesets import card_testing
imports:
  rulesets:
    - library/rulesets/payment_standard.yaml      # imports card_testing
    - library/rulesets/payment_high_value.yaml    # also imports card_testing

# Compiler loads card_testing only once
```

---

## 5. ID Uniqueness

### 5.1 Global Uniqueness Requirement

All Rule IDs and Ruleset IDs must be globally unique across the entire codebase.

**Rule ID Uniqueness**
```yaml
# ✅ Valid - unique IDs
library/rules/fraud/fraud_farm.yaml:
  rule:
    id: fraud_farm_pattern

library/rules/payment/card_testing.yaml:
  rule:
    id: card_testing

# ❌ Invalid - duplicate ID
library/rules/custom/fraud_farm.yaml:
  rule:
    id: fraud_farm_pattern  # ERROR: Already defined!
```

**Ruleset ID Uniqueness**
```yaml
# ✅ Valid - unique IDs
library/rulesets/fraud_detection_core.yaml:
  ruleset:
    id: fraud_detection_core

library/rulesets/payment_standard.yaml:
  ruleset:
    id: payment_standard

# ❌ Invalid - duplicate ID
library/rulesets/fraud_v2.yaml:
  ruleset:
    id: fraud_detection_core  # ERROR: Already defined!
```

### 5.2 Namespace Separation

Rule IDs and Ruleset IDs should not conflict:

```yaml
# ❌ Avoid this - confusing
rule:
  id: fraud_detection        # Rule ID

ruleset:
  id: fraud_detection        # Ruleset ID - same as rule!

# ✅ Better - clear distinction
rule:
  id: fraud_farm_pattern     # Rule ends with _pattern

ruleset:
  id: fraud_detection_core   # Ruleset has purpose suffix
```

### 5.3 ID Naming Conventions

**Rule IDs**
- Format: `<category>_<specific_pattern>`
- Examples: `fraud_farm_pattern`, `payment_velocity_check`, `geo_impossible_travel`

**Ruleset IDs**
- Format: `<domain>_<purpose>_<variant?>`
- Examples: `fraud_detection_core`, `payment_standard`, `payment_high_value`

---

## 6. Circular Dependency Detection

### 6.1 What are Circular Dependencies?

Circular dependencies occur when A imports B, and B imports A (directly or indirectly).

```yaml
# ❌ Direct circular dependency
# ruleset_a.yaml
imports:
  rulesets:
    - library/rulesets/ruleset_b.yaml

# ruleset_b.yaml
imports:
  rulesets:
    - library/rulesets/ruleset_a.yaml  # ERROR: Circular!
```

### 6.2 Compiler Detection

The compiler maintains a loading stack and detects cycles:

```
Loading: ruleset_a.yaml
  Loading: ruleset_b.yaml
    Loading: ruleset_c.yaml
      Loading: ruleset_a.yaml  # ERROR: Already in stack!

Error: Circular dependency detected
  Loading stack: ruleset_a -> ruleset_b -> ruleset_c -> ruleset_a
```

### 6.3 How to Fix

**Option 1: Extract Common Dependencies**
```yaml
# Instead of A ↔ B, create:
# A → C ← B (both depend on C)

# common_rules.yaml
ruleset:
  id: common_fraud_rules
  rules: [rule1, rule2]

# ruleset_a.yaml
imports:
  rulesets:
    - library/rulesets/common_fraud_rules.yaml

# ruleset_b.yaml
imports:
  rulesets:
    - library/rulesets/common_fraud_rules.yaml
```

**Option 2: Flatten Dependencies**
```yaml
# Instead of ruleset importing ruleset,
# import rules directly

# ruleset_a.yaml
imports:
  rules:
    - library/rules/rule1.yaml
    - library/rules/rule2.yaml
```

---

## 7. Directory Structure

### 7.1 Recommended Layout

```
repository/
├── library/
│   ├── rules/              # Reusable rules
│   │   ├── fraud/
│   │   │   ├── fraud_farm.yaml
│   │   │   ├── account_takeover.yaml
│   │   │   └── velocity_abuse.yaml
│   │   ├── payment/
│   │   │   ├── card_testing.yaml
│   │   │   ├── velocity_check.yaml
│   │   │   └── new_account_risk.yaml
│   │   └── geography/
│   │       ├── suspicious_geography.yaml
│   │       └── suspicious_ip.yaml
│   │
│   ├── rulesets/           # Reusable rulesets
│   │   ├── fraud_detection_core.yaml
│   │   ├── payment_standard.yaml
│   │   └── payment_high_value.yaml
│   │
│   └── templates/          # Decision logic templates
│       └── score_based_decision.yaml
│
├── pipelines/              # Business pipelines
│   ├── fraud_detection.yaml
│   └── payment_pipeline.yaml
│
└── configs/                # Runtime configs
    ├── datasources/
    ├── features/
    └── apis/
```

### 7.2 Path Resolution

Import paths are relative to the repository root:

```yaml
# Absolute path from repository root
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

# ❌ Not supported (yet)
imports:
  rules:
    - ../rules/fraud_farm.yaml        # Relative paths
    - ./fraud_farm.yaml                # Current directory
```

---

## 8. Complete Examples

### 8.1 Rule Library File

```yaml
# library/rules/fraud/fraud_farm.yaml
version: "0.1"

rule:
  id: fraud_farm_pattern
  name: Fraud Farm Detection
  description: Detect organized fraud farms with high IP/device association

  when:
    conditions:
      - ip_device_count > 10
      - ip_user_count > 5

  score: 100

  metadata:
    category: fraud
    severity: critical
    tags: [organized_fraud, bot_networks]
    rule_version: "1.0.0"
    last_updated: "2024-12-11"
```

### 8.2 Ruleset with Imports

```yaml
# library/rulesets/fraud_detection_core.yaml
version: "0.1"

# Explicit imports
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
  description: Reusable fraud detection with 6 common patterns

  # Reference imported rules by ID
  rules:
    - fraud_farm_pattern
    - account_takeover_pattern
    - velocity_abuse_pattern
    - amount_outlier_pattern
    - suspicious_geography_pattern
    - new_user_fraud_pattern

  decision_logic:
    # Critical patterns
    - condition: triggered_rules contains "fraud_farm_pattern"
      action: deny
      reason: "Critical: Fraud farm detected"
      terminate: true

    # Score thresholds
    - condition: total_score >= 200
      action: deny
      reason: "Critical risk (score: {total_score})"
      terminate: true

    - condition: total_score >= 100
      action: deny
      reason: "High risk (score: {total_score})"
      terminate: true

    - condition: total_score >= 60
      action: review
      reason: "Medium-high risk (score: {total_score})"

    - default: true
      action: approve
      reason: "Low risk"

  metadata:
    version: "1.0.0"
    last_updated: "2024-12-11"
```

### 8.3 Pipeline with Ruleset Import

```yaml
# pipelines/fraud_detection.yaml
version: "0.1"

# Import ruleset (rules are auto-loaded)
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline
  description: Production fraud detection using reusable components

  when:
    event.type: transaction

  steps:
    - include:
        ruleset: fraud_detection_core
```

### 8.4 Custom Ruleset Extending Library

```yaml
# my_custom_ruleset.yaml
version: "0.1"

# Import library rules + custom rules
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/payment/card_testing.yaml
    - my_custom_rule.yaml                        # Custom rule

---

ruleset:
  id: my_custom_fraud_detection
  name: My Custom Fraud Detection

  rules:
    - fraud_farm_pattern        # From library
    - card_testing              # From library
    - my_custom_rule            # Custom

  decision_logic:
    - condition: total_score >= 150
      action: deny
    - default: true
      action: approve
```

### 8.5 Multiple Rulesets in Pipeline

```yaml
# complex_pipeline.yaml
version: "0.1"

imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml
    - library/rulesets/payment_standard.yaml

---

pipeline:
  id: comprehensive_risk_pipeline
  name: Comprehensive Risk Assessment

  when:
    event.type: transaction

  steps:
    # Step 1: Fraud detection
    - include:
        ruleset: fraud_detection_core

    # Step 2: Payment validation
    - include:
        ruleset: payment_standard
```

---

## 9. Compilation Process

### 9.1 Import Resolution Steps

1. **Parse** the main file
2. **Extract** import declarations
3. **Load** imported files recursively
4. **Detect** circular dependencies
5. **Merge** all rules and rulesets
6. **Deduplicate** by ID
7. **Validate** ID uniqueness
8. **Check** reference validity
9. **Generate** executable IR

### 9.2 Compile-Time Checks

| Check | Description | Error Type |
|-------|-------------|-----------|
| File exists | Import paths must point to valid files | `ImportNotFound` |
| Valid YAML | All files must be valid YAML | `InvalidYaml` |
| Correct structure | Files must contain expected components | `NoRuleInFile`, `NoRulesetInFile` |
| ID uniqueness | All IDs must be globally unique | `DuplicateRuleId`, `DuplicateRulesetId` |
| No namespace conflicts | Rule IDs ≠ Ruleset IDs | `IdConflict` |
| No circular deps | No import cycles | `CircularDependency` |
| Valid references | All rule IDs referenced in rulesets exist | `RuleNotFound` |

### 9.3 Error Messages

**Example: Duplicate Rule ID**
```
Error: Duplicate rule ID: 'fraud_farm_pattern'
  First defined in: library/rules/fraud/fraud_farm.yaml
  Also defined in: library/rules/custom/fraud_farm.yaml

Hint: Each rule must have a globally unique ID
```

**Example: Circular Dependency**
```
Error: Circular dependency detected: 'library/rulesets/ruleset_b.yaml'
  Loading stack: ruleset_a.yaml -> ruleset_b.yaml -> ruleset_c.yaml -> ruleset_a.yaml

Hint: Extract common dependencies to a shared ruleset
```

**Example: Import Not Found**
```
Error: Import not found: 'library/rules/fraud/missing_rule.yaml'
  Imported from: library/rulesets/fraud_detection_core.yaml

Hint: Check the file path and ensure the file exists
```

---

## 10. Best Practices

### 10.1 Naming Conventions

**Rule Files**
- Use descriptive names: `fraud_farm.yaml` (not `rule1.yaml`)
- Match file name to rule ID: `fraud_farm.yaml` → `fraud_farm_pattern`
- Organize by category: `fraud/`, `payment/`, `geography/`

**Ruleset Files**
- Use purpose-based names: `fraud_detection_core.yaml`
- Add variant suffixes: `payment_standard.yaml`, `payment_high_value.yaml`
- Keep names concise but clear

### 10.2 Metadata

Always include comprehensive metadata:

```yaml
rule:
  id: fraud_farm_pattern
  metadata:
    category: fraud
    severity: critical
    tags: [organized_fraud, bot_networks]
    rule_version: "1.0.0"
    last_updated: "2024-12-11"
    author: "Risk Team"
    changelog:
      - version: "1.0.0"
        date: "2024-12-11"
        changes: "Initial version"
```

### 10.3 Documentation

Add clear descriptions:

```yaml
rule:
  id: fraud_farm_pattern
  name: Fraud Farm Detection
  description: |
    Detect organized fraud farms with high IP/device association.

    This rule triggers when:
    - More than 10 devices are seen from the same IP within 5 hours
    - More than 5 users are associated with the same IP

    Common in: organized fraud, bot networks, credential stuffing
```

### 10.4 Testing

Test rules and rulesets independently:

```yaml
# library/rules/fraud/fraud_farm.test.yaml
tests:
  - name: "Fraud farm detected"
    input:
      ip_device_count: 15
      ip_user_count: 8
    expected:
      triggered: true
      score: 100

  - name: "Normal traffic"
    input:
      ip_device_count: 2
      ip_user_count: 1
    expected:
      triggered: false
      score: 0
```

### 10.5 Versioning

Use semantic versioning in metadata:

```yaml
metadata:
  rule_version: "2.1.0"  # major.minor.patch
  changelog:
    - version: "2.1.0"
      date: "2024-12-11"
      changes: "Adjusted threshold from 5 to 10"
    - version: "2.0.0"
      date: "2024-11-01"
      changes: "Added ip_user_count condition"
    - version: "1.0.0"
      date: "2024-10-01"
      changes: "Initial version"
```

---

## 11. Advanced Features (Future)

### 11.1 Wildcard Imports (Planned)

```yaml
# Import all rules from a directory
imports:
  rules:
    - library/rules/fraud/*
    - library/rules/payment/*.yaml
```

### 11.2 Package Imports (Planned)

```yaml
# Import pre-defined packages
imports:
  packages:
    - fraud        # Equivalent to library/rules/fraud/*
    - payment      # Equivalent to library/rules/payment/*
```

### 11.3 Selective Exclusion (Planned)

```yaml
# Import with exclusions
imports:
  rules:
    - library/rules/fraud/*
    exclude:
      - library/rules/fraud/experimental_rule.yaml
```

### 11.4 Aliasing (Planned)

```yaml
# Import with aliasing to avoid conflicts
imports:
  rules:
    - library/rules/fraud/velocity_abuse.yaml as fraud_velocity
    - library/rules/payment/velocity_check.yaml as payment_velocity
```

---

## 12. Migration Guide

### 12.1 From Monolithic to Modular

**Before (Monolithic)**
```yaml
# All in one file (338 lines)
version: "0.1"
---
rule:
  id: fraud_farm_pattern
  ...
---
rule:
  id: account_takeover_pattern
  ...
---
ruleset:
  id: fraud_detection
  rules: [fraud_farm_pattern, account_takeover_pattern]
  ...
---
pipeline:
  id: fraud_detection_pipeline
  ...
```

**After (Modular)**
```yaml
# library/rules/fraud/fraud_farm.yaml (15 lines)
version: "0.1"
rule:
  id: fraud_farm_pattern
  ...

# library/rules/fraud/account_takeover.yaml (15 lines)
version: "0.1"
rule:
  id: account_takeover_pattern
  ...

# library/rulesets/fraud_detection_core.yaml (50 lines)
version: "0.1"
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/account_takeover.yaml
---
ruleset:
  id: fraud_detection_core
  rules: [fraud_farm_pattern, account_takeover_pattern]
  ...

# pipelines/fraud_detection.yaml (24 lines)
version: "0.1"
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml
---
pipeline:
  id: fraud_detection_pipeline
  steps:
    - include:
        ruleset: fraud_detection_core
```

**Result**: 92% reduction in pipeline file size, 100% reusability

---

## 13. Summary

The RDL import system provides:

✅ **Code Reuse**: Define once, use everywhere
✅ **Maintainability**: Update in one place
✅ **Testability**: Test components independently
✅ **Collaboration**: Teams work on separate modules
✅ **Safety**: Compile-time validation
✅ **Performance**: Zero runtime overhead (compile-time merging)
✅ **Flexibility**: Mix library and custom components

**Next Steps**:
- See `rule.md` for rule specification
- See `ruleset.md` for ruleset specification
- See `pipeline.md` for pipeline orchestration
- See `../repository/README.md` for library usage guide
