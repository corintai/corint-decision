# RDL Imports and Module System

## Overview

RDL provides a module system for code reuse. The `import` mechanism allows importing rules and rulesets from library files.

---

## 1. Basic Syntax

### Import Declaration

Imports are declared at the top of an RDL file using YAML multi-document format:

```yaml
version: "0.1"

# Import section (first document)
import:
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
    - fraud_farm_pattern    # Reference imported rule by ID
    - card_testing          # Reference imported rule by ID
```

### Import Types

| Import Type | Purpose |
|------------|---------|
| `rules` | Import individual rule definitions |
| `rulesets` | Import ruleset definitions |
| `pipelines` | Import pipeline definitions (for pipeline steps) |

---

## 2. BNF Grammar

```bnf
<import>          ::= "import:" <import_sections>

<import_sections> ::= ( "rules:" <path_list> )?
                       ( "rulesets:" <path_list> )?
                       ( "pipelines:" <path_list> )?

<path_list>       ::= "- " <file_path> ( "\n- " <file_path> )*

<file_path>       ::= <string>  // Relative to repository root, e.g., "library/rules/fraud/fraud_farm.yaml"

<rdl_document>    ::= <version>? <import>? "---" <definition>

<definition>      ::= <rule> | <ruleset> | <pipeline>
```

---

## 3. Dependency Hierarchy

```
Pipeline   → import rulesets
Ruleset    → import rules
Rule       → no import (leaf)
```

**Rules**: No dependencies, self-contained detection logic  
**Rulesets**: Import rules, combine with decision logic  
**Pipelines**: Import rulesets, rule dependencies auto-propagate

---

## 4. Core Concepts

### 4.1 Dependency Propagation

When a pipeline imports a ruleset, the compiler automatically loads all rules that the ruleset depends on:

```yaml
# Pipeline imports ruleset
import:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

# Compiler automatically loads:
# 1. fraud_detection_core.yaml
# 2. All rules imported by fraud_detection_core.yaml (transitive)
```

### 4.2 Deduplication

If the same rule is imported multiple times, the compiler automatically deduplicates by ID.

### 4.3 Path Resolution

Import paths are **relative to repository root**:

```yaml
import:
  rules:
    - library/rules/fraud/fraud_farm.yaml  # ✅ Valid
    # ❌ Not supported: ../rules/fraud_farm.yaml (relative paths)
    # ❌ Not supported: ./fraud_farm.yaml (current directory)
```

---

## 5. ID Uniqueness

### Global Uniqueness Requirement

All Rule IDs and Ruleset IDs must be **globally unique** across the entire codebase.

**Rule ID Uniqueness**:
```yaml
# ✅ Valid - unique IDs
rule:
  id: fraud_farm_pattern

rule:
  id: card_testing

# ❌ Invalid - duplicate ID
rule:
  id: fraud_farm_pattern  # ERROR: Already defined!
```

**Ruleset ID Uniqueness**:
```yaml
# ✅ Valid - unique IDs
ruleset:
  id: fraud_detection_core

ruleset:
  id: payment_standard

# ❌ Invalid - duplicate ID
ruleset:
  id: fraud_detection_core  # ERROR: Already defined!
```

**Namespace Separation**: Rule IDs and Ruleset IDs should not conflict (avoid same ID for rule and ruleset).

### ID Naming Conventions

- **Rule IDs**: `<category>_<specific_pattern>` (e.g., `fraud_farm_pattern`, `payment_velocity_check`)
- **Ruleset IDs**: `<domain>_<purpose>_<variant?>` (e.g., `fraud_detection_core`, `payment_standard`)

---

## 6. Circular Dependency Detection

Circular dependencies occur when A imports B, and B imports A (directly or indirectly).

**Compiler Detection**: The compiler maintains a loading stack and detects cycles at compile time.

**How to Fix**:
1. Extract common dependencies to a shared ruleset
2. Flatten dependencies (import rules directly instead of rulesets)

---

## 7. Minimal Example

### Rule File

```yaml
# library/rules/fraud/fraud_farm.yaml
version: "0.1"

rule:
  id: fraud_farm_pattern
  when:
    all:
      - ip_device_count > 10
  score: 100
```

### Ruleset with Import

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
    - fraud_farm_pattern
    - account_takeover_pattern
  conclusion:
    - when: total_score >= 100
      signal: decline
    - default: true
      signal: approve
```

### Pipeline with Ruleset Import

```yaml
# pipelines/fraud_detection.yaml
version: "0.1"

import:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

pipeline:
  id: fraud_detection_pipeline
  entry: fraud_check
  when:
    all:
      - event.type == "transaction"
  steps:
    - step:
        id: fraud_check
        type: ruleset
        ruleset: fraud_detection_core
```

---

## 8. Compilation Process

### Import Resolution Steps

1. Parse the main file
2. Extract import declarations
3. Load imported files recursively
4. Detect circular dependencies
5. Merge all rules and rulesets
6. Deduplicate by ID
7. Validate ID uniqueness
8. Check reference validity
9. Generate executable IR

### Compile-Time Checks

| Check | Error Type |
|-------|-----------|
| File exists | `ImportNotFound` |
| Valid YAML | `InvalidYaml` |
| Correct structure | `NoRuleInFile`, `NoRulesetInFile` |
| ID uniqueness | `DuplicateRuleId`, `DuplicateRulesetId` |
| No circular deps | `CircularDependency` |
| Valid references | `RuleNotFound` |

---

## 9. Grammar Summary

```
import:
  rules: [path, ...]
  rulesets: [path, ...]
  pipelines: [path, ...]

Path Format:
  Relative to repository root: "library/rules/category/name.yaml"

Reference:
  Imported rules/rulesets referenced by ID in rules/rulesets arrays

Dependency Flow:
  Pipeline → Rulesets → Rules (auto-propagated)
```

---

**Version**: 0.1  
**Target**: LLM-readable DSL specification  
**Status**: Core import system specification
