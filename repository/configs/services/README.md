# Internal Service Configurations

This directory contains configuration files for internal microservices and message queues used by CORINT decision engine.

## Service Types

### `ms_http` - Internal HTTP Microservices
RESTful internal services accessed via HTTP/HTTPS protocol.

**Example**: `kyc_service.yaml`
- KYC identity verification
- User profile services
- Internal REST APIs

### `ms_grpc` - Internal gRPC Microservices
High-performance RPC services using gRPC protocol.

**Example**: `risk_scoring_service.yaml`
- ML model scoring
- Real-time analytics
- Performance-critical services

### `mq` - Message Queues
Event streaming and async notifications via Kafka or RabbitMQ.

**Example**: `event_bus.yaml`
- Risk decision logging
- Fraud alerts
- User activity tracking

## Configuration Structure

### ms_http Configuration
```yaml
services:
  - id: service_name
    type: ms_http
    base_url: http://service.internal:8080
    endpoints:
      endpoint_name:
        method: POST|GET|PUT|DELETE|PATCH
        path: /api/v1/path
        params: { ... }
        request_body: |
          { ... }
        response:
          mapping: { ... }
          fallback: { ... }
```

### ms_grpc Configuration
```yaml
services:
  - id: service_name
    type: ms_grpc
    connection:
      host: service.internal
      port: 9090
    methods:
      method_name:
        service: GrpcServiceName
        method: MethodName
        params: { ... }
        response:
          mapping: { ... }
          fallback: { ... }
```

### mq Configuration
```yaml
services:
  - id: queue_name
    type: mq
    connection:
      driver: kafka|rabbitmq
      brokers:
        - broker1:9092
        - broker2:9092
    topics:
      topic_name:
        message:
          key: ${event.user.id}
          value: |
            { ... JSON template ... }
```

## Usage in Pipelines

### Calling HTTP Microservice
```yaml
pipeline:
  steps:
    - step:
        id: verify_kyc
        type: service
        service: kyc_service
        endpoint: verify_identity
        next: next_step
```

### Calling gRPC Microservice
```yaml
pipeline:
  steps:
    - step:
        id: calculate_risk
        type: service
        service: risk_scoring_service
        method: calculate_score
        next: next_step
```

### Publishing to Message Queue
```yaml
pipeline:
  steps:
    - step:
        id: publish_event
        type: service
        service: event_bus
        topic: risk_decisions
        next: next_step
```

## Key Differences from External APIs

| Aspect | Internal Service | External API |
|--------|-----------------|--------------|
| **Authentication** | None (trusted network) | Required (API keys, tokens) |
| **Network** | Internal only | Public internet |
| **Config Location** | `configs/services/` | `configs/apis/` |
| **Timeout Default** | 5000ms | 10000ms |

## Related Documentation

- [service.md](../../../docs/dsl/service.md) - Complete specification
- [external.md](../../../docs/dsl/external.md) - External API integration
- [pipeline.md](../../../docs/dsl/pipeline.md) - Pipeline orchestration
