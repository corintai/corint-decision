# CORINT Decision Engine - Context Architecture Guide

## Design Goals

Solve current data organization issues:
- ❌ Current: features, api, service results are mixed in event_data
- ✅ Goal: Clear separation of data from different sources and purposes

---

## Final Design: Flattened Namespace Architecture

### Core Principles

1. **Classify by processing method, not data source**
   - Not "user data vs system data"
   - But "raw data vs simple computation vs complex features vs AI analysis vs external calls"

2. **All namespaces flattened at the same level**
   - Avoid confusion from nesting
   - Overall concept called `context`, no nested `context` sub-namespace

3. **Clear distinction between read-only and writable**
   - Read-only: event, sys, env, results
   - Writable: features, api, service, vars

---

## Overview

### Flattened Namespace Architecture

CORINT uses a **flattened namespace architecture** where all data sources are organized at the same level. This design classifies data by **processing method** rather than data source.

| Namespace | Mutability | Description | Implementation Status |
|-----------|------------|-------------|----------------------|
| `event` | Read-only | User request raw data | ✅ Fully implemented |
| `features` | Writable | Complex feature computations | ✅ Fully implemented |
| `api` | Writable | External API call results | ✅ Fully implemented |
| `service` | Writable | Internal service call results | ✅ Fully implemented |
| `vars` | Writable | Simple variables and calculations | ✅ Fully implemented |
| `sys` | Read-only | System injected metadata | ✅ Fully implemented |
| `env` | Read-only | Environment configuration | ✅ Fully implemented |
| `results` | Read-only | Ruleset execution results | ⚠️ Pipeline execution layer |

**Core Principles**:
1. **Classify by processing method**, not data source
2. **All namespaces at same level** - no nesting confusion
3. **Clear read-only vs writable distinction**
4. **No `ctx.` prefix needed** - namespace names are sufficient

---

## 1. event - User Request Raw Data

### Description

The `event` namespace contains **raw business data** from API requests, **unprocessed** and as submitted by users.

```yaml
Mutability: Read-only
Source: User API requests
Validation: Schema validated, reserved fields prohibited
```

### Event Structure

```yaml
event:
  # Common fields
  type: string                  # Event type: login, transaction, etc.
  timestamp: datetime           # User-submitted timestamp (if any)

  # User data
  user:
    id: string
    email: string
    account_age_days: number
    profile: object

  # Transaction data
  transaction:
    id: string
    amount: number
    currency: string
    merchant: string

  # Device data
  device:
    id: string
    type: string
    ip: string
    user_agent: string

  # Geolocation data
  geo:
    country: string
    city: string
    coordinates: object
```

### Reserved Fields

These fields cannot be submitted by users in the event data:

- `total_score`
- `triggered_rules`
- `sys_*`
- `features_*`
- `api_*`
- `service_*`

### Accessing Event Data

```yaml
# In rule when conditions
rule:
  id: high_value_transaction
  name: High-Value Transaction
  when:
    all:
      - event.type == "transaction"
      - event.amount >= 500
      - event.device_id != ""
  score: 35

# In pipeline when conditions
pipeline:
  id: transaction_pipeline
  when:
    all:
      - event.type == "transaction"
      - event.source == "supabase"

# Accessing nested event fields
rule:
  id: specific_user_check
  when:
    all:
      - event.user_id == "user_001"
      - event.amount > 1000
  score: 50
```

---

## 2. features - Complex Feature Computations

### Description

The `features` namespace contains results from **complex feature calculations** that require historical data queries or database aggregations.

```yaml
Mutability: Writable (by feature steps only)
Source: Pipeline feature steps
Processing: Database queries, time-window statistics, ML models
```

### Use Cases

- Database aggregation queries (SUM, AVG, COUNT)
- Time window statistics (7 days, 30 days, 90 days)
- Historical behavior analysis
- Complex scoring models
- User behavioral patterns

### Example Features

```yaml
features:
  # Transaction patterns
  user_transaction_count_7d: 15
  avg_transaction_amount_30d: 3200.5
  max_transaction_amount_90d: 15000.0

  # Device history
  device_first_seen_days: 45
  device_transaction_count: 127

  # Behavioral scores
  velocity_score: 0.75
  anomaly_score: 0.42
```

### Accessing Features

```yaml
# In rule conditions
rule:
  id: feature_based_check
  when:
    all:
      - features.user_transaction_count_7d > 20
      - features.avg_transaction_amount_30d < 1000
      - features.velocity_score > 0.8
  score: 75
```

---

## 3. api - External API Call Results

### Description

The `api` namespace contains results from calling **external third-party APIs**.

```yaml
Mutability: Writable (by api steps only)
Source: Pipeline api steps
Examples: Device fingerprinting, IP geolocation, credit checks
```

### Use Cases

- Device fingerprinting (FingerprintJS, Seon)
- IP geolocation queries
- Email/phone verification
- Credit score queries
- KYC/AML checks
- Blockchain analysis

### Example API Results

```yaml
api:
  device_fingerprint:
    risk_score: 0.75
    is_vpn: true
    is_proxy: false
    confidence: 0.92

  ip_geolocation:
    country: "US"
    city: "New York"
    is_datacenter: false

  email_verification:
    is_valid: true
    is_disposable: false
    mx_records_valid: true
```

### Accessing API Results

```yaml
# In rule conditions
rule:
  id: api_based_check
  when:
    all:
      - api.device_fingerprint.risk_score > 0.7
      - api.ip_geolocation.country != event.user.registered_country
      - api.email_verification.is_disposable == true
  score: 80
```

---

## 4. service - Internal Service Call Results

### Description

The `service` namespace contains results from calling **internal microservices**.

```yaml
Mutability: Writable (by service steps only)
Source: Pipeline service steps
Examples: User profiles, risk history, inventory
```

### Use Cases

- User profile service
- Risk history records
- Inventory query service
- Order management service
- Points/membership system
- Internal fraud database

### Example Service Results

```yaml
service:
  user_profile:
    vip_level: "gold"
    account_status: "active"
    lifetime_value: 15000.0

  risk_history:
    blacklist_hit: false
    previous_fraud_count: 0
    last_suspicious_activity: null

  inventory:
    stock_available: 100
    reserved_count: 5
```

### Accessing Service Results

```yaml
# In rule conditions
rule:
  id: service_based_check
  when:
    all:
      - service.user_profile.vip_level == "gold"
      - service.risk_history.previous_fraud_count > 0
      - service.inventory.stock_available >= event.amount
  score: 65
```

---

## 5. vars - Simple Variables and Calculations

### Description

The `vars` namespace contains **pipeline variables**, **configuration parameters**, and **simple calculations** that don't require external data.

```yaml
Mutability: Writable
Source: Pipeline vars config + rule calculations
Processing: Arithmetic, string operations, boolean logic
```

### Difference from features

| Aspect | vars | features |
|--------|------|----------|
| **Complexity** | Simple | Complex |
| **Data needs** | No external data | Requires historical data |
| **Examples** | amount × rate | SUM(transactions, 7d) |
| **Performance** | Instant | May require DB query |

### Example Variables

```yaml
vars:
  # Configuration
  high_risk_threshold: 80
  min_transaction_amount: 100
  max_daily_limit: 50000

  # Simple calculations
  risk_multiplier: 1.5
  total_fee: 15.5              # Calculated: amount * 0.031
  is_high_value: true          # Calculated: amount > 10000

  # String operations
  user_display_name: "John D." # Calculated: first_name + " " + last_initial
```

### Using Variables

```yaml
# Defining variables
pipeline:
  steps:
    - step_type: vars
      config:
        high_risk_threshold: 80
        processing_fee_rate: 0.031

# Accessing in rule conditions
rule:
  id: vars_based_check
  when:
    all:
      - event.amount > vars.high_risk_threshold
      - vars.is_high_value == true
  score: 70

# Using in calculations
score: event.transaction.amount * vars.risk_multiplier
```

---

## 6. results - Execution Results

### Description

The `results` namespace provides access to **ruleset execution results** within a pipeline. This enables pipeline routing decisions based on previous ruleset outcomes.

**⚠️ Note:** The `results` namespace is not part of the base ExecutionContext. Availability depends on pipeline execution layer implementation.

```yaml
Mutability: Read-only
Source: Ruleset execution within pipeline
Lifecycle: Updated after each ruleset step completes
```

### Access Patterns

The `results` namespace supports multiple access patterns:

#### Specific Ruleset by ID
```yaml
# Access a specific ruleset's result by ID
results.fraud_detection.signal        # Ruleset signal: "approve", "decline", etc.
results.fraud_detection.total_score   # Cumulative risk score
results.fraud_detection.triggered_rules
```

#### Final Pipeline Decision
```yaml
# Access the final pipeline decision output
results.decision          # Final decision: "approve", "decline", "review", "hold", "pass"
results.actions           # List of actions: ["KYC", "2FA", "BLOCK_DEVICE"]
results.reason            # Decision reason text
```

### Available Fields

```yaml
results:
  # Per-ruleset results (accessed via results.<ruleset_id>.*)
  <ruleset_id>:
    signal: string              # "approve", "decline", "review", "hold", "pass"
    total_score: number         # Cumulative risk score (0-1000)
    reason: string              # Human-readable decision reason
    triggered_rules: array      # List of triggered rule IDs
    triggered_count: number     # Number of triggered rules

  # Final pipeline decision output
  decision: string              # Final decision: "approve", "decline", "review", "hold", "pass"
  actions: array                # List of actions to execute: ["KYC", "OTP", "2FA"]
  reason: string                # Final decision reason
  score: number                 # Final aggregated score
```

### Pipeline Router Usage

The primary use case for `results` is in pipeline routers to make branching decisions:

```yaml
pipeline:
  id: fraud_detection_pipeline
  entry: blacklist_check

  steps:
    - id: blacklist_check
      type: ruleset
      ruleset: blacklist_ruleset
      next: fraud_router

    - id: fraud_router
      type: router
      routes:
        # Route based on ruleset signal
        - when: results.blacklist_ruleset.signal == "decline"
          next: block_transaction

        # Route based on score threshold
        - when: results.blacklist_ruleset.total_score > 80
          next: manual_review

      default: allow_transaction
```

### Multiple Ruleset Example

When multiple rulesets are executed in a pipeline:

```yaml
pipeline:
  steps:
    - id: fraud_detection_step
      type: ruleset
      ruleset: fraud_detection
      next: behavior_step

    - id: behavior_step
      type: ruleset
      ruleset: user_behavior
      next: final_router

    - id: final_router
      type: router
      routes:
        # Access specific ruleset by ID
        - when: results.fraud_detection.signal == "decline"
          next: deny_step

        - when: results.user_behavior.total_score > 50
          next: review_step

        # Combine multiple ruleset results
        - when:
            all:
              - results.fraud_detection.signal == "review"
              - results.user_behavior.signal == "review"
          next: enhanced_review

      default: allow_step

  # Final decision based on signals
  decision:
    - when: results.fraud_detection.signal == "decline"
      result: decline
      actions: ["BLOCK_DEVICE"]
      reason: "Fraud detected"
      terminate: true

    - default: true
      result: approve
```

### Final Decision Output

After pipeline execution completes, the `results` namespace contains the final decision:

```yaml
# Example results after pipeline execution
results:
  # First ruleset result
  fraud_detection:
    signal: "review"
    total_score: 75
    triggered_rules: ["velocity_check", "new_device"]
    reason: "Medium risk detected"

  # Second ruleset result
  user_behavior:
    signal: "approve"
    total_score: 20
    triggered_rules: []

  # Final pipeline decision
  decision: "review"
  actions: ["KYC"]
  reason: "Medium risk - requires review"
  score: 75
```

### Best Practices

✅ **Good:**
```yaml
# Clear, specific field access
- when: results.fraud_detection.signal == "decline"
- when: results.fraud_detection.total_score > 80

# Combine with other conditions
- when: results.fraud_detection.signal != "decline" && event.amount > 10000
```

❌ **Bad:**
```yaml
# Don't use results in rule conditions
rule:
  id: bad_example
  when:
    all:
      - results.fraud_detection.signal == "decline"  # ❌ Not available in rule conditions
  score: 50

# Don't assume results exists before any ruleset executes
- when: results.fraud_detection.signal == "approve"  # May be undefined
```

---

## 7. sys - System Injected Metadata

### Description

The `sys` namespace contains **system auto-generated metadata** and context information.

```yaml
Mutability: Read-only
Source: System auto-generated
Lifecycle: Generated per request
```

### System Variable Categories

#### Request Identification
```yaml
sys.request_id: "550e8400-e29b-41d4-a716-446655440000"
sys.correlation_id: "parent-request-12345"  # Optional
```

#### Time Information
```yaml
sys.timestamp: "2024-01-15T10:30:00Z"       # ISO 8601
sys.timestamp_ms: 1705315800000              # Unix milliseconds
sys.date: "2024-01-15"                       # YYYY-MM-DD
sys.time: "10:30:00"                         # HH:MM:SS
sys.hour: 10                                 # 0-23
sys.day_of_week: "monday"                    # monday-sunday
sys.is_weekend: false                        # boolean
```

#### Environment Information
```yaml
sys.environment: "production"                # development/staging/production
sys.region: "us-west-1"                      # Deployment region
sys.tenant_id: "tenant_abc123"               # Multi-tenant ID (optional)
```

#### Execution Context
```yaml
sys.pipeline_id: "fraud_detection_pipeline"
sys.pipeline_version: "2.1.0"
sys.ruleset_id: "account_takeover_rules"
sys.rule_id: "impossible_travel_detection"  # Available within rule
```

#### Performance Metrics
```yaml
sys.execution_time_ms: 245                   # Current execution time
sys.execution_step: 5                        # Current step number
sys.timeout_ms: 5000                         # Execution timeout limit
```

#### Version Information
```yaml
sys.corint_version: "1.2.3"                  # CORINT engine version
sys.api_version: "v1"                        # API version
```

#### Client Information
```yaml
sys.client_id: "mobile_app_ios_v2.1"
sys.client_ip: "203.0.113.42"                # Client IP (if different from event)
sys.user_agent: "Mozilla/5.0 ..."
```

#### Debug Information
```yaml
sys.debug_mode: false
sys.trace_enabled: true
```

### Using System Variables

```yaml
# In rule conditions
rule:
  id: time_based_check
  when:
    all:
      # Time-based rules
      - sys.hour >= 22 || sys.hour <= 6          # Late night
      - sys.is_weekend == true                    # Weekend
      - sys.day_of_week in ["monday", "friday"]  # Specific days

      # Environment-specific rules
      - sys.environment == "production"
      - sys.region in ["us-east-1", "eu-west-1"]

      # Performance monitoring
      - sys.execution_time_ms < sys.timeout_ms - 1000  # Buffer check
  score: 40
```

---

## 8. env - Environment Configuration

### Description

The `env` namespace contains **configuration** loaded from environment variables and config files.

```yaml
Mutability: Read-only
Source: Environment variables, config files
Lifecycle: Loaded at startup
```

### Use Cases

- Database connection configuration
- API keys and secrets
- Timeout settings
- Feature flags
- Environment-specific configuration
- External service endpoints

### Example Configuration

```yaml
env:
  # Database
  database_url: "postgresql://..."
  db_pool_size: 20

  # API Configuration
  api_timeout_ms: 3000
  max_retries: 3

  # Feature Flags
  feature_flags:
    new_ml_model: true
    advanced_analytics: false

  # External Services
  seon_api_key: "***"
  openai_api_key: "***"
```

### Using Environment Variables

```yaml
# In rule conditions
rule:
  id: env_based_check
  when:
    all:
      - env.feature_flags.new_ml_model == true
      - event.risk_score > env.custom_threshold
  score: 55

# In API configuration
pipeline:
  steps:
    - step_type: api
      config:
        endpoint: env.seon_endpoint
        api_key: env.seon_api_key
        timeout_ms: env.api_timeout_ms
```

---

## Data Classification Decision Tree

When receiving or computing data, use this tree to determine the correct namespace:

```
Received a piece of data, where should it go?
│
├─ Is it raw data from user API request?
│  └─ Yes → event
│
├─ Is it system auto-generated metadata?
│  └─ Yes → sys
│
├─ Is it loaded from environment variables/config files?
│  └─ Yes → env
│
├─ Is it the output of a ruleset/pipeline execution (signal, decision, etc.)?
│  └─ Yes → results
│
├─ Is it computed result based on existing data?
│  │
│  ├─ Simple calculation (arithmetic, string concat)?
│  │  └─ Yes → vars
│  │
│  ├─ Requires querying historical data/database aggregation?
│  │  └─ Yes → features
│  │
│  ├─ Obtained by calling external third-party API?
│  │  └─ Yes → api
│  │
│  └─ Obtained by calling internal microservice?
│     └─ Yes → service
```

---

## Complete Usage Examples

### Comprehensive Rule

```yaml
rule:
  id: comprehensive_fraud_check
  name: Comprehensive Fraud Detection

  when:
    all:
      # 1. event - Raw request data
      - event.transaction.amount > 10000
      - event.user.account_age_days < 30
      - event.device.ip != ""

      # 2. features - Historical features
      - features.user_transaction_count_7d > 20
      - features.avg_transaction_amount_30d < 1000
      - features.velocity_score > 0.8

      # 3. api - External validation
      - api.device_fingerprint.risk_score > 0.7
      - api.ip_geolocation.country != event.user.registered_country

      # 4. service - Internal services
      - service.user_profile.vip_level == "normal"
      - service.risk_history.previous_fraud_count > 0

      # 5. vars - Configuration and calculations
      - event.transaction.amount > vars.high_risk_threshold

      # 6. sys - System information
      - sys.hour >= 22 || sys.hour <= 6        # Late night
      - sys.environment == "production"

      # 7. env - Environment config
      - env.feature_flags.strict_mode == true

  score: 90
```

### Pipeline Example

```yaml
pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline
  entry: fraud_check

  when:
    all:
      - event.type == "transaction"

  steps:
    - step:
        id: fraud_check
        name: Fraud Risk Assessment
        type: ruleset
        ruleset: comprehensive_fraud_ruleset

  decision:
    - when: results.comprehensive_fraud_ruleset.signal == "decline"
      result: decline
      reason: "{results.comprehensive_fraud_ruleset.reason}"
      terminate: true

    - when: results.comprehensive_fraud_ruleset.signal == "review"
      result: review
      actions: ["manual_review"]
      reason: "{results.comprehensive_fraud_ruleset.reason}"
      terminate: true

    - default: true
      result: approve
      reason: "No significant risk detected"
```

---

## Best Practices

### Naming Conventions

✅ **Good:**
```yaml
vars:
  risk_threshold_high: 80
  is_new_user: true
  user_account_age_days: 30

features:
  user_transaction_count_7d: 15
  avg_transaction_amount_30d: 3200.5
```

❌ **Bad:**
```yaml
vars:
  t: 80                        # Unclear
  flag: true                   # Ambiguous
  x: 30                        # Meaningless
```

### Namespace Selection

✅ **Correct:**
```yaml
# Simple math → vars
vars.total_fee: event.amount * 0.031

# Database query → features
features.transaction_count_7d: COUNT(...)

# External API → api
api.device_fingerprint.score: 0.75
```

❌ **Incorrect:**
```yaml
# Don't mix namespaces
features.simple_addition: 1 + 1     # Should be vars
vars.historical_count: COUNT(...)   # Should be features
```

### Avoid Namespace Pollution

✅ **Good:** Clean, minimal data
```yaml
api.ip_check:
  score: 0.75
  risk_level: "high"
  country: "US"
```

❌ **Bad:** Storing unnecessary data
```yaml
api.ip_check:
  raw_response: { ... }       # Large, unused
  debug_info: { ... }         # Not needed
  internal_state: { ... }     # Implementation detail
```

---

## Benefits

### 1. Clarity
- ✅ Data source immediately obvious
- ✅ Rules easier to understand and maintain
- ✅ Faster onboarding for new developers

### 2. Security
- ✅ Prevent users from submitting reserved fields
- ✅ Schema validation
- ✅ Type safety

### 3. Extensibility
- ✅ Adding new data sources only requires new namespace
- ✅ No impact on existing rules
- ✅ Backward compatible

### 4. Observability
- ✅ Clear understanding of which step produced which data
- ✅ Better debugging experience
- ✅ More accurate performance analysis

---

## Design Principles Summary

1. **Single Responsibility** - Each namespace handles only one type of data
2. **Classify by Processing** - Classify by processing method, not data source
3. **Flattened** - All namespaces at the same level
4. **Explicit over Implicit** - Clear namespace prefixes
5. **Read-only Protection** - System data immutable
6. **Backward Compatible** - Phased migration, no breaking changes

---

## Summary

CORINT's flattened namespace architecture provides:

- **Clear data separation** - 8 namespaces (7 core + results pipeline-level)
- **Simple access** - No `ctx.` prefix needed, direct namespace access
- **Type safety** - Read-only vs writable enforcement
- **Extensibility** - Easy to add new data sources
- **Observability** - Clear tracking of data origin and processing

**The 7 Core Namespaces (✅ Implemented):**
1. `event` - User request raw data (read-only)
2. `features` - Complex computations (writable)
3. `api` - External API results (writable)
4. `service` - Internal service results (writable)
5. `vars` - Simple variables (writable)
6. `sys` - System metadata (read-only)
7. `env` - Environment config (read-only)

**Pipeline-Level Namespace (⚠️ Partially Implemented):**
8. `results` - Ruleset execution results (read-only, pipeline execution layer)

---

**Last Updated**: 2024-12-14
**Status**: Design complete, implementation in progress
