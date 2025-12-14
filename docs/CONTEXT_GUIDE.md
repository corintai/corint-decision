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
   - Read-only: event, sys, env
   - Writable: features, api, service, llm, vars

---

## Namespace Definitions

### 1. event - User Request Raw Data
```yaml
Description: Raw business data from API requests (unprocessed)
Source: User API requests
Mutability: Read-only
Examples:
  event.user.id: "user123"
  event.transaction.amount: 5000
  event.device.ip: "203.0.113.1"
```

**Contents**:
- All business data submitted by users
- Validated by schema to ensure no reserved fields
- Does not include system metadata (request_id, timestamp go in sys)

**Validation Rules**:
```rust
// Reserved fields, forbidden for user submission
const RESERVED_FIELDS: &[&str] = &[
    "total_score",
    "triggered_rules",
    "sys_*",
    "features_*",
    "api_*",
    "service_*",
    "llm_*"
];
```

---

### 2. features - Complex Feature Computation Results
```yaml
Description: Complex features computed through feature steps
Source: Pipeline feature steps
Mutability: Writable (by feature steps only)
Examples:
  features.user_transaction_count_7d: 15
  features.avg_transaction_amount_30d: 3200.5
  features.device_first_seen_days: 45
```

**Storage Method**:
```rust
ctx.store_feature("user_transaction_count_7d", Value::Number(15.0));
```

**Use Cases**:
- Database aggregation queries (sum, avg, count)
- Time window statistics (7 days, 30 days, 90 days)
- Historical behavior analysis
- Complex scoring models

---

### 3. api - External API Call Results
```yaml
Description: Results from calling external third-party APIs
Source: Pipeline api steps
Mutability: Writable (by api steps only)
Examples:
  api.device_fingerprint.risk_score: 0.75
  api.ip_geolocation.country: "US"
  api.email_verification.is_valid: true
```

**Storage Method**:
```rust
ctx.store_api_result("device_fingerprint", api_response_value);
```

**Use Cases**:
- Device fingerprinting (FingerprintJS, Seon)
- IP geolocation queries
- Email/phone verification
- Credit score queries
- KYC/AML checks

---

### 4. service - Internal Service Call Results
```yaml
Description: Results from calling internal microservices
Source: Pipeline service steps
Mutability: Writable (by service steps only)
Examples:
  service.user_profile.vip_level: "gold"
  service.risk_history.blacklist_hit: false
  service.inventory.stock_available: 100
```

**Storage Method**:
```rust
ctx.store_service_result("user_profile", service_response);
```

**Use Cases**:
- User profile service
- Risk history records
- Inventory query service
- Order management service
- Points/membership system

---

### 5. llm - LLM Analysis Results
```yaml
Description: Results from large language model analysis
Source: Pipeline llm steps
Mutability: Writable (by llm steps only)
Examples:
  llm.address_verification.is_suspicious: true
  llm.content_moderation.category: "spam"
  llm.fraud_analysis.risk_reason: "Address information contradictory"
```

**Storage Method**:
```rust
ctx.store_llm_result("address_verification", llm_analysis);
```

**Use Cases**:
- Address authenticity verification
- Content moderation
- Fraud reason analysis
- Anomaly behavior explanation
- Intelligent Q&A/dialogue

---

### 6. vars - Simple Variables and Intermediate Calculations
```yaml
Description: Pipeline variables, simple calculations, temporary values
Source: Pipeline vars config + rule calculations
Mutability: Writable
Examples:
  vars.high_risk_threshold: 80
  vars.min_transaction_amount: 100
  vars.risk_multiplier: 1.5
  vars.total_fee: 15.5  # Calculated: amount * 0.031
```

**Storage Method**:
```rust
ctx.store_var("total_fee", calculated_value);
```

**Use Cases**:
- Pipeline configuration parameters
- Rule thresholds
- Simple math calculations (+, -, ×, ÷)
- String concatenation
- Boolean judgment results

**Difference from features**:
- vars: Simple calculations, no external data needed
- features: Complex calculations, requires historical data queries

---

### 7. sys - System Injected Metadata
```yaml
Description: Metadata and context information automatically injected by system
Source: System auto-generated
Mutability: Read-only
Examples:
  sys.request_id: "550e8400-e29b-41d4-a716-446655440000"
  sys.timestamp: "2024-01-15T10:30:00Z"
  sys.environment: "production"
  sys.region: "us-west-1"
```

**Core Field Categories**:

1. **Request Identification**
   - sys.request_id
   - sys.correlation_id

2. **Time Information**
   - sys.timestamp (ISO 8601)
   - sys.timestamp_ms (Unix milliseconds)
   - sys.date (YYYY-MM-DD)
   - sys.time (HH:MM:SS)
   - sys.hour (0-23)
   - sys.day_of_week (monday-sunday)
   - sys.is_weekend (boolean)

3. **Environment Information**
   - sys.environment (development/staging/production)
   - sys.region (deployment region)
   - sys.tenant_id (multi-tenant ID)

4. **Execution Context**
   - sys.pipeline_id
   - sys.pipeline_version
   - sys.ruleset_id
   - sys.rule_id

5. **Performance Metrics**
   - sys.execution_time_ms
   - sys.execution_step
   - sys.timeout_ms

6. **Version Information**
   - sys.corint_version
   - sys.api_version

7. **Client Information**
   - sys.client_id
   - sys.client_ip
   - sys.user_agent

8. **Debug Information**
   - sys.debug_mode
   - sys.trace_enabled

---

### 8. env - Environment Configuration
```yaml
Description: Configuration loaded from environment variables and config files
Source: Environment variables, config files
Mutability: Read-only
Examples:
  env.database_url: "postgresql://..."
  env.api_timeout_ms: 3000
  env.feature_flags.new_ml_model: true
```

**Use Cases**:
- Database connection configuration
- API keys
- Timeout settings
- Feature flags
- Environment-specific configuration

---

## Complete Architecture Code

### ExecutionContext Structure

```rust
use std::collections::HashMap;
use serde_json::Value;

pub struct ExecutionContext {
    // ========== 8 Namespaces ==========

    /// User request raw data (read-only)
    pub event: HashMap<String, Value>,

    /// Complex feature computation results (writable)
    pub features: HashMap<String, Value>,

    /// External API call results (writable)
    pub api: HashMap<String, Value>,

    /// Internal service call results (writable)
    pub service: HashMap<String, Value>,

    /// LLM analysis results (writable)
    pub llm: HashMap<String, Value>,

    /// Simple variables and intermediate calculations (writable)
    pub vars: HashMap<String, Value>,

    /// System injected metadata (read-only)
    pub sys: HashMap<String, Value>,

    /// Environment configuration (read-only)
    pub env: HashMap<String, Value>,

    // ========== Internal Fields ==========

    /// Expression evaluation stack
    pub(crate) stack: Vec<Value>,

    /// Execution result
    pub(crate) result: ExecutionResult,
}

impl ExecutionContext {
    pub fn new(event_data: HashMap<String, Value>) -> Self {
        Self {
            event: event_data,
            features: HashMap::new(),
            api: HashMap::new(),
            service: HashMap::new(),
            llm: HashMap::new(),
            vars: HashMap::new(),
            sys: Self::build_system_vars(),
            env: Self::load_environment_vars(),
            stack: Vec::new(),
            result: ExecutionResult::default(),
        }
    }

    // ========== Data Storage Methods ==========

    /// Store feature computation result
    pub fn store_feature(&mut self, name: &str, value: Value) {
        self.features.insert(name.to_string(), value);
    }

    /// Store API call result
    pub fn store_api_result(&mut self, api_name: &str, result: Value) {
        self.api.insert(api_name.to_string(), result);
    }

    /// Store service call result
    pub fn store_service_result(&mut self, service_name: &str, result: Value) {
        self.service.insert(service_name.to_string(), result);
    }

    /// Store LLM analysis result
    pub fn store_llm_result(&mut self, step_id: &str, analysis: Value) {
        self.llm.insert(step_id.to_string(), analysis);
    }

    /// Store variable
    pub fn store_var(&mut self, name: &str, value: Value) {
        self.vars.insert(name.to_string(), value);
    }

    // ========== Field Lookup (supports dot notation) ==========

    pub fn get_field(&self, field_path: &str) -> Option<Value> {
        // Parse namespace and field path
        let parts: Vec<&str> = field_path.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        let namespace = parts[0];
        let remaining_path = &parts[1..];

        // Lookup by namespace
        let namespace_data = match namespace {
            "event" => &self.event,
            "features" => &self.features,
            "api" => &self.api,
            "service" => &self.service,
            "llm" => &self.llm,
            "vars" => &self.vars,
            "sys" => &self.sys,
            "env" => &self.env,
            _ => return None,
        };

        // If only namespace, return entire namespace
        if remaining_path.is_empty() {
            return Some(Value::Object(
                namespace_data.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            ));
        }

        // Recursively find nested fields
        Self::get_nested_value(namespace_data, remaining_path)
    }

    fn get_nested_value(data: &HashMap<String, Value>, path: &[&str]) -> Option<Value> {
        if path.is_empty() {
            return None;
        }

        let key = path[0];
        let value = data.get(key)?;

        if path.len() == 1 {
            return Some(value.clone());
        }

        // Continue searching down
        match value {
            Value::Object(map) => {
                let remaining = &path[1..];
                let mut hash_map = HashMap::new();
                for (k, v) in map.iter() {
                    hash_map.insert(k.clone(), v.clone());
                }
                Self::get_nested_value(&hash_map, remaining)
            }
            _ => None,
        }
    }

    // ========== System Variables Builder ==========

    fn build_system_vars() -> HashMap<String, Value> {
        let mut sys = HashMap::new();
        let now = chrono::Utc::now();

        // Request identification
        sys.insert(
            "request_id".to_string(),
            Value::String(uuid::Uuid::new_v4().to_string())
        );

        // Time information
        sys.insert("timestamp".to_string(), Value::String(now.to_rfc3339()));
        sys.insert("timestamp_ms".to_string(), Value::Number(now.timestamp_millis().into()));
        sys.insert("date".to_string(), Value::String(now.format("%Y-%m-%d").to_string()));
        sys.insert("time".to_string(), Value::String(now.format("%H:%M:%S").to_string()));
        sys.insert("hour".to_string(), Value::Number(now.hour().into()));

        let day_of_week = match now.weekday() {
            chrono::Weekday::Mon => "monday",
            chrono::Weekday::Tue => "tuesday",
            chrono::Weekday::Wed => "wednesday",
            chrono::Weekday::Thu => "thursday",
            chrono::Weekday::Fri => "friday",
            chrono::Weekday::Sat => "saturday",
            chrono::Weekday::Sun => "sunday",
        };
        sys.insert("day_of_week".to_string(), Value::String(day_of_week.to_string()));

        let is_weekend = matches!(now.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun);
        sys.insert("is_weekend".to_string(), Value::Bool(is_weekend));

        // Environment information
        sys.insert(
            "environment".to_string(),
            Value::String(std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()))
        );

        // Version information
        sys.insert("corint_version".to_string(), Value::String(env!("CARGO_PKG_VERSION").to_string()));

        sys
    }

    fn load_environment_vars() -> HashMap<String, Value> {
        HashMap::new() // TODO: Load from config file
    }
}
```

---

## Usage Examples

### Accessing Different Namespaces in Rules

```yaml
rule:
  id: comprehensive_fraud_check
  name: Comprehensive Fraud Detection

  when:
    conditions:
      # 1. event - Raw request data
      - event.transaction.amount > 10000
      - event.user.account_age_days < 30

      # 2. features - Historical features
      - features.user_transaction_count_7d > 20
      - features.avg_transaction_amount_30d < 1000

      # 3. api - External validation
      - api.device_fingerprint.risk_score > 0.7
      - api.ip_geolocation.country != event.user.registered_country

      # 4. service - Internal services
      - service.user_profile.vip_level == "normal"
      - service.risk_history.previous_fraud_count > 0

      # 5. llm - AI analysis
      - llm.behavior_analysis.is_suspicious == true

      # 6. vars - Configuration and calculations
      - event.transaction.amount > vars.high_risk_threshold

      # 7. sys - System information
      - sys.hour >= 22 || sys.hour <= 6  # Late night transactions
      - sys.environment == "production"

  score: 90
```

### Populating Namespaces in Pipeline

```yaml
pipeline:
  id: fraud_detection
  name: Fraud Detection Pipeline

  steps:
    # 1. Configure variables
    - step_type: vars
      config:
        high_risk_threshold: 50000
        api_timeout_ms: 3000

    # 2. Compute features
    - step_type: feature
      config:
        - name: user_transaction_count_7d
          query: "SELECT COUNT(*) FROM transactions WHERE user_id = $1 AND created_at > NOW() - INTERVAL '7 days'"

    # 3. Call external API
    - step_type: api
      config:
        name: device_fingerprint
        endpoint: "https://api.seon.io/device"
        timeout_ms: 3000

    # 4. Call internal service
    - step_type: service
      config:
        name: user_profile
        endpoint: "http://user-service/profile"

    # 5. LLM analysis
    - step_type: llm
      config:
        prompt: "Analyze if the following transaction is anomalous: {event}"
        model: "gpt-4"

    # 6. Execute rules
    - step_type: ruleset
      config:
        rules:
          - comprehensive_fraud_check
```

---

## Data Classification Decision Tree

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
│  ├─ Obtained by calling internal microservice?
│  │  └─ Yes → service
│  │
│  └─ Obtained through LLM analysis?
│     └─ Yes → llm
```

---

## Migration Plan

### Current Problems

```rust
// ❌ Current implementation issues
pub struct ExecutionContext {
    pub event_data: HashMap<String, Value>,  // All data mixed here
    pub result: ExecutionResult,
}

// features wrongly stored in event_data
ctx.event_data.insert(feature_name.clone(), feature_value.clone());
```

### Migration Steps

1. **Phase 1: Add new fields, maintain compatibility**
   ```rust
   pub struct ExecutionContext {
       // Add 8 new namespaces
       pub event: HashMap<String, Value>,
       pub features: HashMap<String, Value>,
       // ... other 6

       // Keep old field temporarily (mark as deprecated)
       #[deprecated]
       pub event_data: HashMap<String, Value>,
   }
   ```

2. **Phase 2: Update storage logic**
   - pipeline_executor.rs: Modify feature storage location
   - api_executor.rs: Modify API result storage location
   - service_executor.rs: Modify service result storage location

3. **Phase 3: Update field lookup logic**
   - context.rs: Implement new get_field() with namespace support
   - Maintain backward compatibility for old field access

4. **Phase 4: Add input validation**
   - Validate event doesn't contain reserved fields
   - Schema validation

5. **Phase 5: Remove old fields**
   - Remove event_data
   - Clean up compatibility code

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

## Documentation Index

For detailed documentation, refer to:

- [SYS_NAMESPACE_SPEC.md](../repository/test_data/SYS_NAMESPACE_SPEC.md) - sys namespace detailed specification
- [DERIVED_DATA_PLACEMENT.md](../repository/test_data/DERIVED_DATA_PLACEMENT.md) - Derived data placement guide
- [EVENT_DESIGN_CONSIDERATIONS.md](../repository/test_data/EVENT_DESIGN_CONSIDERATIONS.md) - event object design considerations
- [UNIFIED_CONTEXT_PROPOSAL_V3.md](../repository/test_data/UNIFIED_CONTEXT_PROPOSAL_V3.md) - Complete proposal and implementation details

---

**Last Updated**: 2024-12-14
**Status**: Design complete, pending implementation
