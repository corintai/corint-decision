# CORINT Decision Engine - Node.js/TypeScript Bindings

Native Node.js bindings for the CORINT Decision Engine, built with napi-rs.

## Features

- Native performance using Rust
- Full async/await support
- Automatic TypeScript type definitions
- Compatible with Node.js v18+ (including Node.js v23)
- Cross-platform support (macOS, Linux, Windows)

## Installation

```bash
npm install
npm run build
```

## Quick Start

### Running Tests

```bash
# Test with repository loading
node test_repo.js

# Run JavaScript example
npm run example

# Run TypeScript example
npm run example:ts
```

### JavaScript Example

```javascript
const { Engine, version } = require('@corint/decision-engine');

async function main() {
  console.log('Version:', version());

  // Load engine from file system repository
  const engine = await Engine.fromRepository('./repository');

  // Execute decision
  const responseJson = await engine.decideSimple(JSON.stringify({
    type: 'payment',
    payment_amount: 1500,
    ip_address: '192.168.1.1'
  }));

  const response = JSON.parse(responseJson);
  console.log('Decision:', response.decision);
  console.log('Actions:', response.actions);
}

main().catch(console.error);
```

### TypeScript Example

```typescript
import { Engine, version } from '@corint/decision-engine';

async function main() {
  console.log('Version:', version());

  const engine = await Engine.fromRepository('./repository');

  const responseJson = await engine.decideSimple(JSON.stringify({
    type: 'payment',
    payment_amount: 1500
  }));

  const response = JSON.parse(responseJson);
  console.log('Decision:', response.decision);
}

main();
```

## API Reference

### `version(): string`

Returns the version of the CORINT Decision Engine.

```javascript
const { version } = require('@corint/decision-engine');
console.log(version()); // "0.1.0"
```

### `Engine.fromRepository(repositoryPath: string): Promise<Engine>`

Create a new Decision Engine instance from a file system repository.

**Parameters:**
- `repositoryPath` - Path to the rule repository directory

**Returns:** Promise resolving to an Engine instance

```javascript
const engine = await Engine.fromRepository('./repository');
```

### `Engine.fromYaml(pipelineId: string, yamlContent: string): Promise<Engine>`

Create a new Decision Engine instance with inline YAML content.

**Note:** The YAML content must follow the CORINT pipeline format with proper structure.

**Parameters:**
- `pipelineId` - Unique identifier for the pipeline
- `yamlContent` - Complete YAML content of the pipeline

**Returns:** Promise resolving to an Engine instance

```javascript
const yaml = `
version: "0.1"

---

pipeline:
  id: fraud_check
  name: Fraud Check Pipeline
  entry: check_amount

  when:
    all:
      - event.amount > 0

  steps:
    - step:
        id: check_amount
        name: Amount Check
        type: rule_inline
        rule:
          when:
            all:
              - event.amount > 1000
          outcomes:
            - high_amount
          actions:
            - type: block
              reason: "High transaction amount"
`;

const engine = await Engine.fromYaml('fraud_check', yaml);
```

### `engine.decideSimple(eventDataJson: string): Promise<string>`

Execute a decision with simple event data.

**Parameters:**
- `eventDataJson` - Event data as JSON string

**Returns:** Promise resolving to decision response as JSON string

```javascript
const eventData = {
  user_id: 'user_123',
  amount: 5000,
};

const responseJson = await engine.decideSimple(JSON.stringify(eventData));
const response = JSON.parse(responseJson);
```

### `engine.decide(requestJson: string): Promise<string>`

Execute a decision with full request format (including features, API results, metadata, options).

**Parameters:**
- `requestJson` - Full decision request as JSON string

**Returns:** Promise resolving to decision response as JSON string

```javascript
const request = {
  event_data: {
    user_id: 'user_123',
    amount: 5000,
  },
  features: {
    user_risk_score: 0.75,
  },
  metadata: {
    request_id: 'req_123',
  },
  options: {
    enable_trace: true,
  },
};

const responseJson = await engine.decide(JSON.stringify(request));
const response = JSON.parse(responseJson);
```

## Request Format

### Simple Request (decideSimple)

```json
{
  "user_id": "user_123",
  "transaction_amount": 5000,
  "transaction_currency": "USD"
}
```

### Full Request (decide)

```json
{
  "event_data": {
    "user_id": "user_123",
    "transaction_amount": 5000
  },
  "features": {
    "user_risk_score": 0.75
  },
  "api": {
    "fraud_score": 0.85
  },
  "metadata": {
    "request_id": "req_123",
    "source": "api"
  },
  "options": {
    "enable_trace": true
  }
}
```

## Response Format

```json
{
  "decision": "block",
  "outcomes": ["high_risk"],
  "actions": [
    {
      "type": "block",
      "reason": "High fraud score"
    }
  ],
  "trace": {
    "pipeline_id": "fraud_check",
    "execution_time_ms": 5
  }
}
```

## Development

### Build

```bash
# Debug build
npm run build:debug

# Release build
npm run build
```

### Run Examples

```bash
# JavaScript example
npm run example

# TypeScript example
npm run example:ts
```

### Publishing

```bash
npm run prepublishOnly
```

## Platform Support

- macOS (ARM64, x64)
- Linux (x64, ARM64)
- Windows (x64)

## Requirements

- Node.js >= 18
- Rust toolchain (for building from source)

## Troubleshooting

### "Failed to create engine: Invalid rule file"

This error occurs when the YAML content doesn't match the expected pipeline format. Ensure your YAML follows this structure:

```yaml
version: "0.1"

---

pipeline:
  id: your_pipeline_id
  name: Your Pipeline Name
  entry: first_step_id

  when:
    all:
      - condition1

  steps:
    - step:
        id: first_step_id
        # ... step configuration
```

### Repository Loading Issues

When using `fromRepository()`, ensure:
- The path points to a valid directory
- The directory contains a `registry.yaml` file
- Pipeline files are in the `pipelines/` subdirectory
- All imported rulesets/rules exist in the `library/` subdirectory

## License

Elastic-2.0

## Links

- [CORINT Decision Engine](https://github.com/corint/corint-decision)
- [napi-rs](https://napi.rs)
