# CORINT Risk Definition Language (RDL)
## Pipeline Specification (v0.2)

A **Pipeline** defines the full risk‑processing flow in CORINT's Cognitive Risk Intelligence framework.
It represents a declarative Directed Acyclic Graph (DAG) composed of *steps*, *branches*, *parallel flows*, and *aggregation nodes*.

Pipelines orchestrate how events move through feature extraction, cognitive reasoning (LLM), rule execution, scoring, and **final decision making**.

**Important: Three-Layer Decision Model**
- **Rules** detect and score individual risk patterns
- **Rulesets** produce **signals** (`approve`, `decline`, `review`, `hold`, `pass`) based on rule results
- **Pipelines** make **final decisions** based on ruleset signals, outputting both a **signal** and optional **actions**

Note: Both rulesets and pipelines use the same 5 signal types. The difference is naming convention to clarify that ruleset output is intermediate (called "signal"), while pipeline output is the final decision result.

---

## 1. Pipeline Structure

### 1.1 Basic Structure

```yaml
pipeline:
  id: string                    # Optional unique identifier
  name: string                  # Optional human-readable name
  description: string           # Optional description
  when:                         # Optional execution condition
    all: [...]                  # Conditions (event type, expressions)
  steps:                        # Processing steps
    - <step>
    - <step>
    - <branch>
    - <parallel>
    - <aggregate>
    - <include>
  decision:                     # Final decision logic based on signals
    - <decision-rules>
```

### 1.2 Execution Flow

Pipeline execution follows this flow:

```
when条件检查 → entry step → next routing → ... → decision最终决策
```

**Step Routing Rules:**
- `entry: <step_id>` - Defines which step to start with
- `next: <step_id>` - Explicitly routes to the specified step
- `next: end` or **no `next`** - Ends step execution, flows to `decision`

**Key Points:**
- Pipeline is a **DAG (Directed Acyclic Graph)** - routing must be explicit
- Sequential execution requires explicit `next` declarations between steps
- Omitting `next` means "end here" - enables early termination from any step
- `decision` section is the terminal phase for making final actions

### 1.3 Pipeline Metadata

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | No | Unique identifier for the pipeline |
| `name` | string | No | Human-readable name |
| `description` | string | No | Detailed description of pipeline purpose |
| `when` | object | No | Execution condition (event type filter) |
| `steps` | array | Yes | List of processing steps |
| `decision` | array | No | Final decision logic based on signals from rulesets |
| `metadata` | object | No | Arbitrary key-value pairs for documentation, versioning, authorship, etc. |

The `metadata` field allows you to attach information to your pipeline for documentation, versioning, and management purposes.

**Required Fields**:
- `version` (string) - Version number in semver format (e.g., "1.0.0")
- `author` (string) - Author or team name
- `updated` (string) - Last update timestamp in format "YYYY-MM-DD HH:mm:ss"

**Custom Fields**:
- Any additional key/value pairs for business-specific metadata

**Example**:
```yaml
pipeline:
  id: fraud_detection
  name: Fraud Detection Pipeline
  description: Comprehensive fraud detection
  metadata:
    # Required fields
    version: "2.1.0"
    author: "Risk Engineering Team"
    updated: "2025-12-20 14:30:00"

    # Optional custom fields
    owner: "fraud_team"
    environment: "production"
    tags: [fraud, risk_assessment]
```

### 1.4 When Condition

The `when` block controls whether the pipeline executes:

```yaml
pipeline:
  when:
    all:
      - event.type == "payment"  # Only execute for payment events
      - amount > 100             # Additional condition
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

## 6. Imports and Include (Reusable Modules)

Pipelines use the `imports` section to declare dependencies on rulesets and other pipelines. This enables modular, reusable pipeline design.

### 6.1 Import Declaration

Pipelines use multi-document YAML format with `---` separator:

```yaml
version: "0.1"

# First document: Imports
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml
    - library/rulesets/payment_high_value.yaml
  pipelines:
    - library/pipelines/common_feature_extraction.yaml

---

# Second document: Pipeline definition
pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline

  steps:
    - include:
        ruleset: fraud_detection_core
```

**Key Benefits:**
- **Explicit Dependencies** - All dependencies declared upfront
- **Compile-Time Resolution** - Dependencies resolved during compilation
- **Automatic Transitive Dependencies** - Importing a ruleset automatically loads all its rules
- **Zero Runtime Overhead** - All merging happens at compile time

### 6.2 Dependency Propagation

When a pipeline imports a ruleset, it automatically gets all the ruleset's rule dependencies:

```yaml
# Pipeline only needs to declare the ruleset
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml  # This ruleset imports 6 rules

---

pipeline:
  steps:
    - include:
        ruleset: fraud_detection_core  # All 6 rules are automatically available
```

The compiler resolves transitive dependencies automatically:
```
Pipeline imports: fraud_detection_core
  ↓
fraud_detection_core imports:
  - fraud_farm.yaml
  - account_takeover.yaml
  - velocity_abuse.yaml
  - amount_outlier.yaml
  - suspicious_geography.yaml
  - new_user_fraud.yaml
  ↓
All 6 rules are available
```

### 6.3 Ruleset Include (Execution)

After importing a ruleset via the `imports` section, you can execute it in your pipeline steps:

```yaml
steps:
  - include:
      ruleset: login_risk_rules
```

The `include` step:
- Executes all rules in the ruleset
- Evaluates the ruleset's decision logic
- Returns a **signal** (`approve`, `decline`, `review`, `hold`, `pass`)
- Makes results available in `results.<ruleset_id>.signal`

**Note:** Both rulesets and pipelines use the same 5 signal types. The ruleset signal indicates the intermediate decision recommendation, while the pipeline makes the final decision based on signals from all rulesets.

### 6.4 Pipeline Include (Execution)

You can also include sub-pipelines:

```yaml
steps:
  - include:
      pipeline: common_feature_flow
```

### 6.5 Complete Example with Imports

**Fraud Detection Pipeline:**

```yaml
version: "0.1"

# Declare dependencies
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline
  description: Production-grade fraud detection for transaction events

  # Pipeline only executes for transaction events
  when:
    event.type: transaction

  steps:
    # Execute the fraud detection ruleset
    - include:
        ruleset: fraud_detection_core
```

This 24-line pipeline replaces what would have been a 337-line monolithic file!

**Payment Pipeline with Branching:**

```yaml
version: "0.1"

imports:
  rulesets:
    - library/rulesets/payment_standard.yaml
    - library/rulesets/payment_high_value.yaml

---

pipeline:
  id: payment_pipeline
  name: Payment Risk Pipeline
  description: Payment risk assessment with conditional routing

  when:
    event.type: payment

  steps:
    # Feature extraction
    - type: extract
      id: extract_features

    # Branch based on transaction amount
    - branch:
        when:
          # High-value transactions (> $1000)
          - condition: event.transaction.amount > 1000
            pipeline:
              - include:
                  ruleset: payment_high_value  # Stricter thresholds

          # Standard transactions
          - default: true
            pipeline:
              - include:
                  ruleset: payment_standard  # Normal thresholds
```

### 6.6 Import Path Resolution

Import paths are resolved relative to the repository root:

```
repository/
├── library/
│   ├── rulesets/
│   │   ├── fraud_detection_core.yaml
│   │   └── payment_high_value.yaml
│   └── pipelines/
│       └── common_feature_extraction.yaml
└── pipelines/
    └── fraud_detection.yaml  ← You are here
```

From `fraud_detection.yaml`, you import using paths relative to repository root:
```yaml
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml  # ✅ Correct
```

### 6.7 Multiple Rulesets in One Pipeline

You can import and use multiple rulesets in a single pipeline:

```yaml
version: "0.1"

imports:
  rulesets:
    - library/rulesets/device_risk.yaml
    - library/rulesets/geo_risk.yaml
    - library/rulesets/behavioral_risk.yaml

---

pipeline:
  id: comprehensive_risk_pipeline
  name: Comprehensive Risk Assessment

  steps:
    # Step 1: Device risk assessment
    - include:
        ruleset: device_risk

    # Step 2: Geographic risk assessment
    - include:
        ruleset: geo_risk

    # Step 3: Behavioral risk assessment
    - include:
        ruleset: behavioral_risk

    # Step 4: Aggregate all risk scores
    - aggregate:
        method: weighted
        weights:
          results.device_risk.total_score: 0.4
          results.geo_risk.total_score: 0.3
          results.behavioral_risk.total_score: 0.3
```

### 6.8 Conditional Ruleset Execution

You can conditionally execute rulesets based on pipeline state:

```yaml
steps:
  # Execute high-value ruleset only for large transactions
  - include:
      ruleset: payment_high_value
    if: event.transaction.amount > 10000

  # Execute standard ruleset for normal transactions
  - include:
      ruleset: payment_standard
    if: event.transaction.amount <= 10000
```

### 6.9 Benefits of Import-Based Pipelines

1. **Massive Code Reduction** - 80-90% reduction in pipeline file size
2. **Reusability** - Same rulesets used across multiple pipelines
3. **Maintainability** - Update rules in one place, all pipelines benefit
4. **Clarity** - Pipeline focus on orchestration, not rule details
5. **Testability** - Test rules and rulesets independently
6. **Type Safety** - Compiler validates all IDs at compile time

---

## 7. Decision Logic

The `decision` section is where **final decisions** are made based on **signals** from rulesets. This is the core of the three-layer decision model.

### 7.1 Basic Structure

```yaml
pipeline:
  id: my_pipeline
  steps:
    - include:
        ruleset: fraud_detection

  # Decision logic based on signals from rulesets
  decision:
    - when: results.fraud_detection.signal == "decline"
      result: decline
      actions: ["BLOCK_DEVICE", "NOTIFY_SECURITY"]
      reason: "Critical risk detected"
      terminate: true

    - when: results.fraud_detection.signal == "review"
      result: review
      actions: ["KYC", "OTP"]
      reason: "Requires manual review"

    - when: results.fraud_detection.signal == "hold"
      result: hold
      actions: ["2FA"]
      reason: "Additional verification required"

    - default: true
      result: approve
      reason: "Transaction approved"
```

### 7.2 Decision Structure

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `when` | string | Yes* | Condition expression for this decision rule |
| `result` | string | Yes | Decision result: `approve`, `decline`, `review`, `hold`, `pass` |
| `actions` | array | No | List of actions to execute: `["KYC", "OTP", "2FA", "BLOCK_DEVICE"]` |
| `reason` | string | No | Human-readable reason for the decision |
| `terminate` | boolean | No | If true, stop processing further decision rules |
| `default` | boolean | No* | If true, this is the default fallback rule |

*Either `when` or `default` must be specified.

### 7.3 Signal vs Result vs Actions

**Signal** (Ruleset output) - The intermediate decision recommendation from a ruleset:
- Produced by rulesets after evaluating rules
- Available as `results.<ruleset_id>.signal`
- Values: `approve`, `decline`, `review`, `hold`, `pass`

**Result** (Pipeline output) - The final decision outcome:
| Result | Description | Use Case |
|--------|-------------|----------|
| `approve` | Automatically approve the transaction | Low risk, clean transactions |
| `decline` | Automatically reject the transaction | High risk, fraud detected |
| `review` | Send to human review queue | Medium risk, needs judgment |
| `hold` | Temporarily suspend, require additional verification | Suspicious but not definitive |
| `pass` | Skip/no decision, let downstream handle | Rule not applicable, defer to next |

**Actions** - Optional list of specific actions to execute:
| Action | Description |
|--------|-------------|
| `KYC` | Trigger Know Your Customer verification |
| `OTP` | Send one-time password verification |
| `2FA` | Require two-factor authentication |
| `BLOCK_DEVICE` | Block the device from future transactions |
| `NOTIFY_SECURITY` | Alert security team |
| `FREEZE_ACCOUNT` | Temporarily freeze the account |
| *custom* | Any custom action defined by your system |

### 7.4 Available Context for Decisions

Within decision conditions, you can access:

- `results.<ruleset_id>.signal` - Signal from a ruleset (e.g., `approve`, `decline`, `review`)
- `results.<ruleset_id>.total_score` - Aggregate score from ruleset
- `results.<ruleset_id>.triggered_count` - Number of triggered rules
- `results.<ruleset_id>.triggered_rules` - Array of triggered rule IDs
- `event.*` - Original event data
- `features.*` - Calculated features

### 7.5 Multiple Rulesets Example

When using multiple rulesets, combine their signals in decision logic:

```yaml
pipeline:
  id: comprehensive_risk_pipeline

  steps:
    - include:
        ruleset: device_risk
    - include:
        ruleset: geo_risk
    - include:
        ruleset: behavioral_risk

  # Decision based on signals from all rulesets
  decision:
    # Any decline signal = decline
    - when: |
        results.device_risk.signal == "decline" ||
        results.geo_risk.signal == "decline" ||
        results.behavioral_risk.signal == "decline"
      result: decline
      actions: ["BLOCK_DEVICE", "FREEZE_ACCOUNT"]
      reason: "Critical risk detected"
      terminate: true

    # Multiple review signals = decline
    - when: |
        (results.device_risk.signal == "review" ? 1 : 0) +
        (results.geo_risk.signal == "review" ? 1 : 0) +
        (results.behavioral_risk.signal == "review" ? 1 : 0) >= 2
      result: decline
      actions: ["NOTIFY_SECURITY"]
      reason: "Multiple risk signals"

    # Any review signal = review
    - when: |
        results.device_risk.signal == "review" ||
        results.geo_risk.signal == "review" ||
        results.behavioral_risk.signal == "review"
      result: review
      actions: ["KYC"]
      reason: "Risk signal detected"

    # Default: approve
    - default: true
      result: approve
```

### 7.6 Benefits of Pipeline-Level Decisions

1. **Reusability** - Same ruleset can be used with different decision thresholds
2. **Flexibility** - Different pipelines can make different decisions from the same signals
3. **Clarity** - Clear separation between detection (rules) and action (decisions)
4. **Testability** - Test ruleset signals independently from decision logic
5. **Customization** - Easy to adjust thresholds per use case (VIP users, high-value transactions, etc.)

---

## 8. Full Pipeline Example

### 8.1 Login Risk Processing Pipeline

```yaml
version: "0.2"

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

    # Step 3: execute login ruleset (produces signals)
    - include:
        ruleset: login_risk_rules

    # Step 4: score aggregation
    - aggregate:
        method: weighted
        weights:
          rules: 0.5
          llm: 0.3
          ip: 0.2

  # Final decision based on signals
  decision:
    - when: results.login_risk_rules.signal == "decline"
      result: decline
      actions: ["BLOCK_DEVICE", "NOTIFY_SECURITY"]
      reason: "Critical login risk"
      terminate: true

    - when: results.login_risk_rules.signal == "review"
      result: review
      actions: ["KYC"]
      reason: "Requires manual review"

    - when: results.login_risk_rules.signal == "hold"
      result: hold
      actions: ["2FA"]
      reason: "Additional verification required"

    - default: true
      result: approve
```

---

### 8.2 Multi‑Event Pipeline Example

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

### 8.3 Service Integration Pipeline Example

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

## 9. BNF Grammar (Formal)

```
PIPELINE ::=
      "pipeline:"
         "id:" STRING
         [ "name:" STRING ]
         [ "description:" STRING ]
         "entry:" STRING
         [ "when:" WHEN_BLOCK ]
         "steps:" STEP_LIST
         [ "metadata:" METADATA_MAP ]

WHEN_BLOCK ::=
      "all:" CONDITION_LIST
    | "any:" CONDITION_LIST

CONDITION_LIST ::= "-" EXPRESSION { "-" EXPRESSION }

STEP_LIST ::= "-" STEP { "-" STEP }

STEP ::= "step:" STEP_BODY

STEP_BODY ::=
      "id:" STRING
      [ "name:" STRING ]
      "type:" STEP_TYPE
      [ "when:" WHEN_BLOCK ]
      [ STEP_TYPE_PARAMS ]
      [ "next:" STRING ]

STEP_TYPE ::= "ruleset" | "router" | "api" | "llm" | "action" | "custom"

STEP_TYPE_PARAMS ::=
      RULESET_PARAMS
    | ROUTER_PARAMS
    | API_PARAMS
    | LLM_PARAMS

RULESET_PARAMS ::= "ruleset:" STRING

ROUTER_PARAMS ::=
      "routes:" ROUTE_LIST
      [ "default:" STRING ]

ROUTE_LIST ::= "-" ROUTE { "-" ROUTE }

ROUTE ::=
      "next:" STRING
      "when:" WHEN_BLOCK

API_PARAMS ::=
      "api:" STRING
      [ "params:" OBJECT ]

LLM_PARAMS ::=
      "llm:" STRING
      [ "params:" OBJECT ]

METADATA_MAP ::= KEY ":" VALUE { KEY ":" VALUE }

OBJECT ::= KEY ":" VALUE { KEY ":" VALUE }
```

---

## 10. Related Documentation

For comprehensive understanding of pipelines and the CORINT ecosystem:

### Core Concepts
- **[imports.md](imports.md)** - Complete module system and dependency management specification
- **[ruleset.md](ruleset.md)** - Ruleset specification and decision logic
- **[rule.md](rule.md)** - Individual rule specification

### Advanced Topics
- **[expression.md](expression.md)** - Expression language for conditions
- **[context.md](context.md)** - Context management and data flow between steps
- **[feature.md](feature.md)** - Feature engineering and extraction

### Integration
- **[service.md](service.md)** - Internal service integration (databases, caches, microservices)
- **[external.md](external.md)** - External API integration
- **[llm.md](llm.md)** - LLM integration for cognitive reasoning

### Architecture
- **[overall.md](overall.md)** - High-level RDL overview
- **[ARCHITECTURE.md](../ARCHITECTURE.md)** - Three-layer decision architecture

---

## 11. Summary

A CORINT Pipeline:

- Defines the full decision‑making workflow
- Supports conditional logic, branching, parallelism, and aggregation
- Integrates rulesets, cognitive reasoning, and external signals
- Encapsulates reusable and modular risk flows
- **Uses imports to declare dependencies on rulesets and sub-pipelines**
- **Benefits from automatic transitive dependency resolution**
- **Makes final decisions** based on **signals** from rulesets

**Key Points (Three-Layer Model):**
- **Rules** detect and score individual risk patterns
- **Rulesets** produce **signals** (`approve`, `decline`, `review`, `hold`, `pass`)
- **Pipelines** make final decisions with **result** and optional **actions**

**Signal vs Result vs Actions:**
- `signal`: Ruleset output - intermediate decision recommendation
- `result`: Pipeline output - final decision outcome (approve/decline/review/hold/pass)
- `actions`: Optional list of specific actions to execute (KYC, OTP, 2FA, BLOCK_DEVICE, etc.)

**Benefits:**
- Same ruleset can be reused with different decision thresholds
- Clear separation between detection and action
- 80-90% code reduction through modular design
- Dependencies resolved at compile time

It is the highest‑level construct of CORINT's Risk Definition Language (RDL).
