# CORINT RDL Architecture

## Three-Layer Decision Architecture

RDL adopts a clear three-layer architecture with distinct responsibilities:

```
┌─────────────────────────────────────────────────────────┐
│ Layer 1: Rules (Detectors)                              │
├─────────────────────────────────────────────────────────┤
│ Responsibilities:                                        │
│ - Detect individual risk factors                        │
│ - Evaluate if conditions are met                        │
│ - Produce scores                                        │
│                                                          │
│ Does NOT include:                                        │
│ - ❌ Action definitions                                 │
│ - ❌ Decision logic                                     │
└─────────────────────────────────────────────────────────┘
                          ↓ scores
┌─────────────────────────────────────────────────────────┐
│ Layer 2: Rulesets (Decision Makers)                     │
├─────────────────────────────────────────────────────────┤
│ Responsibilities:                                        │
│ - Organize related rules                                │
│ - Evaluate rule combination patterns                    │
│ - Make decisions based on score, count, combinations    │
│ - ✅ Define Actions (via decision_logic)               │
│ - Produce final decisions                               │
└─────────────────────────────────────────────────────────┘
                          ↓ action
┌─────────────────────────────────────────────────────────┐
│ Layer 3: Pipeline (Orchestrator)                        │
├─────────────────────────────────────────────────────────┤
│ Responsibilities:                                        │
│ - Orchestrate execution flow                            │
│ - Define step sequence                                  │
│ - Manage data flow                                      │
│ - Control branching and parallelism                     │
│                                                          │
│ Does NOT include:                                        │
│ - ❌ Decision logic                                     │
│ - ❌ Action definitions                                 │
└─────────────────────────────────────────────────────────┘
```

---

## Complete Example

### Rule (Detection and Scoring Only)

```yaml
rule:
  id: new_device_login
  name: New Device Login
  
  when:
    event.type: login
    conditions:
      - device.id not_in user.known_devices
      
  score: 40
  # ❌ No action
```

### Ruleset (Decision Making)

```yaml
ruleset:
  id: takeover_detection
  name: Account Takeover Detection
  
  rules:
    - new_device_login       # 40 points
    - unusual_location       # 50 points
    - behavior_anomaly       # 60 points
    
  # ✅ Decision logic here
  decision_logic:
    # Specific combination → deny
    - condition: |
        triggered_rules contains "new_device_login" &&
        triggered_rules contains "unusual_location" &&
        triggered_rules contains "behavior_anomaly"
      action: deny
      reason: "Classic takeover pattern"
      
    # High score → AI analysis
    - condition: total_score >= 100
      action: infer
      infer:
        data_snapshot:
          - event.*
          - context.*
      reason: "High risk score"
      
    # Medium score → human review
    - condition: total_score >= 60
      action: review
      reason: "Medium risk"
      
    # Default → approve
    - default: true
      action: approve
```

### Pipeline (Orchestration Only)

```yaml
pipeline:
  # Feature extraction
  - type: extract
    id: features
    
  # Behavior analysis
  - type: reason
    id: behavior_analysis
    
  # Execute ruleset (decision happens here)
  - include:
      ruleset: takeover_detection
      
  # Done! takeover_detection's action is the final decision
  # Pipeline doesn't need to make decisions
```

---

## Decision Flow

```
1. Pipeline starts execution
   ↓
2. Extract features, call APIs, LLM analysis
   ↓
3. Execute Ruleset
   ├─ Evaluate each Rule
   ├─ Collect triggered Rules and total score
   ├─ Execute decision_logic
   └─ Produce Action (approve/deny/review/infer)
   ↓
4. Pipeline completes, returns Ruleset's Action
```

---

## Common Ruleset Decision Logic Patterns

### 1. Score-Based Decisions

```yaml
decision_logic:
  - condition: total_score >= 150
    action: deny
  - condition: total_score >= 100
    action: infer
    infer: {data_snapshot: [...]}
  - condition: total_score >= 50
    action: review
  - default: true
    action: approve
```

### 2. Count-Based Decisions

```yaml
decision_logic:
  - condition: triggered_count >= 3
    action: deny
  - condition: triggered_count >= 2
    action: review
  - default: true
    action: approve
```

### 3. Short-Circuit (Immediate Decision on Specific Rule)

```yaml
decision_logic:
  - condition: triggered_rules contains "critical_breach"
    action: deny
    reason: "Critical security breach"
    terminate: true  # Immediate termination, skip further evaluation
    
  - condition: total_score >= 80
    action: review
  - default: true
    action: approve
```

### 4. Specific Rule Combinations

```yaml
decision_logic:
  # Must satisfy all 3 conditions
  - condition: |
      triggered_rules contains "new_device" &&
      triggered_rules contains "unusual_geo" &&
      triggered_rules contains "behavior_change"
    action: deny
    reason: "Account takeover pattern"
    
  - default: true
    action: approve
```

### 5. Context-Aware Decisions

```yaml
decision_logic:
  # More lenient for VIP users
  - condition: |
      event.user.tier == "vip" &&
      total_score < 80
    action: approve
    
  # Stricter for basic users
  - condition: |
      event.user.tier == "basic" &&
      total_score >= 50
    action: review
    
  - default: true
    action: approve
```

### 6. Hybrid Patterns

```yaml
decision_logic:
  # Priority 1: Short-circuit - critical rules
  - condition: triggered_rules contains "blocked_user"
    action: deny
    terminate: true
    
  # Priority 2: Specific combinations
  - condition: |
      triggered_rules contains "new_device" &&
      triggered_rules contains "high_value"
    action: infer
    infer: {data_snapshot: [...]}
    
  # Priority 3: High score threshold
  - condition: total_score >= 100
    action: review
    
  # Priority 4: Context-based judgment
  - condition: |
      total_score >= 50 &&
      hour(event.timestamp) >= 22
    action: review
    reason: "Risk detected during off-hours"
    
  # Priority 5: Default
  - default: true
    action: approve
```

---

## Built-in Actions

### approve
Automatically approve, low risk.

### deny  
Automatically reject, high risk.

### review
Send to human review queue.

### infer (Async AI Inference)
Send to Cognition AI service for intelligent analysis (asynchronous).

```yaml
action: infer
infer:
  # Only specify which data to send
  data_snapshot:
    - event.transaction
    - event.user
    - context.ruleset_results
```

**Workflow**:
1. Return temporary decision immediately (usually review)
2. Push data to Cognition AI queue
3. AI analysis completes and calls back
4. May update decision

---

## Key Design Principles

1. **Single Responsibility**
   - Rule = Detection
   - Ruleset = Decision  
   - Pipeline = Orchestration

2. **Centralized Decisions**
   - All actions defined in Ruleset's decision_logic
   - Single place of definition, clear and explicit

3. **Composability**
   - Rules can be reused across multiple Rulesets
   - Rulesets can have different decision_logic

4. **Testability**
   - Rule tests: Verify detection logic
   - Ruleset tests: Verify decision logic
   - Pipeline tests: Verify flow orchestration

---

## Comparison: Old vs New Architecture

### ❌ Old Architecture (Rule has action)

```yaml
rule:
  id: high_risk
  score: 80
  action: review  # Rule defines action

pipeline:
  - include:
      ruleset: checks
  # Pipeline needs to decide again
  - if: context.checks.score > 80
    action: deny
```

**Problems**:
- Action definitions scattered
- Logic duplication
- Unclear responsibilities

### ✅ New Architecture (Ruleset has decision_logic)

```yaml
rule:
  id: high_risk
  score: 80
  # No action

ruleset:
  id: checks
  rules: [high_risk]
  decision_logic:
    - condition: total_score > 80
      action: deny  # Centralized definition

pipeline:
  - include:
      ruleset: checks
  # Done! No need to decide again
```

**Benefits**:
- Centralized action definition
- Clear responsibilities
- Easy to maintain

---

## Reference Documentation

- `rule.md` - Detailed Rule specification
- `ruleset.md` - Ruleset and decision_logic specification
- `pipeline.md` - Pipeline orchestration specification 

---

## Complete Documentation Structure

CORINT RDL documentation is organized into five categories for clarity and ease of navigation.

```
📚 CORINT RDL Documentation

├─ 📖 Overview & Architecture
│  ├─ overall.md          High-level overview of RDL, goals, components, and features
│  
│
├─ 🔧 Core Components (Fundamental building blocks)
│  ├─ expression.md       Expression language syntax, operators, and functions (foundational)
│  ├─ rule.md            Rule specification for detecting risk factors and scoring
│  ├─ ruleset.md         Ruleset specification for grouping rules and decision logic
│  └─ pipeline.md        Pipeline orchestration for complete risk processing workflows
│
├─ 📊 Data & Schema (Data structures and type system)
│  ├─ event.md           Standard event types, schemas, and data structures
│  ├─ schema.md          Type system, validation, schema definitions, and validators
│  └─ context.md         Context and variable management for data flow between steps
│
├─ 🚀 Advanced Features (Sophisticated capabilities)
│  ├─ feature.md         Feature engineering, statistical analysis, and aggregations
│  ├─ llm.md             Large Language Model integration for cognitive reasoning
│  ├─ service.md         Internal service integration (DB, cache, microservices, MQ)
│  └─ external.md        External API integration for third-party risk intelligence
│
├─ ⚙️ Operational (Production deployment and monitoring)
│  ├─ error-handling.md  Error handling strategies, fallbacks, retries, and circuit breakers
│  ├─ observability.md   Monitoring, logging, metrics, tracing, and alerting
│  └─ performance.md    Performance optimization, caching, parallelization, and tuning
│
└─ 📝 Examples
   └─ examples/          Real-world pipeline examples and best practice patterns
```

