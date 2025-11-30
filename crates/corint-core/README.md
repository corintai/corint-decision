# corint-core

Core types and definitions for CORINT Decision Engine.

## Overview

`corint-core` provides the fundamental building blocks used across the CORINT ecosystem:

- **Value types** - Runtime data representation (similar to JSON values)
- **AST definitions** - Abstract Syntax Tree for expressions, rules, and pipelines
- **Error types** - Common error handling

## Features

- ✅ Type-safe value system
- ✅ Serde-compatible (JSON serialization/deserialization)
- ✅ Comprehensive AST for rule definitions
- ✅ Expression language support (binary, unary, function calls)
- ✅ Zero runtime dependencies

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
corint-core = "0.1.0"
```

### Example: Creating Values

```rust
use corint_core::Value;

let number = Value::Number(42.0);
let string = Value::String("hello".to_string());
let array = Value::Array(vec![
    Value::Number(1.0),
    Value::Number(2.0),
]);
```

### Example: Building Expression AST

```rust
use corint_core::ast::{Expression, Operator};
use corint_core::Value;

// user.age > 18
let expr = Expression::binary(
    Expression::field_access(vec!["user".to_string(), "age".to_string()]),
    Operator::Gt,
    Expression::literal(Value::Number(18.0)),
);
```

### Example: Creating a Rule

```rust
use corint_core::ast::{Rule, WhenBlock, Expression, Operator};
use corint_core::Value;

let when = WhenBlock::new()
    .with_event_type("login".to_string())
    .add_condition(Expression::binary(
        Expression::field_access(vec!["user".to_string(), "age".to_string()]),
        Operator::Gt,
        Expression::literal(Value::Number(18.0)),
    ));

let rule = Rule::new(
    "age_check".to_string(),
    "Age Check Rule".to_string(),
    when,
    50,
);
```

## Testing

Run tests with:

```bash
cargo test
```

## License

Licensed under Elastic License 2.0.
