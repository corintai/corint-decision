# CORINT Risk Definition Language (RDL)
## Pipeline Specification (v0.1)

A **Pipeline** defines the full risk‑processing flow in CORINT's Cognitive Risk Intelligence framework.
It represents a declarative Directed Acyclic Graph (DAG) composed of processing steps with explicit routing.

Pipelines orchestrate how events move through rule execution, external service integration, and decision-making. Features are defined at the rule level (see [feature.md](feature.md)).

**Current Implementation:**
- **Rules** detect and score individual risk patterns (✅ Implemented)
- **Rulesets** produce **signals** (`approve`, `decline`, `review`, `hold`, `pass`) based on rule results (✅ Implemented)
- **Pipelines** execute rulesets and route events through processing steps (✅ Implemented)
- **Pipeline-level decision logic** can map ruleset signals to final results (✅ Implemented)

---

## 1. Pipeline Structure

### 1.1 Implemented Fields (✅)

```yaml
pipeline:
  id: string                    # ✅ Required: Unique identifier
  name: string                  # ✅ Required: Human-readable name
  description: string           # ✅ Optional: Description
  entry: string                 # ✅ Required: ID of the first step to execute (DAG entry point)
  when:                         # ✅ Optional: Execution condition
    all: [...]                  # Conditions using expression syntax
  steps:                        # ✅ Required: Processing steps (see section 2)
    - step:
        id: string
        name: string
        type: string
        # ... type-specific fields
  decision:                     # ✅ Required: Pipeline-level decision logic
    - when: <condition>
      result: <result>          # Final decision: approve/decline/review/
      actions: [...]            # Optional actions to execute
      reason: <reason>
  metadata:                     # ✅ Optional: Arbitrary key-value pairs
    <key>: <value>
```

**Important:**
- The `entry` field is **required** and specifies which step to start with
- The `decision` field is **required** and allows pipelines to map ruleset signals to final results
- Rulesets produce signals via `conclusion`, pipelines then map these to results via `decision`

### 1.2 Execution Flow

Pipeline execution follows this flow:

```
when condition check → entry step → next routing → ... → final decision
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

### 1.3 When Condition (✅ Implemented)

The `when` block controls whether the pipeline executes. Uses the same syntax as rule conditions.

```yaml
pipeline:
  when:
    all:
      - event.type == "payment"  # Only execute for payment events
      - event.amount > 100       # Additional condition (must use event. prefix)
```

If `when` condition is not met:
- Pipeline is skipped entirely
- No steps are executed
- Processing continues with next pipeline/rule

---

## 2. Step Types

A **step** is the smallest processing unit in a pipeline. All steps are wrapped in a `step:` object.

```yaml
- step:
    id: string                  # Required: Unique step identifier
    name: string                # Required: Human-readable name
    type: string                # Required: Step type (see below)
    when: <optional-condition>  # Optional: Condition for execution
    next: <step-id>             # Optional: Next step to execute
    # ... type-specific fields
```

### 2.1 Implemented Step Types (✅)

| type | Description | Status |
|------|-------------|--------|
| `router` | Pure routing step with conditional routes | ✅ Implemented |
| `ruleset` | Execute a ruleset (rulesets can contain one or more rules) | ✅ Implemented |
| `pipeline` | Call a sub-pipeline | ✅ Implemented |
| `service` | Internal microservice call (ms_http, ms_grpc, mq) | ✅ Implemented |
| `api` | External API lookup (supports single, any, all modes) | ✅ Implemented |


### 2.2 Step Conditions (✅ Implemented)

Every step may include a conditional execution block:

```yaml
- step:
    id: high_value_check
    name: High Value Transaction Check
    type: ruleset
    ruleset: high_value_rules
    when:
      all:
        - event.amount > 1000
```

The step executes **only** if the `when` condition evaluates to true.

---

### 2.3 Complete Pipeline Example (✅ Implemented Syntax)

```yaml
version: "0.1"

pipeline:
  id: payment_risk_pipeline
  name: Payment Risk Control Pipeline
  description: Comprehensive payment risk assessment with routing
  entry: ip_check               # Start with IP check step

  # Condition: Only execute for payment events
  when:
    all:
      - event.type == "payment"

  steps:
    - step:
        id: ip_check
        name: IP Address Check
        type: api
        api: ipinfo
        endpoint: ip_lookup
        params:
          ip: event.ip_address
        next: risk_assessment

    - step:
        id: risk_assessment
        name: Risk Assessment
        type: ruleset
        ruleset: payment_risk_rules

  # Pipeline decision maps signals to final results
  decision:
    - when: results.payment_risk_rules.signal == "decline"
      result: decline
      reason: "Payment declined by risk rules"

    - when: results.payment_risk_rules.signal == "review"
      result: review
      actions: ["manual_review"]
      reason: "Payment requires manual review"

    - default: true
      result: approve
      reason: "Payment approved"
```

---

## 3. Import System (Reusable Modules)

Pipelines use the `import` section to declare dependencies on rulesets and other pipelines. This enables modular, reusable pipeline design.

### 3.1 Import Declaration

Pipelines use multi-document YAML format with `---` separator:

```yaml
version: "0.1"

# First document: Import
import:
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
    - step:
        id: execute_ruleset
        type: ruleset
        ruleset: fraud_detection_core
```

**Key Benefits:**
- **Explicit Dependencies** - All dependencies declared upfront
- **Compile-Time Resolution** - Dependencies resolved during compilation
- **Automatic Transitive Dependencies** - Importing a ruleset automatically loads all its rules
- **Zero Runtime Overhead** - All merging happens at compile time

### 3.2 Dependency Propagation

When a pipeline imports a ruleset, it automatically gets all the ruleset's rule dependencies:

```yaml
# Pipeline only needs to declare the ruleset
import:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml  # This ruleset imports 6 rules

---

pipeline:
  steps:
    - step:
        id: execute_ruleset
        type: ruleset
        ruleset: fraud_detection_core  # All 6 rules are automatically available
```

The compiler resolves transitive dependencies automatically:
```
Pipeline import: fraud_detection_core
  ↓
fraud_detection_core import:
  - fraud_farm.yaml
  - account_takeover.yaml
  - velocity_abuse.yaml
  - amount_outlier.yaml
  - suspicious_geography.yaml
  - new_user_fraud.yaml
  ↓
All 6 rules are available
```

### 3.3 Executing Rulesets in Pipeline Steps

After importing a ruleset via the `import` section, you can execute it in your pipeline steps using the standard `step` syntax:

```yaml
steps:
  - step:
      id: risk_check
      type: ruleset
      ruleset: login_risk_rules
```

The ruleset step:
- Executes all rules in the ruleset
- Evaluates the ruleset's decision logic
- Returns a **signal** (`approve`, `decline`, `review`, `hold`, `pass`)
- Makes results available in `results.<ruleset_id>.signal`

**Note:** Both rulesets and pipelines use the same 5 signal types. The ruleset signal indicates the intermediate decision recommendation, while the pipeline makes the final decision based on signals from all rulesets.

### 3.4 Executing Sub-Pipelines

You can also call sub-pipelines:

```yaml
steps:
  - step:
      id: common_features
      type: pipeline
      pipeline: common_feature_flow
```

### 3.5 Complete Example with Import

This example demonstrates importing multiple rulesets, conditional routing, and decision aggregation:

```yaml
version: "0.1"

import:
  rulesets:
    - library/rulesets/payment_standard.yaml
    - library/rulesets/payment_high_value.yaml

---

pipeline:
  id: payment_pipeline
  name: Payment Risk Pipeline
  description: Payment risk assessment with conditional routing
  entry: amount_router

  when:
    all:
      - event.type == "payment"

  steps:
    # Router step based on transaction amount
    - step:
        id: amount_router
        name: Amount-Based Router
        type: router
        routes:
          # High-value transactions (> $1000)
          - next: high_value_check
            when:
              all:
                - event.transaction.amount > 1000
        default: standard_check

    # High-value transaction processing
    - step:
        id: high_value_check
        name: High Value Payment Check
        type: ruleset
        ruleset: payment_high_value

    # Standard transaction processing
    - step:
        id: standard_check
        name: Standard Payment Check
        type: ruleset
        ruleset: payment_standard

  # Pipeline decision aggregates signals from both rulesets
  decision:
    - when:
        any:
          - results.payment_high_value.signal == "decline"
          - results.payment_standard.signal == "decline"
      result: decline
      reason: "Payment risk check failed"

    - when:
        any:
          - results.payment_high_value.signal == "review"
          - results.payment_standard.signal == "review"
      result: review
      actions: ["manual_review"]
      reason: "Payment requires review"

    - default: true
      result: approve
      reason: "Payment approved"
```

### 3.6 Import Path Resolution

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
import:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml  # ✅ Correct
```

---

## 4. Decision Logic (✅ Implemented)

Pipelines use the required `decision` block to map ruleset signals to final results. This enables pipelines to orchestrate decisions from multiple rulesets.

### 4.1 Two-Level Decision Architecture

CORINT uses a two-level decision architecture:

```
┌─────────────────────────────────────────────────┐
│ Layer 2: Ruleset (Decision Suggestions)        │
│ - Evaluates rules                               │
│ - conclusion: produces signals                  │
│ - Signals: approve/decline/review/hold/pass     │
└────────────────┬────────────────────────────────┘
                 │ signals
                 ▼
┌─────────────────────────────────────────────────┐
│ Layer 3: Pipeline (Final Decision Maker)       │
│ - Maps signals to final results                │
│ - decision: produces final decision             │
│ - Results: approve/decline/review/hold          │
│ - Can add actions and routing                   │
└─────────────────────────────────────────────────┘
```

**Key Concepts:**
- **Rulesets** produce **signals** (suggestions) via `conclusion`
- **Pipelines** make **final decisions** via `decision` by mapping signals to results
- Rulesets are reusable across pipelines with different decision mappings

### 4.2 Decision Rule Structure

```yaml
decision:
  - when: <condition>          # When to apply this decision
    result: <result>           # Final result: approve/decline/review/hold
    actions: [...]             # Optional: actions to execute
    reason: <reason>           # Optional: reason for decision

  - default: true              # Default/catch-all rule
    result: approve
    reason: "No risk detected"
```

**Fields:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `when` | WhenBlock | No (if default) | Condition to evaluate (e.g., `results.fraud_check.signal == "decline"`) |
| `default` | bool | No | If true, this is the catch-all rule (no `when` needed) |
| `result` | string | Yes | Final decision: `approve`, `decline`, `review`, `hold` |
| `actions` | array | No | Actions to execute (e.g., `["KYC", "2FA"]`) |
| `reason` | string | No | Human-readable reason for the decision |

**Important: Sequential Execution and Short-Circuit Logic**

Decision rules are evaluated sequentially from top to bottom:

1. **Sequential Execution**: The system evaluates each `when` condition in order
2. **Short-Circuit Logic**: **When the first `when` condition is satisfied, the corresponding `result` is executed immediately, and all subsequent rules are skipped.**
3. **Default Rule**: The `default: true` rule is only executed when all preceding `when` conditions are not satisfied

This means the order of decision rules is crucial! They should be arranged from **most specific to most general**.

**Example:**

```yaml
decision:
  # Rule 1: Critical risk - immediate decline
  - when: results.fraud_check.signal == "decline"
    result: decline
    reason: "Fraud detected"

  # Rule 2: Only checked if rule 1 is not satisfied
  - when: results.fraud_check.signal == "review"
    result: review
    reason: "Manual review required"

  # Default rule: Only executed when all preceding rules are not satisfied
  - default: true
    result: approve
    reason: "All checks passed"
```

**Execution Flow Examples:**

- If `fraud_check.signal = "decline"`: Rule 1 satisfied → `decline` → **stop** (rules 2, default skipped)
- If `fraud_check.signal = "review"`: Rule 1 not satisfied, Rule 2 satisfied → `review` → **stop**
- If `fraud_check.signal = "approve"`: Rules 1, 2 not satisfied → execute default → `approve`

### 4.3 When to Use Pipeline Decision

**Use Pipeline `decision` when:**
- Orchestrating multiple rulesets with different signals
- Mapping ruleset signals to different final results
- Adding pipeline-specific actions or routing
- Overriding or transforming ruleset suggestions

**Use Ruleset `conclusion` when:**
- Single ruleset producing the final decision
- Decision logic is tightly coupled to the rules
- Want maximum reusability of rulesets

**Example - Multiple Rulesets:**

```yaml
pipeline:
  id: comprehensive_risk
  entry: step1

  steps:
    - step:
        id: step1
        type: ruleset
        ruleset: fraud_detection
        next: step2

    - step:
        id: step2
        type: ruleset
        ruleset: compliance_check

  # Combine signals from both rulesets
  decision:
    # If either signals decline, decline
    - when:
        any:
          - results.fraud_detection.signal == "decline"
          - results.compliance_check.signal == "decline"
      result: decline
      reason: "Failed risk or compliance check"

    # If both approve, approve
    - when:
        all:
          - results.fraud_detection.signal == "approve"
          - results.compliance_check.signal == "approve"
      result: approve
      reason: "Passed all checks"

    # Otherwise, review
    - default: true
      result: review
      actions: ["manual_review"]
      reason: "Mixed signals from risk engines"
```

---

## 5. Full Pipeline Examples

### 5.1 Multi-Event Router Pipeline (✅ Correct Syntax)

```yaml
version: "0.1"

pipeline:
  id: multi_event_router
  name: Multi-Event Type Router
  description: Routes different event types to appropriate processing pipelines
  entry: event_router

  steps:
    # Router step to direct to different flows
    - step:
        id: event_router
        name: Event Type Router
        type: router
        routes:
          - next: login_flow
            when:
              all:
                - event.type == "login"
          - next: payment_flow
            when:
              all:
                - event.type == "payment"
          - next: crypto_flow
            when:
              all:
                - event.type == "crypto_transfer"
        default: default_flow

    # Login processing
    - step:
        id: login_flow
        name: Login Risk Check
        type: ruleset
        ruleset: login_risk_rules

    # Payment processing
    - step:
        id: payment_flow
        name: Payment Risk Check
        type: ruleset
        ruleset: payment_risk_rules

    # Crypto processing
    - step:
        id: crypto_flow
        name: Crypto Risk Check
        type: ruleset
        ruleset: web3_wallet_risk

    # Default processing
    - step:
        id: default_flow
        name: Default Processing
        type: ruleset
        ruleset: default_rules

  # Pipeline decision aggregates signals from all rulesets
  decision:
    # Check signals from all possible rulesets
    - when:
        any:
          - results.login_risk_rules.signal == "decline"
          - results.payment_risk_rules.signal == "decline"
          - results.web3_wallet_risk.signal == "decline"
          - results.default_rules.signal == "decline"
      result: decline
      reason: "Risk check failed"

    - when:
        any:
          - results.login_risk_rules.signal == "review"
          - results.payment_risk_rules.signal == "review"
          - results.web3_wallet_risk.signal == "review"
          - results.default_rules.signal == "review"
      result: review
      actions: ["manual_review"]
      reason: "Manual review required"

    - default: true
      result: approve
      reason: "All checks passed"
```

---

### 5.2 Service Integration Pipeline (✅ Correct Syntax)

```yaml
version: "0.1"

pipeline:
  id: service_integration_pipeline
  name: Service Integration Example
  entry: verify_kyc

  steps:
    # Step 1: Call internal HTTP microservice for KYC verification
    - step:
        id: verify_kyc
        name: Verify KYC
        type: service
        service: kyc_service
        endpoint: verify_identity
        next: calculate_risk

    # Step 2: Call internal gRPC microservice for risk scoring
    - step:
        id: calculate_risk
        name: Calculate Risk Score
        type: service
        service: risk_scoring_service
        method: calculate_score
        next: ip_reputation

    # Step 3: Check external API
    - step:
        id: ip_reputation
        name: IP Reputation Check
        type: api
        api: maxmind
        endpoint: ip_lookup
        params:
          ip: event.ip_address
        next: risk_assessment

    # Step 4: Execute rules with all context
    - step:
        id: risk_assessment
        name: Risk Assessment
        type: ruleset
        ruleset: comprehensive_risk_check
        next: publish_decision

    # Step 5: Publish decision to message queue
    - step:
        id: publish_decision
        name: Publish Decision
        type: service
        service: event_bus
        topic: risk_decisions

  # Pipeline decision maps signals to final results
  decision:
    - when: results.comprehensive_risk_check.signal == "decline"
      result: decline
      reason: "Risk check failed"

    - when: results.comprehensive_risk_check.signal == "review"
      result: review
      actions: ["manual_review", "enhanced_verification"]
      reason: "Requires manual review"

    - default: true
      result: approve
      reason: "Risk assessment passed"
```

---

## 6. BNF Grammar (Formal)

```
PIPELINE ::=
      "pipeline:"
         "id:" STRING
         [ "name:" STRING ]
         [ "description:" STRING ]
         "entry:" STRING
         [ "when:" WHEN_BLOCK ]
         "steps:" STEP_LIST
         "decision:" DECISION_LIST
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

STEP_TYPE ::= "router" | "ruleset" | "pipeline" | "service" | "api"

STEP_TYPE_PARAMS ::=
      ROUTER_PARAMS
    | RULESET_PARAMS
    | PIPELINE_PARAMS
    | SERVICE_PARAMS
    | API_PARAMS

ROUTER_PARAMS ::=
      "routes:" ROUTE_LIST
      [ "default:" STRING ]

ROUTE_LIST ::= "-" ROUTE { "-" ROUTE }

ROUTE ::=
      "next:" STRING
      "when:" WHEN_BLOCK

RULESET_PARAMS ::= "ruleset:" STRING

PIPELINE_PARAMS ::= "pipeline:" STRING

SERVICE_PARAMS ::=
      "service:" STRING
      ( "endpoint:" STRING | "method:" STRING | "topic:" STRING )
      [ "params:" OBJECT ]

API_PARAMS ::=
      "api:" STRING
      [ "endpoint:" STRING ]
      [ "params:" OBJECT ]

DECISION_LIST ::= "-" DECISION_RULE { "-" DECISION_RULE }

DECISION_RULE ::=
      [ "when:" WHEN_BLOCK ]
      [ "default:" BOOLEAN ]
      "result:" RESULT_TYPE
      [ "actions:" ARRAY ]
      [ "reason:" STRING ]

RESULT_TYPE ::= "approve" | "decline" | "review" | "hold"

METADATA_MAP ::= KEY ":" VALUE { KEY ":" VALUE }

OBJECT ::= KEY ":" VALUE { KEY ":" VALUE }

ARRAY ::= "[" VALUE { "," VALUE } "]"
```

---

## 7. Related Documentation

For comprehensive understanding of pipelines and the CORINT ecosystem:

### Core Concepts
- [import.md](import.md) - Complete module system and dependency management specification
- [ruleset.md](ruleset.md) - Ruleset specification and decision logic
- [rule.md](rule.md) - Individual rule specification

### Advanced Topics
- [expression.md](expression.md) - Expression language for conditions
- [context.md](context.md) - Context management and data flow between steps
- [feature.md](feature.md) - Feature engineering and extraction

### Integration
- [service.md](service.md) - Internal service integration (microservices and message queues)
- [api.md](api.md) - External API integration

### Development Tools
- [LLM_GUIDE.md](../LLM_GUIDE.md) - LLM code generation guide (development-time only)

### Architecture
- [overall.md](overall.md) - High-level RDL overview
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Three-layer decision architecture

---

## 8. Summary

### 8.1 What's Implemented (✅)

A CORINT Pipeline currently supports:

**Core Structure:**
- ✅ `id`, `name`, `description` - Basic metadata
- ✅ `entry` - Explicit DAG entry point (required)
- ✅ `when` - Conditional pipeline execution
- ✅ `steps` - Processing step orchestration
- ✅ `decision` - Pipeline-level decision logic (required)
- ✅ `metadata` - Arbitrary key-value metadata

**Step Types:**
- ✅ `router` - Conditional routing with routes
- ✅ `ruleset` - Ruleset execution (produces signals, can contain one or more rules)
- ✅ `pipeline` - Sub-pipeline calls
- ✅ `service` - Internal service calls (ms_http, ms_grpc, mq for message queue/webhook/notification)
- ✅ `api` - External API calls (single, any, all modes)

**Features:**
- ✅ Import system for modular composition
- ✅ Automatic transitive dependency resolution
- ✅ Compile-time validation
- ✅ Router steps for conditional flows
- ✅ Sequential execution with explicit `next`
- ✅ Pipeline-level decision logic for mapping signals to results

### 8.2 What's Planned (📋)

**Not Yet Implemented:**
- 📋 Parallel execution in new format (legacy format only)
- 📋 Branching in new format (use `router` step instead)
- 📋 Aggregation steps (use expression features instead)
- 📋 `score` and `action` step types

### 8.3 Current Architecture

**Three-Layer Architecture:**
```
Pipeline (Orchestration & Final Decision)
    ↓
Ruleset (Rule Evaluation & Signal Generation)
    ↓
Rules (Individual Risk Pattern Detection)
```

**Layer Responsibilities:**
- **Layer 1 - Rules** (✅) - Detect and score individual risk patterns
- **Layer 2 - Rulesets** (✅) - Evaluate rules and produce **signals** via `conclusion` blocks
  - Signals: `approve`, `decline`, `review`, `hold`, `pass`
  - Actions: `KYC`, `OTP`, `2FA`, `BLOCK_DEVICE`, etc.
  - A ruleset can contain one or more rules
- **Layer 3 - Pipelines** (✅) - Orchestrate rulesets, route events, make final decisions
  - Pipeline-level `decision` logic maps ruleset signals to final results
  - Enables orchestration of multiple rulesets with different decision mappings
  - **Pipelines only interact with Rulesets**, never directly with individual Rules

**Key Architectural Decisions:**
- Same ruleset can be reused across pipelines with different decision mappings
- Clear separation of concerns: Rules detect, Rulesets suggest, Pipelines decide
- Unified interface: Pipelines work exclusively with rulesets (even for single-rule scenarios)

### 8.4 Benefits

- ✅ 80-90% code reduction through modular design
- ✅ Dependencies resolved at compile time
- ✅ Type-safe ID references
- ✅ Reusable rulesets across pipelines
- ✅ Clear separation of concerns

It is the highest-level construct of CORINT's Risk Definition Language (RDL).
