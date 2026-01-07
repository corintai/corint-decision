# CORINT Decision Engine

<div align="center">

**High-performance, AI-augmented risk decision engine with unified DSL**

[![License](https://img.shields.io/badge/license-Elastic-blue.svg)](LICENSE)
[![Documentation](https://img.shields.io/badge/docs-latest-green.svg)](doc/dsl/)

*Part of the **CORINT – Cognitive Risk Intelligence Framework***

[Overview](#-overview) •
[Key Features](#-key-features) •
[Quick Start](#-quick-start) •
[Comparison](#-comparison-with-alternatives) • 
[Documentation](#-documentation) 

</div>

---

## 🚀 Overview

**CORINT Decision** is a modern, real-time risk decision engine that uniquely combines:

- 🎯 **Unified DSL** - Define features, rules, and decision logic in a single, expressive YAML-based language designed for LLM comprehension, enabling AI-powered rule generation, modification, and autonomous agent integration
- 🤖 **AI-Augmented** - Native LLM integration for cognitive reasoning alongside traditional rules
- ⚡ **Real-time Performance** - Millisecond-level latency for high-throughput decision-making
- 🦀 **Minimal Resource Footprint** - Built with Rust for low memory and CPU usage, enabling cost-effective deployment at scale
- 📊 **Feature Engineering** - Built-in statistical analysis with time-window aggregations and association metrics
- 🔍 **Full Observability** - Complete audit trails, distributed tracing, and explainable decisions

### What Makes CORINT Different?

Unlike traditional rule engines (Drools, Easy Rules) that are just frameworks requiring extensive integration work, CORINT is a **production-ready, complete system** that you can deploy and use immediately—no custom development needed.

**Key Differentiators:**

- 🚀 **Ready-to-Deploy System** - Not a framework, but a complete decision engine with HTTP/gRPC APIs, database integration, and observability built-in. Deploy and start making decisions in minutes.

- 🎯 **Unified DSL for Everything** - Single declarative language for features, rules, decisions, and orchestration. No need to learn multiple DSLs or stitch together different systems.

- 🔌 **Powerful Extensibility** - Modular architecture with ruleset inheritance, parameterized rules, decision templates, and plugin system. Extend without modifying core code.

- 🤖 **LLM-Native Design** - DSL specifically designed for AI comprehension. LLMs can generate, modify, and optimize rules autonomously—perfect for AI agent workflows and automated risk management.

```yaml
# One unified DSL for everything

# Define rules that reference features
rule:
  id: high_velocity
  when:
    all:
      - transaction_velocity > 10
  score: 75

---

# Ruleset produces signals based on rules
ruleset:
  id: fraud_detection
  rules:
    - high_velocity
  conclusion:
    - when: total_score >= 100
      signal: decline
      reason: "High risk detected"

---

# Pipeline orchestrates the flow
pipeline:
  id: fraud_pipeline
  entry: fraud_check
  when:
    all:
      - event.type == "transaction"
  steps:
    - step:
        id: fraud_check
        type: ruleset
        ruleset: fraud_detection
  decision:
    - when: results.fraud_detection.signal == "decline"
      result: decline
      reason: "${results.fraud_detection.reason}"
```

---

## ✨ Key Features

### 🎯 Unified Risk Definition Language (RDL)

Define your entire risk stack in a single, declarative language:

- **Rules** - Pattern matching with complex conditions
- **Features** - Statistical aggregations and transformations
- **Pipelines** - Orchestrated decision flows
- **LLM Integration** - Cognitive reasoning as first-class citizens
- **Decision Logic** - Score-based, pattern-based, or hybrid decisions

### 📊 Advanced Feature Engineering

Built-in support for risk control analytics:

**✅ Currently Implemented:**
- **Aggregation Features**: count, sum, avg, min, max, distinct
- **Expression Features**: Compute from other features
- **Lookup Features**: Pre-computed values from feature stores

**📋 Planned Features:**
- **State Features**: z_score, outlier detection, baseline comparison
- **Sequence Features**: Pattern matching, consecutive counts, trends
- **Graph Features**: Network analysis, centrality, community detection
- **Advanced Statistics**: percentile, stddev, median, mode (SQL support exists, feature orchestration in progress)

```yaml
features:
  # ✅ Implemented: Behavioral patterns
  - name: login_count_24h
    description: "Number of login events in last 24 hours"
    type: aggregation
    method: count
    datasource: supabase_events
    entity: events
    dimension: user_id
    dimension_value: "${event.user_id}"
    window: 24h
    timestamp_field: event_timestamp
    when: event.event_type == "login"

  # ✅ Implemented: Association analysis (unique counts)
  - name: unique_devices_7d
    description: "Number of distinct devices used in last 7 days"
    type: aggregation
    method: distinct
    datasource: supabase_events
    entity: events
    dimension: user_id
    dimension_value: "${event.user_id}"
    field: device_id
    window: 7d
    timestamp_field: event_timestamp

  # ✅ Implemented: Aggregation features
  - name: transaction_sum_7d
    description: "Total transaction amount in last 7 days"
    type: aggregation
    method: sum
    datasource: supabase_events
    entity: events
    dimension: user_id
    dimension_value: "${event.user_id}"
    field: amount
    window: 7d
    timestamp_field: event_timestamp
    when: event.event_type == "transaction"

  # ✅ Implemented: Lookup features
  - name: user_risk_score
    description: "Pre-computed user risk score from feature store"
    type: lookup
    datasource: redis_features
    key: "user_features:${event.user_id}:risk_score"
    fallback: 0.0

  # 📋 Planned: Expression features for complex calculations
  # - name: amount_zscore
  #   type: expression
  #   value: (amount - avg(amounts, last_30d)) / stddev(amounts, last_30d)

  # 📋 Planned: Statistical anomaly detection
  # - name: is_outlier
  #   type: expression
  #   value: amount > percentile(amounts, last_90d, p=95)
```
 

### 🔄 Three-Layer Decision Architecture

Clean separation of concerns:

```
┌─────────────────────────────────────┐
│ Layer 1: Rules (Detectors)         │
│ - Detect risk patterns             │
│ - Produce scores                   │
│ - No actions defined               │
└─────────────────────────────────────┘
              ↓ scores
┌─────────────────────────────────────┐
│ Layer 2: Rulesets                  │
│         (Decision Suggestions)     │
│ - Evaluate rule combinations       │
│ - Analyze risk levels              │
│ - Produce signals (suggestions)    │
│   (approve/decline/review/hold)    │
└─────────────────────────────────────┘
              ↓ signals
┌─────────────────────────────────────┐
│ Layer 3: Pipeline (Orchestrator)   │
│ - Orchestrate execution flow       │
│ - Make final decisions             │
│ - Control actions and routing      │
└─────────────────────────────────────┘
```

### ⚡ High Performance

- **Low Latency**: < 10ms p99 for pure rule evaluation
- **High Throughput**: 10,000+ decisions per second per instance
- **Multi-level Caching**: Feature cache, LLM response cache, result cache
- **Parallel Execution**: Concurrent feature extraction and external API calls
- **Lazy Evaluation**: Short-circuit optimization for rules

### 🔍 Full Observability

- **Structured Logging**: With sampling and filtering
- **Metrics**: Counters and histograms (basic implementation)
- **Distributed Tracing**: Basic span and tracer support
- **Audit Trails**: Complete decision history
- **Explainability**: Rule-by-rule breakdown of decisions

### 🔄 Modular Architecture

CORINT provides modularity at two levels: system architecture and DSL composition.

#### System Architecture Modularity

Clean separation of concerns with independent, reusable crates:

```
┌─────────────────────────────────────────────────────────┐
│ corint-server (HTTP/gRPC API Server)                   │
│   ├─ REST API endpoints                                │
│   └─ gRPC service implementation                       │
└────────────────────┬────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────┐
│ corint-sdk (High-level Decision API)                   │
│   ├─ Decision engine interface                         │
│   ├─ Feature calculation                               │
│   └─ FFI bindings (C/C++/Python/Node.js)              │
└────────────────────┬────────────────────────────────────┘
                     │
        ┌────────────┼────────────┐
        │            │            │
┌───────▼──────┐ ┌──▼────────┐ ┌─▼──────────┐
│ corint-      │ │ corint-   │ │ corint-    │
│ compiler     │ │ runtime   │ │ repository │
│              │ │           │ │            │
│ - Parse DSL  │ │ - Execute │ │ - Load     │
│ - Compile IR │ │ - VM      │ │ - Save     │
└──────┬───────┘ └───────────┘ └────────────┘
       │
┌──────▼───────┐
│ corint-      │
│ parser       │
│ - Tokenize   │
│ - Parse AST  │
└──────┬───────┘
       │
┌──────▼───────┐
│ corint-core  │
│ - Types      │
│ - IR         │
└──────────────┘
```

**Multi-Language Support via FFI:**
- **Rust SDK**: Native performance with zero-cost abstractions
- **C/C++ Bindings**: Direct FFI integration for system-level applications
- **Python/Node.js**: Language bindings for web and data science workflows
- **Any Language**: Standard C FFI interface for custom integrations

#### DSL Composition Modularity

Reusable rule definitions with inheritance and import system:

```yaml
# Define reusable rules in separate files
# File: repository/library/rules/velocity_check.yaml
rule:
  id: velocity_check
  name: High Velocity Detection
  when:
    all:
      - features.transaction_count_1h > 10
  score: 75

---

# File: repository/library/rules/geo_mismatch.yaml
rule:
  id: geo_mismatch
  name: Geographic Mismatch
  when:
    all:
      - features.country_change_detected == true
  score: 60

---

# Base ruleset with core fraud checks
# File: repository/library/rulesets/fraud_detection_base.yaml
ruleset:
  id: fraud_detection_base
  name: Base Fraud Detection
  rules:
    - velocity_check      # References imported rule
    - geo_mismatch        # References imported rule
  conclusion:
    - when: total_score >= 100
      signal: decline
      reason: "High risk detected"

---

# Specialized ruleset extending base
# File: repository/library/rulesets/payment_fraud.yaml
ruleset:
  id: payment_fraud_detection
  extends: fraud_detection_base    # Inherits all rules and conclusion logic
  rules:
    - high_amount_check            # Additional payment-specific rule
```

**Key Features:**
- **Ruleset Inheritance** (`extends`) - Build hierarchies of specialized rulesets
- **Import System** - Reference rules/rulesets from separate files, automatic dependency resolution
- **Single Responsibility** - Each file defines one rule or ruleset for clarity
- **Version Control Friendly** - Modular files enable better collaboration and change tracking

### 🗄️ Flexible Storage Backend

CORINT supports multiple repository backends, enabling flexible deployment strategies from local development to distributed cloud environments:

```
┌────────────────────────────────────────┐
│      Repository Interface (Trait)      │
│   - load_rule / save_rule              │
│   - load_ruleset / save_ruleset        │
│   - load_pipeline / save_pipeline      │
└──────────────┬─────────────────────────┘
               │
       ┌───────┼────────┐
       │       │        │
┌──────▼──┐ ┌─▼──────┐ ┌▼────────────┐
│ File    │ │Database│ │Remote API   │
│ System  │ │        │ │             │
│         │ │        │ │             │
│ YAML    │ │Postgres│ │HTTP/gRPC    │
│ files   │ │        │ │endpoints    │
└─────────┘ └────────┘ └─────────────┘
```

#### 1. Filesystem Repository

**Use Case:** Local development, version control, GitOps workflows

```rust
// Load rules from local YAML files
let repo = FileSystemRepository::new("repository")?;
let (rule, _) = repo.load_rule("fraud_check").await?;
```

**Benefits:**
- **Version Control Friendly**: Track rule changes in Git
- **Human Readable**: Direct YAML editing
- **Fast Local Development**: No database required
- **GitOps Ready**: Deploy rules via CI/CD pipelines

#### 2. Database Repository

**Use Case:** Production environments requiring versioning, audit trails, and dynamic updates

```rust
// PostgreSQL with automatic versioning
let mut repo = PostgresRepository::new(database_url).await?;

// Save with auto-versioning
repo.save_rule(&rule).await?;        // Version auto-incremented
repo.save_ruleset(&ruleset).await?;  // Audit logged

// Load latest version
let (rule, version) = repo.load_rule("fraud_check").await?;
```

**Features:**
- **Automatic Versioning**: Every update creates a new version
- **Audit Trail**: Full history of rule changes with timestamps
- **Transaction Support**: ACID guarantees for rule updates
- **Multi-tenancy**: Isolate rules by organization/tenant
- **Hot Reload**: Update rules without server restart

#### 3. Remote API Repository 📋 *Planned*

**Use Case:** WASM edge-side risk control, distributed systems, microservices, centralized rule management

```rust
// Load rules from remote API (planned)
let repo = HttpRepository::new("https://rule-api.example.com")?;
let (rule, _) = repo.load_rule("fraud_check").await?;
```

**Primary Use Case - WASM Edge-Side Risk Control:**
- **Browser/Mobile WASM**: Load rules from server to run risk checks client-side
- **Edge Computing**: Deploy lightweight WASM modules that fetch rules from central API
- **Offline Capability**: Cache rules locally for offline decision-making
- **Dynamic Updates**: Update client-side rules without app redistribution

**Additional Benefits:**
- **Centralized Management**: Single source of truth for multiple services
- **Dynamic Loading**: Fetch rules on-demand from remote endpoints
- **API Gateway Integration**: Load rules via existing API infrastructure
- **Cloud-Native**: Integrate with managed rule services
- **Smart Caching**: Built-in HTTP cache to reduce API calls

**Common Features Across All Backends:**
- ✅ **Unified Interface**: Same API regardless of backend
- ✅ **Async I/O**: Non-blocking operations with Tokio
- ✅ **Caching Layer**: Built-in TTL-based caching for performance
- ✅ **Error Handling**: Basic error types and fallback support (advanced features planned)
- ✅ **Hot Reload**: Reload rules without restarting the server

### 📋 Custom Lists (Blocklists/Allowlists)

Efficient membership checks against predefined sets:

```yaml
# Check if email is in blocklist
rule:
  id: blocked_email_check
  when:
    all:
      - user.email in list.email_blocklist
  score: 500

# VIP user bypass
rule:
  id: vip_bypass
  when:
    all:
      - user.id in list.vip_users
  score: -100

# Multiple list checks
rule:
  id: sanctions_check
  when:
    any:
      - user.name in list.ofac_sanctions
      - user.country in list.high_risk_countries
  score: 200
```

**Supported Backends:**
- **PostgreSQL** - Persistent storage with metadata and expiration
- **Redis** - High-performance lookups for hot data
- **File** - Static read-only lists (country codes, domains)
- **Memory** - Small frequently-used lists
- **SQLite** - Embedded database for local deployments

**Features:**
- Simple `in list.xxx` and `not in list.xxx` syntax
- Multiple backend support for different use cases
- Automatic caching for performance
- Expiration support for temporary entries
- REST API for list management

---

## 🎯 Use Cases

### Fraud Detection
- Real-time transaction monitoring
- Account takeover detection
- Payment fraud prevention

### Identity Verification
- KYC risk assessment
- Document verification
- Behavioral biometrics

### Credit Risk
- Loan application evaluation
- Credit scoring
- Income verification

### Compliance & AML
- Transaction monitoring
- Sanctions screening
- PEP detection

---

## 🚀 Quick Start

### Running the Server

The CORINT Decision Engine includes an HTTP/REST API server for production use:

```bash
# Clone repository
git clone https://github.com/corint/corint-decision.git
cd corint-decision

# Install dependencies (Rust)
cargo build

# Run tests
cargo test

# initialize SQLite database
# if you want to use postgresql/clickhouse/redit as the datasource backend
# please install them at first, then run the corresponding script init_xxx.sh
./scripts/init_sqlite.sh

# setup the server config, you can customize it
cp config/server-example.yaml config/server.yaml

# Start the server
cargo run -p corint-server

# Health check
curl http://localhost:8080/health

# Make a decision
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "user_id": "user_001",
      "type": "transaction"
    }
  }'
```

### API Endpoints

#### REST API

**GET** `/health` - Health check endpoint

**POST** `/v1/decide` - Execute decision rules

**POST** `/v1/repo/reload` - Reload rules and configurations

**Request Example:**
```json
{
  "event": {
    "user_id": "user_001",
    "device_id": "device_001",
    "type": "transaction",
    "source": "supabase" 
  }
}
```

**Response Example:**
```json
{
  "request_id": "req_20251229060953_f4fabb",
  "status": 200,
  "process_time_ms": 1162,
  "pipeline_id": "supabase_transaction_pipeline",
  "decision": {
    "result": "REVIEW",
    "actions": [
      "KYC",
      "2FA"
    ],
    "scores": {
      "canonical": 12,
      "raw": 60
    },
    "evidence": {
      "triggered_rules": [
        "suspicious_device_pattern"
      ]
    },
    "cognition": {
      "summary": "suspicious device pattern matched",
      "reason_codes": []
    }
  }
}
```


#### gRPC API

**Service:** `corint.decision.v1.DecisionService`

**Methods:**
- `Decide(DecideRequest) → DecideResponse` - Make a decision
- `HealthCheck(HealthCheckRequest) → HealthCheckResponse` - Health check
- `ReloadRepository(ReloadRepositoryRequest) → ReloadRepositoryResponse` - Reload rules

**Example using grpcurl:**
```bash
# Health check
grpcurl -plaintext localhost:50051 corint.decision.v1.DecisionService/HealthCheck

# Make a decision
grpcurl -plaintext -d '{
  "event": {
    "user_id": {"string_value": "user_001"},
    "type": {"string_value": "transaction"},
    "amount": {"double_value": 100.0}
  }
}' localhost:50051 corint.decision.v1.DecisionService/Decide
```

For detailed gRPC documentation, see [crates/corint-server/GRPC.md](crates/corint-server/GRPC.md).

### Server Features

- ✅ **REST API**: Execute decision rules via HTTP endpoints
- ✅ **gRPC API**: High-performance Protocol Buffers-based API
- ✅ **Auto Rule Loading**: Automatically loads rules from configured directory
- ✅ **Lazy Feature Calculation**: Features are calculated on-demand from data sources during rule execution
- ✅ **Feature Caching**: Calculated feature values are cached for performance
- ✅ **Decision Result Persistence**: Automatically saves decision results to PostgreSQL for audit and analysis
- ✅ **Health Check**: Health check endpoint for monitoring
- ✅ **CORS Support**: Cross-origin resource sharing enabled
- ✅ **Request Tracing**: Built-in request/response tracing
- ✅ **Hot Reload**: Reload rules and configurations without server restart

### Configuration

The server is primarily configured via the `config/server.yaml` file, with environment variables for sensitive credentials.
 
**Note:** Feature calculation datasources (for runtime feature computation) are configured separately in `repository/configs/datasources/`.


### Logging

```bash
# Basic log levels
RUST_LOG=info cargo run -p corint-server      # Info (default)
RUST_LOG=debug cargo run -p corint-server     # Debug (detailed)
RUST_LOG=trace cargo run -p corint-server     # Trace (all details)
 
```
 
### Troubleshooting

**Server won't start:**
- Check if port is in use: `lsof -i :8080`
- Verify rules directory exists: `ls -la repository`
- View detailed logs: `RUST_LOG=debug cargo run -p corint-server`

**Rules not loading:**
- Ensure rule files have `.yaml` or `.yml` extension
- Check rule file syntax
- View server startup logs for rule loading information

**Feature calculation fails:**
- Verify the database connection
- Check data source configuration (`repository/configs/datasources/*.yaml`)
- Check features configuration (`repository/configs/features/*.yaml`)
- Verify test data exists in database

**API returns 500 error:**
- Check server logs for error stack traces
- Verify request body format is correct
- Ensure `event` object contains all required fields

---

## 📚 Documentation

### DSL Documentation

#### DSL Overview

| Document | Description |
|----------|-------------| 
| [**overvall.md**](docs/dsl/overall.md) | overall document |

#### DSL Core Concepts

| Document | Description |
|----------|-------------| 
| [**expression.md**](docs/dsl/expression.md) | Expression language reference |
| [**rule.md**](docs/dsl/rule.md) | Rule specification and patterns |
| [**ruleset.md**](docs/dsl/ruleset.md) | Ruleset and decision logic |
| [**pipeline.md**](docs/dsl/pipeline.md) | Pipeline orchestration |
| [**registry.md**](docs/dsl/registry.md) | Pipeline Registry |

#### Advanced Features

| Document | Description |
|----------|-------------|
| [**import.md**](docs/dsl/import.md) | Import rules or rulesets |
| [**context.md**](docs/dsl/context.md) | Context and variable management |
| [**feature.md**](docs/FEATURE_ENGINEERING.md) ⭐ | **Feature engineering and statistical analysis** |
| [**list.md**](docs/dsl/list.md) ⭐ | **Custom lists (blocklists/allowlists)** | 
| [**api.md**](docs/dsl/api.md) | External API defination|
| [**service.md**](docs/dsl/service.md) | Internal services defination |

### Extensible

| Document | Description |
|----------|-------------| 
| [**Architechture**](docs/ARCHITECHTURE.md) | Expression language reference |
| [**API Request**](docs/API_REQUEST.md) | Rule specification and patterns |

---



## 🔄 Comparison with Alternatives

### vs. Traditional Rule Engines (Drools, Easy Rules)

| Feature | Drools/Easy Rules | CORINT |
|---------|------------------|---------|
| Rules Definition | ✅ Yes (DRL/Java) | ✅ Yes (YAML) |
| Feature Engineering | ❌ External | ✅ Built-in |
| LLM Integration | ❌ Manual | ✅ Native |
| Statistical Functions | ❌ No | ✅ Yes (count_distinct, percentile, etc.) |
| Time-window Queries | ❌ Manual | ✅ Built-in (last_7d, last_5h, etc.) |
| Modern DSL | ❌ Java-like | ✅ Declarative YAML |

### vs. Feature Stores (Feast, Tecton)

| Feature | Feast/Tecton | CORINT |
|---------|-------------|---------|
| Feature Definition | ✅ Yes | ✅ Yes (Aggregation/Expression/Lookup) |
| Online Features | ✅ Yes | ✅ Yes (cached) |
| Feature Caching | ✅ Yes | ✅ Yes |
| Advanced Analytics | ✅ Full support | 🟡 Partial (State/Sequence/Graph planned) |
| Rules Engine | ❌ No | ✅ Built-in |
| Decision Logic | ❌ External | ✅ Integrated |
| LLM Integration | ❌ No | ✅ Native |
| Real-time Decisions | ❌ Separate service needed | ✅ End-to-end |



### vs. Cloud Services (AWS Fraud Detector, Stripe Radar)

| Feature | Cloud Services | CORINT |
|---------|---------------|---------|
| Hosted Solution | ✅ Yes | ❌ Self-hosted |
| Customization | ⚠️ Limited | ✅ Full control |
| Cost | 💰 Per-decision pricing | ✅ Open source |
| Data Privacy | ⚠️ Cloud | ✅ On-premise |
| Vendor Lock-in | ❌ Yes | ✅ Open source |
| LLM Choice | ❌ Predefined | ✅ Any provider |
---

## 🛣️ Roadmap

### Completed ✅

- ✅ Core DSL (Rules, Rulesets, Pipelines)
- ✅ Expression language with rich operators
- ✅ Feature engineering with statistical functions
- ✅ LLM integration framework
- ✅ Type system and schema validation
- ✅ Basic error handling (advanced retry strategies planned)
- ✅ Basic observability (metrics and tracing)
- ✅ Testing framework
- ✅ Performance optimization (caching, parallelization)
- ✅ Comprehensive documentation
- ✅ HTTP/REST API server (`corint-server`)
- ✅ gRPC API server
- ✅ Supabase PostgreSQL integration
- ✅ Lazy feature calculation
- ✅ Decision result persistence
- ✅ Custom lists (blocklists/allowlists/watchlists)
- ✅ Multiple list backends (PostgreSQL, Redis, File, Memory, SQLite)
- ✅ Hot reload (repository reload endpoint)
- ✅ Modular architecture with inheritance
- ✅ Flexible storage backend (File System, PostgreSQL)
- ✅ FFI bindings for C/C++ integration

### In Progress 🚧

- 🚧 Feature Store integration (Feast compatibility)
- 🚧 Visual rule editor
- 🚧 Advanced statistical features (z_score, percentile, outlier detection)

### Planned 📋

- 📋 Python/TypeScript/Go client SDKs
- 📋 Web UI for rule management
- 📋 A/B testing framework
- 📋 Machine learning model integration
- 📋 Prebuilt rule libraries for common scenarios
- 📋 Compliance templates (PCI-DSS, GDPR)
- 📋 Multi-region deployment support
- 📋 GraphQL API
- 📋 Kubernetes operator
- 📋 Advanced caching strategies (distributed cache)
- 📋 Streaming support (Kafka, Kinesis)

---
 

## 🔒 Security

- **Input Validation**: Schema-based validation for all inputs
- **Sandboxed Execution**: (Planned) Safe execution of custom expressions
- **Audit Logging**: Complete audit trail of all decisions
- **Rate Limiting**: Built-in rate limiting for external API calls
- **Secret Management**: Secure handling of API keys and credentials

---

## 🤝 Contributing

We welcome contributions! Here's how you can help:

- 🐛 **Report bugs** - Open an issue with reproduction steps
- 💡 **Suggest features** - Share your ideas in discussions
- 📝 **Improve docs** - Help make our documentation better
- 🔧 **Submit PRs** - Fix bugs or implement new features

## 📄 License

This project is licensed under the **Elastic License 2.0**.



---

<div align="center">

**Built with ❤️ for the risk control community**

If you find CORINT useful, please give us a ⭐ on GitHub!

</div>
