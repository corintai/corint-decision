# CORINT Decision Engine Server

HTTP/REST API server for CORINT Decision Engine.

## Features

- **REST API**: Execute decision rules via HTTP endpoints
- **Health Check**: Monitor server health status
- **Auto Rule Loading**: Automatically loads rules from configured directory
- **CORS Support**: Cross-origin resource sharing enabled
- **Request Tracing**: Built-in request/response tracing

## Quick Start

### Build and Run

```bash
# Build the server
cargo build --release --bin corint-server

# Run the server
cargo run --bin corint-server
```

### Configuration

The server can be configured via:

1. **Environment Variables** (prefix: `CORINT_`)
2. **Config File** (`config/server.yaml`)
3. **`.env` File**

#### Environment Variables

```bash
CORINT_HOST=0.0.0.0
CORINT_PORT=8080
CORINT_RULES_DIR=examples/rules
CORINT_ENABLE_METRICS=true
CORINT_ENABLE_TRACING=true
CORINT_LOG_LEVEL=info
```

#### Config File Example

Create `config/server.yaml`:

```yaml
host: "0.0.0.0"
port: 8080
rules_dir: "examples/rules"
enable_metrics: true
enable_tracing: true
log_level: "info"
```

## API Endpoints

### Health Check

**GET** `/health`

Returns server health status.

**Response:**

```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

### Make Decision

**POST** `/v1/decide`

Execute decision rules with event data.

**Request Body:**

```json
{
  "event_data": {
    "user_id": "user_001",
    "device_id": "device_001",
    "ip_address": "203.0.113.1",
    "event.type": "transaction",
    "event.user_id": "user_001",
    "event.device_id": "device_001",
    "event.ip_address": "203.0.113.1",
    "event.event_type": "transaction"
  }
}
```

**Response:**

```json
{
  "action": "Approve",
  "score": 35,
  "triggered_rules": ["high_value_transaction"],
  "explanation": "Transaction approved with moderate risk",
  "processing_time_ms": 125
}
```


## Usage Examples

### Using cURL

```bash
# Health check
curl http://localhost:8080/health

# Make a decision
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
      "user_id": "user_001",
      "event.type": "transaction",
      "event.user_id": "user_001",
      "transaction_amount": 100.0
    }
  }'
```

### Using HTTPie

```bash
# Health check
http GET :8080/health

# Make a decision
http POST :8080/v1/decide \
  event_data:='{"user_id": "user_001", "event.type": "transaction"}'
```

### Using Python

```python
import requests
import json

# Health check
response = requests.get("http://localhost:8080/health")
print(response.json())

# Make a decision
event_data = {
    "user_id": "user_001",
    "device_id": "device_001",
    "event.type": "transaction",
    "event.user_id": "user_001"
}

response = requests.post(
    "http://localhost:8080/v1/decide",
    json={"event_data": event_data}
)

result = response.json()
print(f"Action: {result['action']}")
print(f"Score: {result['score']}")
print(f"Triggered Rules: {result['triggered_rules']}")
```

## Development

### Run with Custom Log Level

```bash
RUST_LOG=debug cargo run --bin corint-server
```

### Run with Custom Port

```bash
CORINT_PORT=9090 cargo run --bin corint-server
```

### Testing

```bash
# Run unit tests
cargo test --package corint-server

# Run with test coverage
cargo tarpaulin --package corint-server
```

## Architecture

```
┌─────────────────────────────────────────┐
│         HTTP Request                     │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│    Axum Router (Tower Middleware)       │
│    - CORS Layer                         │
│    - Trace Layer                        │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│       REST API Handler                  │
│    - Parse request                      │
│    - Validate input                     │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│      Decision Engine (SDK)              │
│    - Load rules                         │
│    - Execute decision                   │
│    - Calculate features (lazy)          │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│         HTTP Response                   │
└─────────────────────────────────────────┘
```

## Performance

- **Lazy Feature Calculation**: Features are calculated on-demand during rule execution
- **Feature Caching**: Calculated features are cached for subsequent rule evaluations
- **Async I/O**: Non-blocking database queries and API calls
- **Connection Pooling**: Database connections are pooled for efficiency

## Deployment

### Docker (Future)

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin corint-server

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/corint-server /usr/local/bin/
EXPOSE 8080
CMD ["corint-server"]
```

### Systemd Service

```ini
[Unit]
Description=CORINT Decision Engine Server
After=network.target

[Service]
Type=simple
User=corint
WorkingDirectory=/opt/corint
ExecStart=/opt/corint/corint-server
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

## Monitoring

### Metrics (Placeholder)

Future versions will expose Prometheus metrics at `/metrics`.

### Health Check

Use `/health` endpoint for liveness and readiness probes.

## Troubleshooting

### Server won't start

1. Check if port is already in use: `lsof -i :8080`
2. Verify rules directory exists: `ls -la examples/rules`
3. Check logs for detailed error messages

### Rules not loading

1. Ensure rules directory is correct in config
2. Verify YAML files have `.yaml` or `.yml` extension
3. Check file permissions

### Decision returns errors

1. Verify event_data format matches rule expectations
2. Check that all required fields are present
3. Review rule definitions for syntax errors

## License

Elastic-2.0

