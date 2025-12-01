# CORINT Risk Definition Language (RDL)
## External Integration Specification (v0.1)

This document defines how external APIs and third-party services are configured, invoked, and managed within CORINT's Risk Definition Language.

External APIs enable integration with third-party risk intelligence services such as:
- Blockchain analytics (Chainalysis, Elliptic)
- Device fingerprinting (FingerprintJS, DeviceAtlas)
- IP reputation (MaxMind, IPQualityScore)
- Email validation (EmailRep, Hunter)
- Identity verification (Jumio, Onfido)

---

## 1. API Definition Structure

### 1.1 Basic Structure

```yaml
external_apis:
  - id: string                    # Unique identifier
    name: string                  # Human-readable name
    description: string           # Purpose description
    base_url: string              # Base URL
    version: string               # API version
    auth: <auth-config>           # Authentication
    endpoints: [<endpoint>]       # Available endpoints
    rate_limit: <rate-config>     # Rate limiting
    timeout: <timeout-config>     # Timeout settings
    retry: <retry-config>         # Retry policy
    cache: <cache-config>         # Caching strategy
    on_error: <error-config>      # Error handling
```

### 1.2 Complete Example

```yaml
external_apis:
  - id: chainalysis
    name: Chainalysis Risk API
    description: Blockchain transaction and wallet risk analysis
    base_url: https://api.chainalysis.com/v2
    version: "2.0"

    auth:
      type: api_key
      header: X-API-Key
      key: ${CHAINALYSIS_API_KEY}

    endpoints:
      - name: wallet_risk
        path: /risk/wallet/{address}
        method: GET
        description: Get wallet risk score

        params:
          - name: address
            type: string
            required: true
            source: event.wallet.address

        response_schema:
          risk_score:
            type: number
            min: 0
            max: 100
          risk_level:
            type: string
            enum: [low, medium, high, severe]
          categories:
            type: array
            items: string
          last_updated:
            type: datetime

        mapping:
          risk_score: response.riskScore
          risk_level: response.riskLevel
          categories: response.categories

      - name: transaction_risk
        path: /risk/transaction/{hash}
        method: GET
        description: Analyze transaction risk

    rate_limit:
      requests_per_second: 100
      requests_per_minute: 5000
      burst: 200

    timeout:
      connect: 2000              # ms
      read: 5000                 # ms
      total: 10000               # ms

    retry:
      max_attempts: 3
      backoff: exponential
      initial_delay: 1000        # ms
      max_delay: 10000           # ms
      retry_on:
        - timeout
        - 429                    # Rate limited
        - 500                    # Server error
        - 502                    # Bad gateway
        - 503                    # Service unavailable

    cache:
      enabled: true
      ttl: 3600                  # seconds
      key_template: "chainalysis:{endpoint}:{address}"
      backend: redis

    on_error:
      action: fallback
      fallback:
        risk_score: 50
        risk_level: medium
        categories: []
      log_level: warn
```

---

## 2. Authentication

### 2.1 API Key Authentication

```yaml
auth:
  type: api_key
  header: X-API-Key              # Or: query, cookie
  key: ${API_KEY_ENV_VAR}
```

### 2.2 Bearer Token

```yaml
auth:
  type: bearer
  token: ${BEARER_TOKEN}

  # Optional: Token refresh
  refresh:
    enabled: true
    endpoint: /oauth/token
    method: POST
    body:
      grant_type: client_credentials
      client_id: ${CLIENT_ID}
      client_secret: ${CLIENT_SECRET}
    expires_in_field: expires_in
    token_field: access_token
```

### 2.3 OAuth 2.0

```yaml
auth:
  type: oauth2
  flow: client_credentials       # or: authorization_code, password

  client_id: ${OAUTH_CLIENT_ID}
  client_secret: ${OAUTH_CLIENT_SECRET}
  token_url: https://auth.provider.com/oauth/token

  scopes:
    - read:risk
    - read:wallet

  # Token caching
  cache_token: true
  refresh_before_expiry: 300     # seconds
```

### 2.4 Basic Auth

```yaml
auth:
  type: basic
  username: ${API_USERNAME}
  password: ${API_PASSWORD}
```

### 2.5 HMAC Signature

```yaml
auth:
  type: hmac
  algorithm: sha256
  secret: ${HMAC_SECRET}

  signature:
    include:
      - timestamp
      - request_body
      - path
    header: X-Signature
    timestamp_header: X-Timestamp
```

### 2.6 mTLS (Mutual TLS)

```yaml
auth:
  type: mtls
  cert: ${CLIENT_CERT_PATH}
  key: ${CLIENT_KEY_PATH}
  ca: ${CA_CERT_PATH}
```

---

## 3. Endpoint Definition

### 3.1 Basic Endpoint

```yaml
endpoints:
  - name: get_risk_score
    path: /risk/{entity_type}/{entity_id}
    method: GET
    description: Get risk score for an entity
```

### 3.2 Path Parameters

```yaml
endpoints:
  - name: wallet_lookup
    path: /wallets/{chain}/{address}
    method: GET

    params:
      - name: chain
        type: string
        required: true
        source: event.transaction.chain
        enum: [ethereum, bitcoin, polygon]

      - name: address
        type: string
        required: true
        source: event.wallet.address
        validation:
          pattern: "^0x[a-fA-F0-9]{40}$"
```

### 3.3 Query Parameters

```yaml
endpoints:
  - name: transaction_history
    path: /transactions
    method: GET

    query:
      - name: wallet
        type: string
        required: true
        source: event.wallet.address

      - name: limit
        type: integer
        required: false
        default: 100
        max: 1000

      - name: start_date
        type: datetime
        required: false
        source: context.query_start_date
```

### 3.4 Request Body (POST/PUT)

```yaml
endpoints:
  - name: batch_risk_check
    path: /risk/batch
    method: POST

    headers:
      Content-Type: application/json

    body:
      type: json
      schema:
        addresses:
          type: array
          source: event.addresses
          max_items: 100
        options:
          type: object
          properties:
            include_history: true
            risk_threshold: 70
```

### 3.5 Response Mapping

```yaml
endpoints:
  - name: ip_reputation
    path: /ip/{ip_address}
    method: GET

    response_schema:
      score:
        type: number
        min: 0
        max: 100
      is_proxy:
        type: boolean
      is_vpn:
        type: boolean
      is_tor:
        type: boolean
      country:
        type: string
      isp:
        type: string

    # Map API response to internal fields
    mapping:
      risk_score: response.fraud_score
      is_anonymous: response.is_proxy || response.is_vpn || response.is_tor
      geo_country: response.country_code
      provider: response.ISP

    # Transform values
    transform:
      risk_score: |
        # Normalize to 0-100 scale
        min(max(response.fraud_score * 100, 0), 100)
```

---

## 4. Rate Limiting

### 4.1 Basic Rate Limits

```yaml
rate_limit:
  requests_per_second: 100
  requests_per_minute: 5000
  requests_per_hour: 100000
  burst: 200                     # Allow burst up to this limit
```

### 4.2 Endpoint-Specific Limits

```yaml
rate_limit:
  global:
    requests_per_second: 100

  endpoints:
    wallet_risk:
      requests_per_second: 50
    batch_check:
      requests_per_second: 10
```

### 4.3 Rate Limit Handling

```yaml
rate_limit:
  requests_per_second: 100

  on_limit:
    action: queue               # queue | reject | fallback
    queue_timeout: 5000         # ms

  # Respect API rate limit headers
  headers:
    remaining: X-RateLimit-Remaining
    reset: X-RateLimit-Reset
    limit: X-RateLimit-Limit
```

---

## 5. Timeout Configuration

### 5.1 Timeout Settings

```yaml
timeout:
  connect: 2000                  # Connection timeout (ms)
  read: 5000                     # Read timeout (ms)
  write: 3000                    # Write timeout (ms)
  total: 10000                   # Total request timeout (ms)
```

### 5.2 Endpoint-Specific Timeouts

```yaml
timeout:
  default:
    total: 5000

  endpoints:
    batch_check:
      total: 30000               # Longer timeout for batch
    quick_lookup:
      total: 2000                # Faster timeout for quick checks
```

---

## 6. Retry Policy

### 6.1 Basic Retry

```yaml
retry:
  max_attempts: 3
  backoff: exponential           # exponential | linear | fixed
  initial_delay: 1000            # ms
  max_delay: 10000               # ms
  multiplier: 2                  # For exponential backoff
```

### 6.2 Conditional Retry

```yaml
retry:
  max_attempts: 3
  backoff: exponential

  # Retry on these conditions
  retry_on:
    status_codes:
      - 429                      # Rate limited
      - 500                      # Server error
      - 502                      # Bad gateway
      - 503                      # Service unavailable
      - 504                      # Gateway timeout
    exceptions:
      - timeout
      - connection_reset
      - dns_failure

  # Do not retry on these
  no_retry_on:
    status_codes:
      - 400                      # Bad request
      - 401                      # Unauthorized
      - 403                      # Forbidden
      - 404                      # Not found
```

### 6.3 Circuit Breaker

```yaml
circuit_breaker:
  enabled: true

  # Open circuit after N failures
  failure_threshold: 5
  failure_window: 60000          # ms

  # Keep circuit open for this duration
  open_duration: 30000           # ms

  # Test with N calls before fully closing
  half_open_max_calls: 3

  # What counts as failure
  failure_conditions:
    - timeout
    - status_code >= 500
```

---

## 7. Caching

### 7.1 Basic Caching

```yaml
cache:
  enabled: true
  ttl: 3600                      # seconds
  backend: redis                 # redis | memcached | memory
```

### 7.2 Advanced Caching

```yaml
cache:
  enabled: true

  # Cache key generation
  key:
    template: "{api_id}:{endpoint}:{hash(params)}"
    # Or specify fields
    include:
      - endpoint
      - params.address
      - params.chain
    exclude:
      - params.timestamp

  # TTL by endpoint
  ttl:
    default: 3600
    endpoints:
      wallet_risk: 1800          # 30 minutes for risk data
      static_info: 86400         # 24 hours for static data

  # Cache backend
  backend:
    type: redis
    host: ${REDIS_HOST}
    port: 6379
    db: 2
    password: ${REDIS_PASSWORD}

  # Conditional caching
  cache_if:
    - response.status_code == 200
    - response.risk_score is_not_null

  # Cache warming
  warm_on_startup:
    enabled: true
    endpoints:
      - high_risk_wallets
      - blocked_addresses
```

---

## 8. Error Handling

### 8.1 Error Handling Strategies

```yaml
on_error:
  # Strategy: fail | skip | fallback | retry
  action: fallback

  # Fallback values
  fallback:
    risk_score: 50
    risk_level: medium
    error: true

  # Logging
  log_level: warn                # debug | info | warn | error

  # Include error details in context
  expose_error: true
  error_field: context.api_errors.{api_id}
```

### 8.2 Error-Specific Handling

```yaml
on_error:
  default:
    action: fallback
    fallback:
      risk_score: 50

  by_status:
    401:
      action: fail
      message: "API authentication failed"
      alert: critical

    429:
      action: retry
      delay: 5000

    404:
      action: skip
      log_level: debug

  by_exception:
    timeout:
      action: fallback
      fallback:
        risk_score: 70           # Higher risk on timeout
        timeout: true
```

---

## 9. Request/Response Transformation

### 9.1 Request Transformation

```yaml
endpoints:
  - name: risk_check
    path: /check
    method: POST

    request_transform:
      # Rename fields
      rename:
        event.wallet.address: walletAddress
        event.user.id: userId

      # Add computed fields
      add:
        timestamp: now()
        request_id: uuid()

      # Remove sensitive fields
      remove:
        - event.user.password
        - event.user.ssn
```

### 9.2 Response Transformation

```yaml
endpoints:
  - name: risk_check

    response_transform:
      # Normalize score to 0-100
      risk_score: |
        response.score * 100

      # Map risk levels
      risk_level: |
        case response.riskCategory
          when "LOW" then "low"
          when "MEDIUM" then "medium"
          when "HIGH", "SEVERE" then "high"
          else "unknown"
        end

      # Flatten nested response
      categories: response.analysis.categories

      # Combine fields
      is_high_risk: |
        response.score > 0.7 || response.riskCategory in ["HIGH", "SEVERE"]
```

---

## 10. Using External APIs

### 10.1 In Rule Conditions

```yaml
rule:
  id: high_risk_wallet
  name: High Risk Wallet Detection

  when:
    event.type: crypto_transfer
    conditions:
      - external_api.chainalysis.risk_score > 80
      - external_api.chainalysis.risk_level == "severe"
      - external_api.chainalysis.categories contains "sanctions"

  score: 100
```

### 10.2 In Pipeline Steps

```yaml
pipeline:
  - type: api
    id: wallet_risk_check
    api: chainalysis
    endpoint: wallet_risk

    params:
      address: event.wallet.address

    output: context.wallet_risk

    # Conditional execution
    if: event.transaction.amount > 10000
```

### 10.3 Parallel API Calls

```yaml
pipeline:
  - parallel:
      - type: api
        id: ip_check
        api: maxmind
        endpoint: ip_lookup

      - type: api
        id: device_check
        api: fingerprintjs
        endpoint: device_risk

      - type: api
        id: email_check
        api: emailrep
        endpoint: email_reputation

    merge:
      method: all
      timeout: 5000
```

### 10.4 Chained API Calls

```yaml
pipeline:
  # First: Get wallet info
  - type: api
    id: wallet_info
    api: blockchain_explorer
    endpoint: wallet_details
    output: context.wallet_info

  # Then: Check risk using wallet info
  - type: api
    id: risk_check
    api: chainalysis
    endpoint: wallet_risk

    # Use output from previous step
    params:
      address: context.wallet_info.address
      chain: context.wallet_info.chain
```

---

## 11. API Registry

### 11.1 Centralized API Definition

```yaml
# api-registry.yml
api_registry:
  version: "1.0"

  apis:
    # Risk Intelligence
    - id: chainalysis
      category: blockchain_risk
      file: apis/chainalysis.yml

    - id: elliptic
      category: blockchain_risk
      file: apis/elliptic.yml

    # Device Intelligence
    - id: fingerprintjs
      category: device_fingerprint
      file: apis/fingerprintjs.yml

    # IP Intelligence
    - id: maxmind
      category: ip_reputation
      file: apis/maxmind.yml

    - id: ipqualityscore
      category: ip_reputation
      file: apis/ipqualityscore.yml

    # Email Intelligence
    - id: emailrep
      category: email_validation
      file: apis/emailrep.yml

  # Default settings for all APIs
  defaults:
    timeout:
      total: 5000
    retry:
      max_attempts: 3
      backoff: exponential
    cache:
      enabled: true
      ttl: 3600
```

### 11.2 Environment-Specific Configuration

```yaml
# apis/chainalysis.yml
external_api:
  id: chainalysis

  environments:
    development:
      base_url: https://sandbox.chainalysis.com/v2
      auth:
        key: ${CHAINALYSIS_SANDBOX_KEY}
      rate_limit:
        requests_per_second: 10

    staging:
      base_url: https://staging-api.chainalysis.com/v2
      auth:
        key: ${CHAINALYSIS_STAGING_KEY}
      rate_limit:
        requests_per_second: 50

    production:
      base_url: https://api.chainalysis.com/v2
      auth:
        key: ${CHAINALYSIS_PROD_KEY}
      rate_limit:
        requests_per_second: 100
```

---

## 12. Observability

### 12.1 Logging

```yaml
observability:
  logging:
    enabled: true
    level: info

    include:
      - request_id
      - endpoint
      - latency
      - status_code
      - cache_hit

    exclude:
      - request_body
      - response_body
      - auth_headers

    # Sample logging for high-volume endpoints
    sampling:
      default: 1.0               # Log all
      endpoints:
        high_volume_check: 0.1   # Log 10%
```

### 12.2 Metrics

```yaml
observability:
  metrics:
    enabled: true

    counters:
      - api_requests_total
      - api_errors_total
      - api_cache_hits_total

    histograms:
      - api_request_duration_seconds
      - api_response_size_bytes

    labels:
      - api_id
      - endpoint
      - status_code
      - cache_hit
```

### 12.3 Tracing

```yaml
observability:
  tracing:
    enabled: true

    capture:
      - request_headers
      - response_status
      - latency
      - retry_count

    propagate:
      - X-Request-ID
      - X-Correlation-ID
```

---

## 13. Security

### 13.1 Secrets Management

```yaml
security:
  secrets:
    provider: vault              # vault | aws_secrets | env

    vault:
      address: https://vault.company.com
      path: secret/data/corint/apis
      auth:
        method: kubernetes
        role: corint-api
```

### 13.2 Request Signing

```yaml
security:
  request_signing:
    enabled: true
    algorithm: hmac-sha256

    sign:
      - timestamp
      - path
      - body_hash

    headers:
      signature: X-Signature
      timestamp: X-Timestamp
```

### 13.3 Response Validation

```yaml
security:
  response_validation:
    # Validate response signature
    verify_signature: true
    signature_header: X-Response-Signature

    # Validate response schema
    validate_schema: true
    on_validation_error: warn    # warn | fail
```

---

## 14. Testing

### 14.1 Mock Configuration

```yaml
testing:
  mocks:
    chainalysis:
      endpoints:
        wallet_risk:
          - match:
              params:
                address: "0x123..."
            response:
              risk_score: 85
              risk_level: high

          - match:
              params:
                address: "0x456..."
            response:
              risk_score: 10
              risk_level: low

          # Default response
          - default: true
            response:
              risk_score: 50
              risk_level: medium
```

### 14.2 Test Cases

```yaml
tests:
  - name: "High risk wallet detection"
    api: chainalysis
    endpoint: wallet_risk

    input:
      address: "0xknown_high_risk_address"

    expected:
      risk_score: ">= 80"
      risk_level: "high"

  - name: "API timeout handling"
    api: chainalysis
    endpoint: wallet_risk

    simulate:
      latency: 10000             # Simulate timeout

    expected:
      fallback_used: true
      risk_score: 50
```

---

## 15. Best Practices

### 15.1 API Design

**Good:**
```yaml
external_api:
  id: risk_provider

  # Clear naming
  endpoints:
    - name: wallet_risk_score
      description: "Get risk score for cryptocurrency wallet"

  # Explicit timeouts
  timeout:
    total: 5000

  # Defined fallbacks
  on_error:
    action: fallback
    fallback:
      risk_score: 50
```

**Avoid:**
```yaml
external_api:
  id: api1                       # Unclear name

  endpoints:
    - name: check                # Vague endpoint name
      # No description
      # No timeout (uses default)
      # No error handling
```

### 15.2 Performance

- Use caching for frequently accessed data
- Set appropriate timeouts (don't wait forever)
- Implement circuit breakers for unreliable APIs
- Use parallel calls when possible
- Consider rate limits in your design

### 15.3 Reliability

- Always define fallback values
- Use retry with exponential backoff
- Monitor API health and latency
- Have backup providers for critical APIs

---

## 16. Summary

CORINT's External API specification provides:

- **Unified API definition** format for all external services
- **Flexible authentication** supporting API keys, OAuth, mTLS
- **Robust error handling** with fallbacks and circuit breakers
- **Performance optimization** through caching and rate limiting
- **Full observability** with logging, metrics, and tracing
- **Security features** including secrets management and request signing
- **Testing support** with mock configurations

This enables reliable integration with third-party risk intelligence services while maintaining performance and resilience.

---

## 17. Related Documentation

- `rule.md` - Using external API results in rules
- `pipeline.md` - API steps in pipelines
- `error-handling.md` - Error handling strategies
- `performance.md` - Performance optimization
- `observability.md` - Monitoring and logging
