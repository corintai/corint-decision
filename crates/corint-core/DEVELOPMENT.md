# corint-core Development Guide

## Development Approach

This crate was developed using **Test-Driven Development (TDD)**:

1. ✅ Write tests first (Red)
2. ✅ Implement code to pass tests (Green)
3. ✅ Refactor if needed (Refactor)

## Project Structure

```
corint-core/
├── Cargo.toml              # Crate configuration
├── README.md               # User documentation
├── DEVELOPMENT.md          # This file
├── src/
│   ├── lib.rs              # Crate entry point
│   ├── value.rs            # Value type (with tests)
│   ├── error.rs            # Error types
│   └── ast/
│       ├── mod.rs          # AST module entry
│       ├── operator.rs     # Operators (with tests)
│       ├── expression.rs   # Expression AST (with tests)
│       └── rule.rs         # Rule AST (with tests)
├── examples/
│   └── basic_usage.rs      # Usage examples
└── tests/                  # Integration tests (future)
```

## What's Implemented

### 1. Value Type (`value.rs`)

A runtime value system similar to JSON:

- `Value::Null`
- `Value::Bool(bool)`
- `Value::Number(f64)`
- `Value::String(String)`
- `Value::Array(Vec<Value>)`
- `Value::Object(HashMap<String, Value>)`

**Tests:** 9 tests covering all value types and serialization

### 2. Operators (`ast/operator.rs`)

Binary operators for expressions:

- **Comparison:** `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Arithmetic:** `+`, `-`, `*`, `/`, `%`
- **Logical:** `&&`, `||`
- **String:** `contains`, `starts_with`, `ends_with`, `regex`
- **Membership:** `in`, `not_in`

**Tests:** 5 tests covering operator types and classification

### 3. Expression AST (`ast/expression.rs`)

Expression node types:

- `Literal(Value)` - Literal values
- `FieldAccess(Vec<String>)` - Field access (e.g., `user.age`)
- `Binary { left, op, right }` - Binary operations
- `Unary { op, operand }` - Unary operations (!,  -)
- `FunctionCall { name, args }` - Function calls
- `Ternary { condition, true_expr, false_expr }` - Ternary operator

**Tests:** 8 tests covering all expression types

### 4. Rule AST (`ast/rule.rs`)

Complete rule definition:

```rust
struct Rule {
    id: String,
    name: String,
    description: Option<String>,
    when: WhenBlock,
    score: i32,
}

struct WhenBlock {
    event_type: Option<String>,
    conditions: Vec<Expression>,
}
```

**Tests:** 5 tests covering rule creation and complex scenarios

## Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_value_number

# Run with verbose mode
cargo test -- --test-threads=1 --nocapture
```

## Running Examples

```bash
# Run basic usage example
cargo run --example basic_usage
```

## Test Coverage Summary

| Module | Tests | Coverage |
|--------|-------|----------|
| value.rs | 9 | ✅ All value types |
| ast/operator.rs | 5 | ✅ All operators |
| ast/expression.rs | 8 | ✅ All expression types |
| ast/rule.rs | 5 | ✅ Rule creation |
| **Total** | **27** | **100%** |

## Next Steps

### Phase 2: IR (Intermediate Representation)

Create `src/ir/` module with:

1. `instruction.rs` - IR instructions
2. `program.rs` - IR program structure

### Phase 3: Integration Tests

Create `tests/` directory with end-to-end tests.

### Phase 4: Performance

Add benchmarks using `criterion`:

```toml
[dev-dependencies]
criterion = "0.5"
```

## Design Decisions

### Why f64 for Numbers?

- Simplifies implementation (no need for int/float distinction)
- Compatible with JSON
- Sufficient precision for risk scores
- Can be optimized later if needed

### Why Box<Expression> in Binary/Unary?

- Prevents infinite size (recursive type)
- Enables tree structures
- Standard Rust pattern for AST

### Why HashMap for Object?

- Fast lookups (O(1))
- JSON-compatible
- Flexible for dynamic data

## Common Patterns

### Creating an Expression

```rust
// Simple: user.age > 18
Expression::binary(
    Expression::field_access(vec!["user".into(), "age".into()]),
    Operator::Gt,
    Expression::literal(Value::Number(18.0)),
)
```

### Creating a Rule

```rust
let when = WhenBlock::new()
    .with_event_type("login".into())
    .add_condition(/* expression */);

Rule::new("rule_id".into(), "Rule Name".into(), when, 50)
```

## Troubleshooting

### Compilation Errors

```bash
# Check for errors
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy
```

### Test Failures

```bash
# Run failing test with output
cargo test failing_test_name -- --nocapture
```

## Contributing

When adding new features:

1. Write tests first (TDD)
2. Implement minimal code to pass
3. Add documentation
4. Update this guide

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Serde Documentation](https://serde.rs/)
- [AST Design Patterns](https://rust-unofficial.github.io/patterns/)
