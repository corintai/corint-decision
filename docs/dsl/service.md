# CORINT Risk Definition Language (RDL)
## Internal Service Integration Specification (v0.1)

This document defines how internal microservices and message queues are configured, invoked, and managed within CORINT's Risk Definition Language.

**Note:** For database and cache access, use **Datasources** (see `repository/configs/datasources/`). For third-party HTTP APIs, use **External APIs** (see `api.md`).

Internal services enable integration with:
- **HTTP microservices** (`ms_http`) - RESTful internal services
- **gRPC microservices** (`ms_grpc`) - gRPC internal services
- **Message queues** (`mq`) - Event streaming and async notifications

---

## 1. Service Type Overview

### 1.1 Service Types

| Type | Purpose | Protocol | Authentication |
|------|---------|----------|----------------|
| `ms_http` | Internal HTTP/REST microservices | HTTP/HTTPS | None (internal) |
| `ms_grpc` | Internal gRPC microservices | gRPC | None (internal) |
| `mq` | Message queue event streaming | Kafka, RabbitMQ | None (internal) |

### 1.2 Comparison with External Systems

| Aspect | Internal Service | External API | Datasource |
|--------|-----------------|--------------|------------|
| **Use Case** | Internal microservices, MQ | Third-party APIs | Database, Cache, Feature Store |
| **Network** | Internal network | Public internet | Internal (DB/Cache) |
| **Authentication** | None | Required (API keys, tokens) | Connection strings |
| **Configuration** | service.yaml | configs/apis/ | configs/datasources/ |
| **Retry Strategy** | Conservative (1-2) | Aggressive (3+) | Built-in |

### 1.3 When to Use What

- **Datasource** → Database queries, Redis cache, Feature engineering
- **External API** → Third-party HTTP APIs (IPInfo, fraud detection services)
- **Internal Service (ms_http)** → Internal HTTP microservices (KYC, scoring)
- **Internal Service (ms_grpc)** → Internal gRPC services (ML models, real-time scoring)
- **Internal Service (mq)** → Async event publishing (Kafka, RabbitMQ)

---

## 2. HTTP Microservice (`ms_http`)

### 2.1 Basic Structure

**Note**: `ms_http` follows the same structure as External API (see `api.md`) but **without authentication** since it's for internal services.

```yaml
services:
  - id: <string>                # Required: Unique service identifier
    type: ms_http               # Required: HTTP microservice type
    name: <string>              # Required: Human-readable name
    description: <string>       # Optional: Service description
    base_url: <string>          # Required: Base URL (e.g., http://service.internal:8080)
    timeout_ms: <integer>       # Optional: Default timeout in milliseconds (default: 5000)

    endpoints:                  # Required: Endpoint definitions
      <endpoint_name>:          # Endpoint identifier (key, not list item)
        method: <http_method>   # Required: GET | POST | PUT | PATCH | DELETE
        path: <string>          # Required: URL path, can include {placeholders}
        timeout_ms: <integer>   # Optional: Override default timeout for this endpoint
        params:                 # Optional: Parameter mapping from context
          <param_name>: ${<context_path>}  # e.g., user_id: ${event.user.id}
          <param_name>: <literal>          # e.g., api_version: "v1"
        query_params:           # Optional: Query parameter names
          - <param_name>
        request_body: <string>  # Optional: JSON template for POST/PUT/PATCH
                                # Use ${param_name} for substitution
        response:               # Optional: Response handling
          mapping:              # Optional: Field mapping/renaming
            <output_field>: <response_field>
          fallback:             # Optional: Default value on error
            <field>: <value>
```

### 2.2 Complete Example

```yaml
services:
  - id: kyc_service
    type: ms_http
    name: KYC Verification Service
    description: Internal KYC identity verification service
    base_url: http://kyc-service.internal:8080
    timeout_ms: 5000

    endpoints:
      # POST endpoint with request body
      verify_identity:
        method: POST
        path: /api/v1/verify/identity
        timeout_ms: 10000
        params:
          user_id: ${event.user.id}
          document_type: ${event.kyc.document_type}
          document_number: ${event.kyc.document_number}
        request_body: |
          {
            "user_id": "${user_id}",
            "document_type": "${document_type}",
            "document_number": "${document_number}"
          }
        response:
          mapping:
            is_verified: verified
            confidence_score: confidence
            level: verification_level
          fallback:
            is_verified: false
            confidence_score: 0.0
            level: "unverified"

      # GET endpoint with path placeholder
      get_status:
        method: GET
        path: /api/v1/status/{user_id}
        params:
          user_id: ${event.user.id}
        response:
          fallback:
            status: "unknown"
            verified: false

      # GET endpoint with query parameters
      search_users:
        method: GET
        path: /api/v1/users/search
        params:
          email: ${event.user.email}
          phone: ${event.user.phone}
          limit: 10
        query_params:
          - email
          - phone
          - limit
```

### 2.3 Usage in Pipeline

```yaml
pipeline:
  id: kyc_verification_flow
  name: KYC Verification Flow
  entry: verify_kyc

  steps:
    # Call KYC verification
    - step:
        id: verify_kyc
        type: service
        service: kyc_service
        endpoint: verify_identity
        # Parameters automatically read from context based on endpoint config
        # Result stored in: service.kyc_service.verify_identity
        next: check_status

    # Call with parameter override
    - step:
        id: check_status
        type: service
        service: kyc_service
        endpoint: get_status
        params:
          user_id: ${event.different_user_id}  # Override default mapping
        output: service.kyc_status  # Custom output location
        next: kyc_check

    # Use results in rules
    - step:
        id: kyc_check
        type: ruleset
        ruleset: kyc_verification_rules
```

### 2.4 Accessing Results

```yaml
rule:
  id: kyc_verified_check
  when:
    all:
      - service.kyc_service.verify_identity.is_verified == true
      - service.kyc_service.verify_identity.confidence_score > 0.8
  score: -20  # Reduce risk score if verified
```

---

## 3. gRPC Microservice (`ms_grpc`)

### 3.1 Basic Structure

```yaml
services:
  - id: <string>              # Required: Unique service identifier
    type: ms_grpc             # Required: gRPC microservice type
    name: <string>            # Required: Human-readable name
    description: <string>     # Optional: Service description

    connection:
      host: <string>          # Required: Service host
      port: <integer>         # Required: Service port
      discovery:              # Optional: Service discovery
        type: <discovery_type>  # consul | kubernetes | static
        service_name: <string>  # Service name in registry

    timeout_ms: <integer>     # Optional: Default timeout (default: 5000)

    methods:                  # Required: gRPC method definitions
      <method_name>:          # Method identifier (key, not list item)
        service: <string>     # Required: gRPC service name
        method: <string>      # Required: gRPC method name
        params:               # Optional: Parameter mapping
          <param_name>: ${<context_path>}
        response:             # Optional: Response handling
          mapping:
            <output_field>: <response_field>
          fallback:
            <field>: <value>
```

### 3.2 Complete Example

```yaml
services:
  - id: risk_scoring_service
    type: ms_grpc
    name: Risk Scoring Service
    description: Internal ML-based risk scoring service

    connection:
      host: risk-scoring.internal
      port: 9090
      discovery:
        type: consul
        service_name: risk-scoring

    timeout_ms: 5000

    methods:
      calculate_score:
        service: RiskScoringService
        method: CalculateScore
        params:
          user_id: ${event.user.id}
          transaction_amount: ${event.transaction.amount}
          features: ${features}
        response:
          mapping:
            score: risk_score
            factors: risk_factors
            version: model_version
          fallback:
            score: 0.0
            factors: []
            version: "unknown"

      get_model_info:
        service: RiskScoringService
        method: GetModelInfo
        params:
          model_id: "default"
```

### 3.3 Usage in Pipeline

```yaml
pipeline:
  id: risk_scoring_flow
  name: Risk Scoring Flow
  entry: calculate_risk

  steps:
    # Call gRPC service
    - step:
        id: calculate_risk
        type: service
        service: risk_scoring_service
        method: calculate_score
        # Result stored in: service.risk_scoring_service.calculate_score
        next: risk_evaluation

    # Use results
    - step:
        id: risk_evaluation
        type: ruleset
        ruleset: risk_assessment_rules
```

---

## 4. Message Queue (`mq`)

### 4.1 Basic Structure

```yaml
services:
  - id: <string>              # Required: Unique service identifier
    type: mq                  # Required: Message queue type
    name: <string>            # Required: Human-readable name
    description: <string>     # Optional: Service description

    connection:
      driver: <driver_type>   # Required: kafka | rabbitmq
      brokers:                # Required: Broker list
        - <broker_url>

    timeout_ms: <integer>     # Optional: Send timeout (default: 5000)

    topics:                   # Required: Topic definitions (topics must already exist)
      <topic_name>:           # Topic identifier (key, not list item)
        message:              # Required: Message template
          key: <string>       # Optional: Message key (supports ${} placeholders)
          value: <string>     # Required: JSON template for message payload
                              # Use ${placeholder} for substitution
                              # String values: "${placeholder}" (with quotes)
                              # Number/boolean values: ${placeholder} (without quotes)
```

### 4.2 Complete Example (Kafka)

```yaml
services:
  - id: event_bus
    type: mq
    name: Event Bus
    description: Kafka event streaming for risk decisions

    connection:
      driver: kafka
      brokers:
        - kafka-1.internal:9092
        - kafka-2.internal:9092
        - kafka-3.internal:9092

    timeout_ms: 5000

    topics:
      risk_decisions:
        message:
          key: ${event.user.id}
          value: |
            {
              "user_id": "${event.user.id}",
              "event_type": "${event.type}",
              "risk_score": ${vars.final_risk_score},
              "decision": "${vars.decision}",
              "timestamp": "${sys.timestamp}",
              "metadata": {
                "request_id": "${sys.request_id}",
                "pipeline_version": "v1.0"
              }
            }

      fraud_alerts:
        message:
          key: ${event.user.id}
          value: |
            {
              "alert_type": "high_risk",
              "user_id": "${event.user.id}",
              "risk_score": ${vars.risk_score},
              "triggered_rules": ${vars.triggered_rules},
              "timestamp": "${sys.timestamp}",
              "context": {
                "event_type": "${event.type}",
                "amount": ${event.transaction.amount}
              }
            }
```

### 4.3 Usage in Pipeline

```yaml
pipeline:
  id: fraud_detection_flow
  name: Fraud Detection Flow
  entry: risk_evaluation

  steps:
    # Execute risk evaluation
    - step:
        id: risk_evaluation
        type: ruleset
        ruleset: fraud_detection
        next: publish_decision

    # Publish decision to event bus (non-blocking by default)
    - step:
        id: publish_decision
        type: service
        service: event_bus
        topic: risk_decisions
```

---

## 5. Service Usage Patterns

### 5.1 Sequential Service Calls

```yaml
pipeline:
  id: sequential_service_example
  name: Sequential Service Calls Example
  entry: verify_kyc

  steps:
    # Step 1: Call KYC service
    - step:
        id: verify_kyc
        name: Verify KYC
        type: service
        service: kyc_service
        endpoint: verify_identity
        next: calculate_risk

    # Step 2: Call risk scoring service
    - step:
        id: calculate_risk
        name: Calculate Risk Score
        type: service
        service: risk_scoring_service
        method: calculate_score
        next: combined_check

    # Step 3: Use both results
    - step:
        id: combined_check
        name: Combined Verification
        type: ruleset
        ruleset: combined_verification_rules
```

### 5.2 Conditional Service Calls

```yaml
pipeline:
  id: conditional_service_example
  name: Conditional Service Calls Example
  entry: ml_risk_scoring

  steps:
    # Only call expensive service if needed
    - step:
        id: ml_risk_scoring
        name: ML Risk Scoring
        type: service
        service: risk_scoring_service
        method: calculate_score
        when:
          any:
            - event.transaction.amount > 10000
            - vars.basic_risk_score > 70
        next: final_decision

    - step:
        id: final_decision
        name: Final Decision
        type: ruleset
        ruleset: decision_rules
```

### 5.3 Event Publishing

```yaml
pipeline:
  id: event_publishing_example
  name: Event Publishing Example
  entry: risk_check

  steps:
    # Step 1: Evaluate risk
    - step:
        id: risk_check
        name: Risk Check
        type: ruleset
        ruleset: fraud_detection
        next: publish_event

    # Step 2: Publish to message queue (non-blocking by default)
    - step:
        id: publish_event
        name: Publish Event
        type: service
        service: event_bus
        topic: risk_decisions
        next: final_decision

    # Step 3: Continue execution immediately
    - step:
        id: final_decision
        name: Final Decision
        type: ruleset
        ruleset: decision_logic
```

---

## 6. Best Practices

### 6.1 Service Design

**Good**:
```yaml
services:
  - id: kyc_verification_service
    type: ms_http
    name: KYC Verification Service
    description: Internal identity verification service for user onboarding
    base_url: ${KYC_SERVICE_URL}  # Use environment variable

    endpoints:
      verify_identity:
        method: POST
        path: /api/v1/verify/identity
        timeout_ms: 10000  # Longer timeout for complex verification
        response:
          fallback:
            verified: false
            confidence: 0.0
```

**Avoid**:
```yaml
services:
  - id: svc1                    # ❌ Unclear name
    type: ms_http
    # ❌ No description
    base_url: http://localhost  # ❌ Hardcoded URL

    endpoints:
      endpoint1:                # ❌ Vague name
        method: POST
        path: /verify
        # ❌ No fallback handling
```

### 6.2 Reliability

- **Timeouts**: Set appropriate timeouts for each endpoint
- **Fallbacks**: Provide sensible fallback values for non-critical services
- **Message queues**: MQ publishing is non-blocking by default

### 6.3 Performance

- **Conditional calls**: Use `when` to avoid unnecessary service calls
- **Caching**: Cache frequently accessed data in Datasources
- **Connection pooling**: Configure appropriate connection pools (handled by runtime)

### 6.4 Maintainability

- **Descriptive names**: Use clear service and endpoint names
- **Documentation**: Add descriptions for all services and endpoints
- **Environment variables**: Use `${env.port}` for environment-specific configs
- **Version control**: Track service configuration changes
- **Monitoring**: Add logging and monitoring for all service calls

---

## 7. Related Documentation

- `api.md` - External third-party API integration (same HTTP format as `ms_http` + auth)
- `context.md` - Context and variable management
- `pipeline.md` - Service integration in pipelines
