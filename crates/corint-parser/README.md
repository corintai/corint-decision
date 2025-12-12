# corint-parser

**YAML Parser for CORINT Decision Engine**

## Overview

`corint-parser` converts YAML rule definitions into Abstract Syntax Tree (AST) representations. It handles the parsing of rules, rulesets, pipelines, features, templates, and registry configurations.

## Key Responsibilities

- **YAML to AST Conversion**: Parse YAML files into strongly-typed AST structures
- **Expression Parsing**: Convert string expressions into AST expression trees
- **Syntax Validation**: Basic syntax checking and structure validation
- **Multi-document Support**: Handle YAML files with multiple documents (---separated)

## Architecture

```
┌─────────────────────────────────────┐
│   YAML Definition Files            │
│   rules/*.yaml                     │
│   rulesets/*.yaml                  │
│   pipelines/*.yaml                 │
└─────────────┬───────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│   YamlParser                       │
│   - Load and parse YAML            │
│   - Validate basic structure       │
└─────────────┬───────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│   Specialized Parsers              │
│   - RuleParser                     │
│   - RulesetParser                  │
│   - PipelineParser                 │
│   - TemplateParser                 │
│   - RegistryParser                 │
└─────────────┬───────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│   AST (corint-core types)          │
│   Rule, Ruleset, Pipeline, etc.    │
└─────────────────────────────────────┘
```

## Core Components

### RuleParser
Parses individual rule definitions:
```rust
pub struct RuleParser;

impl RuleParser {
    /// Parse a rule from YAML string
    pub fn parse(yaml: &str) -> Result<Rule>;

    /// Parse a rule from pre-loaded YAML value
    pub fn parse_from_yaml(yaml: &YamlValue) -> Result<Rule>;
}
```

### RulesetParser
Parses ruleset definitions with decision logic:
```rust
pub struct RulesetParser;

impl RulesetParser {
    /// Parse a ruleset from YAML string
    pub fn parse(yaml: &str) -> Result<Ruleset>;

    /// Parse ruleset with inheritance support (Phase 3)
    pub fn parse_with_extends(yaml: &str) -> Result<Ruleset>;
}
```

### PipelineParser
Parses complete pipeline definitions:
```rust
pub struct PipelineParser;

impl PipelineParser {
    /// Parse a pipeline from YAML string
    pub fn parse(yaml: &str) -> Result<Pipeline>;

    /// Parse multi-document YAML file (rules + ruleset + pipeline)
    pub fn parse_multi(yaml: &str) -> Result<Vec<AstNode>>;
}
```

### RegistryParser
Parses pipeline registry configurations:
```rust
pub struct RegistryParser;

impl RegistryParser {
    /// Parse registry.yaml configuration
    pub fn parse(yaml: &str) -> Result<PipelineRegistry>;
}
```

### ExpressionParser
Parses condition expressions:
```rust
pub struct ExpressionParser;

impl ExpressionParser {
    /// Parse expression string into AST
    pub fn parse(expr: &str) -> Result<Expression>;
}
```

Supported expressions:
- Binary operations: `amount > 1000`, `user.age >= 18`
- Logical operations: `amount > 1000 AND country == "US"`
- Function calls: `count(transactions, last_7d) > 10`
- Field access: `event.user.email`, `context.features.score`

## Usage

### Parsing a Simple Rule

```rust
use corint_parser::RuleParser;

let yaml = r#"
rule:
  id: high_amount
  name: High Transaction Amount
  when:
    conditions:
      - amount > 1000
  score: 50
"#;

let rule = RuleParser::parse(yaml)?;
assert_eq!(rule.id, "high_amount");
assert_eq!(rule.score, 50);
```

### Parsing a Ruleset

```rust
use corint_parser::RulesetParser;

let yaml = r#"
ruleset:
  id: fraud_detection
  rules:
    - velocity_check
    - geo_mismatch
  decision_logic:
    - condition: total_score >= 100
      action: deny
    - condition: total_score >= 60
      action: review
    - default: true
      action: approve
"#;

let ruleset = RulesetParser::parse(yaml)?;
assert_eq!(ruleset.rules.len(), 2);
```

### Parsing Multi-Document Files

```rust
use corint_parser::PipelineParser;

let yaml = r#"
version: "0.1"

pipeline:
  id: login_pipeline
  steps:
    - include:
        ruleset: login_risk

---

rule:
  id: too_many_failures
  when:
    conditions:
      - failed_logins_1h > 5
  score: 60

---

ruleset:
  id: login_risk
  rules:
    - too_many_failures
  decision_logic:
    - condition: total_score >= 60
      action: deny
"#;

let artifacts = PipelineParser::parse_multi(yaml)?;
// Returns: [Pipeline, Rule, Ruleset]
```

### Parsing Registry Configuration

```rust
use corint_parser::RegistryParser;

let yaml = r#"
version: "0.1"

registry:
  - pipeline: fraud_detection_pipeline
    when:
      event.type: transaction
    description: Fraud detection for transactions

  - pipeline: payment_pipeline
    when:
      event.type: payment
    description: Payment risk assessment
"#;

let registry = RegistryParser::parse(yaml)?;
assert_eq!(registry.registry.len(), 2);
```

### Expression Parsing

```rust
use corint_parser::ExpressionParser;

// Simple comparison
let expr = ExpressionParser::parse("amount > 1000")?;

// Complex expression
let expr = ExpressionParser::parse(
    "amount > 1000 AND (country == 'US' OR country == 'CA')"
)?;

// Function call
let expr = ExpressionParser::parse(
    "count_distinct(device_id, {ip == event.ip}, last_5h) > 10"
)?;
```

## Error Handling

The parser provides detailed error messages with context:

```rust
pub enum ParseError {
    /// YAML syntax error
    YamlSyntax { line: usize, column: usize, message: String },

    /// Missing required field
    MissingField { field: String },

    /// Invalid field value
    InvalidValue { field: String, message: String },

    /// Expression parsing error
    ExpressionError { expr: String, reason: String },

    /// Unsupported feature
    UnsupportedFeature { feature: String },
}
```

Example:
```rust
let yaml = r#"
rule:
  id: test
  # Missing 'when' field
  score: 50
"#;

match RuleParser::parse(yaml) {
    Err(ParseError::MissingField { field }) => {
        println!("Missing field: {}", field); // "Missing field: when"
    }
    _ => {}
}
```

## Features

### Phase 3: Import System Support
- Parse `import` statements
- Parse `extends` in rulesets
- Parse `decision_template` references
- Parse rule `params` definitions

### Registry Support
- Parse pipeline registry with flexible `when` conditions
- Support both `event.type: value` and `conditions: [...]` syntax
- First-match-wins semantics

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rule_with_conditions() {
        let yaml = r#"
        rule:
          id: test_rule
          when:
            event.type: transaction
            conditions:
              - amount > 1000
              - country == "US"
          score: 75
        "#;

        let rule = RuleParser::parse(yaml).unwrap();
        assert_eq!(rule.when.conditions.len(), 2);
    }

    #[test]
    fn test_parse_registry() {
        let yaml = r#"
        registry:
          - pipeline: test_pipeline
            when:
              event.type: transaction
              event.source: supabase
        "#;

        let registry = RegistryParser::parse(yaml).unwrap();
        let entry = &registry.registry[0];
        assert_eq!(entry.when.conditions.len(), 1); // event.source converted to condition
    }
}
```

## Dependencies

- `corint-core`: AST type definitions
- `serde_yaml`: YAML parsing
- `serde`: Serialization framework

## Related Documentation

- [Rule DSL](../../docs/dsl/rule.md)
- [Ruleset DSL](../../docs/dsl/ruleset.md)
- [Pipeline DSL](../../docs/dsl/pipeline.md)
- [Expression Language](../../docs/dsl/expression.md)
- [Registry Configuration](../../docs/dsl/pipeline.md#registry)
