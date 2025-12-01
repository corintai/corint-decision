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
  rules:
    - <rule-id-1>
    - <rule-id-2>
    - <rule-id-3>
  decision_logic:
    - <decision-rules>
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

## 6. `decision_logic`

**This is where actions are defined.**

Decision logic evaluates the combined results of all rules and determines the final action.

### 6.1 Basic Structure

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

### 6.2 Available Context

Within decision_logic conditions, you can access:

- `total_score` - Sum of all triggered rule scores
- `triggered_count` - Number of rules that triggered
- `triggered_rules` - Array of triggered rule IDs
- `context.*` - Any pipeline context data

### 6.3 Built-in Actions

| Action | Description | Use Case |
|--------|-------------|----------|
| `approve` | Automatically approve | Low risk |
| `deny` | Automatically reject | High risk |
| `review` | Send to human review | Needs judgment |
| `infer` | Send to AI analysis (async) | Complex patterns |

---

## 7. Common Decision Patterns

### 7.1 Score-Based Decisions

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

### 7.2 Count-Based Decisions

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

### 7.3 Short-Circuit (Early Termination)

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

### 7.4 Specific Rule Combinations

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

### 7.5 Weighted Scoring with Multipliers

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

### 7.6 Context-Aware Decisions

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

### 7.7 Time-Based Decisions

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

### 7.8 Hybrid: Rules + Score + Combination

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

## 8. Complete Examples

### 8.1 Account Takeover Detection

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

### 8.2 Transaction Fraud Detection

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

### 8.3 Credit Application Risk

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

### 8.4 Login Risk with Context

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

## 9. Best Practices

### 9.1 Decision Logic Order

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

### 9.2 Use Meaningful Reasons

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

### 9.3 Consider Business Context

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

## 10. Summary

A CORINT Ruleset:

- Groups multiple rules into a reusable logical unit  
- **Defines decision logic** through `decision_logic`
- Evaluates rule combinations and context
- **Produces final actions** (approve/deny/review/infer)
- Integrates cleanly with CORINT Pipelines  
- Improves modularity and maintainability  

**Key Points:**
- Rules detect and score
- Rulesets decide and act
- Pipelines orchestrate flow

Rulesets are the decision-making foundation of CORINT's Risk Definition Language (RDL).
