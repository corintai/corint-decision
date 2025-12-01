# Logging Guide

## Overview

Corint Decision Engine uses the `tracing` crate for structured logging, with support for controlling log levels via environment variables.

## Log Levels

- `ERROR`: Error messages
- `WARN`: Warning messages
- `INFO`: General information (default level)
- `DEBUG`: Debug information (shows key execution steps)
- `TRACE`: Detailed trace information (shows every instruction execution)

## Usage

### 1. Default Run (INFO level)

```bash
cargo run --example fraud_detection
```

No debug output will be shown, only basic information.

### 2. Enable DEBUG Logging

```bash
RUST_LOG=corint_runtime=debug cargo run --example fraud_detection
```

Shows key execution steps, including:
- Program instruction count
- Action setting operations (SetAction)

### 3. Enable TRACE Logging

```bash
RUST_LOG=corint_runtime=trace cargo run --example fraud_detection
```

Shows detailed execution information, including:
- List of all instructions
- Execution of each instruction
- Detailed comparison operations
- Detailed jump instruction information

### 4. Global DEBUG Logging

```bash
RUST_LOG=debug cargo run --example fraud_detection
```

Shows debug logs from all modules.

### 5. Multiple Module Logging

```bash
RUST_LOG=corint_runtime=debug,corint_compiler=trace cargo run --example fraud_detection
```

## Log Output Examples

### DEBUG Level Output:
```
[2025-12-01T12:39:32.852768Z DEBUG corint_runtime::engine::pipeline_executor] Program has 12 instructions
[2025-12-01T12:39:32.852900Z DEBUG corint_runtime::engine::pipeline_executor] SetAction called with action: Approve
```

### TRACE Level Output:
```
[2025-12-01T12:39:48.272691Z TRACE corint_runtime::engine::pipeline_executor]   [0]: CheckEventType { expected: "transaction" }
[2025-12-01T12:39:48.272704Z TRACE corint_runtime::engine::pipeline_executor]   [1]: LoadField { path: ["ip_device_count"] }
[2025-12-01T12:39:48.272803Z TRACE corint_runtime::engine::pipeline_executor] Executing pc=0: CheckEventType { expected: "transaction" }
[2025-12-01T12:39:48.272817Z TRACE corint_runtime::engine::pipeline_executor] Compare Number(2.0) Gt Number(10.0)
[2025-12-01T12:39:48.272825Z TRACE corint_runtime::engine::pipeline_executor] Compare result: false
```

## Best Practices

1. **Development**: Use `RUST_LOG=debug` for debugging issues
2. **Detailed Debugging**: Use `RUST_LOG=trace` to track specific instruction execution
3. **Production**: Use the default `INFO` level or don't set `RUST_LOG`
4. **Performance Testing**: Don't set `RUST_LOG` to avoid logging overhead

## Using Logging in Code

Use tracing macros in Rust code:

```rust
use tracing::{debug, trace, info, warn, error};

// Debug information
tracing::debug!("Processing {} items", count);

// Detailed trace
tracing::trace!("Value: {:?}", value);

// General information
tracing::info!("Server started on port {}", port);

// Warning
tracing::warn!("Deprecated feature used");

// Error
tracing::error!("Failed to process: {}", error);
```
