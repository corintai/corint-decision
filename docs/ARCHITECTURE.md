# CORINT Decision Engine Architecture

## Overview

CORINT is a high-performance, flexible decision engine designed for real-time risk assessment and fraud detection. It provides a complete framework for defining, compiling, and executing complex decision logic through a multi-layered architecture.

## Design Philosophy

### Core Principles

1. **Separation of Concerns**: Clear boundaries between detection (Rules), decision-making (Rulesets), and orchestration (Pipelines)
2. **Unified Integration**: Single SDK entry point for all integration scenarios (HTTP Server, WASM, FFI)
3. **Configuration as Code**: Business logic defined in declarative YAML, separate from application configuration
4. **Performance First**: Compile-time optimization, JIT compilation, and runtime efficiency
5. **Extensibility**: Plugin architecture for external services, features, and data sources

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Integration Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ HTTP Server  │  │     WASM     │  │ FFI Bindings │          │
│  │   (Axum)     │  │ (Browser/API)│  │ (Python/Go)  │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                  │                  │                  │
│         │    config/server.yaml               │                  │
│         │   (Application Config)              │                  │
│         └──────────────────┼──────────────────┘                  │
│                            │ Depends on corint-sdk               │
└────────────────────────────┼─────────────────────────────────────┘
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                      corint-sdk (Unified Entry Point)           │
│                                                                  │
│  DecisionEngineBuilder::new()                                   │
│      .with_repository(RepositoryConfig::file_system("repo"))    │
│      .with_feature_executor(executor)                           │
│      .with_list_service(list_service)                           │
│      .enable_metrics(true)                                      │
│      .build()                                                   │
│                                                                  │
│  → DecisionEngine::decide(request) → DecisionResult            │
└────────────────────────────┬────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        ▼                    ▼                    ▼
┌──────────────┐  ┌──────────────────┐  ┌─────────────────┐
│  corint-     │  │  corint-         │  │  corint-        │
│  repository  │  │  compiler        │  │  runtime        │
│              │  │                  │  │                 │
│ Load all     │  │ Parse → AST      │  │ Execute VM      │
│ business     │  │ Analyze → Opts   │  │ Features        │
│ configs:     │  │ Codegen → Bytecode│  │ Lists          │
│ - Pipelines  │  │                  │  │ Data Sources   │
│ - Rules      │  │                  │  │ Persistence    │
│ - Features   │  │                  │  │ Observability  │
│ - Lists      │  │                  │  │                 │
│ - APIs       │  │                  │  │                 │
└──────┬───────┘  └─────────┬────────┘  └────────┬────────┘
       │                    │                     │
       └────────────────────┼─────────────────────┘
                            │ All depend on
                            ▼
       ┌─────────────────────────────────────────────────┐
       │           corint-core (Foundation)              │
       │                                                  │
       │  Shared Types & Abstractions:                   │
       │  - AST (Abstract Syntax Tree)                   │
       │  - Value System (runtime data)                  │
       │  - Bytecode (VM instructions)                   │
       │  - Error Types (standardized errors)            │
       │  - Common Utilities                             │
       └─────────────────────────────────────────────────┘
```

---

## Component Architecture

### 1. Integration Layer

**Responsibility**: Expose decision engine to different runtime environments

**Components**:
- **HTTP Server** (`corint-server`): REST API for decision execution
- **WASM** (`corint-wasm`): Browser/edge runtime support
- **FFI** (`corint-ffi`): Language bindings (Python, Go, etc.)

**Key Characteristics**:
- Only load application configuration (host, port, logging, etc.)
- Depend solely on `corint-sdk` for decision engine functionality
- Lightweight, focused on protocol/transport concerns

**Example (HTTP Server)**:
```rust
// config/server.yaml - Application configuration only
let config = ServerConfig::load()?;

// Use SDK for all business logic
let engine = DecisionEngineBuilder::new()
    .with_repository(RepositoryConfig::file_system("repository"))
    .enable_metrics(config.enable_metrics)
    .build()
    .await?;

// Execute decisions
let result = engine.decide(request).await?;
```

---

### 2. SDK Layer (`corint-sdk`)

**Responsibility**: Unified entry point for all integrations

**Core API**:
```rust
pub struct DecisionEngineBuilder {
    repository_config: Option<RepositoryConfig>,
    feature_executor: Option<Arc<FeatureExecutor>>,
    list_service: Option<Arc<ListService>>,
    result_writer: Option<Arc<DecisionResultWriter>>,
    // ... configuration options
}

impl DecisionEngineBuilder {
    pub fn with_repository(self, config: RepositoryConfig) -> Self;
    pub fn with_feature_executor(self, executor: Arc<FeatureExecutor>) -> Self;
    pub fn with_list_service(self, service: Arc<ListService>) -> Self;
    pub async fn build(self) -> Result<DecisionEngine>;
}

pub struct DecisionEngine {
    pub async fn decide(&self, request: DecisionRequest) -> Result<DecisionResult>;
    pub async fn decide_batch(&self, requests: Vec<DecisionRequest>) -> Result<Vec<DecisionResult>>;
}
```

**Key Features**:
- Builder pattern for flexible configuration
- Automatic repository content loading
- Integration with runtime services (features, lists, persistence)
- Metrics and tracing support

**Re-exports**:
- `RepositoryConfig`, `RepositorySource` from `corint-repository`
- `DecisionEngine`, `DecisionRequest`, `DecisionResult`
- Configuration types for external consumption

---

### 3. Repository Layer (`corint-repository`)

**Responsibility**: Load all business configuration from various sources

**Configuration Sources**:
```rust
pub enum RepositorySource {
    FileSystem,  // Load from local file system
    Database,    // Load from PostgreSQL
    Api,         // Load from HTTP API
    Memory,      // In-memory (for testing/WASM)
}

pub struct RepositoryConfig {
    pub source: RepositorySource,
    pub base_path: Option<String>,        // for FileSystem
    pub database_url: Option<String>,     // for Database
    pub api_url: Option<String>,          // for Api
    pub api_key: Option<String>,
}
```

**Loaded Content**:
```rust
pub struct RepositoryContent {
    // Decision logic
    pub registry: Option<String>,           // Pipeline routing
    pub pipelines: Vec<(String, String)>,   // Entry points
    pub rules: Vec<(String, String)>,       // Detection rules
    pub rulesets: Vec<(String, String)>,    // Decision rulesets
    pub templates: Vec<(String, String)>,   // Reusable templates

    // Runtime configuration
    pub api_configs: Vec<ApiConfig>,              // External APIs
    pub datasource_configs: Vec<DataSourceConfig>, // Data sources
    pub feature_definitions: Vec<FeatureDefinition>, // Features
    pub list_configs: Vec<ListConfig>,            // Blocklists/allowlists
}
```

**Repository Structure**:
```
repository/
├── registry.yaml                    # Pipeline routing configuration
├── pipelines/                       # Pipeline definitions (entry points)
│   ├── fraud_detection.yaml
│   ├── payment_pipeline.yaml
│   └── login_risk_pipeline.yaml
├── library/
│   ├── rules/                       # Rule definitions
│   │   ├── fraud/
│   │   ├── compliance/
│   │   └── common/
│   ├── rulesets/                    # Ruleset definitions
│   │   ├── fraud/
│   │   └── kyc/
│   └── templates/                   # Reusable templates
│       └── common/
└── configs/
    ├── apis/                        # External API configs
    │   └── ipinfo.yaml
    ├── datasources/                 # Data source configs
    │   ├── postgres_events.yaml
    │   └── redis_feature_store.yaml
    ├── features/                    # Feature definitions
    │   ├── user_features.yaml
    │   └── transaction_features.yaml
    └── lists/                       # List configurations
        ├── ip_blocklist.yaml
        └── high_risk_countries.yaml
```

**Loading Flow**:
```rust
let loader = RepositoryLoader::new(config);
let content = loader.load_all().await?;
// Returns RepositoryContent with all artifacts loaded
```

---

### 4. Compiler Layer (`corint-compiler`)

**Responsibility**: Transform YAML definitions into optimized bytecode

**Pipeline**:
```
YAML Input
    ↓
┌─────────────────┐
│ Parser          │  Parse YAML → AST
│ (corint-parser) │  - Pipelines, Rules, Rulesets
└────────┬────────┘
         ↓
┌─────────────────┐
│ Semantic        │  Analyze AST
│ Analysis        │  - Type checking
│                 │  - Variable resolution
│                 │  - Dependency analysis
└────────┬────────┘
         ↓
┌─────────────────┐
│ Optimization    │  Optimize AST
│                 │  - Constant folding
│                 │  - Dead code elimination
│                 │  - Expression simplification
└────────┬────────┘
         ↓
┌─────────────────┐
│ Code Generation │  Generate bytecode
│                 │  - Pipeline → VM instructions
│                 │  - Rules → Condition evaluation
│                 │  - Expressions → Stack operations
└────────┬────────┘
         ↓
    Bytecode (VM Instructions)
```

**Key Components**:
- **Parser** (`corint-parser`): YAML → AST transformation
- **Semantic Analyzer**: Type checking, validation
- **Optimizer**: Constant folding, dead code elimination
- **Code Generator**: AST → Bytecode compilation

**Optimizations**:
- Compile-time constant evaluation
- Expression tree simplification
- Dead code elimination
- Type inference and checking

---

### 5. Runtime Layer (`corint-runtime`)

**Responsibility**: Execute compiled bytecode and provide runtime services

#### 5.1 Virtual Machine (VM)

**Execution Model**:
```rust
pub struct VirtualMachine {
    // Execution context
    context: ExecutionContext,

    // Compiled bytecode
    instructions: Vec<Instruction>,

    // Execution state
    program_counter: usize,
    stack: Vec<Value>,
}

pub enum Instruction {
    // Value operations
    Push(Value),
    Pop,
    Load(String),        // Load variable
    Store(String),       // Store variable

    // Arithmetic
    Add, Sub, Mul, Div, Mod,

    // Comparison
    Eq, Ne, Lt, Le, Gt, Ge,

    // Logical
    And, Or, Not,

    // Control flow
    Jump(usize),
    JumpIfFalse(usize),
    Call(String),
    Return,

    // Rule evaluation
    EvalRule(String),
    EvalRuleset(String),

    // Feature operations
    GetFeature(String),

    // List operations
    ListLookup(String, Value),
}
```

**Execution Flow**:
```
1. Load bytecode into VM
2. Initialize execution context with input data
3. Execute instructions sequentially
4. Handle control flow (jumps, calls)
5. Evaluate rules and rulesets
6. Collect results and produce decision
```

#### 5.2 Feature System

**Architecture**:
```rust
pub struct FeatureExecutor {
    registry: FeatureRegistry,        // Feature definitions
    datasources: HashMap<String, DataSourceClient>,
    cache: Option<FeatureCache>,
}

pub struct FeatureDefinition {
    pub name: String,
    pub datasource: String,
    pub query: Query,
    pub aggregations: Vec<Aggregation>,
    pub filters: Vec<Filter>,
    pub time_window: Option<TimeWindow>,
    pub cache: Option<CacheConfig>,
}
```

**Execution Flow**:
```
Feature Request
    ↓
┌─────────────────┐
│ Check Cache     │  → Hit? Return cached value
└────────┬────────┘
         ↓ Miss
┌─────────────────┐
│ Select Data     │  Query data source
│ Source          │  - PostgreSQL
│                 │  - Redis
│                 │  - ClickHouse
└────────┬────────┘
         ↓
┌─────────────────┐
│ Execute Query   │  Run query with filters
└────────┬────────┘
         ↓
┌─────────────────┐
│ Apply           │  Statistical functions
│ Aggregations    │  - count, sum, avg, min, max
│                 │  - std_dev, percentile
└────────┬────────┘
         ↓
┌─────────────────┐
│ Cache Result    │  Store in cache if configured
└────────┬────────┘
         ↓
    Return Feature Value
```

#### 5.3 List Service

**Architecture**:
```rust
pub struct ListService {
    backends: HashMap<String, Box<dyn ListBackend>>,
}

pub trait ListBackend: Send + Sync {
    async fn lookup(&self, list_id: &str, value: &Value) -> Result<bool>;
    async fn add(&mut self, list_id: &str, value: Value) -> Result<()>;
    async fn remove(&mut self, list_id: &str, value: &Value) -> Result<()>;
}

// Implementations
pub struct FileBackend { /* JSON file storage */ }
pub struct PostgresBackend { /* Database storage */ }
pub struct RedisBackend { /* Redis storage */ }
```

**Use Cases**:
- IP blocklists/allowlists
- Email domain blocklists
- High-risk country lists
- Device fingerprint lists
- Custom business rules lists

#### 5.4 Decision Result Persistence

**Architecture**:
```rust
pub struct DecisionResultWriter {
    pool: PgPool,
    buffer: Arc<Mutex<Vec<DecisionTrace>>>,
    flush_interval: Duration,
}

pub struct DecisionTrace {
    pub request_id: String,
    pub pipeline_id: String,
    pub decision: String,
    pub score: Option<i32>,
    pub triggered_rules: Vec<String>,
    pub execution_time_ms: i64,
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
}
```

**Storage**:
```sql
CREATE TABLE decision_results (
    id UUID PRIMARY KEY,
    request_id TEXT NOT NULL,
    pipeline_id TEXT NOT NULL,
    decision TEXT NOT NULL,
    score INTEGER,
    triggered_rules TEXT[],
    execution_time_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL,
    metadata JSONB
);
```

#### 5.5 Observability

**Metrics** (Prometheus):
```rust
// Execution metrics
decision_executions_total{pipeline, decision}
decision_execution_duration_seconds{pipeline}

// Feature metrics
feature_executions_total{feature, datasource}
feature_cache_hits_total{feature}
feature_cache_misses_total{feature}

// List metrics
list_lookups_total{list_id}

// Error metrics
errors_total{component, error_type}
```

**Tracing** (Basic implementation):
```rust
// Span hierarchy
span: decide
  ├─ span: load_features
  │   ├─ span: query_postgres
  │   └─ span: aggregate
  ├─ span: execute_pipeline
  │   ├─ span: eval_rule[new_device_login]
  │   ├─ span: eval_rule[unusual_location]
  │   └─ span: eval_ruleset[takeover_detection]
  └─ span: persist_result
```

---

### 6. Core Layer (`corint-core`)

**Responsibility**: Foundation library providing shared types and abstractions used by all other components

**Core Modules**:

```rust
// AST (Abstract Syntax Tree) - Represents parsed YAML structure
pub mod ast {
    pub struct Pipeline {
        pub id: String,
        pub name: String,
        pub when: Option<WhenBlock>,
        pub steps: Vec<PipelineStep>,
    }

    pub struct Rule {
        pub id: String,
        pub name: String,
        pub when: WhenBlock,
        pub score: Option<i32>,
    }

    pub struct Ruleset {
        pub id: String,
        pub name: String,
        pub rules: Vec<String>,
        pub decision_logic: Vec<DecisionLogicRule>,
    }

    pub struct Expression { /* ... */ }
    pub enum Operator { /* ... */ }
}

// Value System - Runtime data representation
pub mod value {
    pub enum Value {
        Null,
        Bool(bool),
        Number(Number),
        String(String),
        Array(Vec<Value>),
        Object(HashMap<String, Value>),
    }

    impl Value {
        pub fn get_type(&self) -> ValueType;
        pub fn as_bool(&self) -> Option<bool>;
        pub fn as_number(&self) -> Option<&Number>;
        // ... type conversion methods
    }
}

// Bytecode - VM instructions
pub mod bytecode {
    pub enum Instruction {
        // Stack operations
        Push(Value),
        Pop,
        Dup,

        // Variable operations
        Load(String),
        Store(String),

        // Arithmetic
        Add, Sub, Mul, Div, Mod,

        // Comparison
        Eq, Ne, Lt, Le, Gt, Ge,

        // Logical
        And, Or, Not,

        // Control flow
        Jump(usize),
        JumpIfFalse(usize),
        JumpIfTrue(usize),
        Call(String),
        Return,

        // Rule evaluation
        EvalRule(String),
        EvalRuleset(String),

        // Feature operations
        GetFeature(String),

        // List operations
        ListLookup { list_id: String, value: Value },
    }

    pub struct BytecodeProgram {
        pub instructions: Vec<Instruction>,
        pub constants: Vec<Value>,
        pub metadata: ProgramMetadata,
    }
}

// Error Types - Standardized error handling
pub mod error {
    pub enum CoreError {
        ParseError(String),
        TypeError { expected: String, found: String },
        RuntimeError(String),
        CompileError(String),
        ValidationError(String),
    }

    pub type Result<T> = std::result::Result<T, CoreError>;
}

// Common Utilities
pub mod utils {
    // String manipulation
    pub fn normalize_identifier(id: &str) -> String;

    // Type conversions
    pub fn value_to_json(value: &Value) -> serde_json::Value;
    pub fn json_to_value(json: &serde_json::Value) -> Value;
}
```

**Key Characteristics**:
- **Zero external dependencies**: Pure Rust core library (only serde for serialization)
- **Shared across all crates**: Used by parser, compiler, runtime, and SDK
- **Type-safe**: Strong typing with compile-time guarantees
- **Serializable**: All types implement `Serialize`/`Deserialize` for persistence
- **Versioned**: Maintains backward compatibility for bytecode format

**Dependency Graph**:
```
corint-parser    ──┐
                   ├──> corint-core (AST, Value, Error)
corint-compiler  ──┤
                   │
corint-runtime   ──┤──> corint-core (Bytecode, Value, Error)
                   │
corint-repository──┘──> corint-core (Error, Value)
```

**Design Benefits**:
1. **Decoupling**: Changes to core types propagate automatically to all consumers
2. **Consistency**: Single source of truth for data structures
3. **Testability**: Core types can be tested independently
4. **Maintainability**: Clear separation between data definitions and business logic

---

## Decision Logic Architecture

### Three-Layer Decision Model

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
│ - ❌ Signal definitions                                 │
│ - ❌ Decision logic                                     │
└─────────────────────────────────────────────────────────┘
                          ↓ scores
┌─────────────────────────────────────────────────────────┐
│ Layer 2: Rulesets (Signal Producers)                    │
├─────────────────────────────────────────────────────────┤
│ Responsibilities:                                        │
│ - Organize related rules                                │
│ - Evaluate rule combination patterns                    │
│ - Produce signals based on score, count, combinations   │
│ - ✅ Define Signals (via conclusion)                   │
│ - Output: critical_risk, high_risk, medium_risk, etc.  │
│                                                          │
│ Does NOT include:                                        │
│ - ❌ Final decisions (approve/deny/review/challenge)   │
└─────────────────────────────────────────────────────────┘
                          ↓ signal
┌─────────────────────────────────────────────────────────┐
│ Layer 3: Pipeline (Orchestrator + Decision Maker)       │
├─────────────────────────────────────────────────────────┤
│ Responsibilities:                                        │
│ - Orchestrate execution flow                            │
│ - Define step sequence                                  │
│ - Manage data flow                                      │
│ - Control branching and parallelism                     │
│ - ✅ Make final decisions (via decision section)       │
│ - ✅ Decide based on signals from rulesets             │
│ - Output: approve, deny, review, challenge              │
└─────────────────────────────────────────────────────────┘
```

**Key Design Principle**: Rulesets produce **signals** (risk levels), Pipelines make **decisions** (actions). This separation allows:
- Same ruleset reused with different decision thresholds
- VIP users can have lenient decision rules for the same signals
- Easy A/B testing of decision thresholds without changing detection logic

### Example

**Rule** (Detection only):
```yaml
rule:
  id: new_device_login
  name: New Device Login

  when:
    event.type: login
    conditions:
      - device.id not_in user.known_devices

  score: 40
  # ❌ No action - rules only detect
```

**Ruleset** (Signal Production):
```yaml
ruleset:
  id: takeover_detection
  name: Account Takeover Detection

  rules:
    - new_device_login       # 40 points
    - unusual_location       # 50 points
    - behavior_anomaly       # 60 points

  # ✅ Conclusion produces signals, NOT final decisions
  conclusion:
    # Specific combination → critical risk signal
    - when: |
        triggered_rules contains "new_device_login" &&
        triggered_rules contains "unusual_location"
      signal: critical_risk
      reason: "Account takeover pattern detected"

    # High score → high risk signal
    - when: total_score >= 100
      signal: high_risk
      reason: "High risk score threshold exceeded"

    # Medium score → medium risk signal
    - when: total_score >= 60
      signal: medium_risk
      reason: "Medium risk score"

    # Default → normal signal
    - default: true
      signal: normal
```

**Pipeline** (Orchestration + Final Decisions):
```yaml
pipeline:
  id: login_security
  name: Login Security Check

  when: event.type == "login"

  entry: user_tier_router

  steps:
    # Route to different checks based on user tier
    - step:
        id: user_tier_router
        name: User Tier Router
        type: router
        routes:
          - next: vip_security_check
            when:
              all:
                - event.user.tier == "vip"
          - next: basic_security_check
            when:
              all:
                - event.user.tier == "basic"
        default: default_security_check

    # VIP user security check (more lenient decisions)
    - step:
        id: vip_security_check
        name: VIP Security Check
        type: ruleset
        ruleset: takeover_detection
        next: end

    # Basic user security check
    - step:
        id: basic_security_check
        name: Basic Security Check
        type: ruleset
        ruleset: takeover_detection
        next: end

    # Default security check
    - step:
        id: default_security_check
        name: Default Security Check
        type: ruleset
        ruleset: takeover_detection
        next: end

  # ✅ Final decisions based on signals from rulesets
  decision:
    # Critical risk → always deny
    - when: context.takeover_detection.signal == "critical_risk"
      action: deny
      reason: "Critical security risk detected"

    # High risk for VIP → review (lenient)
    - when: |
        context.takeover_detection.signal == "high_risk" &&
        event.user.tier == "vip"
      action: review
      reason: "VIP user high risk - manual review"

    # High risk for others → deny
    - when: context.takeover_detection.signal == "high_risk"
      action: deny
      reason: "High security risk detected"

    # Medium risk → challenge
    - when: context.takeover_detection.signal == "medium_risk"
      action: challenge
      reason: "Additional verification required"

    # Default → approve
    - default: true
      action: approve
      reason: "Login approved"
```

---

## Data Flow Architecture

### Request-Response Flow

```
1. HTTP Request
   │
   ├─→ POST /v1/decide
   │   Body: { "event": {...}, "context": {...} }
   │
   ↓
2. SDK Layer
   │
   ├─→ DecisionEngine::decide(request)
   ├─→ Route to pipeline (via registry)
   │
   ↓
3. Repository Layer
   │
   ├─→ Load pipeline definition
   ├─→ Resolve includes (rules, rulesets)
   │
   ↓
4. Compiler Layer
   │
   ├─→ Parse YAML → AST
   ├─→ Semantic analysis
   ├─→ Optimize
   ├─→ Generate bytecode
   │
   ↓
5. Runtime Layer (VM Execution)
   │
   ├─→ Initialize execution context
   ├─→ Execute instructions
   │   │
   │   ├─→ Load features (FeatureExecutor)
   │   │   └─→ Query datasources
   │   │
   │   ├─→ Evaluate rules
   │   │   └─→ Check list membership (ListService)
   │   │
   │   ├─→ Evaluate rulesets
   │   │   └─→ Execute decision_logic
   │   │
   │   └─→ Produce decision
   │
   ├─→ Persist result (DecisionResultWriter)
   ├─→ Emit metrics/traces
   │
   ↓
6. Response
   {
     "decision": "deny",
     "score": 90,
     "triggered_rules": ["new_device_login", "unusual_location"],
     "reason": "Account takeover pattern detected",
     "request_id": "req_123",
     "execution_time_ms": 45
   }
```

---

## Multi-Environment Support

### 1. HTTP Server

**Use Case**: Production API service

**Configuration**:
```yaml
# config/server.yaml
host: 0.0.0.0
port: 8080
repository:
  type: file_system
  path: repository
enable_metrics: true
enable_tracing: true
database_url: postgresql://...
```

**Integration**:
```rust
let engine = DecisionEngineBuilder::new()
    .with_repository(RepositoryConfig::file_system("repository"))
    .with_result_writer(pool)
    .build()
    .await?;
```

### 2. WASM (Browser/Edge)

**Use Case**: Client-side risk scoring, edge computing

**Configuration**:
```rust
// Load from backend API
let engine = DecisionEngineBuilder::new()
    .with_repository(
        RepositoryConfig::api("https://api.example.com/repository")
            .with_api_key("secret-key")
    )
    .build()
    .await?;
```

**Features**:
- No file system dependency
- Smaller binary size
- API-based configuration loading
- Browser-compatible

### 3. FFI (Python/Go/etc.)

**Use Case**: Language bindings for integration

**Example (Python)**:
```python
import corint

# From file system
engine = corint.Engine.from_filesystem("repository")

# From database
engine = corint.Engine.from_database("postgresql://localhost/corint")

# Execute decision
result = engine.decide({
    "amount": 1000,
    "country": "US"
})
```

---

## Performance Characteristics

### Compilation

- **Parse time**: ~1-5ms per YAML file
- **Compile time**: ~10-50ms per pipeline
- **Optimization**: Reduces bytecode size by 20-40%

### Runtime

- **Decision latency**: p50: <10ms, p99: <50ms
- **Throughput**: 10,000+ decisions/sec (single instance)
- **Memory**: ~50-200MB (depending on loaded content)

### Caching

- **Feature cache**: Redis-based, TTL configurable
- **Compiled bytecode**: In-memory, persistent across requests
- **List lookup**: In-memory for file-based, external for DB/Redis

---

## Security Architecture

### Input Validation

- Schema-based event validation
- Type checking at compile-time
- Expression sandboxing (no arbitrary code execution)

### Data Isolation

- Request-scoped execution context
- No shared mutable state
- Thread-safe runtime components

### Audit Trail

- Complete decision trace persistence
- Triggered rules and scores logged
- Execution timeline recorded

---

## Extensibility Points

### 1. Custom Data Sources

```rust
impl DataSourceClient {
    pub fn new(config: DataSourceConfig) -> Self {
        match config.type {
            DataSourceType::Postgres => /* ... */,
            DataSourceType::Redis => /* ... */,
            DataSourceType::Custom(name) => /* plugin loader */,
        }
    }
}
```

### 2. Custom Functions

```rust
// Register custom functions in expression evaluator
context.register_function("custom_risk_score", |args| {
    // Custom logic
    Ok(Value::Number(score))
});
```

### 3. Custom Actions

```yaml
decision_logic:
  - condition: score > 80
    action: custom_action
    custom_action:
      type: webhook
      url: https://...
      payload: {...}
```

---

## Deployment Architectures

### Single Instance

```
┌─────────────────┐
│  Load Balancer  │
└────────┬────────┘
         │
    ┌────┴────┐
    │ Server  │
    │  Port   │
    │  8080   │
    └────┬────┘
         │
    ┌────┴─────┐
    │PostgreSQL│
    └──────────┘
```

### High Availability

```
┌─────────────────┐
│  Load Balancer  │
└────────┬────────┘
         │
    ┌────┼────┬────┐
    │    │    │    │
┌───┴┐ ┌─┴─┐ ┌┴──┐ │
│Srv1│ │Srv2│ │Srv3││
└───┬┘ └─┬─┘ └┬──┘ │
    └────┼────┴────┘
         │
    ┌────┴────────┐
    │ PostgreSQL  │
    │  (Primary)  │
    └─────────────┘
```

### Edge Deployment

```
┌────────────┐
│  Browser   │  WASM Instance
└─────┬──────┘
      │
      │ API (config loading)
      │
┌─────┴──────┐
│   Backend  │  HTTP Server
│   API      │
└─────┬──────┘
      │
┌─────┴──────┐
│ PostgreSQL │
└────────────┘
```

---

## Testing Architecture

### Unit Tests

- Parser: YAML → AST correctness
- Compiler: Optimization correctness
- Runtime: VM instruction execution
- Features: Aggregation logic

### Integration Tests

- Repository loading
- End-to-end compilation
- Feature execution with real data sources
- Decision persistence

### Pipeline Tests

```yaml
test:
  pipeline: fraud_detection
  cases:
    - name: high_risk_transaction
      input:
        amount: 10000
        country: "NG"
      expected:
        decision: deny
        triggered_rules: ["high_amount", "high_risk_country"]
```

---

## Future Roadmap

### Short-term

- [ ] GraphQL API support
- [ ] Real-time rule updates (hot reload)
- [ ] Advanced ML model integration
- [ ] A/B testing framework

### Long-term

- [ ] Distributed execution (multi-region)
- [ ] Visual rule builder UI
- [ ] Auto-optimization based on metrics
- [ ] Serverless deployment support

---

## Reference Documentation

- **DSL Design**: `docs/DSL_DESIGN.md` - Three-layer decision model, design patterns, and best practices for rule authors
- **DSL Syntax Reference**: `docs/dsl/` - Complete DSL specification (rule.md, ruleset.md, pipeline.md, expression.md, etc.)
- **SDK/Server Separation Plan**: `docs/SDK_SERVER_SEPARATION_PLAN.md` - Architecture refactoring and unified integration approach
- **API Documentation**: `docs/api/`
- **Examples**: `repository/pipelines/`, `examples/`

---

## Glossary

- **Pipeline**: Orchestration workflow that defines execution steps
- **Rule**: Detection logic that identifies risk factors and produces scores
- **Ruleset**: Collection of rules with decision logic
- **Feature**: Computed or aggregated data from data sources
- **List**: Blocklist/allowlist for quick membership checks
- **Bytecode**: Compiled VM instructions from YAML definitions
- **Repository**: Collection of all business configuration (pipelines, rules, features, etc.)
- **SDK**: Software Development Kit providing unified entry point for integration
