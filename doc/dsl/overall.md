# CORINT Risk Definition Language (RDL)
## Overall Specification (v0.1)

**RDL is the domain-specific language used by CORINT (Cognitive Risk Intelligence Network Technology) to define rules, rule groups, reasoning logic, and full risk‑processing pipelines.**  
It enables modern hybrid risk engines to combine deterministic logic with LLM‑based reasoning in a unified, explainable, high‑performance format.

---

## 1. Goals of RDL

RDL is designed to:

- Provide a declarative, human-readable format for risk logic  
- Support both traditional rule engines and LLM cognitive reasoning  
- Compile into a Rust‑friendly IR (AST) for high‑performance execution  
- Represent end‑to‑end risk processing flows  
- Enable transparency, auditability, and explainability  
- Be cloud‑native, language‑agnostic, and extensible  

---

## 2. Top-Level Components

An RDL file may contain one of the following:

```yaml
version: "0.1"

rule: {...}
# OR
ruleset: {...}
# OR
pipeline: [...]
```

Components:

| Component | Purpose |
|----------|---------|
| **rule** | A single risk logic unit |
| **ruleset** | A group of rules |
| **pipeline** | The full risk processing DAG |

---

## 3. Component Specification

### 3.1 Rule

A Rule is the smallest executable logic unit.

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

### 3.1.1 `when` Block

#### Event Filter

```yaml
when:
  event.type: login
```

#### Conditions

```yaml
conditions:
  - user.age > 60
  - geo.country in ["RU", "NG"]
```

Operators:

- `==`, `!=`
- `<`, `>`, `<=`, `>=`
- `in`
- `regex`
- `exists`, `missing`

---

### 3.1.2 LLM-Based Conditions

```yaml
- LLM.reason(event) contains "suspicious"
- LLM.tags contains "device_mismatch"
- LLM.score > 0.7
- LLM.output.risk_score > 0.3
```

---

### 3.1.3 External API Conditions

```yaml
- external_api.Chainalysis.risk_score > 80
```

---

### 3.1.4 Actions

- approve  
- deny  
- review  
- escalate  
- custom object-based actions  

---

## 3.2 Ruleset

```yaml
ruleset:
  id: string
  rules:
    - rule_id_1
    - rule_id_2
    - rule_id_3
```

Rulesets allow grouping and reuse.

---

## 3.3 Pipeline

A pipeline defines the entire risk‑processing DAG.  
It supports:

- Sequential steps  
- Conditional steps  
- Branching  
- Parallel execution  
- Merge strategies  
- Score aggregation  
- Ruleset inclusion  
- External API calls  

(See `pipeline.md` for full details.)

---

## 4. Expression Language

### 4.1 Field Access

```
user.profile.age
trade.amount
device.id
geo.ip
```

### 4.2 Operators

| Operator | Description |
|----------|-------------|
| `==` | equality |
| `!=` | not equal |
| `<, >, <=, >=` | numeric compare |
| `in` | list/array membership |
| `exists`, `missing` | presence check |
| `regex` | regex match |

---

### 4.3 LLM Expressions

```yaml
LLM.reason(obj) contains "anomaly"
LLM.tags contains "ip_mismatch"
LLM.score > 0.8
LLM.output.behavior_stability < 0.4
```

---

## 5. External API Integration

```yaml
external_api.<provider>.<field>
```

Example:

```yaml
external_api.Chainalysis.risk_score > 80
```

---

## 6. Examples

### 6.1 Login Risk Example

```yaml
version: "0.1"

rule:
  id: high_risk_login
  name: High-Risk Login Detection
  description: Detect risky login behavior using rules + LLM reasoning

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

### 6.2 Loan Application Consistency

```yaml
version: "0.1"

rule:
  id: loan_inconsistency
  name: Loan Application Inconsistency
  description: Detect mismatch between declared information and LLM inference

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

## 7. BNF Grammar (Formal)

```
RDL ::= "version" ":" STRING
        (RULE | RULESET | PIPELINE)

RULE ::= "rule:" RULE_BODY

RULE_BODY ::=
      "id:" STRING
      "name:" STRING
      "description:" STRING
      "when:" CONDITION_BLOCK
      "score:" NUMBER
      "action:" ACTION

CONDITION_BLOCK ::=
      EVENT_FILTER
      "conditions:" CONDITION_LIST

EVENT_FILTER ::= KEY ":" VALUE

CONDITION_LIST ::= 
      "-" CONDITION { "-" CONDITION }

CONDITION ::=
      EXPRESSION
    | LLM_EXPR
    | EXTERNAL_EXPR

EXPRESSION ::= FIELD OP VALUE

FIELD ::= IDENT ("." IDENT)*

OP ::= "==" | "!=" | "<" | ">" | "<=" | ">=" | "in" | "regex" | "exists" | "missing"

LLM_EXPR ::=
      "LLM.reason(" ARG ")" MATCH_OP VALUE
    | "LLM.tags" MATCH_OP STRING
    | "LLM.score" OP NUMBER
    | "LLM.output." FIELD OP VALUE

MATCH_OP ::= "contains" | "not_contains"

EXTERNAL_EXPR ::=
      "external_api." IDENT "." FIELD OP VALUE

ACTION ::= "approve" | "deny" | "review" | "escalate" | OBJECT

RULESET ::= "ruleset:" 
              "id:" STRING
              "rules:" RULE_ID_LIST

PIPELINE ::= defined in pipeline.md
```

---

## 8. Compilation Model

RDL compiles into:

1. AST  
2. Rust IR  
3. Explainability trace  
4. Deterministic + LLM hybrid execution plan  

---

## 9. Roadmap

- Type system  
- Static analysis  
- Visual editor  
- WASM sandbox execution  
- Code generator (Rust / Python / JS)  
- Prebuilt rule libraries  

---

## 10. Summary

RDL provides a modern, explainable, AI‑augmented DSL for advanced risk engines:

- Rules + LLM reasoning in one language  
- Modular (Rule → Ruleset → Pipeline)  
- High‑performance and auditable  
- Designed for banks, fintech, e‑commerce, and Web3  

This DSL is the foundation of the Cognitive Risk Intelligence Platform (CORINT).
