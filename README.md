# CORINT Decision Engine

<div align="center">

**High-performance, AI-augmented risk decision engine with unified DSL**

[![License](https://img.shields.io/badge/license-Elastic-blue.svg)](LICENSE)
[![Documentation](https://img.shields.io/badge/docs-latest-green.svg)](doc/dsl/)

*Part of the **CORINT – Cognitive Risk Intelligence Framework***

[Features](#-key-features) •
[Quick Start](#-quick-start) •
[Documentation](#-documentation) •
[Why CORINT](#-why-corint) •
[Comparison](#-comparison-with-alternatives)

</div>

---

## 🚀 Overview

**CORINT Decision** is a modern, real-time risk decision engine that uniquely combines:

- 🎯 **Unified DSL** - Define features, rules, and decision logic in a single, expressive YAML-based language
- 🤖 **AI-Augmented** - Native LLM integration for cognitive reasoning alongside traditional rules
- ⚡ **Real-time Performance** - Millisecond-level latency for high-throughput decision-making
- 📊 **Feature Engineering** - Built-in statistical analysis with time-window aggregations and association metrics
- 🔍 **Full Observability** - Complete audit trails, distributed tracing, and explainable decisions

### What Makes CORINT Different?

Unlike traditional rule engines or feature stores that require stitching together multiple systems, CORINT provides an **end-to-end solution** for modern risk control with a single, coherent DSL.

```yaml
# One unified DSL for everything
pipeline:
  # Feature engineering
  - type: extract
    features:
      - name: devices_per_ip_5h
        value: count_distinct(device.id, {ip == event.ip}, last_5h)
  
  # LLM reasoning
  - type: reason
    provider: openai
    model: gpt-4-turbo
    prompt: "Analyze this login pattern..."
  
  # Rules evaluation
  - include:
      ruleset: fraud_detection
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

```yaml
features:
  # Behavioral patterns
  - login_count_7d: count(user.logins, last_7d)
  
  # Association analysis (unique counts)
  - devices_per_ip_5h: count_distinct(device.id, {ip == event.ip}, last_5h)
  - users_per_device_24h: count_distinct(user.id, {device.id == event.device.id}, last_24h)
  
  # Statistical anomaly detection
  - amount_zscore: (amount - avg(amounts, last_30d)) / stddev(amounts, last_30d)
  - is_outlier: amount > percentile(amounts, last_90d, p=95)
  
  # Velocity tracking
  - login_velocity: count(logins, last_24h) / count(logins, last_7d) * 7
```

### 🤖 Native LLM Integration

Seamlessly combine deterministic rules with AI reasoning:

- Multiple LLM providers (OpenAI, Anthropic, custom)
- Structured output schemas
- Automatic caching and cost optimization
- Fallback strategies
- Integration with rule conditions

```yaml
- type: reason
  id: behavior_analysis
  provider: openai
  model: gpt-4-turbo
  prompt:
    template: |
      Analyze login pattern for user {user.id}:
      - Current device: {device.type} from {geo.country}
      - Failed attempts: {failed_count}
      - Time: {timestamp}
  output_schema:
    risk_level: string
    confidence: float
    reason: string

# Use LLM output in rules
rule:
  conditions:
    - context.behavior_analysis.risk_level == "high"
    - context.behavior_analysis.confidence > 0.8
  score: 90
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
│ Layer 2: Rulesets (Decision Makers)│
│ - Evaluate rule combinations       │
│ - Define decision logic            │
│ - Produce actions (approve/deny)   │
└─────────────────────────────────────┘
              ↓ action
┌─────────────────────────────────────┐
│ Layer 3: Pipeline (Orchestrator)   │
│ - Orchestrate execution flow       │
│ - Manage data flow                 │
│ - Control parallelism              │
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
- **Metrics**: Counters, gauges, histograms (Prometheus compatible)
- **Distributed Tracing**: OpenTelemetry support
- **Audit Trails**: Complete decision history
- **Explainability**: Rule-by-rule breakdown of decisions

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
# Run the server locally
cargo run -p corint-server

# Server will start on http://127.0.0.1:8080
```

For detailed server documentation, see [Server Quick Start Guide](crates/corint-server/QUICKSTART.md).

### Simple Login Risk Detection Example

```yaml
version: "0.1"

# Extract features
pipeline:
  - type: extract
    id: features
    features:
      - name: failed_logins_1h
        value: count(user.failed_logins, last_1h)
      
      - name: devices_per_ip_5h
        value: count_distinct(device.id, {ip == event.ip}, last_5h)
  
  # Evaluate rules
  - include:
      ruleset: login_risk

---

# Define rules
rule:
  id: too_many_failures
  name: Too Many Failed Login Attempts
  when:
    event.type: login
    conditions:
      - context.features.failed_logins_1h > 5
  score: 60

---

rule:
  id: suspicious_ip
  name: Suspicious IP Association
  when:
    event.type: login
    conditions:
      - context.features.devices_per_ip_5h > 10
  score: 80

---

# Decision logic
ruleset:
  id: login_risk
  rules:
    - too_many_failures
    - suspicious_ip
  
  decision_logic:
    - condition: total_score >= 100
      action: deny
      reason: "High risk score"
    
    - condition: total_score >= 60
      action: review
      reason: "Medium risk"
    
    - default: true
      action: approve
```

### More Examples

See the [`examples/`](doc/dsl/examples/) directory for complete, real-world examples:

- [Account Takeover Detection](doc/dsl/examples/account-takeover-complete.yml) - Comprehensive takeover prevention
- [Statistical Analysis](doc/dsl/examples/statistical-analysis.yml) - Advanced feature engineering
- [Intelligent Inference](doc/dsl/examples/intelligent-inference.yml) - LLM-powered decisions
- [Loan Application](doc/dsl/examples/loan.yml) - Credit risk assessment

---

## 🌐 Server API

CORINT Decision Engine includes a production-ready HTTP/REST API server (`corint-server`).

### Quick Start

```bash
# Start the server
cargo run -p corint-server

# Health check
curl http://localhost:8080/health

# Make a decision
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
      "user_id": "user_001",
      "event.type": "transaction"
    }
  }'
```

### API Endpoints

#### Health Check

**GET** `/health`

Returns server health status.

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

#### Make Decision

**POST** `/v1/decide`

Execute decision rules with event data.

**Request:**
```json
{
  "event_data": {
    "user_id": "user_001",
    "device_id": "device_001",
    "event.type": "transaction",
    "event.user_id": "user_001"
  }
}
```

**Response:**
```json
{
  "action": "Approve",
  "score": 35,
  "triggered_rules": ["high_value_transaction"],
  "explanation": "Transaction approved with moderate risk",
  "processing_time_ms": 125
}
```

### Server Features

- ✅ **REST API**: Execute decision rules via HTTP endpoints
- ✅ **Auto Rule Loading**: Automatically loads rules from configured directory
- ✅ **Lazy Feature Calculation**: Features are calculated on-demand from data sources during rule execution
- ✅ **Feature Caching**: Calculated feature values are cached for performance
- ✅ **Health Check**: Health check endpoint for monitoring
- ✅ **CORS Support**: Cross-origin resource sharing enabled
- ✅ **Request Tracing**: Built-in request/response tracing

### Configuration

The server can be configured via:

1. **Environment Variables** (prefix: `CORINT_`)
2. **Config File** (`config/server.yaml`)
3. **Default Values**

**Environment Variables:**
```bash
CORINT_HOST=127.0.0.1
CORINT_PORT=8080
CORINT_RULES_DIR=examples/rules
CORINT_ENABLE_METRICS=true
CORINT_ENABLE_TRACING=true
CORINT_LOG_LEVEL=info
```

**Config File Example** (`config/server.yaml`):
```yaml
host: "127.0.0.1"
port: 8080
rules_dir: "examples/rules"
enable_metrics: true
enable_tracing: true
log_level: "info"
```

### Integration with Supabase

The server supports on-demand feature calculation from Supabase PostgreSQL:

1. **Prerequisites:**
   - Supabase database configured with test data (see `docs/schema/postgres-examples.sql`)
   - Data source configured (`examples/configs/datasources/supabase_events.yaml`)
   - Rules reference features (e.g., `examples/rules/supabase_feature_ruleset.yaml`)

2. **Workflow:**
   - Start the server: `cargo run -p corint-server`
   - Send decision requests via `/v1/decide`
   - When rules reference features (e.g., `transaction_sum_7d > 5000`), the server automatically queries Supabase to calculate features
   - Feature values are cached for subsequent rule evaluations
   - Decision result is returned

3. **Feature Calculation Logs:**
```
INFO corint_runtime::datasource::client: Executing PostgreSQL query: 
  SELECT SUM((attributes->>'amount')::numeric) AS sum 
  FROM events 
  WHERE user_id = 'user_003' AND event_type = 'transaction' 
  AND event_timestamp >= NOW() - INTERVAL '604800 seconds'

DEBUG corint_runtime::engine::pipeline_executor: 
  Feature 'transaction_sum_7d' calculated: Number(8000.0)
```

### Performance Optimization

- **Lazy Feature Calculation**: Features are only calculated when referenced by rules, avoiding unnecessary database queries
- **Feature Caching**: Calculated feature values are cached in `event_data` for subsequent rule accesses within the same request
- **Async I/O**: All database queries and external API calls are non-blocking, supporting high concurrency
- **Connection Pooling**: Database connections are managed via connection pools to reduce connection overhead

### Logging

```bash
# Basic log levels
RUST_LOG=info cargo run -p corint-server      # Info (default)
RUST_LOG=debug cargo run -p corint-server     # Debug (detailed)
RUST_LOG=trace cargo run -p corint-server     # Trace (all details)

# Module-level logging
RUST_LOG=info,corint_server=debug cargo run -p corint-server
RUST_LOG=corint_runtime::datasource=debug cargo run -p corint-server  # SQL queries
RUST_LOG=corint_runtime::feature=debug cargo run -p corint-server    # Feature calculation
```

### Production Deployment

For detailed production deployment instructions, including:
- Step-by-step deployment guide
- Systemd service configuration
- Nginx reverse proxy setup
- HTTPS configuration
- Monitoring and logging
- Security recommendations
- Troubleshooting

See [Server Quick Start Guide](crates/corint-server/QUICKSTART.md).

### Troubleshooting

**Server won't start:**
- Check if port is in use: `lsof -i :8080`
- Verify rules directory exists: `ls -la examples/rules`
- View detailed logs: `RUST_LOG=debug cargo run -p corint-server`

**Rules not loading:**
- Ensure rule files have `.yaml` or `.yml` extension
- Check rule file syntax
- View server startup logs for rule loading information

**Feature calculation fails:**
- Verify Supabase database connection
- Check data source configuration (`examples/configs/datasources/supabase_events.yaml`)
- View SQL queries and errors: `RUST_LOG=corint_runtime::datasource=debug cargo run -p corint-server`
- Verify test data exists in database

**API returns 500 error:**
- Check server logs for error stack traces
- Verify request body format is correct
- Ensure `event_data` contains all required fields

---

## 📚 Documentation

### Core Concepts

| Document | Description |
|----------|-------------|
| [**overall.md**](doc/dsl/overall.md) | High-level overview and architecture |
| [**ARCHITECTURE.md**](doc/dsl/ARCHITECTURE.md) | Three-layer decision architecture |
| [**rule.md**](doc/dsl/rule.md) | Rule specification and patterns |
| [**ruleset.md**](doc/dsl/ruleset.md) | Ruleset and decision logic |
| [**pipeline.md**](doc/dsl/pipeline.md) | Pipeline orchestration |

### Advanced Features

| Document | Description |
|----------|-------------|
| [**expression.md**](doc/dsl/expression.md) | Expression language reference |
| [**feature.md**](doc/dsl/feature.md) ⭐ | **Feature engineering and statistical analysis** |
| [**context.md**](doc/dsl/context.md) | Context and variable management |
| [**llm.md**](doc/dsl/llm.md) | LLM integration guide |
| [**schema.md**](doc/dsl/schema.md) | Type system and data schemas |

### Operations & Best Practices

| Document | Description |
|----------|-------------|
| [**error-handling.md**](doc/dsl/error-handling.md) | Error handling strategies |
| [**observability.md**](doc/dsl/observability.md) | Monitoring, logging, and tracing |
| [**test.md**](doc/dsl/test.md) | Testing framework and patterns |
| [**performance.md**](doc/dsl/performance.md) | Performance optimization guide |

### Quick References

- **Feature Engineering**: For statistical analysis like "login count in the past 7 days" or "number of device IDs associated with the same IP", see [**feature.md**](doc/dsl/feature.md)
- **LLM Integration**: For adding AI reasoning to your rules, see [**llm.md**](doc/dsl/llm.md)
- **Testing Your Rules**: See [**test.md**](doc/dsl/test.md) for comprehensive testing strategies

---

## 💡 Why CORINT?

### The Problem with Traditional Approaches

Building a modern risk control system typically requires stitching together multiple tools:

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│   Feast     │ →   │   Drools     │ →   │ LangChain   │
│ (Features)  │     │ (Rules)      │     │ (LLM)       │
└─────────────┘     └──────────────┘     └─────────────┘
       ↓                    ↓                    ↓
  Different DSL      Different DSL       Different API
  
  Result: Complex integration, maintenance burden, poor observability
```

### The CORINT Approach

One unified DSL for everything:

```yaml
# Everything in one place, one language
version: "0.1"

pipeline:
  # Features
  - type: extract
    features: [...]
  
  # LLM reasoning
  - type: reason
    model: gpt-4-turbo
    prompt: "..."
  
  # Rules
  - include:
      ruleset: fraud_detection
  
  # Decision
  decision_logic:
    - condition: total_score >= 100
      action: deny
```

### Key Advantages

| Aspect | Traditional Stack | CORINT |
|--------|------------------|---------|
| **Languages** | Multiple (SQL, Drools DRL, Python, etc.) | Single YAML-based DSL |
| **LLM Integration** | Manual API calls, custom code | Native, first-class support |
| **Feature Store** | Separate system (Feast, Tecton) | Built-in with time-window aggregations |
| **Rules Engine** | Separate (Drools, Easy Rules) | Integrated with features and LLM |
| **Observability** | Fragmented across systems | Unified tracing and logging |
| **Testing** | Multiple frameworks | Single testing framework |
| **Deployment** | Multiple services to coordinate | Single decision engine |
| **Learning Curve** | Steep (multiple systems) | Gentle (one DSL) |

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
| Feature Definition | ✅ Yes | ✅ Yes |
| Online Features | ✅ Yes | ✅ Yes (cached) |
| Feature Caching | ✅ Yes | ✅ Yes |
| Rules Engine | ❌ No | ✅ Built-in |
| Decision Logic | ❌ External | ✅ Integrated |
| LLM Integration | ❌ No | ✅ Native |
| Real-time Decisions | ❌ Separate service needed | ✅ End-to-end |

### vs. LLM Orchestration (LangChain, Semantic Kernel)

| Feature | LangChain | CORINT |
|---------|-----------|---------|
| LLM Integration | ✅ Excellent | ✅ Native |
| Prompt Engineering | ✅ Yes | ✅ Yes with templates |
| Rules Engine | ❌ No | ✅ Built-in |
| Feature Engineering | ❌ No | ✅ Built-in |
| Decision Logic | ❌ Code-based | ✅ Declarative DSL |
| Production-ready | ⚠️ Requires work | ✅ Yes (caching, monitoring) |

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

## 🏗️ Architecture

### System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      CORINT Decision API                     │
│                    (REST / gRPC / SDK)                       │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    Request Processing                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │ Validate │→ │ Feature  │→ │ Rules    │→ │ Decision │  │
│  │  Input   │  │ Extract  │  │ Engine   │  │  Logic   │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    Integration Layer                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │ Feature  │  │   LLM    │  │ External │  │  Cache   │  │
│  │  Store   │  │ Provider │  │   APIs   │  │  Layer   │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                   Observability Stack                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │  Logs    │  │ Metrics  │  │  Traces  │  │  Audit   │  │
│  │(Loki etc)│  │(Prom etc)│  │(Jaeger)  │  │  Trail   │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Server Architecture

The HTTP server (`corint-server`) architecture:

```
┌─────────────────────────────────────────┐
│         HTTP Request                     │
│  POST /v1/decide                         │
│  { "event_data": {...} }                │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│    Axum Router + Middleware             │
│    - CORS Layer                         │
│    - Trace Layer                        │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│       REST API Handler                  │
│    - Parse JSON                         │
│    - Convert to DecisionRequest         │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│      Decision Engine (SDK)              │
│    - Load compiled rules                │
│    - Execute ruleset                    │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│    Pipeline Executor (Runtime)          │
│    - Execute IR instructions            │
│    - LoadField: check event_data        │
└─────────────────┬───────────────────────┘
                  │
                  ▼ (field not found)
┌─────────────────────────────────────────┐
│      Feature Executor                   │
│    - Check if registered feature        │
│    - Build SQL query                    │
│    - Query Supabase                     │
│    - Calculate feature value            │
│    - Cache in event_data                │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│         HTTP Response                   │
│  { "action": "Approve",                 │
│    "score": 35,                         │
│    "triggered_rules": [...],            │
│    "processing_time_ms": 125 }          │
└─────────────────────────────────────────┘
```

### Execution Flow

```
Event → Pipeline → Extract Features → Evaluate Rules → Decision Logic → Response
           ↓              ↓                  ↓                ↓
        Validate      [Cache Check]     [Parallel]      [Action]
           ↓              ↓                  ↓                ↓
        Context       Aggregations       Scoring         approve/deny/
                      count_distinct                     review/infer
                      percentile
                      velocity
```

---

## 🛣️ Roadmap

### Completed ✅

- ✅ Core DSL (Rules, Rulesets, Pipelines)
- ✅ Expression language with rich operators
- ✅ Feature engineering with statistical functions
- ✅ LLM integration framework
- ✅ Type system and schema validation
- ✅ Error handling and retry strategies
- ✅ Observability infrastructure
- ✅ Testing framework
- ✅ Performance optimization (caching, parallelization)
- ✅ Comprehensive documentation

### In Progress 🚧

- 🚧 Rust runtime implementation
- 🚧 gRPC API
- 🚧 Feature Store integration (Feast compatibility)
- 🚧 Visual rule editor
- 🚧 Real-time rule updates (hot reload)
- ✅ HTTP/REST API server (`corint-server`)
- ✅ Supabase PostgreSQL integration
- ✅ Lazy feature calculation

### Planned 📋

- 📋 Python/TypeScript SDKs
- 📋 Web UI for rule management
- 📋 A/B testing framework
- 📋 Machine learning model integration
- 📋 Prebuilt rule libraries for common scenarios
- 📋 Compliance templates (PCI-DSS, GDPR)
- 📋 Multi-region deployment support
- 📋 GraphQL API

---

## 🧪 Testing

CORINT includes a comprehensive testing framework:

```yaml
# Rule unit test
test:
  rule: high_risk_transaction
  cases:
    - name: "High amount triggers rule"
      input:
        event:
          type: transaction
          amount: 15000
      expect:
        triggered: true
        score: 80
    
    - name: "Low amount does not trigger"
      input:
        event:
          amount: 100
      expect:
        triggered: false
```

See [test.md](doc/dsl/test.md) for complete testing guide.

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

### Development Setup

```bash
# Clone repository
git clone https://github.com/corint/corint-decision.git
cd corint-decision

# Install dependencies (Rust)
cargo build

# Run tests
cargo test

# Build documentation
cargo doc --no-deps --open
```

---

## 📄 License

This project is licensed under the **Elastic License 2.0**.

---

## 🙏 Acknowledgments

Inspired by and learning from:

- **Feast** - Feature Store architecture
- **Drools** - Rule engine design patterns
- **LangChain** - LLM orchestration patterns
- **OpenTelemetry** - Observability standards

---

## 📞 Contact & Community

- **Documentation**: [doc/dsl/](doc/dsl/)
- **Issues**: [GitHub Issues](https://github.com/corint/corint-decision/issues)
- **Discussions**: [GitHub Discussions](https://github.com/corint/corint-decision/discussions)

---

<div align="center">

**Built with ❤️ for the risk control community**

If you find CORINT useful, please give us a ⭐ on GitHub!

</div>
