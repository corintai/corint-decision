# CORINT LLM Code Generation (v1.0)

**⚠️ IMPORTANT: LLM is NO LONGER a runtime step type in CORINT pipelines.**

This document describes the `corint-llm` crate, which provides **development-time code generation** capabilities. LLM is used to generate YAML configurations from natural language descriptions, not for real-time decision execution.

---

## Architecture Decision

### Why LLM Was Removed from Runtime

LLM integration was removed from the CORINT runtime for the following reasons:

1. **Latency Incompatibility**
   - LLM inference: 2-5 seconds per call
   - CORINT requirement: <100-300ms decision latency
   - **10-50x latency mismatch** makes real-time use impractical

2. **Reliability Concerns**
   - External API dependencies introduce failure points
   - Network timeouts and rate limits
   - Model version changes can affect consistency

3. **Cost and Complexity**
   - Per-request LLM calls are expensive at scale
   - Complex error handling and fallback logic
   - Observability and debugging challenges

### New Approach: Offline Code Generation

Instead of using LLM at runtime, CORINT now uses LLM for **development-time code generation**:

```
┌──────────────────────────────────────────────────────────┐
│ Development Time (Offline)                                │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  Natural Language Description                             │
│         ↓                                                  │
│  corint-llm (LLM-powered generator)                       │
│         ↓                                                  │
│  Generated YAML Configurations                            │
│  - Rules                                                   │
│  - Rulesets                                               │
│  - Pipelines                                              │
│  - API Configs                                            │
│         ↓                                                  │
│  Developer Review & Version Control                       │
│                                                            │
└──────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│ Production Time (Real-time <100ms)                       │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  Event Input                                              │
│         ↓                                                  │
│  CORINT Runtime (No LLM)                                  │
│  - Rules                                                   │
│  - Rulesets                                               │
│  - Pipelines                                              │
│  - External APIs                                          │
│  - Data Sources                                           │
│         ↓                                                  │
│  Decision Output (<100-300ms)                             │
│                                                            │
└──────────────────────────────────────────────────────────┘
```

---

## The `corint-llm` Crate

### Overview

`corint-llm` is a standalone Rust crate that generates CORINT YAML configurations from natural language descriptions.

**Location**: `crates/corint-llm/`

**Purpose**: Development-time code generation, NOT runtime execution

### Features

- **Rule Generation**: Create detection rules from natural language
- **Ruleset Generation**: Generate rule collections with decision logic
- **Pipeline Generation**: Build complete decision workflows
- **API Config Generation**: Define external API integrations
- **Complete Flow Generation**: Generate full decision flows with all components

### Installation

```toml
[dependencies]
corint-llm = { path = "../corint-llm" }
tokio = { version = "1.0", features = ["full"] }
```

---

## Usage Examples

### 1. Generate a Single Rule

```rust
use corint_llm::{RuleGenerator, OpenAIProvider};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize LLM provider
    let provider = Arc::new(OpenAIProvider::new(
        std::env::var("OPENAI_API_KEY")?
    ));

    // Create generator
    let generator = RuleGenerator::with_defaults(provider);

    // Generate rule from description
    let description = "Flag transactions over $10,000 from new accounts (< 30 days old)";
    let rule_yaml = generator.generate(description).await?;

    // Save to repository
    std::fs::write("repository/library/rules/fraud/high_amount_new_account.yaml", rule_yaml)?;

    println!("Rule generated successfully!");
    Ok(())
}
```

**Generated YAML**:
```yaml
rule:
  id: high_amount_new_account
  name: High Amount from New Account
  description: Flag transactions over $10,000 from accounts less than 30 days old

  when:
    all:
      - event.type == "transaction"
      - event.amount > 10000
      - event.account.age_days < 30

  score: 80
  reason: "High-value transaction from new account"
```

### 2. Generate a Ruleset

```rust
use corint_llm::{RulesetGenerator, AnthropicProvider};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = Arc::new(AnthropicProvider::new(
        std::env::var("ANTHROPIC_API_KEY")?
    ));

    let generator = RulesetGenerator::with_defaults(provider);

    let description = r#"
    Create a fraud detection ruleset that:
    - Includes rules for high amounts, velocity, and geo-anomalies
    - Uses score_sum strategy
    - Signals 'critical_risk' if score > 100
    - Signals 'high_risk' if score > 60
    - Default signal is 'normal'
    "#;

    let ruleset_yaml = generator.generate(description).await?;
    std::fs::write("repository/library/rulesets/fraud_detection.yaml", ruleset_yaml)?;

    Ok(())
}
```

### 3. Generate a Complete Decision Flow

```rust
use corint_llm::{DecisionFlowGenerator, GeminiProvider};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = Arc::new(GeminiProvider::new(
        std::env::var("GEMINI_API_KEY")?
    ));

    let generator = DecisionFlowGenerator::with_defaults(provider);

    let description = r#"
    Create a payment fraud detection system that:
    1. Checks IP reputation using IPInfo API
    2. Flags high-value transactions (>$10,000)
    3. Detects velocity anomalies (>5 transactions per hour)
    4. Blocks high amounts from new accounts (<90 days)
    5. Routes based on risk score:
       - Score > 70: Decline
       - Score > 40: Manual review
       - Otherwise: Approve
    "#;

    let flow = generator.generate(description).await?;

    println!("Generated {} rules", flow.rule_count);
    println!("Generated {} rulesets", flow.ruleset_count);
    println!("Generated {} pipelines", flow.pipeline_count);
    println!("Generated {} API configs", flow.api_config_count);

    // Save each component to appropriate directory
    for (i, api_config) in flow.api_configs().iter().enumerate() {
        std::fs::write(
            format!("repository/configs/apis/generated_api_{}.yaml", i),
            api_config
        )?;
    }

    for (i, rule) in flow.rules().iter().enumerate() {
        std::fs::write(
            format!("repository/library/rules/fraud/generated_rule_{}.yaml", i),
            rule
        )?;
    }

    for (i, ruleset) in flow.rulesets().iter().enumerate() {
        std::fs::write(
            format!("repository/library/rulesets/generated_ruleset_{}.yaml", i),
            ruleset
        )?;
    }

    for (i, pipeline) in flow.pipelines().iter().enumerate() {
        std::fs::write(
            format!("repository/pipelines/generated_pipeline_{}.yaml", i),
            pipeline
        )?;
    }

    Ok(())
}
```

---

## Supported LLM Providers

The `corint-llm` crate supports multiple LLM providers:

### 1. OpenAI

```rust
use corint_llm::OpenAIProvider;

let provider = OpenAIProvider::new(api_key)
    .with_model("gpt-4-turbo")
    .with_base_url("https://api.openai.com/v1");
```

**Models**: `gpt-4-turbo`, `gpt-4`, `gpt-3.5-turbo`

### 2. Anthropic

```rust
use corint_llm::AnthropicProvider;

let provider = AnthropicProvider::new(api_key)
    .with_model("claude-3-5-sonnet-20241022");
```

**Models**: `claude-3-5-sonnet-20241022`, `claude-3-opus-20240229`, `claude-3-haiku-20240307`

### 3. Google Gemini

```rust
use corint_llm::GeminiProvider;

let provider = GeminiProvider::new(api_key)
    .with_model("gemini-1.5-pro");
```

**Models**: `gemini-1.5-pro`, `gemini-1.5-flash`

### 4. DeepSeek

```rust
use corint_llm::DeepSeekProvider;

let provider = DeepSeekProvider::new(api_key);
```

**Models**: `deepseek-chat`, `deepseek-reasoner`

### 5. Mock Provider (for Testing)

```rust
use corint_llm::MockProvider;

let provider = MockProvider::with_response(r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    all:
      - event.amount > 1000
  score: 50
"#.to_string());
```

---

## Configuration

### Generator Configuration

```rust
use corint_llm::{RuleGenerator, RuleGeneratorConfig};

let config = RuleGeneratorConfig {
    model: Some("gpt-4-turbo".to_string()),
    max_tokens: Some(2000),
    temperature: Some(0.3),
    enable_thinking: true,
};

let generator = RuleGenerator::new(provider, config);
```

**Options**:
- `model`: LLM model to use (optional, uses provider default)
- `max_tokens`: Maximum tokens in response (default: varies by generator)
- `temperature`: Sampling temperature 0.0-2.0 (default: 0.3 for consistency)
- `enable_thinking`: Enable Claude extended thinking (default: true)

---

## Best Practices

### 1. Be Specific in Descriptions

✅ **Good**:
```rust
let description = r#"
Create a rule that flags transactions where:
- Amount exceeds $5,000
- User account is less than 7 days old
- Transaction is from a country different from registration country
Score should be 75 points
"#;
```

❌ **Bad**:
```rust
let description = "Flag risky transactions";
```

### 2. Review Generated Code

Always review LLM-generated YAML before committing:

```rust
let rule_yaml = generator.generate(description).await?;

// Review the generated YAML
println!("Generated rule:\n{}", rule_yaml);

// Parse and validate
let rule = RuleParser::parse(&rule_yaml)?;
println!("Validated: {}", rule.id);

// Save after review
std::fs::write("repository/library/rules/fraud/new_rule.yaml", rule_yaml)?;
```

### 3. Version Control

Treat generated YAML like any code:
- Commit to version control
- Use pull requests for review
- Add comments explaining business logic

### 4. Test Generated Configurations

```rust
// Test the generated rule
let test_event = json!({
    "amount": 6000,
    "account": {"age_days": 5},
    "country": "US",
    "registration_country": "UK"
});

let result = engine.evaluate_rule("new_rule", &test_event).await?;
assert!(result.triggered);
assert_eq!(result.score, 75);
```

---

## Development Workflow

### Recommended Process

1. **Describe Requirements** in natural language
2. **Generate YAML** using `corint-llm`
3. **Review Output** for correctness
4. **Test Configuration** with sample events
5. **Refine Description** if needed and regenerate
6. **Commit to Repository** after validation
7. **Deploy** through normal CICD pipeline

### Example Workflow

```bash
# 1. Generate fraud rules
cargo run --example generate_fraud_rules

# 2. Review generated files
ls repository/library/rules/fraud/

# 3. Test generated rules
cargo test --test fraud_rule_tests

# 4. Commit if tests pass
git add repository/library/rules/fraud/
git commit -m "Add LLM-generated fraud detection rules"

# 5. Deploy
./scripts/deploy_to_staging.sh
```

---

## Examples

See the `crates/corint-llm/examples/` directory for complete examples:

- **generate_rule.rs**: Simple rule generation
- **generate_ruleset.rs**: Ruleset creation
- **generate_pipeline.rs**: Pipeline workflow generation
- **generate_api_config.rs**: External API configuration
- **generate_decision_flow.rs**: Complete decision flow with all components

Run an example:

```bash
cd crates/corint-llm
export OPENAI_API_KEY="sk-..."
cargo run --example generate_decision_flow
```

---

## Migration from Old LLM Steps

If you have old pipeline configurations with `type: reason` steps, they must be removed:

**OLD** (No longer supported):
```yaml
pipeline:
  steps:
    - type: reason
      id: llm_analysis
      provider: openai
      model: gpt-4
      prompt: "Analyze this transaction"
```

**NEW** (Generate rules instead):
```rust
// Use corint-llm to generate a rule
let description = "Analyze transaction patterns and flag suspicious behavior";
let rule_yaml = generator.generate(description).await?;

// The generated rule will use standard CORINT conditions
// No LLM calls at runtime
```

---

## Summary

**corint-llm provides**:
- Fast YAML generation from natural language
- Support for multiple LLM providers
- Type-safe integration with CORINT DSL
- Development-time code generation only

**corint-llm does NOT**:
- Execute at runtime
- Affect production decision latency
- Require LLM API access in production
- Block or slow down real-time decisions

For runtime decision logic, use standard CORINT DSL components:
- **Rules**: Detection logic with conditions
- **Rulesets**: Rule collections with conclusion logic
- **Pipelines**: Orchestration workflows
- **External APIs**: Third-party service calls (non-LLM)
- **Data Sources**: Feature extraction

LLM is a development tool, not a runtime component.
