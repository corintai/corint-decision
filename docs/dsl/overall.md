# CORINT Risk Definition Language (RDL)
## Overall Specification (v0.1)

**RDL is the domain-specific language used by CORINT (Cognitive Risk Intelligence) to define rules, rule groups, and full risk‑processing pipelines.**
It enables modern hybrid risk engines to combine deterministic logic with external data sources and APIs in a unified, explainable, high‑performance format.

---

## 1. Goals of RDL

RDL is designed to:

- Provide a declarative, human-readable format for risk logic
- Be LLM-friendly for automated generation and modification  
- Support traditional rule engines with modern data integration
- Compile into a Rust‑friendly IR (AST) for high‑performance execution
- Represent end‑to‑end risk processing flows
- Enable transparency, auditability, and explainability
- Be cloud‑native, language‑agnostic, and extensible

---

## 2. Top-Level Components

An RDL file may contain one of the following:

```yaml
version: "0.1"

# Optional: Import dependencies
import:
  rules:
    - library/rules/fraud/fraud_farm.yaml
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

rule: {...}
# OR
ruleset: {...}
# OR
pipeline: [...]
```

Components:

| Component | Purpose |
|----------|---------|
| **import** | Declare dependencies on external rules/rulesets (optional) |
| **rule** | A single risk logic unit |
| **ruleset** | A group of rules |
| **pipeline** | The full risk processing DAG |

### 2.1 Import (Module System)

RDL supports a module system for code reuse and maintainability:

```yaml
version: "0.1"

# Declare dependencies
import:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/payment/card_testing.yaml
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

# Use imported components
ruleset:
  id: my_custom_ruleset
  rules:
    - fraud_farm_pattern      # From imported rule
    - card_testing            # From imported rule
```

**Key Principles:**
- **Explicit Dependencies**: All dependencies must be declared via `import`
- **Compile-Time Merging**: Dependencies resolved during compilation
- **Dependency Propagation**: Ruleset import automatically loads their rule dependencies
- **No Circular Dependencies**: Compiler detects and prevents circular import
- **ID Uniqueness**: All Rule IDs and Ruleset IDs must be globally unique

**Dependency Hierarchy:**
```
Pipeline
  ↓ import rulesets
Ruleset
  ↓ import rules
Rule (leaf, no dependencies)
```

(See `import.md` for complete specification.)

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
```

---

### 3.1.1 `when` Block

#### Event Filter

```yaml
when:
  all:
    - event.type == "login"
```

#### Conditions

```yaml
when:
  all:
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

### 3.1.2 External API Conditions

```yaml
- api.Chainalysis.risk_score > 80
```

---

### 3.1.3 Decision Making

**Actions are not defined in Rules.**  
Rules only detect risk factors and provide scores.

Signals are produced in Ruleset's `conclusion`:

```yaml
ruleset:
  id: fraud_detection
  rules:
    - rule_1  # Just scores
    - rule_2  # Just scores

  conclusion:
    - when: total_score >= 100
      signal: decline  # Signals defined here
      reason: "High risk detected"

    - when: total_score >= 50
      signal: review
      reason: "Medium risk - needs review"

    - default: true
      signal: approve
      reason: "No significant risk"
```

**Built-in Signals:**
- `approve` - Approve the request
- `decline` - Decline/block the request
- `review` - Send to human review
- `hold` - Temporarily suspend (waiting for verification)
- `pass` - Skip/no decision  

---

## 3.2 Ruleset

```yaml
ruleset:
  id: string
  name: string
  rules:
    - rule_id_1
    - rule_id_2
    - rule_id_3
  conclusion:
    - when: <expression>
      signal: <signal-type>
      reason: <string>
    - default: true
      signal: <signal-type>
      reason: <string>
```

Rulesets group rules and produce decision signals via conclusion logic based on rule combinations.

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

RDL provides a powerful expression language for defining conditions and computations.

Key features:
- Logical operators (AND, OR, NOT)
- Comparison and membership operators
- String operations and regex
- Time-based aggregations
- Built-in functions
- External API integration

(See `expression.md` for complete specification.)

---

## 4.5 Feature Engineering and Statistical Analysis

RDL includes comprehensive feature engineering capabilities for risk control scenarios.

**Implementation Status:**

**🟢 Currently Available (Production-Ready):**
- **Aggregation Features:**
  - Basic: count, sum, avg, min, max
  - Association analysis: count_distinct for unique value counting
  - Expression features: compute from other features
  - Lookup features: retrieve pre-computed values

**📋 In Development:**
- **Advanced Statistics:** percentile, stddev, median, mode (SQL generation exists, orchestration in progress)
- **State Features:** z_score, outlier detection, baseline comparison
- **Sequence Features:** pattern matching, consecutive counts, trends
- **Graph Features:** network analysis, centrality, community detection

**Usage Examples:**

**✅ Available Now:**
```yaml
# Feature Definition
features:
  # Distinct Count - Association analysis
  - name: distinct_userid_device_5h
    description: "Number of unique devices in last 5 hours"
    type: aggregation
    method: distinct
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "{event.user_id}"
    field: device_id
    window: 5h
    when:
      all:
        - type == "login"
        - geo.ip == "{event.geo.ip}"

  # Basic aggregations
  - name: sum_userid_txn_amt_24h
    description: "Total transaction amount in last 24 hours"
    type: aggregation
    method: sum
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "{event.user_id}"
    field: amount
    window: 24h
    when: type == "transaction"

  - name: avg_userid_txn_amt_7d
    description: "Average transaction amount in last 7 days"
    type: aggregation
    method: avg
    datasource: postgresql_events
    entity: events
    dimension: user_id
    dimension_value: "{event.user_id}"
    field: amount
    window: 7d
    when: type == "transaction"

# Usage in Rules
rule:
  id: high_risk_detection
  when:
    all:
      - features.distinct_userid_device_5h > 10
      - features.sum_userid_txn_amt_24h > 5000
      - features.avg_userid_txn_amt_7d > 1000
  score: 80
```

**📋 Coming Soon:**
```yaml
# Statistical Functions - In development
features:
  - name: p95_userid_txn_amt_90d
    type: aggregation
    method: percentile
    percentile: 95
    field: amount
    window: 90d

  - name: stddev_userid_txn_amt_30d
    type: aggregation
    method: stddev
    field: amount
    window: 30d

  - name: zscore_userid_txn_amt
    type: state
    method: z_score
    field: amount
    current_value: "{event.amount}"
    window: 90d
```

**Common Use Cases (✅ Available Now):**
- Login count for an account in the past 7 days ✅
- Number of device IDs associated with the same IP in the past 5 hours ✅
- Number of users associated with the same device ✅
- Transaction sum/avg over time windows ✅
- Custom velocity ratios using expression features ✅

**Future Capabilities (📋 Planned):**
- Statistical outlier detection (above 95th percentile)
- Z-score based anomaly detection
- Temporal pattern analysis (impossible travel)
- Behavioral velocity tracking
- Sequence pattern matching

(See `feature.md` for complete specification and examples.)

---

## 5. Context and Variable Management

CORINT uses a **flattened namespace architecture** with 8 namespaces organized by processing method:

| Namespace | Mutability | Purpose |
|-----------|------------|---------|
| `event` | Read-only | Raw user request data |
| `features` | Writable | Complex feature computations (DB queries, aggregations) |
| `api` | Writable | External third-party API results |
| `service` | Writable | Internal microservice results |
| `vars` | Writable | Simple variables and calculations |
| `sys` | Read-only | System auto-generated metadata |
| `env` | Read-only | Environment configuration |
| `results` | Read-only | Ruleset execution results (pipeline-level) |

**Access Pattern:**
All namespaces use dot notation for field access:
```yaml
event.user.id                    # Nested field access
features.transaction_count_7d     # Feature value
api.device_fingerprint.risk_score # Multi-level nesting
results.fraud_detection.signal    # Ruleset result
```

**Usage Example:**
```yaml
rule:
  when:
    all:
      - event.amount > 1000
      - features.transaction_count_7d > 20
      - api.device_fingerprint.risk_score > 0.7
      - vars.high_risk_threshold < 80
      - sys.hour >= 22
```

(See `context.md` for complete details and BNF grammar.)

---

## 7. Error Handling

Production-grade error handling ensures reliability and graceful degradation.

Current implementation:
- **Basic error types** - RuntimeError and ServerError definitions
- **Simple fallback** - Fallback values for external API calls (implemented at runtime level)
- **Timeout support** - Timeout configuration for API calls and datasources
- **Error action types** - ErrorAction enum (Fallback, Skip, Fail, Retry) defined in AST

Note: Advanced error handling features (retry logic, circuit breaker, fallback chains) are planned but not yet implemented.

---

## 8. Internal Service Integration

RDL provides integration with internal microservices and message queues.

Service types:
- **HTTP microservices** (`ms_http`) - Internal RESTful services
- **gRPC microservices** (`ms_grpc`) - Internal gRPC services
- **Message queues** (`mq`) - Kafka, RabbitMQ event streaming

Example:
```yaml
pipeline:
  # Call internal HTTP microservice
  - type: service
    id: verify_kyc
    service: kyc_service
    endpoint: verify_identity

  # Call internal gRPC service
  - type: service
    id: calculate_risk
    service: risk_scoring_service
    method: calculate_score

  # Publish to message queue
  - type: service
    id: publish_event
    service: event_bus
    topic: risk_decisions
```

**Note:** For database and cache access, use **Datasources** (see datasources configuration). For third-party HTTP APIs, use **External APIs** (see `external.md`).

(See `service.md` for complete specification.)

---

## 9. External API Integration

```yaml
external_api.<provider>.<field>
```

Example:

```yaml
external_api.Chainalysis.risk_score > 80
```

(See `external.md` for complete specification.)

---

## 10. Documentation Structure

RDL documentation is organized as follows:

### Overview & Architecture
- **overall.md** (this file) - High-level overview and introduction
- **ARCHITECTURE.md** - Three-layer decision architecture (design philosophy)

### Core Components
- **expression.md** - Expression language (fundamental syntax)
- **rule.md** - Rule specification (including dynamic thresholds and dependencies)
- **ruleset.md** - Ruleset specification
- **pipeline.md** - Pipeline orchestration
- **import.md** - Module system and dependency management (NEW)

### Data & Context
- **event.md** - Standard event types and schemas
- **context.md** - Context and variable management

### Advanced Features
- **feature.md** - Feature engineering and statistical analysis
- **list.md** - Custom List feature (blocklists, allowlists, multi-backend support)
- **service.md** - Internal service integration (microservices, message queues)
- **external.md** - External API integration (third-party services)

### Operational
- (Error handling implemented at runtime level)

### Examples
- **examples/** - Real-world pipeline examples
- **repository/** - Production rule library and reusable components

---

## 11. Examples

### 11.1 Login Risk Example

```yaml
version: "0.1"

rule:
  id: high_risk_login
  name: High-Risk Login Detection
  description: Detect risky login behavior using rules and external data

  when:
    all:
      - event.type == "login"
      - device.is_new == true
      - geo.country in ["RU", "UA", "NG"]
      - user.login_failed_count > 3
      - features.device_change_velocity > 0.7

  score: +80
```

---

### 11.2 High Value Transaction

```yaml
version: "0.1"

rule:
  id: high_value_transaction
  name: High Value Transaction Detection
  description: Detect high-value transactions from new accounts

  when:
    all:
      - event.type == "transaction"
      - event.amount > 10000
      - features.account_age_days < 30
      - features.transaction_count_7d > 5

  score: +120
```

---

## 12. BNF Grammar (Formal)

```
RDL ::= "version" ":" STRING
        (RULE | RULESET | PIPELINE)

RULE ::= "rule:" RULE_BODY

RULE_BODY ::=
      "id:" STRING
      "name:" STRING
      "description:" STRING
      [ "params:" PARAMS_MAP ]
      "when:" CONDITION_BLOCK
      "score:" NUMBER
      [ "metadata:" METADATA_MAP ]

CONDITION_BLOCK ::=
      "all:" CONDITION_LIST
    | "any:" CONDITION_LIST
    | EVENT_FILTER

EVENT_FILTER ::= KEY ":" VALUE

CONDITION_LIST ::=
      "-" CONDITION { "-" CONDITION }

CONDITION ::=
      EXPRESSION
    | EXTERNAL_EXPR

EXPRESSION ::= FIELD OP VALUE

FIELD ::= IDENT ("." IDENT)*

OP ::= "==" | "!=" | "<" | ">" | "<=" | ">=" | "in" | "regex" | "exists" | "missing"

MATCH_OP ::= "contains" | "not_contains"

EXTERNAL_EXPR ::=
      "external_api." IDENT "." FIELD OP VALUE

SIGNAL ::= "approve" | "decline" | "review" | "hold" | "pass"

RULESET ::= "ruleset:"
              "id:" STRING
              [ "name:" STRING ]
              [ "description:" STRING ]
              "rules:" RULE_ID_LIST
              [ "conclusion:" CONCLUSION_LIST ]
              [ "metadata:" METADATA_MAP ]

CONCLUSION_LIST ::= "-" CONCLUSION_RULE { "-" CONCLUSION_RULE }

CONCLUSION_RULE ::=
      "when:" EXPRESSION
      "signal:" SIGNAL
      [ "reason:" STRING ]
    | "default:" BOOLEAN
      "signal:" SIGNAL
      [ "reason:" STRING ]

PIPELINE ::= defined in pipeline.md

METADATA_MAP ::= KEY ":" VALUE { KEY ":" VALUE }

PARAMS_MAP ::= KEY ":" VALUE { KEY ":" VALUE }
```

---

## 13. Decision Architecture

RDL uses a three-layer decision architecture:

### Layer 1: Rules (Detectors)
- Detect individual risk factors
- Produce scores
- **No actions defined**

### Layer 2: Rulesets (Decision Makers)
- Combine rule results
- Evaluate patterns and thresholds
- **Define actions through decision_logic**
- Produce final decisions

### Layer 3: Pipelines (Orchestrators)
- Orchestrate execution flow
- Manage data flow between steps
- **No decision logic** - uses ruleset decisions

**Decision Flow:**
```
Rules → Scores → Ruleset → Action → Pipeline Output
```

## 14. Compilation Model

RDL compiles into:

1. **AST (Abstract Syntax Tree)** - Intermediate representation
2. **Rust IR** - High-performance execution format
3. **Type-checked IR** - With schema validation
4. **Explainability trace** - For decision transparency
5. **Optimized bytecode** - For efficient execution

The compilation process includes:
- Syntax validation
- Type checking
- Dependency resolution
- Optimization passes
- Error detection
- Bytecode generation

---

## 15. Summary

RDL provides a modern, explainable DSL for advanced risk engines:

- Declarative rule definition with powerful expressions
- Modular (Rule → Ruleset → Pipeline)
- High‑performance and auditable
- Dynamic thresholds and adaptive rules
- Comprehensive feature engineering and statistical analysis
- Internal service integration (microservices and message queues)
- External API integration (third-party services)
- Designed for banks, fintech, e‑commerce, and Web3

This DSL is the foundation of the Cognitive Risk Intelligence Platform (CORINT).
