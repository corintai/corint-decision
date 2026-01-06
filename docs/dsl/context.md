# CORINT Context Namespace Reference

> **Purpose**: This document provides a concise reference for LLM agents working with CORINT's context system. For detailed documentation, see [CONTEXT_GUIDE.md](../CONTEXT_GUIDE.md).

## Quick Reference

CORINT uses a **flattened namespace architecture** with 7 namespaces organized by processing method:

| Namespace | Mutability | Purpose |
|-----------|------------|---------|
| `event` | Read-only | Raw user request data |
| `features` | Writable | Complex feature computations (DB queries, aggregations) |
| `api` | Writable | External third-party API results |
| `service` | Writable | Internal microservice results |
| `vars` | Writable | Simple variables and calculations |
| `sys` | Read-only | System auto-generated metadata |
| `results` | Read-only | Ruleset execution results (pipeline-level) |

## Access Pattern

All namespaces use dot notation for field access:

```yaml
event.user.id                    # Nested field access
features.transaction_count_7d    # Feature value
api.device_fingerprint.risk_score # Multi-level nesting
results.fraud_detection.signal    # Ruleset result
```

## Namespace Quick Guide

### event - User Request Data
- **Source**: User API requests
- **Access**: `event.<field>` or `event.<nested.field>`
- **Example**: `event.transaction.amount`, `event.user.id`

### features - Complex Computations
- **Source**: Pipeline feature steps (database queries)
- **Access**: `features.<feature_name>`
- **Example**: `features.user_transaction_count_7d`, `features.velocity_score`

### api - External API Results
- **Source**: Pipeline api steps
- **Access**: `api.<api_name>.<field>`
- **Example**: `api.device_fingerprint.risk_score`, `api.ip_geolocation.country`

### service - Internal Service Results
- **Source**: Pipeline service steps
- **Access**: `service.<service_name>.<field>`
- **Example**: `service.user_profile.vip_level`, `service.risk_history.blacklist_hit`

### vars - Simple Variables
- **Source**: Pipeline config + rule calculations
- **Access**: `vars.<variable_name>`
- **Example**: `vars.high_risk_threshold`, `vars.total_fee`

### sys - System Metadata
- **Source**: System auto-generated
- **Access**: `sys.<field>`
- **Common fields**: `sys.request_id`, `sys.timestamp`, `sys.hour`, `sys.environment`, `sys.pipeline_id`

### results - Ruleset Results
- **Source**: Ruleset execution within pipeline
- **Access**: `results.<ruleset_id>.<field>` or `results.<field>` for final decision
- **Example**: `results.fraud_detection.signal`, `results.decision`, `results.actions`

## Decision Tree

```
Data classification:
├─ User request data? → event
├─ System metadata? → sys
├─ Ruleset/pipeline result? → results
└─ Computed data?
   ├─ Simple calculation? → vars
   ├─ Database query/aggregation? → features
   ├─ External API call? → api
   └─ Internal service call? → service
```

## BNF Grammar

### Formal Syntax

```bnf
<namespace-access> ::= <namespace> "." <field-path>
                     | <namespace>

<namespace> ::= "event"
              | "features"
              | "api"
              | "service"
              | "vars"
              | "sys"
              | "results"

<field-path> ::= <identifier>
               | <identifier> "." <field-path>

<identifier> ::= <letter> <identifier-rest>*

<identifier-rest> ::= <letter>
                    | <digit>
                    | "_"

<letter> ::= "a".."z" | "A".."Z"

<digit> ::= "0".."9"
```

### Syntax Examples

**Valid namespace access patterns:**
```yaml
event                                    # Entire namespace
event.type                               # Top-level field
event.user.id                            # Nested field
features.transaction_count_7d            # Feature field
api.device_fingerprint.risk_score        # Multi-level nesting
results.fraud_detection.signal           # Ruleset result
```

**Invalid patterns:**
```yaml
Event.type                   # ❌ Namespace must be lowercase
event._private              # ❌ Field cannot start with _
event.user..id              # ❌ Double dots not allowed
.event.type                 # ❌ Cannot start with dot
event.type.                 # ❌ Cannot end with dot
```

### Context Structure Grammar

```bnf
<execution-context> ::= <namespace-map>+

<namespace-map> ::= <namespace> ":" <value-map>

<value-map> ::= "{" <key-value-pair> ("," <key-value-pair>)* "}"
              | "{" "}"

<key-value-pair> ::= <identifier> ":" <value>

<value> ::= <primitive>
          | <object>
          | <array>

<primitive> ::= <string>
              | <number>
              | <boolean>
              | <null>

<object> ::= "{" <key-value-pair> ("," <key-value-pair>)* "}"
           | "{" "}"

<array> ::= "[" <value> ("," <value>)* "]"
          | "[" "]"
```

## Common Usage Patterns

### Rule Conditions
```yaml
rule:
  when:
    all:
      - event.amount > 1000
      - features.transaction_count_7d > 20
      - api.device_fingerprint.risk_score > 0.7
      - vars.high_risk_threshold < 80
      - sys.hour >= 22
```

### Pipeline Routing
```yaml
pipeline:
  steps:
    - type: router
      routes:
        - when: results.fraud_detection.signal == "decline"
          next: block_step
        - when: results.fraud_detection.total_score > 80
          next: review_step
```

### Pipeline Decision
```yaml
pipeline:
  decision:
    - when: results.fraud_detection.signal == "decline"
      result: decline
      actions: ["BLOCK_DEVICE"]
    - when: results.fraud_detection.signal == "review"
      result: review
      actions: ["KYC"]
```

---

**For detailed documentation, examples, and best practices, see [CONTEXT_GUIDE.md](../CONTEXT_GUIDE.md)**
