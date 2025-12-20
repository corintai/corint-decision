# CORINT Risk Definition Language (RDL)
## Pipeline Registry Specification (v0.1)

A **Pipeline Registry** defines the entry point routing for event processing in CORINT's Cognitive Risk Intelligence framework.  
It provides a declarative, ordered list of pipeline matching rules that determine which pipeline should execute for a given event.

The registry enables centralized pipeline routing with priority-based matching, ensuring only the first matching pipeline executes for each event.

---

## 1. Registry Structure

### 1.1 Basic Structure

```yaml
version: "0.1"

registry:
  - pipeline: login_pipeline
    when:
      event.type: login

  - pipeline: register_pipeline
    when:
      event.type: register

  - pipeline: payment_br_pipeline
    when:
      event.type: payment
      conditions:
        - geo.country == "BR"

  - pipeline: payment_main_pipeline
    when:
      event.type: payment

  - pipeline: loan_pipeline
    when:
      event.type: loan_application

  - pipeline: payment_shadow_pipeline
    when:
      event.type: payment
      conditions:
        - event.shadow == true
```

### 1.2 Registry Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `registry` | array | Yes | Ordered list of pipeline routing entries |
| `pipeline` | string | Yes | Pipeline ID that references a defined pipeline |
| `when` | object | Yes | When block that determines if this entry matches the event (same format as rule/pipeline when blocks) |

---

## 2. Matching Behavior

### 2.1 Order-Based Matching

The registry evaluates entries **top-to-bottom** in the order they are defined:

1. Each entry's `when` expression is evaluated against the incoming event
2. The **first matching entry** triggers its pipeline execution
3. **No further entries are evaluated** after a match is found
4. If no entry matches, no pipeline executes (unless a default fallback is provided)

### 2.2 Priority and Specificity

More specific conditions should be placed **before** more general ones:

```yaml
registry:
  # ✅ Correct: Specific conditions first
  - pipeline: payment_br_pipeline
    when:
      event.type: payment
      conditions:
        - geo.country == "BR"
  
  - pipeline: payment_main_pipeline
    when:
      event.type: payment

  # ❌ Wrong: General condition first (would always match first)
  - pipeline: payment_main_pipeline
    when:
      event.type: payment
  
  - pipeline: payment_br_pipeline
    when:
      event.type: payment
      conditions:
        - geo.country == "BR"
```

### 2.3 Default Fallback

To provide a default pipeline when no other conditions match, you can use a catch-all entry at the end. However, since registry `when` blocks use the same format as rules, you cannot use `when: "true"`. Instead, omit the `when` block or use a condition that always evaluates to true:

```yaml
registry:
  - pipeline: login_pipeline
    when:
      event.type: login
  
  - pipeline: payment_pipeline
    when:
      event.type: payment
  
  # Default fallback - no when block means always matches
  # Or use: when: { conditions: ["1 == 1"] }
  - pipeline: default_pipeline
```

---

## 3. When Block Structure

The `when` field uses the **same structure as rule and pipeline `when` blocks**, ensuring consistency across the DSL.

### 3.1 Basic Structure

```yaml
when:
  all:
    - event.type == "login"      # Event type filter
    - geo.country == "BR"        # Additional condition
    - amount > 1000              # Additional condition
```

### 3.2 Event Field Filtering

You can filter on **any event field** for fast matching. The `when` block supports direct field matching:

```yaml
# Filter by event type
when:
  event.type: login

# Filter by channel
when:
  event.channel: stripe

# Filter by nested fields
when:
  event.country.city: sao_paulo

# Combine multiple field filters
when:
  event.type: payment
  event.channel: stripe
  event.currency: BRL
```

**Note**: All field filters in the `when` block are combined with **AND** logic. All must match for the entry to be selected.

### 3.3 Conditions

The `conditions` array contains expressions that are evaluated with **AND** logic:

```yaml
when:
  event.type: payment
  when:
    all:
    - geo.country == "BR"
    - amount > 1000
    - user.verified == true
```

### 3.4 Supported Operators

Conditions support the same operators as rule conditions:

| Operator | Meaning | Example |
|----------|---------|---------|
| `==` | equal | `geo.country == "BR"` |
| `!=` | not equal | `geo.country != "US"` |
| `<, >, <=, >=` | numeric comparison | `amount > 1000` |
| `in` | membership in array | `geo.country in ["BR", "MX"]` |
| `regex` | regular-expression match | `email regex ".*@example.com"` |
| `exists` | field exists | `event.shadow exists` |
| `missing` | field is missing | `event.shadow missing` |

### 3.5 Complex Conditions

You can use logical operators within conditions:

```yaml
when:
  event.type: payment
  when:
    all:
    - geo.country == "BR" || geo.country == "MX"
    - amount > 1000 && amount < 10000
    - user.verified == true || user.whitelisted == true
```

### 3.6 Field Access

Fields can be accessed using dot notation:

```yaml
when:
  event.type: payment
  when:
    all:
    - geo.country == "US"              # Nested object field
    - user.profile.tier == "premium"    # Deep nesting
    - event.shadow == true              # Event data field
    - event.amount > 1000               # Event data field
```

---

## 4. Pipeline Reference

### 4.1 Pipeline ID Mapping

The `pipeline` field must reference a pipeline defined in your rule files:

```yaml
# registry.yaml
registry:
  - pipeline: login_pipeline
    when: "event.type == 'login'"

# login_rules.yaml
pipeline:
  id: login_pipeline  # Must match the registry reference
  when:
    event.type: login
  steps:
    - include:
        ruleset: login_risk_assessment
```

### 4.2 Pipeline When Condition

Pipelines defined in rule files **retain their own `when` conditions**:

- **Registry `when`**: Entry-level routing (determines which pipeline to try)
- **Pipeline `when`**: Final validation (executed inside the pipeline)

Both conditions must be satisfied for the pipeline to execute:

```yaml
# Registry entry
registry:
  - pipeline: payment_pipeline
    when: event.type == 'payment'  # Entry-level check

# Pipeline definition
pipeline:
  id: payment_pipeline
  when:
    event.channel: stripe              # Final validation
    all:
      - amount > 0                    # Additional validation
  steps:
    - include:
        ruleset: payment_rules
```

---

## 5. Execution Flow

### 5.1 Request Processing

```
Event Request
    ↓
Registry Matching (top-to-bottom)
    ↓
First Matching Entry Found?
    ├─ Yes → Execute Referenced Pipeline
    │         ↓
    │    Pipeline When Check
    │         ↓
    │    Pipeline Steps Execution
    │         ↓
    │    Return Decision Result
    │
    └─ No → No Pipeline Executed
            (or default fallback if configured)
```

### 5.2 Example Execution

Given this registry:

```yaml
registry:
  - pipeline: payment_br_pipeline
    when:
      event.type: payment
      conditions:
        - geo.country == "BR"
  
  - pipeline: payment_main_pipeline
    when:
      event.type: payment
```

**Event 1**: `{event.type: "payment", geo.country: "BR"}`
- ✅ Matches first entry → `payment_br_pipeline` executes
- ❌ Second entry not evaluated

**Event 2**: `{event.type: "payment", geo.country: "US"}`
- ❌ First entry doesn't match (condition fails)
- ✅ Matches second entry → `payment_main_pipeline` executes

**Event 3**: `{event.type: "login"}`
- ❌ No entries match → No pipeline executes

---

## 6. Best Practices

### 6.1 Ordering

- **Place specific conditions before general ones**
- **Group related pipelines together**
- **Add default fallback at the end** (if needed)

### 6.2 Naming

- Use descriptive pipeline IDs that indicate their purpose
- Follow consistent naming conventions (e.g., `{event_type}_pipeline`)

### 6.3 Organization

- Keep registry file separate from rule files
- One registry file per environment or domain
- Document complex matching logic

### 6.4 Performance

- Registry matching is fast (expression evaluation)
- Only the first matching pipeline executes
- Pipeline `when` conditions provide additional filtering

---

## 7. Complete Example

### 7.1 Registry File (`registry.yaml`)

```yaml
version: "0.1"

registry:
  # Login events
  - pipeline: login_pipeline
    when:
      event.type: login

  # Registration events
  - pipeline: register_pipeline
    when:
      event.type: register

  # Stripe payment events
  - pipeline: stripe_payment_pipeline
    when:
      event.type: payment
      event.channel: stripe

  # Payment events - Brazil specific
  - pipeline: payment_br_pipeline
    when:
      event.type: payment
      conditions:
        - geo.country == "BR"

  # Payment events - Main pipeline
  - pipeline: payment_main_pipeline
    when:
      event.type: payment

  # Loan application events
  - pipeline: loan_pipeline
    when:
      event.type: loan_application

  # High-value transactions (filter by event field)
  - pipeline: high_value_pipeline
    when:
      event.type: transaction
      conditions:
        - event.amount > 10000

  # City-specific routing
  - pipeline: sao_paulo_pipeline
    when:
      event.country.city: sao_paulo

  # Shadow mode payments (testing)
  - pipeline: payment_shadow_pipeline
    when:
      event.type: payment
      conditions:
        - event.shadow == true

  # Default fallback (no when block = always matches)
  - pipeline: default_pipeline
```

### 7.2 Pipeline Definition Example (`payment_rules.yaml`)

```yaml
version: "0.1"

pipeline:
  id: payment_main_pipeline
  name: Payment Risk Assessment Pipeline
  description: Main payment processing pipeline with risk assessment
  
  when:
    event.type: payment
    all:
      - amount > 0
  
  steps:
    - include:
        ruleset: payment_risk_assessment
```

### 7.3 Integration

The registry is loaded at engine startup and used for every decision request:

1. Engine loads registry from `registry.yaml`
2. Engine loads pipeline definitions from rule files
3. On request, registry matches event to pipeline
4. Matched pipeline executes with its own `when` validation
5. Decision result returned

---

## 8. Error Handling

### 8.1 Missing Pipeline Reference

If a registry entry references a pipeline that doesn't exist:

- Engine logs a warning at startup
- Registry entry is skipped during matching
- Request continues to next registry entry

### 8.2 Invalid When Block

If a `when` block is invalid:

- Engine logs an error at startup
- Registry entry is skipped
- Request may fail if no valid entries remain

### 8.3 No Matching Entry

If no registry entry matches the event:

- No pipeline executes
- Decision engine returns a default result (if configured)
- Or returns an error indicating no matching pipeline

---

## 9. Summary

The Pipeline Registry provides:

- ✅ **Centralized routing**: Single entry point for pipeline selection
- ✅ **Priority-based matching**: First match wins, no duplicates
- ✅ **Clear ordering**: Explicit control over pipeline execution order
- ✅ **Expression-based**: Flexible matching using CORINT expression syntax
- ✅ **Backward compatible**: Pipeline `when` conditions still work as final validation

This design ensures predictable, efficient pipeline routing while maintaining the flexibility of pipeline-level conditions.
