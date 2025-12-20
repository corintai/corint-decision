# CORINT FFI - Foreign Function Interface

Multi-language bindings for the CORINT Decision Engine, enabling integration with Python, Go, TypeScript/Node.js, and Java.

## Overview

CORINT FFI provides C-compatible interfaces that allow calling the CORINT Decision Engine from various programming languages. This enables organizations to integrate fraud detection and risk management capabilities into applications written in their preferred language.

## Supported Languages

- **Python** - Using ctypes
- **Go** - Using cgo
- **TypeScript/Node.js** - Using ffi-napi
- **Java** - Using JNA (Java Native Access)

## Building the FFI Library

First, build the CORINT FFI library:

```bash
# Debug build
cargo build --package corint-ffi

# Release build (recommended for production)
cargo build --package corint-ffi --release
```

This will generate the shared library in `target/debug/` or `target/release/`:
- macOS: `libcorint_ffi.dylib`
- Linux: `libcorint_ffi.so`
- Windows: `corint_ffi.dll`

## Language-Specific Usage

### Python

#### Installation

No installation required - just ensure the FFI library is built.

#### Usage

```python
from corint import DecisionEngine, DecisionRequest

# Create engine
engine = DecisionEngine(repository_path="repository")

# Create request
request = DecisionRequest(
    event_data={
        "user_id": "user123",
        "amount": 1000.0
    },
    enable_trace=True
)

# Execute decision
response = engine.decide(request)
print(f"Decision: {response.decision}")
print(f"Actions: {response.actions}")

engine.close()
```

#### Running the Example

```bash
cd crates/corint-ffi/bindings/python
python3 example.py
```

### Go

#### Installation

The Go bindings use cgo to interface with the FFI library.

#### Usage

```go
package main

import (
    "github.com/corint/corint-go"
)

func main() {
    // Create engine
    engine, err := corint.NewEngine("repository")
    if err != nil {
        panic(err)
    }
    defer engine.Close()

    // Create request
    request := &corint.DecisionRequest{
        EventData: map[string]interface{}{
            "user_id": "user123",
            "amount":  1000.0,
        },
        Options: corint.DecisionOptions{
            EnableTrace: true,
        },
    }

    // Execute decision
    response, err := engine.Decide(request)
    if err != nil {
        panic(err)
    }

    println("Decision:", response.Decision)
}
```

#### Running the Example

```bash
cd crates/corint-ffi/bindings/go/example
go run main.go
```

### TypeScript/Node.js

#### Installation

```bash
cd crates/corint-ffi/bindings/typescript
npm install
npm run build
```

#### Usage

```typescript
import { DecisionEngine, DecisionRequest } from '@corint/decision-engine';

// Create engine
const engine = new DecisionEngine({ repositoryPath: 'repository' });

try {
    // Create request
    const request: DecisionRequest = {
        event_data: {
            user_id: 'user123',
            amount: 1000.0
        },
        options: {
            enableTrace: true
        }
    };

    // Execute decision
    const response = engine.decide(request);
    console.log(`Decision: ${response.decision}`);
    console.log(`Actions: ${JSON.stringify(response.actions)}`);
} finally {
    engine.close();
}
```

#### Running the Example

```bash
cd crates/corint-ffi/bindings/typescript
npm install
npx ts-node example.ts
```

### Java

#### Installation

The Java bindings use Maven. Build with:

```bash
cd crates/corint-ffi/bindings/java
mvn clean install
```

#### Usage

```java
import com.corint.DecisionEngine;
import com.corint.DecisionRequest;
import com.corint.DecisionResponse;

import java.util.HashMap;
import java.util.Map;

public class Example {
    public static void main(String[] args) {
        // Create engine
        try (DecisionEngine engine = new DecisionEngine("repository")) {
            // Create request
            Map<String, Object> eventData = new HashMap<>();
            eventData.put("user_id", "user123");
            eventData.put("amount", 1000.0);

            DecisionRequest request = new DecisionRequest(eventData);
            request.getOptions().setEnableTrace(true);

            // Execute decision
            DecisionResponse response = engine.decide(request);
            System.out.println("Decision: " + response.getDecision());
            System.out.println("Actions: " + response.getActions());
        }
    }
}
```

#### Running the Example

```bash
cd crates/corint-ffi/bindings/java
mvn compile
mvn exec:java -Dexec.mainClass="com.corint.Example"
```

## API Reference

### Core Functions

All language bindings expose the following core functionality:

#### `version()` / `Version()`
Get the CORINT library version.

#### `initLogging()` / `InitLogging()`
Initialize the logging system.

#### `DecisionEngine(repositoryPath)` / `NewEngine(repositoryPath)`
Create a decision engine from a file system repository.

#### `DecisionEngine.fromDatabase(databaseUrl)` / `NewEngineFromDatabase(databaseUrl)`
Create a decision engine from a PostgreSQL database.

#### `engine.decide(request)`
Execute a decision with the given request.

#### `engine.close()` / `engine.Close()`
Close the engine and free resources.

### DecisionRequest Structure

```json
{
  "event_data": {
    "user_id": "string",
    "amount": "number",
    ...
  },
  "features": { ... },      // Optional
  "api": { ... },          // Optional
  "service": { ... },      // Optional
  "llm": { ... },          // Optional
  "vars": { ... },         // Optional
  "metadata": { ... },     // Optional
  "options": {
    "enable_trace": false  // Enable execution trace
  }
}
```

### DecisionResponse Structure

```json
{
  "decision": "approve|decline|review",
  "actions": [...],
  "trace": { ... },        // If enabled
  "metadata": { ... }
}
```

## Configuration

### Repository Modes

**File System Repository:**
```
DecisionEngine(repositoryPath: "path/to/repository")
```

**Database Repository:**
```
DecisionEngine.fromDatabase(databaseUrl: "postgresql://...")
```

### Environment Variables

- `RUST_LOG` - Set logging level (e.g., `RUST_LOG=info`)
- `DATABASE_URL` - Default database URL for database mode

## Performance Considerations

1. **Reuse Engine Instances**: Creating an engine is expensive. Reuse the same instance for multiple decisions.

2. **Connection Pooling**: When using database mode, the engine maintains an internal connection pool.

3. **Memory Management**: Always call `close()` / `Close()` when done with an engine to free resources.

4. **Async vs Sync**: The FFI interface is synchronous. For async applications, wrap calls in your language's async primitives.

## Troubleshooting

### Library Not Found

If you get a "library not found" error:

1. Ensure the FFI library is built: `cargo build --package corint-ffi --release`
2. Check the library is in the expected location (`target/release/`)
3. For Linux, you may need to set `LD_LIBRARY_PATH`
4. For macOS, you may need to set `DYLD_LIBRARY_PATH`

### Version Mismatch

Ensure the FFI library and language bindings are from the same CORINT version.

### Memory Issues

If you encounter memory leaks or crashes:

1. Ensure `close()` is called for all engines
2. Don't use an engine after calling `close()`
3. Check that all strings returned by FFI functions are properly freed

## Security Considerations

1. **Input Validation**: Always validate input data before passing to the engine
2. **Database Credentials**: Use secure methods to manage database URLs (environment variables, secret managers)
3. **Library Loading**: Ensure the FFI library is loaded from a trusted location
4. **Resource Limits**: Set appropriate timeouts and resource limits for decision execution

## License

This project is licensed under the Elastic License 2.0.

## Support

For issues or questions:
- File an issue on GitHub
- Check the main CORINT documentation
- Contact the CORINT team
