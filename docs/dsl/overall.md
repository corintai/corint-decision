# CORINT Risk Definition Language (RDL)
## Overall Specification (v0.1)

**RDL is the domain-specific language used by CORINT (Cognitive Risk Intelligence) to define rules, rule groups, reasoning logic, and full risk‑processing pipelines.**
It enables modern hybrid risk engines to combine deterministic logic with external data sources and APIs in a unified, explainable, high‑performance format.

---

## 1. Goals of RDL

RDL is designed to:

- Provide a declarative, human-readable format for risk logic
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

**Note:** Rules do not define actions. Actions are defined in Ruleset's `decision_logic`.

---

### 3.1.1 `when` Block

#### Event Filter

**Note**: The old event filter syntax (`event.type: value`) is deprecated. Use condition syntax instead.

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
- external_api.Chainalysis.risk_score > 80
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
# Distinct Count - Association analysis
count_distinct(device.id, {geo.ip == event.geo.ip}, last_5h) > 10

# Basic aggregations
sum(transaction.amount, last_24h)
avg(transaction.amount, last_7d)
```

**📋 Coming Soon:**
```yaml
# Statistical Functions - In development
percentile(amounts, last_90d, p=95)
stddev(amounts, last_30d)
z_score(current_amount, amounts, last_90d)
```

**Feature Access**: All calculated features must be accessed using the `features.` namespace prefix:
  ```yaml
  when:
    all:
    - features.transaction_sum_7d > 5000
    - features.login_count_24h > 10
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

## 5. Data Types and Schema

RDL includes a comprehensive type system for data validation and safety.

Features:
- Primitive types (string, number, boolean, datetime)
- Composite types (arrays, objects, maps)
- Custom type definitions
- Schema validation
- Format validators

(See `schema.md` for full specification.)

---

## 6. Context and Variable Management

Context management enables data flow between pipeline steps.

Context layers:
- **event** - Input event data (read-only)
- **vars** - Pipeline variables (read-only)
- **context** - Step outputs (read-write)
- **sys** - System variables (read-only)
- **env** - Environment configuration (read-only)

(See `context.md` for complete details.)

---

## 7. LLM Code Generation (Development-Time Only)

**⚠️ IMPORTANT**: LLM is **NOT** a runtime component. It is a development-time code generation tool.

The `corint-llm` crate provides LLM-powered YAML configuration generation during development:

**Development-Time Capabilities**:
- Generate rules from natural language descriptions
- Generate rulesets with decision logic
- Generate complete pipelines
- Generate API configurations
- Multiple LLM providers (OpenAI, Anthropic, Gemini, DeepSeek)

**Key Points**:
- ✅ LLM generates YAML configurations offline
- ✅ Developers review and commit generated YAML
- ✅ CORINT runtime executes compiled YAML with zero LLM calls
- ❌ NO LLM calls at runtime
- ❌ NO performance impact on production decisions

(See [LLM_GUIDE.md](../LLM_GUIDE.md) for complete usage guide.)

---

## 8. Error Handling

Production-grade error handling ensures reliability and graceful degradation.

Current implementation:
- **Basic error types** - RuntimeError and ServerError definitions
- **Simple fallback** - Fallback values for external API calls (implemented at runtime level)
- **Timeout support** - Timeout configuration for API calls and datasources
- **Error action types** - ErrorAction enum (Fallback, Skip, Fail, Retry) defined in AST

Note: Advanced error handling features (retry logic, circuit breaker, fallback chains) are planned but not yet implemented.

---

## 9. Observability

Comprehensive observability for monitoring and debugging.

Features:
- Metrics collection (Counter, Histogram) - implemented at runtime level
- Distributed tracing (Span, Tracer) - implemented at runtime level
- Health check endpoint - `/health` endpoint available

Note: Observability features are implemented at the runtime level rather than through DSL configuration.

---

## 10. Testing

Robust testing framework for validation and quality assurance.

Test types:
- Unit tests (individual rules)
- Integration tests (rulesets)
- Pipeline tests (end-to-end)
- Regression tests (historical cases)
- Performance tests (load and stress)

---

## 11. Performance Optimization

High-performance execution for real-time decisioning.

Optimizations:
- Multi-level caching (feature-level caching implemented)
- Parallelization (feature dependencies executed in parallel)
- Connection pooling (at datasource level)

Note: Advanced performance optimizations are implemented at the runtime level rather than through DSL configuration.

---

## 12. Internal Service Integration

RDL provides comprehensive integration with internal services for data access and computation.

Service types:
- **Database services** - MySQL, PostgreSQL, MongoDB, Cassandra
- **Cache services** - Redis, Memcached
- **Feature Store** - Pre-computed features
- **Microservices** - Internal RESTful/gRPC services
- **Message queues** - Kafka, RabbitMQ
- **Search services** - Elasticsearch

Example:
```yaml
pipeline:
  # Database query
  - type: service
    id: load_user
    service: user_db
    query: get_user_profile
    output: service.user_profile

  # Cache lookup
  - type: service
    id: check_cache
    service: redis_cache
    operation: get_user_risk_cache
```

(See `service.md` for complete specification.)

---

## 13. External API Integration

```yaml
external_api.<provider>.<field>
```

Example:

```yaml
external_api.Chainalysis.risk_score > 80
```

(See `external.md` for complete specification.)

---

## 14. Documentation Structure

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

### Data & Schema
- **event.md** - Standard event types and schemas
- **schema.md** - Type system and data schemas
- **context.md** - Context and variable management

### Advanced Features
- **feature.md** - Feature engineering and statistical analysis
- **list.md** - Custom List feature (blocklists, allowlists, multi-backend support)
- **service.md** - Internal service integration (databases, caches, microservices)
- **external.md** - External API integration (third-party services)

### Development Tools
- **[LLM_GUIDE.md](../LLM_GUIDE.md)** - LLM code generation guide (development-time only)

### Operational
- (Error handling implemented at runtime level)

### Examples
- **examples/** - Real-world pipeline examples
- **repository/** - Production rule library and reusable components

---

## 15. Examples

### 14.1 Login Risk Example

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

### 14.2 High Value Transaction

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

## 16. BNF Grammar (Formal)

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

## 17. Decision Architecture

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

## 18. Compilation Model

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

## 19. Summary

RDL provides a modern, explainable DSL for advanced risk engines:

- Declarative rule definition with powerful expressions
- Modular (Rule → Ruleset → Pipeline)
- High‑performance and auditable
- Dynamic thresholds and adaptive rules
- Comprehensive feature engineering and statistical analysis
- Complete internal service integration (databases, caches, microservices)
- Comprehensive external API integration
- LLM-powered development tools (code generation)
- Designed for banks, fintech, e‑commerce, and Web3

This DSL is the foundation of the Cognitive Risk Intelligence Platform (CORINT).
