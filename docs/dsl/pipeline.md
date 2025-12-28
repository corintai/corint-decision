# CORINT Risk Definition Language (RDL)
## Pipeline Specification (v0.2)

A **Pipeline** defines the full risk‑processing flow in CORINT's Cognitive Risk Intelligence framework.
It represents a declarative Directed Acyclic Graph (DAG) composed of processing steps with explicit routing.

Pipelines orchestrate how events move through feature extraction, rule execution, and external service integration.

> **⚠️ Important:** This document clearly marks **✅ Implemented** vs **📋 Planned** features.

**Current Implementation:**
- **Rules** detect and score individual risk patterns (✅ Implemented)
- **Rulesets** produce **signals** (`approve`, `decline`, `review`, `hold`, `pass`) based on rule results (✅ Implemented)
- **Pipelines** execute rulesets and route events through processing steps (✅ Implemented)
- **Pipeline-level decision logic** is documented below but **📋 NOT YET IMPLEMENTED**

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
  metadata:                     # ✅ Optional: Arbitrary key-value pairs
    <key>: <value>
```

### 1.2 Planned Fields (📋 Not Yet Implemented)

```yaml
pipeline:
  decision:                     # 📋 NOT IMPLEMENTED: Pipeline-level decision logic
    - <decision-rules>          # Currently: Only rulesets have decision logic via 'conclusion'
```

**Important:**
- The `entry` field is **required** and specifies which step to start with
- The `decision` field is documented in Section 7 but **NOT implemented** in the Pipeline AST
- For decision logic, use **Ruleset conclusion** instead (see [ruleset.md](ruleset.md))

### 1.3 Execution Flow

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

### 1.4 Pipeline Metadata (✅ Implemented)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique identifier for the pipeline |
| `name` | string | Yes | Human-readable name |
| `description` | string | No | Detailed description of pipeline purpose |
| `entry` | string | Yes | ID of the first step to execute |
| `when` | object | No | Execution condition (event type filter) |
| `steps` | array | Yes | List of processing steps |
| `metadata` | object | No | Arbitrary key-value pairs for documentation, versioning, authorship, etc. |

The `metadata` field allows you to attach arbitrary information to your pipeline for documentation, versioning, and management purposes.

**Note:** Unlike what was previously documented, there are **no required fields** within metadata. All metadata fields are optional and user-defined.

**Example**:
```yaml
pipeline:
  id: fraud_detection
  name: Fraud Detection Pipeline
  entry: initial_check
  description: Comprehensive fraud detection
  metadata:
    # All fields are optional - define what you need
    version: "2.1.0"
    author: "Risk Engineering Team"
    updated: "2025-12-20 14:30:00"
    owner: "fraud_team"
    environment: "production"
    tags: [fraud, risk_assessment]
```

### 1.5 When Condition (✅ Implemented)

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

Each element is one of the pipeline constructs described below.

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
| `function` | Pure computation step | ✅ Implemented |
| `rule` | Execute a single rule | ✅ Implemented |
| `ruleset` | Execute a ruleset | ✅ Implemented |
| `pipeline` | Call a sub-pipeline | ✅ Implemented |
| `service` | Internal service call (database, cache, microservice, etc.) | ✅ Implemented |
| `api` | External API lookup (supports single, any, all modes) | ✅ Implemented |
| `trigger` | External action (message queue, webhook, notification) | ✅ Implemented |
| `extract` | Feature extraction (legacy format) | ✅ Implemented (legacy) |
| `reason` | LLM cognitive reasoning step (legacy format) | ✅ Implemented (legacy) |

### 2.2 Planned Step Types (📋)

| type | Description | Status |
|------|-------------|--------|
| `score` | Score computation or normalization | 📋 Planned (use expression features instead) |
| `action` | Produces final decision outcome | 📋 Planned (use ruleset conclusion instead) |
| `branch` | Conditional branching | 📋 Planned (use router step instead) |
| `parallel` | Parallel execution | 📋 Planned (currently in legacy format only) |
| `aggregate` | Aggregation of results | 📋 Planned |

### 2.3 Step Conditions (✅ Implemented)

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

## 2.4 Complete Pipeline Example (✅ Implemented Syntax)

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
```

---

## 3. Branching (📋 Legacy Format Only)

> **⚠️ Note:** Branch syntax shown below is in **legacy format only**. For new pipelines, use **Router steps** with conditional routes instead.

A branch selects between multiple sub‑pipelines based on conditions (legacy format):

```yaml
# LEGACY FORMAT - use router step instead for new pipelines
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

**Modern Alternative (✅ Implemented):**

Use router steps with conditional routes:

```yaml
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
    default: default_flow
```

---

## 4. Parallel Execution (📋 Legacy Format Only)

> **⚠️ Note:** Parallel execution is **not implemented** in the new PipelineStep format. It exists only in the legacy Step enum.

```yaml
# LEGACY FORMAT ONLY - not supported in new format
- parallel:
    - device_fingerprint
    - ip_reputation
    - llm_reasoning
  merge:
    method: all
```

**Current Workaround:**
Execute steps sequentially or use external orchestration.

---

## 5. Aggregation (📋 Not Implemented)

> **⚠️ Note:** Aggregation steps are **not implemented** in the current version.

```yaml
# NOT IMPLEMENTED
- aggregate:
    method: weighted
    weights:
      rules_engine: 0.5
      llm_reasoning: 0.3
      chainalysis: 0.2
```

**Current Workaround:**
Use expression features to compute weighted scores, or aggregate in ruleset conclusion logic.  

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

## 7. Decision Logic (⚠️ NOT IMPLEMENTED - Ignored by Parser)

> **⚠️ CRITICAL:** The `decision` field is **NOT IMPLEMENTED** in Pipeline. While you may see it in test files, it is **silently ignored** by the parser and has no effect.

**Evidence:**
1. Pipeline AST has no `decision` field (`crates/corint-core/src/ast/pipeline.rs` lines 18-44)
2. Pipeline parser does not parse `decision` field (`crates/corint-parser/src/pipeline/parser.rs` lines 75-129)
3. The `decision` block in YAML is silently ignored by serde (no `deny_unknown_fields`)

**Current Implementation:**
- **Rulesets** have `conclusion` blocks that produce decision signals (✅ Implemented)
- **Pipelines** do NOT have decision logic - they only execute steps (✅ Implemented)
- For decision-making, use **Ruleset conclusion** instead

### 7.1 Why Test Files Have `decision` Blocks

You may see `decision` blocks in test files like `tests/e2e_repo/pipelines/transaction_test.yaml`:

```yaml
pipeline:
  steps:
    - step:
        type: ruleset
        ruleset: transaction_risk_ruleset

  # This block is IGNORED by the parser!
  decision:
    - when: results.transaction_risk_ruleset.signal == "decline"
      result: decline
```

**These blocks are ignored and have no effect.** Tests pass because:
1. The **Ruleset's `conclusion`** already produces the decision signal
2. The Pipeline's `decision` block is silently ignored
3. Tests verify the Ruleset output, not the Pipeline decision

### 7.2 Correct Approach: Use Ruleset Conclusion

Since pipeline-level decisions are not implemented, use **Ruleset conclusion** instead:

```yaml
# ✅ THIS WORKS - Decision logic in Ruleset
ruleset:
  id: transaction_risk_ruleset
  name: Transaction Risk Ruleset
  rules:
    - high_risk_country
    - velocity_abuse
    - amount_outlier

  # Conclusion provides the decision logic
  conclusion:
    # Critical risk - blocked user
    - when: triggered_rules contains "user_blocked"
      signal: decline
      reason: "User is blocked"
      terminate: true

    # High risk - multiple indicators or high score
    - when: total_score >= 150
      signal: decline
      reason: "High risk score threshold exceeded"
      terminate: true

    # Medium risk - review
    - when: total_score >= 80
      signal: review
      reason: "Medium risk score - manual review required"
      terminate: true

    # Low risk - approve (default)
    - default: true
      signal: approve
      reason: "No significant risk indicators"
```

Then reference this ruleset in your pipeline:

```yaml
# ✅ THIS WORKS - Pipeline executes ruleset
pipeline:
  id: transaction_pipeline
  name: Transaction Risk Pipeline
  entry: fraud_check

  steps:
    - step:
        id: fraud_check
        name: Execute Fraud Detection
        type: ruleset
        ruleset: transaction_risk_ruleset
        # The ruleset's conclusion produces the final decision
```

### 7.3 Architecture: Where Decisions Are Made

```
┌─────────────────────────────────────────────────┐
│ Pipeline (Orchestration Layer)                  │
│ - Routes events to rulesets                     │
│ - NO decision logic                             │
│ - decision: field is IGNORED                    │
└────────────────┬────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────┐
│ Ruleset (Decision Layer)                        │
│ - Evaluates rules                               │
│ - conclusion: produces decision signals         │
│ - Signals: approve/decline/review/hold/pass     │
└─────────────────────────────────────────────────┘
```

**Key Point:** Decision logic belongs in **Rulesets**, not Pipelines.

### 7.4 Why Pipeline Decisions Are Not Implemented

1. **Sufficient:** Ruleset `conclusion` blocks already provide all decision logic needed
2. **Simpler Architecture:** Having decision logic only in rulesets keeps the system simpler
3. **Current Pattern:** Pipelines orchestrate execution, Rulesets make decisions
4. **No Parser Support:** The pipeline parser does not parse or store decision blocks

**Future Enhancement:** Pipeline-level decisions may be added later for multi-ruleset orchestration scenarios, but this would require:
- Adding `decision` field to Pipeline AST
- Implementing parser support
- Implementing runtime execution logic

---

## 8. Full Pipeline Example

### 8.1 Login Risk Processing Pipeline (✅ Correct Syntax)

```yaml
version: "0.1"

pipeline:
  id: login_risk_pipeline
  name: Login Risk Assessment Pipeline
  description: Comprehensive login risk evaluation
  entry: ip_check

  # Only process login events
  when:
    all:
      - event.type == "login"

  steps:
    # Step 1: Check IP reputation
    - step:
        id: ip_check
        name: IP Reputation Check
        type: api
        api: ip_reputation_service
        endpoint: check_ip
        params:
          ip_address: event.ip_address
        next: login_risk_check

    # Step 2: Execute login risk ruleset
    - step:
        id: login_risk_check
        name: Login Risk Assessment
        type: ruleset
        ruleset: login_risk_rules
```

**Note:** The ruleset `login_risk_rules` would contain the `conclusion` block with decision logic (signals, actions, reasons). See [ruleset.md](ruleset.md) for details.

### 8.2 Legacy Format Example (With Unsupported Features)

```yaml
# THIS IS LEGACY/ASPIRATIONAL FORMAT - Some features not implemented
version: "0.2"

pipeline:
  id: legacy_example
  name: Legacy Format Example

  steps:
    # Legacy extract step (still supported)
    - type: extract
      id: extract_device

    # Parallel execution (legacy format only)
    - parallel:
        - device_fingerprint
        - ip_reputation
      merge:
        method: all

    # Include ruleset (legacy shorthand)
    - include:
        ruleset: login_risk_rules

    # Aggregation (NOT IMPLEMENTED)
    - aggregate:
        method: weighted
        weights:
          rules: 0.5
          ip: 0.5

  # Pipeline decision (NOT IMPLEMENTED)
  decision:
    - when: results.login_risk_rules.signal == "decline"
      result: decline
      actions: ["BLOCK_DEVICE"]
      reason: "High risk"
      terminate: true
```

---

### 8.3 Multi-Event Router Pipeline (✅ Correct Syntax)

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
```

---

### 8.4 Service Integration Pipeline (✅ Correct Syntax)

```yaml
version: "0.1"

pipeline:
  id: service_integration_pipeline
  name: Service Integration Example
  entry: load_user_profile

  steps:
    # Step 1: Load user profile from database
    - step:
        id: load_user_profile
        name: Load User Profile
        type: service
        service: user_db
        query: get_user_profile
        params:
          user_id: event.user.id
        output: context.user_profile
        next: check_risk_cache

    # Step 2: Check cache for existing risk score
    - step:
        id: check_risk_cache
        name: Check Risk Cache
        type: service
        service: redis_cache
        query: get_user_risk_cache
        output: context.cached_risk
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
        type: trigger
        target: event_bus.risk_decisions
        params:
          decision: results.comprehensive_risk_check.signal
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

### 11.1 What's Implemented (✅)

A CORINT Pipeline currently supports:

**Core Structure:**
- ✅ `id`, `name`, `description` - Basic metadata
- ✅ `entry` - Explicit DAG entry point (required)
- ✅ `when` - Conditional pipeline execution
- ✅ `steps` - Processing step orchestration
- ✅ `metadata` - Arbitrary key-value metadata

**Step Types:**
- ✅ `router` - Conditional routing with routes
- ✅ `function` - Pure computation
- ✅ `rule` - Single rule execution
- ✅ `ruleset` - Ruleset execution (produces signals)
- ✅ `pipeline` - Sub-pipeline calls
- ✅ `service` - Internal service calls
- ✅ `api` - External API calls (single, any, all modes)
- ✅ `trigger` - External actions
- ✅ `extract`, `reason` - Legacy format support

**Features:**
- ✅ Imports system for modular composition
- ✅ Automatic transitive dependency resolution
- ✅ Compile-time validation
- ✅ Router steps for conditional flows
- ✅ Sequential execution with explicit `next`

### 11.2 What's Planned (📋)

**Not Yet Implemented:**
- 📋 Pipeline-level `decision` blocks (use Ruleset `conclusion` instead)
- 📋 Parallel execution in new format (legacy format only)
- 📋 Branching in new format (use `router` step instead)
- 📋 Aggregation steps (use expression features instead)
- 📋 `score` and `action` step types

### 11.3 Current Architecture

**Decision-Making Model:**
- **Rules** (✅) - Detect and score individual risk patterns
- **Rulesets** (✅) - Produce **signals** via `conclusion` blocks
  - Signals: `approve`, `decline`, `review`, `hold`, `pass`
  - Actions: `KYC`, `OTP`, `2FA`, `BLOCK_DEVICE`, etc.
- **Pipelines** (✅) - Orchestrate execution, route events
  - Currently: NO decision logic at pipeline level
  - Decision logic resides in Rulesets

**Why This Works:**
- Rulesets already provide all needed decision capabilities
- Simpler architecture with single decision point
- Same ruleset can be reused across multiple pipelines
- Clear separation: Pipelines orchestrate, Rulesets decide

### 11.4 Benefits

- ✅ 80-90% code reduction through modular design
- ✅ Dependencies resolved at compile time
- ✅ Type-safe ID references
- ✅ Reusable rulesets across pipelines
- ✅ Clear separation of concerns

It is the highest-level construct of CORINT's Risk Definition Language (RDL).
