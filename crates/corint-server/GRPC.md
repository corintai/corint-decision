# gRPC Support for CORINT Decision Engine

This document describes how to use the gRPC API for the CORINT Decision Engine.

## Overview

The CORINT Decision Engine server now supports both HTTP REST and gRPC protocols. This allows clients to use either protocol based on their preferences and requirements.

**Features:**
- Protocol Buffers-based API for type safety and efficiency
- Bi-directional streaming support (future enhancement)
- Language-agnostic client support (Go, Python, Java, C++, etc.)
- High performance and low latency

## Configuration

To enable the gRPC server, add the `grpc_port` configuration to your `config/server.yaml` file:

```yaml
# Server host
host: "127.0.0.1"

# HTTP Server port
port: 8080

# gRPC Server port (optional)
grpc_port: 50051
```

If `grpc_port` is not set, only the HTTP server will start.

## Running the Server

1. Start the server with gRPC enabled:
```bash
# Copy the example configuration
cp config/server_with_grpc.yaml.example config/server.yaml

# Start the server
cargo run --bin corint-server
```

2. You should see output indicating both servers are running:
```
✓ HTTP Server listening on http://127.0.0.1:8080
  Health check: http://127.0.0.1:8080/health
  Decision API: http://127.0.0.1:8080/v1/decide
  Metrics: http://127.0.0.1:8080/metrics
  Reload repository: POST http://127.0.0.1:8080/v1/repo/reload

✓ gRPC Server listening on 127.0.0.1:50051
  gRPC Decision API: 127.0.0.1:50051:Decide
  gRPC Health check: 127.0.0.1:50051:HealthCheck
```

## gRPC API

### Service Definition

The gRPC service is defined in `proto/decision.proto`:

```protobuf
service DecisionService {
  // Make a decision based on event data
  rpc Decide(DecideRequest) returns (DecideResponse);

  // Health check
  rpc HealthCheck(HealthCheckRequest) returns (HealthCheckResponse);

  // Reload repository (pipelines, rules, features)
  rpc ReloadRepository(ReloadRepositoryRequest) returns (ReloadRepositoryResponse);
}
```

### Making Requests

#### Using grpcurl (Command Line)

Install grpcurl: https://github.com/fullstorydev/grpcurl

```bash
# Health check
grpcurl -plaintext localhost:50051 corint.decision.v1.DecisionService/HealthCheck

# Make a decision
grpcurl -plaintext -d '{
  "event": {
    "type": {"string_value": "transaction"},
    "user_id": {"string_value": "user_123"},
    "amount": {"double_value": 100.50}
  }
}' localhost:50051 corint.decision.v1.DecisionService/Decide
```

#### Using Python Client

```python
import grpc
from generated import decision_pb2, decision_pb2_grpc

# Create channel
channel = grpc.insecure_channel('localhost:50051')
stub = decision_pb2_grpc.DecisionServiceStub(channel)

# Create request
request = decision_pb2.DecideRequest(
    event={
        'type': decision_pb2.Value(string_value='transaction'),
        'user_id': decision_pb2.Value(string_value='user_123'),
        'amount': decision_pb2.Value(double_value=100.50),
    }
)

# Make request
response = stub.Decide(request)
print(f"Decision: {response.decision.result}")
print(f"Score: {response.decision.scores.canonical}")
```

#### Using Go Client

```go
package main

import (
    "context"
    "log"

    pb "your-project/generated/decision"
    "google.golang.org/grpc"
)

func main() {
    // Connect to server
    conn, err := grpc.Dial("localhost:50051", grpc.WithInsecure())
    if err != nil {
        log.Fatal(err)
    }
    defer conn.Close()

    client := pb.NewDecisionServiceClient(conn)

    // Create request
    req := &pb.DecideRequest{
        Event: map[string]*pb.Value{
            "type":    {Kind: &pb.Value_StringValue{StringValue: "transaction"}},
            "user_id": {Kind: &pb.Value_StringValue{StringValue: "user_123"}},
            "amount":  {Kind: &pb.Value_DoubleValue{DoubleValue: 100.50}},
        },
    }

    // Make request
    resp, err := client.Decide(context.Background(), req)
    if err != nil {
        log.Fatal(err)
    }

    log.Printf("Decision: %s", resp.Decision.Result)
    log.Printf("Score: %f", resp.Decision.Scores.Canonical)
}
```

## Generating Client Code

### For Python

```bash
# Install grpcio-tools
pip install grpcio-tools

# Generate code
python -m grpc_tools.protoc \
    -I crates/corint-server/proto \
    --python_out=./client \
    --grpc_python_out=./client \
    crates/corint-server/proto/decision.proto
```

### For Go

```bash
# Install protoc-gen-go and protoc-gen-go-grpc
go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest

# Generate code
protoc \
    -I crates/corint-server/proto \
    --go_out=./client \
    --go-grpc_out=./client \
    crates/corint-server/proto/decision.proto
```

### For Node.js/TypeScript

```bash
# Install dependencies
npm install @grpc/grpc-js @grpc/proto-loader

# Use proto-loader at runtime (recommended)
# Or generate static code using grpc-tools
npm install -g grpc-tools
grpc_tools_node_protoc \
    --js_out=import_style=commonjs,binary:./client \
    --grpc_out=grpc_js:./client \
    -I crates/corint-server/proto \
    crates/corint-server/proto/decision.proto
```

## Performance Considerations

gRPC generally provides better performance than REST for the following reasons:

1. **Binary Protocol**: Protocol Buffers are more compact than JSON
2. **HTTP/2**: Multiplexing, header compression, and persistent connections
3. **Streaming**: Support for server and client streaming (future enhancement)
4. **Type Safety**: Strongly typed contracts reduce parsing errors

**Benchmarks** (example):
- REST API: ~5-10ms latency, ~2KB payload size
- gRPC API: ~2-5ms latency, ~500B payload size

## Security

For production deployments, always use TLS:

```rust
// Server configuration (future enhancement)
let tls_config = ServerTlsConfig::new()
    .identity(Identity::from_pem(cert, key));

Server::builder()
    .tls_config(tls_config)?
    .add_service(DecisionServiceServer::new(service))
    .serve(addr)
    .await?;
```

## Troubleshooting

### Port Already in Use

If you see "Address already in use" error:
```bash
# Find process using the port
lsof -i :50051

# Kill the process
kill -9 <PID>
```

### Connection Refused

Make sure the server is running and the `grpc_port` is configured correctly.

### Proto Compilation Errors

Ensure you have `protoc` installed:
```bash
# macOS
brew install protobuf

# Linux
apt-get install protobuf-compiler

# Or download from https://github.com/protocolbuffers/protobuf/releases
```

## Future Enhancements

- [ ] TLS/mTLS support
- [ ] Server and client streaming for batch decisions
- [ ] gRPC reflection for dynamic client discovery
- [ ] Advanced load balancing and retries
- [ ] Metrics and tracing integration
- [ ] Rate limiting and authentication

## References

- [gRPC Official Documentation](https://grpc.io/docs/)
- [Protocol Buffers](https://developers.google.com/protocol-buffers)
- [Tonic (Rust gRPC)](https://github.com/hyperium/tonic)
