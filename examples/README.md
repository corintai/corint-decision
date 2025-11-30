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

### 2. Fraud Detection (`fraud_detection.rs`)

A realistic fraud detection scenario demonstrating:
- Multiple fraud detection rules
- Risk scoring across different scenarios
- Multiple test cases with varying risk levels

**Run:**
```bash
cargo run --example fraud_detection
```

**What it demonstrates:**
- High-value transaction detection
- Cross-border transaction checks
- High-risk country detection
- Combining multiple risk factors

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
- `fraud_detection.yaml` - Comprehensive fraud detection ruleset
- `complete_pipeline.yaml` - Full pipeline with context, features, and decision logic

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
