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
  action: approve | deny | review | escalate | <custom>
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

---

## 9. `action`

The action to take when a rule triggers.

Supported built-in actions:

- `approve`
- `deny`
- `review`
- `escalate`

Custom object-based actions may also be defined:

```yaml
action:
  type: escalate
  to: human_agent
```

---

## 10. Complete Examples

### 10.1 Login Risk Example

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
  action: review
```

---

### 10.2 Loan Application Consistency Rule

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
  action: deny
```

---

## 11. Summary

A CORINT Rule:

- Defines an atomic risk decision  
- Combines structured logic + LLM reasoning  
- Produces a score and an action  
- Forms the basis of reusable Rulesets  
- Integrates seamlessly into Pipelines  

This document establishes the authoritative specification of RDL Rules for CORINT v0.1.
