# CORINT Risk Definition Language (RDL)
## Ruleset Specification (v0.1)

A **Ruleset** is a named collection of rules that can be reused, grouped, and executed as a unit within CORINT’s Cognitive Risk Intelligence framework.  
Rulesets enable modular design, separation of concerns, and cleaner pipeline logic.

---

## 1. Ruleset Structure

```yaml
ruleset:
  id: string
  rules:
    - <rule-id-1>
    - <rule-id-2>
    - <rule-id-3>
```

---

## 2. `id`

A globally unique identifier for the ruleset.

Example:

```yaml
id: login_risk_rules
```

---

## 3. `rules`

An ordered list of rule identifiers that belong to this ruleset.

The rules referenced here must exist in the system, typically defined in separate RDL rule files.

Example:

```yaml
rules:
  - high_risk_login
  - device_mismatch
  - ip_reputation_flag
```

Rules are executed **in the given order**, unless overridden by pipeline logic.

---

## 4. Recommended Usage

- Group rules by **domain** (e.g., login, payment, KYC, loan)  
- Group rules by **behavior type** (e.g., device anomalies, IP anomalies)  
- Combine rules that share inputs or features  
- Reference rulesets inside pipelines for modular flows  

Example:

```yaml
include:
  ruleset: login_risk_rules
```

---

## 5. Ruleset Execution Model

Rulesets follow this model:

1. Evaluate each rule in defined order  
2. Collect triggered rule scores  
3. Optionally combine results in pipeline via `aggregate`  
4. Optionally return an action if a rule produces one  

Rulesets themselves do not have actions—actions belong to individual rules.

---

## 6. Complete Examples

### 6.1 Login Risk Ruleset

```yaml
version: "0.1"

ruleset:
  id: login_risk_rules
  rules:
    - high_risk_login
    - device_risk_check
    - ip_reputation_check
    - llm_behavior_mismatch
```

---

### 6.2 Payment Risk Ruleset

```yaml
version: "0.1"

ruleset:
  id: payment_risk_rules
  rules:
    - high_amount_transfer
    - velocity_anomaly
    - beneficiary_risk_flag
    - llm_transfer_reasoning
```

---

### 6.3 Web3 Risk Ruleset

```yaml
version: "0.1"

ruleset:
  id: web3_wallet_risk
  rules:
    - chainalysis_wallet_risk
    - llm_wallet_intel
    - new_wallet_behavior
```

---

## 7. Summary

A CORINT Ruleset:

- Groups multiple rules into a reusable logical unit  
- Helps organize risk logic by domain or behavior type  
- Integrates cleanly with CORINT Pipelines  
- Improves modularity and maintainability  

Rulesets are a foundational building block of CORINT’s Risk Definition Language (RDL).
