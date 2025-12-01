# CORINT Examples

This directory contains example implementations demonstrating how to use the CORINT Decision Engine SDK.

## Examples Overview

### 1. Simple Rule (`simple_rule.rs`)

A basic example showing how to:
- Create a DecisionEngine with a simple rule file
- Execute a decision request
- Handle the decision response

**Run:**
```bash
cargo run --example simple_rule
```

**What it does:** Checks if a transaction amount exceeds $100 and flags it for review.

### 2. Fraud Detection (`fraud_detection.rs`) ⭐ RECOMMENDED

**A comprehensive fraud detection system covering all core concepts:**

This example demonstrates a complete multi-layer fraud detection system with:
- 6-layer decision logic (critical → very high → high → medium-high → medium → low risk)
- Score-based thresholds and progressive risk assessment
- Feature-driven evaluation framework
- Action assignment (approve/review/deny)
- Terminate control for early exit
- Comprehensive documentation with DSL mapping

**Run:**
```bash
cargo run --example fraud_detection
```

**What it demonstrates:**
- **Multi-layer Decision Logic**: 6 risk levels with progressive thresholds
- **Feature Framework**: Structure for association, statistical, temporal, behavioral features
- **Action Assignment**: Approve, review, deny based on risk conditions
- **Terminate Control**: Early exit on high-risk decisions
- **Explainability**: Reason tracking for transparency
- **DSL Mapping**: Complete mapping to documentation concepts

**Concepts Covered** (from DSL documentation):
- Association Features: IP/Device/User relationship analysis (`devices_per_ip`, `users_per_device`)
- Statistical Features: Z-scores, percentiles, outlier detection (`amount_zscore`, `is_amount_outlier`)
- Velocity Features: Rate of change detection (`velocity_ratio`, `transaction_count_24h`)
- Temporal Features: Time patterns (`is_off_hours`, `days_since_last`)
- Behavioral Features: Activity patterns (`is_new_device`, `failed_login_count`)

**Test Scenarios:**
1. $50 → Approve (low risk)
2. $750 → Review (medium risk)
3. $2,500 → Review (medium-high risk)
4. $7,500 → Deny (high risk)
5. $15,000 → Deny (very high risk)
6. $25,000 → Deny (critical risk)

### 3. Complete Pipeline (`complete_pipeline.rs`)

A comprehensive example showing the full SDK capabilities:
- LLM integration (with mock provider)
- External service integration (with mock clients)
- Complete pipeline execution with multiple stages
- Metrics collection and observability
- All compiler optimizations enabled

**Run:**
```bash
cargo run --example complete_pipeline
```

**What it demonstrates:**
- Full DecisionEngineBuilder API
- LLM and Service configuration
- Complex event data processing
- Multi-stage pipeline execution
- Comprehensive metrics reporting

## Rule Files

All example rule files are located in `examples/rules/`:

- `simple_rule.yaml` - Basic amount threshold check
- `fraud_detection.yaml` - **⭐ Comprehensive example** covering all core concepts with 6-layer decision logic and complete DSL mapping
- `complete_pipeline.yaml` - Full pipeline with context, features, and decision logic (LLM and service integrations)

## Running All Examples

To build all examples:
```bash
cargo build --examples
```

To run all examples sequentially:
```bash
cargo run --example simple_rule
cargo run --example fraud_detection
cargo run --example complete_pipeline
```

**Recommended Learning Path:**
1. **simple_rule** - Learn basic concepts and rule execution
2. **fraud_detection** ⭐ - Understand all core features (multi-layer decisions, feature framework, DSL mapping)
3. **complete_pipeline** - Advanced integrations (LLM, external services)

## Key Concepts Demonstrated

### DecisionEngineBuilder

The builder pattern for configuring the decision engine:

```rust
let engine = DecisionEngineBuilder::new()
    .add_rule_file("path/to/rule.yaml")
    .with_llm(llm_config)
    .with_service(service_config)
    .enable_metrics(true)
    .enable_tracing(true)
    .build()
    .await?;
```

### DecisionRequest

Creating a request with event data and metadata:

```rust
let mut event_data = HashMap::new();
event_data.insert("amount".to_string(), Value::Number(150.0));

let request = DecisionRequest::new(event_data)
    .with_metadata("request_id".to_string(), "req-001".to_string());
```

### DecisionResponse

The response includes:
- `result.action` - The recommended action (Approve, Review, Block)
- `result.score` - Aggregate risk score
- `result.triggered_rules` - List of rules that matched
- `result.explanation` - Human-readable explanation
- `processing_time_ms` - Execution time

### Metrics

Accessing engine metrics:

```rust
let metrics = engine.metrics();
println!("Executions: {}", metrics.counter("executions_total").value());
```

## Next Steps

1. **Customize Rules**: Modify the YAML files in `examples/rules/` to match your use case
2. **Add Real Integrations**: Replace mock LLM and service providers with real implementations
3. **Extend Examples**: Add more complex scenarios based on your requirements
4. **Performance Testing**: Use these examples as a baseline for performance benchmarking

## Testing

All examples are compiled as part of the test suite:

```bash
cargo test
```

This ensures the examples stay up-to-date with the SDK API.
