# CORINT Risk Definition Language (RDL)
## Pipeline Specification (v0.1)

A **Pipeline** defines the full risk‑processing flow in CORINT’s Cognitive Risk Intelligence framework.  
It represents a declarative Directed Acyclic Graph (DAG) composed of *steps*, *branches*, *parallel flows*, and *aggregation nodes*.

Pipelines orchestrate how events move through feature extraction, cognitive reasoning (LLM), rule execution, scoring, and final actions.

---

## 1. Pipeline Structure

### 1.1 Basic Structure

```yaml
pipeline:
  id: string                    # Optional unique identifier
  name: string                  # Optional human-readable name
  description: string           # Optional description
  when:                         # Optional execution condition
    event.type: string          # Event type filter
    conditions: [...]           # Additional conditions
  steps:                        # Processing steps
    - <step>
    - <step>
    - <branch>
    - <parallel>
    - <aggregate>
    - <include>
```

### 1.2 Pipeline Metadata

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | No | Unique identifier for the pipeline |
| `name` | string | No | Human-readable name |
| `description` | string | No | Detailed description of pipeline purpose |
| `when` | object | No | Execution condition (event type filter) |
| `steps` | array | Yes | List of processing steps |

### 1.3 When Condition

The `when` block controls whether the pipeline executes:

```yaml
pipeline:
  when:
    event.type: payment        # Only execute for payment events
    conditions:                # Optional additional conditions
      - amount > 100
```

If `when` condition is not met:
- Pipeline is skipped entirely
- No steps are executed
- Processing continues with next pipeline/rule

Each element is one of the pipeline constructs described below.

---

## 2. Step Types

A **step** is the smallest processing unit in a pipeline.

```yaml
step:
  id: string
  type: extract | reason | rules | api | score | action | custom
  if: <optional-condition>
  params: <key-value-map>
```

### 2.1 `type` definitions

| type | Description |
|------|-------------|
| `extract` | Feature extraction (device info, geo-IP, KYC profile, etc.) |
| `reason` | LLM cognitive reasoning step |
| `rules` | Execute a ruleset |
| `service` | Internal service call (database, cache, microservice, etc.) |
| `api` | External API lookup (Chainalysis, MaxMind, device fingerprint, etc.) |
| `score` | Score computation or normalization |
| `action` | Produces final decision outcome |
| `custom` | User‑defined function (Python/Rust/etc.) |

### 2.2 `if` conditional

Every step may include a conditional:

```yaml
if: "event.amount > 1000"
```

The step executes **only** if the condition evaluates to true.

---

## 2.3 Complete Pipeline Example

```yaml
version: "0.1"

pipeline:
  id: payment_risk_pipeline
  name: Payment Risk Control Pipeline
  description: Comprehensive payment risk assessment with routing
  when:
    event.type: payment         # Only execute for payment events
  
  steps:
    - type: api
      id: ip_check
      api: ipinfo
      endpoint: ip_lookup
      params:
        ip: event.ip_address
    
    - branch:
        when:
          - condition: payment_amount > 1000
            pipeline:
              - include:
                  ruleset: high_value_rules
```

---

## 3. Branching

A branch selects between multiple sub‑pipelines based on conditions.

```yaml
- branch:
    when:
      - condition: "event.type == 'login'"
        pipeline:
          - extract_login
          - login_rules

      - condition: "event.type == 'payment'"
        pipeline:
          - extract_payment
          - payment_rules
```

Branch rules:

- Conditions are evaluated **top‑to‑bottom**
- First matching condition executes its pipeline
- Branch pipelines may contain any valid pipeline structures

---

## 4. Parallel Execution

Run multiple steps or pipelines concurrently.

```yaml
- parallel:
    - device_fingerprint
    - ip_reputation
    - llm_reasoning
  merge:
    method: all
```

### 4.1 Merge Methods

| method | Description |
|--------|-------------|
| `all` | Wait for all parallel tasks |
| `any` | Return on first successful completion |
| `fastest` | Use first response (for redundant reasoning/rules) |
| `majority` | Wait until >50% of tasks complete |

---

## 5. Aggregation

Aggregates multiple outputs into a unified representation, typically scores.

```yaml
- aggregate:
    method: weighted
    weights:
      rules_engine: 0.5
      llm_reasoning: 0.3
      chainalysis: 0.2
```

### 5.1 Methods

- `sum` – sum of all values  
- `max` – maximum value  
- `weighted` – custom weighted formula  

---

## 6. Include (Reusable Modules)

Pipelines may import:

- A ruleset
- Another pipeline (sub‑pipeline)

### 6.1 Ruleset include

```yaml
- include:
    ruleset: login_risk_rules
```

### 6.2 Pipeline include

```yaml
- include:
    pipeline: common_feature_flow
```

---

## 7. Full Pipeline Example

### 7.1 Login Risk Processing Pipeline

```yaml
version: "0.1"

pipeline:
  id: login_risk_pipeline
  name: Login Risk Assessment Pipeline
  description: Comprehensive login risk evaluation with parallel checks and LLM reasoning
  when:
    event.type: login
  
  steps:
    # Step 1: base feature extraction
    - type: extract
      id: extract_device

    - type: extract
      id: extract_geo

    # Step 2: parallel intelligence checks
    - parallel:
        - device_fingerprint
        - ip_reputation
        - llm_reasoning
      merge:
        method: all

    # Step 3: execute login ruleset
    - include:
        ruleset: login_risk_rules

    # Step 4: score aggregation
    - aggregate:
        method: weighted
        weights:
          rules: 0.5
          llm: 0.3
          ip: 0.2

    # Step 5: final action
    - type: action
```

---

### 7.2 Multi‑Event Pipeline Example

```yaml
version: "0.1"

pipeline:
  id: multi_event_router
  name: Multi-Event Type Router
  description: Routes different event types to appropriate processing pipelines
  
  steps:
    - branch:
        when:
        - condition: "event.type == 'login'"
          pipeline:
            - extract_login
            - include:
                ruleset: login_risk_rules

        - condition: "event.type == 'payment'"
          pipeline:
            - extract_payment
            - include:
                ruleset: payment_risk_rules

        - condition: "event.type == 'crypto_transfer'"
          pipeline:
            - extract_web3
            - include:
                ruleset: web3_wallet_risk

  - aggregate:
      method: sum

  - type: action
```

---

### 7.3 Service Integration Pipeline Example

```yaml
version: "0.1"

pipeline:
  # Step 1: Load user profile from database
  - type: service
    id: load_user_profile
    service: user_db
    query: get_user_profile
    params:
      user_id: event.user.id
    output: context.user_profile

  # Step 2: Check cache for existing risk score
  - type: service
    id: check_risk_cache
    service: redis_cache
    operation: get_user_risk_cache
    output: context.cached_risk

  # Step 3: Parallel external intelligence checks
  - parallel:
      # Load pre-computed features
      - type: service
        id: load_features
        service: feature_store
        features: [user_behavior_7d, device_profile]

      # Check external API
      - type: api
        id: ip_reputation
        api: maxmind
        endpoint: ip_lookup

      # LLM reasoning
      - type: reason
        id: behavior_analysis
        provider: openai

    merge:
      method: all

  # Step 4: Execute rules with all context
  - include:
      ruleset: comprehensive_risk_check

  # Step 5: Publish decision to message queue
  - type: service
    id: publish_decision
    service: event_bus
    topic: risk_decisions
    async: true
```

---

## 8. BNF Grammar (Formal)

```
PIPELINE ::= "pipeline:" STEP_LIST

STEP_LIST ::= "-" STEP { "-" STEP }

STEP ::= SEQUENTIAL_STEP
       | CONDITIONAL_STEP
       | BRANCH_STEP
       | PARALLEL_STEP
       | AGGREGATE_STEP
       | INCLUDE_STEP

SEQUENTIAL_STEP ::= IDENT | OBJECT_STEP

OBJECT_STEP ::= 
      "id:" STRING
      "type:" STEP_TYPE
      [ "if:" CONDITION ]
      [ "params:" OBJECT ]

STEP_TYPE ::= "extract" | "reason" | "rules" | "service" | "api"
            | "score" | "action" | "custom"

BRANCH_STEP ::= 
      "branch:" 
         "when:" 
            "-" "condition:" CONDITION 
               "pipeline:" STEP_LIST
            { "-" "condition:" CONDITION "pipeline:" STEP_LIST }

PARALLEL_STEP ::= 
      "parallel:" STEP_LIST
      "merge:" MERGE_STRATEGY

MERGE_STRATEGY ::= "method:" ("all" | "any" | "fastest" | "majority")

AGGREGATE_STEP ::= 
      "aggregate:"
         "method:" ("sum" | "max" | "weighted")
         [ "weights:" OBJECT ]

INCLUDE_STEP ::= 
      "include:" ("ruleset:" STRING | "pipeline:" STRING)
```

---

## 9. Summary

A CORINT Pipeline:

- Defines the full decision‑making workflow  
- Supports conditional logic, branching, parallelism, and aggregation  
- Integrates rulesets, cognitive reasoning, and external signals  
- Encapsulates reusable and modular risk flows  

It is the highest‑level construct of CORINT’s Risk Definition Language (RDL).
