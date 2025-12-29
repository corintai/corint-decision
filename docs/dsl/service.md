# CORINT Risk Definition Language (RDL)
## Internal Service Integration Specification (v0.1)

This document defines how internal services (databases, caches, microservices, message queues, etc.) are configured, invoked, and managed within CORINT's Risk Definition Language.

Internal services enable integration with:
- **Database services** (MySQL, PostgreSQL, MongoDB, Cassandra)
- **Cache services** (Redis, Memcached)
- **Feature Store** (pre-computed features)
- **Internal microservices** (User Service, Account Service, KYC Service)
- **Message queues** (Kafka, RabbitMQ)
- **Search services** (Elasticsearch)

---

## 1. Service vs External API

### 1.1 Key Differences

| Aspect | Internal Service | External API |
|--------|-----------------|--------------|
| **Network** | Internal network | Public internet |
| **Reliability** | High (99.9%+) | Variable |
| **Latency** | Low (< 10ms) | High (50-500ms) |
| **Authentication** | Simple internal auth | Complex (OAuth, API Key) |
| **Retry Strategy** | Conservative (1-2 retries) | Aggressive (3+ retries) |
| **Circuit Breaker** | Optional | Recommended |
| **Caching** | Short TTL or none | Long TTL |
| **Cost Model** | Internal resource | Pay-per-call |

### 1.2 Design Philosophy

Internal services prioritize:
- **Performance** - Low latency, high throughput
- **Consistency** - Strong consistency guarantees
- **Simplicity** - Minimal configuration overhead
- **Reliability** - Fail fast, clear error messages

---

## 2. Service Definition Structure

### 2.1 Basic Structure

```yaml
services:
  - id: string                      # Unique identifier
    name: string                    # Human-readable name
    type: database | cache | microservice | mq | search | feature_store
    description: string             # Purpose description

    connection: <connection-config> # Connection settings
    timeout: <timeout-config>       # Timeout settings
    retry: <retry-config>           # Retry policy (optional)
    cache: <cache-config>           # Result caching (optional)
    on_error: <error-config>        # Error handling
```

---

## 3. Database Services

### 3.1 Relational Database (MySQL/PostgreSQL)

```yaml
services:
  - id: user_db
    name: User Database
    type: database
    description: Primary user data store

    connection:
      driver: postgresql              # postgresql | mysql
      host: ${DB_HOST}
      port: 5432
      database: corint_users
      username: ${DB_USER}
      password: ${DB_PASSWORD}

      # Connection pool
      pool:
        min_size: 5
        max_size: 20
        idle_timeout: 300000          # ms

      # SSL/TLS
      ssl:
        enabled: true
        verify_cert: true

    # Query definitions
    queries:
      - name: get_user_profile
        sql: |
          SELECT user_id, tier, risk_score, kyc_verified, created_at
          FROM users
          WHERE user_id = :user_id
        params:
          - name: user_id
            type: string
            source: event.user.id
        cache:
          enabled: true
          ttl: 300                    # 5 minutes

      - name: get_user_transactions
        sql: |
          SELECT transaction_id, amount, currency, created_at
          FROM transactions
          WHERE user_id = :user_id
            AND created_at >= :start_date
          ORDER BY created_at DESC
          LIMIT :limit
        params:
          - name: user_id
            type: string
            source: event.user.id
          - name: start_date
            type: datetime
            source: vars.query_start_date
          - name: limit
            type: integer
            default: 100

      - name: count_failed_logins
        sql: |
          SELECT COUNT(*) as count
          FROM login_attempts
          WHERE user_id = :user_id
            AND success = false
            AND created_at >= NOW() - INTERVAL :window
        params:
          - name: user_id
            type: string
            source: event.user.id
          - name: window
            type: string
            default: "24 hours"

    timeout:
      query: 5000                     # ms
      connection: 2000                # ms

    retry:
      max_attempts: 2
      backoff: linear
      retry_on:
        - connection_timeout
        - read_timeout

    on_error:
      action: fail
      log_level: error
```

### 3.2 NoSQL Database (MongoDB)

```yaml
services:
  - id: session_store
    name: Session Store
    type: database
    description: User session data store

    connection:
      driver: mongodb
      uri: ${MONGODB_URI}
      database: sessions

      options:
        read_preference: primary
        write_concern: majority

    queries:
      - name: get_session
        collection: user_sessions
        operation: find_one
        filter:
          user_id: "{{ event.user.id }}"
          expires_at: { $gt: "{{ sys.timestamp }}" }
        projection:
          session_id: 1
          device_id: 1
          ip_address: 1
          created_at: 1

      - name: get_user_devices
        collection: user_devices
        operation: find
        filter:
          user_id: "{{ event.user.id }}"
        sort:
          last_seen: -1
        limit: 10
```

### 3.3 Using Database Queries in Pipeline

```yaml
pipeline:
  # Database query step
  - type: service
    id: load_user_profile
    service: user_db
    query: get_user_profile

    # Override parameters
    params:
      user_id: event.user.id

    # Store result in context
    output: context.user_profile

  # Use result in conditions
  - type: rules
    id: check_user_tier
    conditions:
      - context.user_profile.tier == "premium"
      - context.user_profile.kyc_verified == true
```

---

## 4. Cache Services

### 4.1 Redis Cache

```yaml
services:
  - id: redis_cache
    name: Redis Cache
    type: cache
    description: Primary caching layer

    connection:
      driver: redis
      host: ${REDIS_HOST}
      port: 6379
      password: ${REDIS_PASSWORD}
      db: 0

      pool:
        min_size: 5
        max_size: 50

      options:
        connect_timeout: 1000         # ms
        socket_timeout: 2000          # ms

    operations:
      - name: get_user_risk_cache
        operation: get
        key: "risk:user:{{ event.user.id }}"
        deserialize: json

      - name: set_user_risk_cache
        operation: set
        key: "risk:user:{{ event.user.id }}"
        value: "{{ context.risk_result }}"
        ttl: 3600                     # seconds
        serialize: json

      - name: get_ip_reputation
        operation: get
        key: "ip:reputation:{{ event.geo.ip }}"
        deserialize: json

      - name: incr_login_attempts
        operation: incr
        key: "login:attempts:{{ event.user.id }}"
        expire: 3600

      - name: get_device_trust_score
        operation: hget
        key: "device:trust"
        field: "{{ event.device.id }}"

    timeout:
      command: 1000                   # ms

    on_error:
      action: skip                    # Cache miss is not fatal
      log_level: warn
```

### 4.2 Cache Patterns

```yaml
pipeline:
  # Cache-aside pattern
  - type: service
    id: check_cache
    service: redis_cache
    operation: get_user_risk_cache
    output: context.cached_risk

  - branch:
      when:
        # If cache hit, use cached value
        - condition: "context.cached_risk exists"
          pipeline:
            - type: action
              decision: context.cached_risk

        # If cache miss, compute and cache
        - condition: "context.cached_risk missing"
          pipeline:
            - type: rules
              id: compute_risk
              output: context.computed_risk

            - type: service
              id: update_cache
              service: redis_cache
              operation: set_user_risk_cache
              params:
                value: context.computed_risk
```

---

## 5. Feature Store Services

### 5.1 Feature Store Definition

```yaml
services:
  - id: feature_store
    name: Feature Store
    type: feature_store
    description: Pre-computed risk features

    connection:
      backend: redis                  # redis | dynamodb | custom
      host: ${FEATURE_STORE_HOST}
      port: 6379

    features:
      - name: user_behavior_7d
        key_template: "features:user:{{ event.user.id }}:7d"
        schema:
          login_count: integer
          transaction_count: integer
          avg_transaction_amount: float
          devices_used: integer
          ips_used: integer
        ttl: 3600

      - name: user_association_5h
        key_template: "features:assoc:{{ event.user.id }}:5h"
        schema:
          devices_per_ip: integer
          users_per_device: integer
          users_per_ip: integer
        ttl: 300

      - name: device_profile
        key_template: "features:device:{{ event.device.id }}"
        schema:
          trust_score: float
          first_seen: datetime
          last_seen: datetime
          users_count: integer
        ttl: 1800

    on_miss:
      action: compute                 # compute | fail | skip
      fallback_ttl: 60
```

### 5.2 Using Feature Store

```yaml
pipeline:
  # Load pre-computed features
  - type: service
    id: load_features
    service: feature_store
    features:
      - user_behavior_7d
      - user_association_5h
      - device_profile

    output: context.features

  # Use features in rules
  - type: rules
    id: feature_based_rules
    conditions:
      - context.features.user_behavior_7d.login_count > 100
      - context.features.user_association_5h.devices_per_ip > 10
      - context.features.device_profile.trust_score < 0.5
```

---

## 6. Internal Microservices

### 6.1 RESTful Microservice

```yaml
services:
  - id: kyc_service
    name: KYC Service
    type: microservice
    description: Internal KYC verification service

    connection:
      protocol: http
      base_url: http://kyc-service.internal:8080
      version: v1

      # Internal authentication
      auth:
        type: internal_token
        token: ${INTERNAL_SERVICE_TOKEN}

    endpoints:
      - name: verify_identity
        path: /verify/identity
        method: POST

        request:
          headers:
            Content-Type: application/json
          body:
            user_id: "{{ event.user.id }}"
            document_type: "{{ event.kyc.document_type }}"
            document_number: "{{ event.kyc.document_number }}"

        response:
          verified: boolean
          confidence: float
          verification_level: string

      - name: get_verification_status
        path: /status/{user_id}
        method: GET

        params:
          - name: user_id
            type: string
            source: event.user.id

    timeout:
      connect: 500
      read: 3000

    retry:
      max_attempts: 2
      backoff: exponential
```

### 6.2 gRPC Microservice

```yaml
services:
  - id: risk_scoring_service
    name: Risk Scoring Service
    type: microservice
    description: Internal ML-based risk scoring

    connection:
      protocol: grpc
      host: risk-scoring.internal
      port: 9090

      # Service discovery (optional)
      discovery:
        type: consul                  # consul | kubernetes | static
        service_name: risk-scoring

    methods:
      - name: calculate_risk_score
        service: RiskScoringService
        method: CalculateScore

        request:
          user_id: "{{ event.user.id }}"
          transaction_amount: "{{ event.transaction.amount }}"
          features: "{{ context.features }}"

        response:
          risk_score: float
          risk_factors: array
          model_version: string
```

---

## 7. Message Queue Services

### 7.1 Kafka Producer

```yaml
services:
  - id: event_bus
    name: Event Bus
    type: message_queue
    description: Kafka event streaming

    connection:
      driver: kafka
      brokers:
        - kafka-1.internal:9092
        - kafka-2.internal:9092
        - kafka-3.internal:9092

      producer:
        acks: 1                       # 0 | 1 | all
        compression: snappy
        batch_size: 16384
        linger_ms: 10

    topics:
      - name: risk_decisions
        partitions: 12
        replication_factor: 3

        message:
          key: "{{ event.user.id }}"
          value:
            user_id: "{{ event.user.id }}"
            event_type: "{{ event.type }}"
            risk_score: "{{ context.final_risk_score }}"
            decision: "{{ context.decision }}"
            timestamp: "{{ sys.timestamp }}"

      - name: fraud_alerts
        partitions: 6

        message:
          key: "{{ event.user.id }}"
          value:
            alert_type: high_risk
            user_id: "{{ event.user.id }}"
            risk_score: "{{ context.risk_score }}"

    timeout:
      send: 5000

    on_error:
      action: retry
      max_retries: 3
```

### 7.2 Using Message Queue in Pipeline

```yaml
pipeline:
  # Process risk decision
  - type: rules
    id: risk_evaluation
    output: context.risk_result

  # Publish decision to event bus
  - type: service
    id: publish_decision
    service: event_bus
    topic: risk_decisions

    # Async publishing (non-blocking)
    async: true

  # Conditional alert publishing
  - type: service
    id: publish_alert
    service: event_bus
    topic: fraud_alerts
    if: context.risk_result.score > 80
    async: true
```

---

## 8. Search Services

### 8.1 Elasticsearch

```yaml
services:
  - id: user_search
    name: User Search Service
    type: search
    description: Elasticsearch user data

    connection:
      driver: elasticsearch
      hosts:
        - es-1.internal:9200
        - es-2.internal:9200

      auth:
        username: ${ES_USER}
        password: ${ES_PASSWORD}

    indices:
      - name: users
        queries:
          - name: search_similar_users
            body:
              query:
                bool:
                  must:
                    - match:
                        email_domain: "{{ email_domain(event.user.email) }}"
                  filter:
                    - range:
                        created_at:
                          gte: "now-30d"
              size: 100

          - name: find_shared_devices
            body:
              query:
                terms:
                  device_id: "{{ context.user_devices }}"
              aggs:
                unique_users:
                  cardinality:
                    field: user_id

      - name: transactions
        queries:
          - name: high_value_transactions
            body:
              query:
                bool:
                  must:
                    - term:
                        user_id: "{{ event.user.id }}"
                    - range:
                        amount:
                          gte: 10000
                  filter:
                    - range:
                        created_at:
                          gte: "now-7d"
```

---

## 9. Service Usage Patterns

### 9.1 Sequential Service Calls

```yaml
pipeline:
  # Step 1: Load user profile from database
  - type: service
    id: load_user
    service: user_db
    query: get_user_profile
    output: context.user_profile

  # Step 2: Get cached risk score
  - type: service
    id: check_cache
    service: redis_cache
    operation: get_user_risk_cache
    output: context.cached_risk

  # Step 3: Call KYC service
  - type: service
    id: verify_kyc
    service: kyc_service
    endpoint: get_verification_status
    output: context.kyc_status

  # Step 4: Use all results
  - type: rules
    id: combined_check
    conditions:
      - context.user_profile.tier == "premium"
      - context.kyc_status.verified == true
```

### 9.2 Parallel Service Calls

```yaml
pipeline:
  - parallel:
      # Load user data
      - type: service
        id: load_user
        service: user_db
        query: get_user_profile

      # Load features
      - type: service
        id: load_features
        service: feature_store
        features: [user_behavior_7d]

      # Check KYC
      - type: service
        id: check_kyc
        service: kyc_service
        endpoint: get_verification_status

    merge:
      method: all
      timeout: 3000

  # All results available after merge
  - type: rules
    conditions:
      - context.load_user.tier == "premium"
      - context.load_features.login_count > 10
      - context.check_kyc.verified == true
```

### 9.3 Conditional Service Calls

```yaml
pipeline:
  # Only call expensive service if needed
  - type: service
    id: ml_risk_scoring
    service: risk_scoring_service
    method: calculate_risk_score

    # Conditional execution
    if: |
      event.transaction.amount > 10000 ||
      context.basic_risk_score > 70

    output: context.ml_risk_score
```

---

## 10. Service Configuration Management

### 10.1 Environment-Specific Configuration

```yaml
services:
  - id: user_db
    name: User Database
    type: database

    environments:
      development:
        connection:
          host: localhost
          port: 5432
          database: corint_dev
          pool:
            max_size: 5

      staging:
        connection:
          host: db-staging.internal
          port: 5432
          database: corint_staging
          pool:
            max_size: 20

      production:
        connection:
          host: db-primary.internal
          port: 5432
          database: corint_prod
          pool:
            max_size: 100
          # Read replica for analytics queries
          read_replica:
            host: db-replica.internal
```

### 10.2 Service Registry

```yaml
# service-registry.yml
service_registry:
  version: "1.0"

  services:
    # Databases
    - id: user_db
      category: database
      file: services/databases/user_db.yml

    - id: transaction_db
      category: database
      file: services/databases/transaction_db.yml

    # Caches
    - id: redis_cache
      category: cache
      file: services/cache/redis.yml

    # Microservices
    - id: kyc_service
      category: microservice
      file: services/microservices/kyc.yml

    - id: user_service
      category: microservice
      file: services/microservices/user.yml

    # Feature Store
    - id: feature_store
      category: feature_store
      file: services/feature_store/main.yml

  # Default settings for all services
  defaults:
    timeout:
      connect: 1000
      read: 3000
    retry:
      max_attempts: 2
      backoff: linear
```

---

## 11. Error Handling

### 11.1 Service-Specific Error Handling

```yaml
services:
  - id: user_db
    name: User Database
    type: database

    on_error:
      default:
        action: fail
        log_level: error

      by_error_type:
        connection_timeout:
          action: retry
          max_retries: 2

        query_timeout:
          action: fallback
          fallback:
            tier: "standard"
            risk_score: 50

        connection_refused:
          action: fail
          alert: critical

        deadlock:
          action: retry
          max_retries: 3
          backoff: exponential
```

### 11.2 Graceful Degradation

```yaml
pipeline:
  # Try to load from feature store
  - type: service
    id: load_features
    service: feature_store
    features: [user_behavior_7d]

    on_error:
      action: fallback
      fallback_service: user_db
      fallback_query: compute_features_on_demand

  # Use fallback if feature store failed
  - type: rules
    conditions:
      - context.load_features.login_count > 10
      # Works whether from feature store or computed on-demand
```

---

## 12. Performance Optimization

### 12.1 Connection Pooling

```yaml
services:
  - id: user_db
    type: database

    connection:
      pool:
        min_size: 10                  # Minimum connections
        max_size: 100                 # Maximum connections
        idle_timeout: 300000          # Close idle connections after 5min
        max_lifetime: 1800000         # Rotate connections every 30min

        # Health check
        health_check:
          enabled: true
          interval: 30000             # Check every 30s
          query: "SELECT 1"
```

### 12.2 Query Result Caching

```yaml
services:
  - id: user_db
    type: database

    queries:
      - name: get_user_profile
        sql: SELECT * FROM users WHERE user_id = :user_id

        # Cache query results
        cache:
          enabled: true
          backend: redis_cache
          key_template: "db:user:{{ user_id }}"
          ttl: 300                    # 5 minutes

          # Cache conditions
          cache_if:
            - response.tier is_not_null
```

### 12.3 Batch Operations

```yaml
services:
  - id: user_db
    type: database

    queries:
      # Batch query
      - name: get_users_batch
        sql: |
          SELECT user_id, tier, risk_score
          FROM users
          WHERE user_id = ANY(:user_ids)
        params:
          - name: user_ids
            type: array<string>
            source: context.user_id_list

# Usage in pipeline
pipeline:
  - type: service
    id: load_users
    service: user_db
    query: get_users_batch
    params:
      user_ids: [user1, user2, user3, ...]
```

---

## 13. Observability

### 13.1 Service Metrics

```yaml
observability:
  metrics:
    enabled: true

    # Service-level metrics
    service_metrics:
      - service_calls_total
      - service_call_duration_seconds
      - service_errors_total
      - service_cache_hits_total

      # Connection pool metrics
      - connection_pool_active
      - connection_pool_idle
      - connection_pool_wait_duration

    labels:
      - service_id
      - service_type
      - query_name
      - error_type
```

### 13.2 Service Tracing

```yaml
observability:
  tracing:
    enabled: true

    # Distributed tracing
    propagate_trace_context: true

    capture:
      - service_id
      - query_name
      - query_duration
      - cache_hit
      - connection_time

    # Trace sampling
    sampling_rate: 0.1                # 10% of requests
    always_sample_on_error: true
```

### 13.3 Service Health Checks

```yaml
services:
  - id: user_db
    type: database

    health_check:
      enabled: true
      interval: 30000                 # ms
      timeout: 5000                   # ms

      check:
        type: query
        query: "SELECT 1"

      on_failure:
        action: alert
        alert_level: critical

        # Circuit breaker
        consecutive_failures: 3
        circuit_open_duration: 60000  # 1 minute
```

---

## 14. Security

### 14.1 Internal Authentication

```yaml
services:
  - id: kyc_service
    type: microservice

    connection:
      auth:
        # Service-to-service token
        type: internal_token
        token: ${INTERNAL_SERVICE_TOKEN}

        # Or mTLS
        type: mtls
        cert: ${SERVICE_CERT}
        key: ${SERVICE_KEY}
        ca: ${CA_CERT}
```

### 14.2 Query Parameter Sanitization

```yaml
services:
  - id: user_db
    type: database

    security:
      # SQL injection prevention
      parameterized_queries: true
      allow_dynamic_sql: false

      # Query validation
      validate_params: true
      max_query_length: 10000
```

### 14.3 Sensitive Data Masking

```yaml
observability:
  logging:
    # Mask sensitive fields in logs
    mask_fields:
      - password
      - ssn
      - credit_card
      - api_key

    # Redact patterns
    redact_patterns:
      - regex: '\b\d{4}-\d{4}-\d{4}-\d{4}\b'  # Credit cards
      - regex: '\b\d{3}-\d{2}-\d{4}\b'        # SSN
```

---

## 15. Testing

### 15.1 Service Mocks

```yaml
testing:
  mocks:
    user_db:
      queries:
        get_user_profile:
          - match:
              params:
                user_id: "test_user_1"
            response:
              user_id: test_user_1
              tier: premium
              risk_score: 10
              kyc_verified: true

          - match:
              params:
                user_id: "test_user_2"
            response:
              user_id: test_user_2
              tier: standard
              risk_score: 50
              kyc_verified: false

          # Default response
          - default: true
            response:
              user_id: "{{ params.user_id }}"
              tier: standard
              risk_score: 30
```

### 15.2 Integration Testing

```yaml
tests:
  - name: "Database connection test"
    service: user_db
    query: get_user_profile

    input:
      user_id: test_user_1

    expected:
      response.tier: "premium"
      response.kyc_verified: true

  - name: "Cache hit test"
    service: redis_cache
    operation: get_user_risk_cache

    setup:
      - cache_key: "risk:user:test_user_1"
        cache_value: { score: 85 }

    expected:
      cache_hit: true
      response.score: 85
```

---

## 16. Best Practices

### 16.1 Service Design

**Good:**
```yaml
services:
  - id: user_profile_db
    name: User Profile Database
    type: database
    description: "Primary user data store for profile information"

    # Clear connection settings
    connection:
      driver: postgresql
      host: ${DB_HOST}
      pool:
        max_size: 50

    # Well-defined queries
    queries:
      - name: get_user_by_id
        description: "Retrieve user profile by user ID"
        sql: SELECT * FROM users WHERE user_id = :user_id
        params:
          - name: user_id
            type: string
            required: true
```

**Avoid:**
```yaml
services:
  - id: db1                           # Unclear name
    type: database
    # No description
    queries:
      - name: query1                  # Vague name
        sql: SELECT * FROM users WHERE user_id = :id
        # No parameter definitions
```

### 16.2 Performance

- **Use connection pooling** for all database services
- **Enable caching** for read-heavy queries
- **Batch operations** when loading multiple records
- **Parallel calls** for independent service requests
- **Query optimization** - use indexes, limit result sets

### 16.3 Reliability

- **Fail fast** for critical services
- **Graceful degradation** for optional services
- **Circuit breakers** for unstable services
- **Health checks** to detect failures early
- **Connection retries** with exponential backoff

### 16.4 Maintainability

- **Descriptive names** for services and queries
- **Clear documentation** for all service operations
- **Environment separation** for dev/staging/prod
- **Version control** for service configurations
- **Monitoring and alerting** for all services

---

## 17. Summary

CORINT's Internal Service Integration provides:

- **Unified service definition** for databases, caches, microservices, message queues
- **Performance optimization** through connection pooling, caching, batching
- **Reliability features** including retries, circuit breakers, health checks
- **Flexible error handling** with fallback strategies
- **Full observability** with metrics, tracing, and logging
- **Security controls** for authentication, authorization, data masking
- **Testing support** with mocks and integration tests

This enables efficient integration with internal infrastructure while maintaining performance, reliability, and security.

---

## 18. Related Documentation

- `external.md` - External third-party API integration
- `feature.md` - Feature engineering and statistical analysis
- `context.md` - Context and variable management
- `pipeline.md` - Service integration in pipelines
