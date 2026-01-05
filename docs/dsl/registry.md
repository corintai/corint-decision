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
      all:
        - event.type == "login"

  - pipeline: register_pipeline
    when:
      all:
        - event.type == "register"

  - pipeline: payment_br_pipeline
    when:
      all:
        - event.type == "payment"
        - geo.country == "BR"

  - pipeline: payment_main_pipeline
    when:
      all:
        - event.type == "payment"

  - pipeline: loan_pipeline
    when:
      all:
        - event.type == "loan_application"

  - pipeline: payment_shadow_pipeline
    when:
      all:
        - event.type == "payment"
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
      all:
        - event.type == "payment"
        - geo.country == "BR"

  - pipeline: payment_main_pipeline
    when:
      all:
        - event.type == "payment"

  # ❌ Wrong: General condition first (would always match first)
  - pipeline: payment_main_pipeline
    when:
      all:
        - event.type == "payment"

  - pipeline: payment_br_pipeline
    when:
      all:
        - event.type == "payment"
        - geo.country == "BR"
```

### 2.3 Default Fallback

To provide a default pipeline when no other conditions match, you can use a catch-all entry at the end:

```yaml
registry:
  - pipeline: login_pipeline
    when:
      all:
        - event.type == "login"

  - pipeline: payment_pipeline
    when:
      all:
        - event.type == "payment"

  # Default fallback - no when block means always matches
  - pipeline: default_pipeline
```

---

## 3. When Block Structure

The `when` field uses the **same structure as rule and pipeline `when` blocks**, ensuring consistency across the DSL.

**Basic example:**

```yaml
when:
  all:
    - event.type == "login"
    - geo.country == "BR"
    - amount > 1000
```

**Logical operators:**
- `all:` - AND logic (all conditions must match)
- `any:` - OR logic (at least one condition must match)

For complete `when` block syntax, supported operators, and expression details, see [expression.md](expression.md) and [rule.md](rule.md).

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
    all:
      - event.type == "login"
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
    all:
      - event.channel == "stripe"        # Final validation
      - amount > 0                       # Additional validation
  steps:
    - include:
        ruleset: payment_rules
```

**Execution flow:**
```
Event Request → Registry Matching (top-to-bottom) → First Match?
                                                      ├─ Yes → Execute Pipeline
                                                      └─ No  → No pipeline (or default)
```

---

## 5. Best Practices

### 5.1 Ordering

- **Place specific conditions before general ones**
- **Group related pipelines together**
- **Add default fallback at the end** (if needed)

### 5.2 Naming

- Use descriptive pipeline IDs that indicate their purpose
- Follow consistent naming conventions (e.g., `{event_type}_pipeline`)

### 5.3 Organization

- Keep registry file separate from rule files
- One registry file per environment or domain
- Document complex matching logic

### 5.4 Performance

- Registry matching is fast (expression evaluation)
- Only the first matching pipeline executes
- Pipeline `when` conditions provide additional filtering

---

## 6. Complete Example

### 6.1 Registry File (`registry.yaml`)

```yaml
version: "0.1"

registry:
  # Login events
  - pipeline: login_pipeline
    when:
      all:
        - event.type == "login"

  # Payment events - Brazil specific (more specific condition first)
  - pipeline: payment_br_pipeline
    when:
      all:
        - event.type == "payment"
        - geo.country == "BR"

  # Payment events - Main pipeline (general condition after specific)
  - pipeline: payment_main_pipeline
    when:
      all:
        - event.type == "payment"

  # Loan application events
  - pipeline: loan_pipeline
    when:
      all:
        - event.type == "loan_application"

  # Default fallback
  - pipeline: default_pipeline
```

### 6.2 Pipeline Definition Example (`payment_rules.yaml`)

```yaml
version: "0.1"

pipeline:
  id: payment_main_pipeline
  name: Payment Risk Assessment Pipeline
  description: Main payment processing pipeline with risk assessment

  when:
    all:
      - event.type == "payment"
      - amount > 0

  steps:
    - include:
        ruleset: payment_risk_assessment
```

### 6.3 Integration

The registry is loaded at engine startup and used for every decision request:

1. Engine loads registry from `registry.yaml`
2. Engine loads pipeline definitions from rule files
3. On request, registry matches event to pipeline
4. Matched pipeline executes with its own `when` validation
5. Decision result returned

---

## 7. Error Handling

### 7.1 Missing Pipeline Reference

If a registry entry references a pipeline that doesn't exist:

- Engine logs a warning at startup
- Registry entry is skipped during matching
- Request continues to next registry entry

### 7.2 Invalid When Block

If a `when` block is invalid:

- Engine logs an error at startup
- Registry entry is skipped
- Request may fail if no valid entries remain

### 7.3 No Matching Entry

If no registry entry matches the event:

- No pipeline executes
- Decision engine returns a default result (if configured)
- Or returns an error indicating no matching pipeline

---

## 8. Summary

The Pipeline Registry provides:

- ✅ **Centralized routing**: Single entry point for pipeline selection
- ✅ **Priority-based matching**: First match wins, no duplicates
- ✅ **Clear ordering**: Explicit control over pipeline execution order
- ✅ **Expression-based**: Flexible matching using CORINT expression syntax
- ✅ **Backward compatible**: Pipeline `when` conditions still work as final validation

This design ensures predictable, efficient pipeline routing while maintaining the flexibility of pipeline-level conditions.
