# Template Syntax Reference

This document defines the unified template and reference syntax across all CORINT DSL components.

## Three Syntax Systems

CORINT uses three distinct syntax systems for different purposes:

| Syntax | When | Timing | Example |
|--------|------|--------|---------|
| **Direct Reference** | `namespace.field` | Runtime evaluation | `event.amount > 100` |
| **String Interpolation** | `${namespace.field}` | Runtime substitution | `"User: ${event.user.id}"` |
| **Config Reference** | `@{config.path}` | **Compile-time** substitution | `@{fraud_detector.api_key}` |

---

## 1. Direct Reference (Runtime Evaluation)

**Used in**: Conditions, params mapping, field access

**Syntax**: `namespace.field` (no wrapper)

**Valid Namespaces**: `event`, `features`, `api`, `service`, `vars`, `sys`, `results`

**Examples**:

### Conditions
```yaml
when:
  all:
    - event.amount > 1000                   # ✅ Direct field evaluation
    - features.risk_score >= 80             # ✅ Direct field evaluation
    - api.device_check.is_proxy == true     # ✅ Direct field evaluation
```

### Params Mapping
```yaml
params:
  user_id: event.user.id                    # ✅ Direct path mapping
  amount: event.transaction.amount          # ✅ Direct path mapping
  risk_score: features.risk_score           # ✅ Direct path mapping
```

---

## 2. String Interpolation (Runtime Substitution)

**Used in**: Feature definitions, request bodies, reason messages, any string that needs runtime value substitution

**Syntax**: `${namespace.field}` or `${param}` (inside string)

**When**: Runtime - each request execution

**Examples**:

### Feature Definitions (SQL Generation)
```yaml
- name: user_transaction_count
  type: aggregation
  method: count
  dimension: user_id
  dimension_value: "${event.user_id}"       # ✅ Interpolate into SQL WHERE clause
  window: 24h
  when:
    all:
      - type == "transaction"               # Database column (no prefix)
      - user_id == "${event.user_id}"       # Request value (interpolated)
```

**Generated SQL** (conceptual):
```sql
SELECT COUNT(*)
FROM events
WHERE type = 'transaction'
  AND user_id = $1                          -- Parameterized with event.user_id value
  AND timestamp > NOW() - INTERVAL '24 hours'
```

### Request Body Templates
```yaml
endpoint: analyze_transaction
  method: POST
  params:
    transaction_id: event.transaction_id    # Direct mapping
    amount: event.amount
    verified: event.user.verified
  request_body: |
    {
      "transaction_id": "${transaction_id}",   # String: quoted
      "amount": ${amount},                     # Number: not quoted
      "verified": ${verified},                 # Boolean: not quoted
      "user_email": "${event.user.email}"      # Direct context reference
    }
```

**Type-Aware Quoting**:
- **String values**: `"${param}"` (with JSON quotes)
- **Number values**: `${param}` (without JSON quotes)
- **Boolean values**: `${param}` (without JSON quotes)

### Reason Messages with Interpolation
```yaml
# Pipeline decision with dynamic reason
decision:
  - when: total_score >= 100
    signal: decline
    reason: "Risk score ${total_score} exceeds threshold"   # ✅ Dynamic

  - when: triggered_count >= 3
    signal: review
    reason: "Multiple indicators: ${triggered_rules}"       # ✅ Dynamic

  - default: true
    signal: approve
    reason: "Low risk, approved"                            # ✅ Static (no interpolation needed)
```

---

## 3. Config Reference (Compile-time Substitution)

**Used in**: API/Service configuration files (auth, base_url, timeouts)

**Syntax**: `@{config.path}` where `config.path` maps to nested YAML in `config/server.yaml`

**When**: **Compile-time** - substituted during DSL compilation, zero runtime overhead

**Examples**:

### Configuration File Structure
```yaml
# config/server.yaml
fraud_detector:
  api_key: "sk-abc123..."
  base_url: "https://fraud-api.example.com"
  timeout_ms: 5000

supabase:
  url: "https://xxx.supabase.co"
  anon_key: "eyJhbGc..."
  service_role_key: "eyJhbGc..."

redis:
  host: "localhost"
  port: 6379
  password: "secret123"
```

### Using Config References
```yaml
# repository/configs/apis/fraud_detector.yaml
apis:
  - id: fraud_detector
    name: Fraud Detection API
    base_url: "@{fraud_detector.base_url}"      # ✅ Compile-time substitution
    timeout_ms: "@{fraud_detector.timeout_ms}"
    auth:
      type: header
      name: "X-API-Key"
      value: "@{fraud_detector.api_key}"        # ✅ Compile-time substitution

    endpoints:
      analyze:
        method: POST
        path: /v1/analyze
        params:
          user_id: event.user.id                # ✅ Runtime data (direct reference)
          amount: event.transaction.amount
```

### Service Configuration
```yaml
# repository/configs/services/postgres.yaml
services:
  - id: postgres_main
    type: ms_http
    base_url: "@{postgres.base_url}"            # ✅ Compile-time
    auth:
      type: header
      name: "apikey"
      value: "@{postgres.service_role_key}"     # ✅ Compile-time
```

**Benefits of Compile-time Substitution:**
- ✅ **Zero runtime overhead** - config already resolved
- ✅ **Security** - secrets don't appear in compiled IR
- ✅ **Early validation** - missing config caught at compile time
- ✅ **YAML-friendly** - nested paths match natural YAML structure

---

## 4. Syntax Comparison

| Syntax | Timing | Data Source | Use Case | Example |
|--------|--------|-------------|----------|---------|
| `namespace.field` | Runtime | Context namespaces | Direct evaluation | `event.amount > 100` |
| `${namespace.field}` | Runtime | Context namespaces | String interpolation | `"User: ${event.user.id}"` |
| `@{config.path}` | **Compile-time** | `config/server.yaml` | Configuration | `@{fraud_detector.api_key}` |

---

## 5. Special Cases

### URL Path Parameters

**URL path parameters** use single curly braces `{param}` (RESTful API standard):

```yaml
# ✅ Correct - URL path parameter (REST standard)
service:
  endpoints:
    get_user:
      path: /api/v1/users/{user_id}/orders/{order_id}
      params:
        user_id: event.user.id         # Direct mapping
        order_id: event.order.id
```

**Why different from `${}`?**
- `{placeholder}` in URL paths → HTTP client library standard (OpenAPI/Swagger)
- `${template}` in strings → CORINT template engine

---

## 6. Common Mistakes

### ❌ Don't wrap direct references with templates
```yaml
# Wrong
params:
  user_id: "${event.user.id}"

# Correct
params:
  user_id: event.user.id
```

### ❌ Don't use templates in conditions
```yaml
# Wrong
when:
  all:
    - "${event.amount}" > 1000

# Correct
when:
  all:
    - event.amount > 1000
```

### ❌ Don't forget ${} in string interpolation
```yaml
# Wrong - won't substitute
dimension_value: "event.user_id"

# Correct
dimension_value: "${event.user_id}"
```

### ❌ Don't use ${} for config references
```yaml
# Wrong - tries runtime substitution, will fail
auth:
  value: "${env.fraud_detector.api_key}"

# Correct - compile-time config reference
auth:
  value: "@{fraud_detector.api_key}"
```

### ❌ Don't use runtime namespaces in config references
```yaml
# Wrong - @{} only accepts config paths
base_url: "@{event.base_url}"

# Correct - use runtime interpolation for dynamic values
base_url: "${vars.computed_base_url}"
```

---

## 7. Migration Guide

| Old Pattern | New Pattern | Reason |
|-------------|-------------|--------|
| `${env.fraud_detector.api_key}` | `@{fraud_detector.api_key}` | Config reference uses `@{}` |
| `params: "${event.x}"` | `params: event.x` | Direct mapping, no template |
| `dimension_value: "{event.x}"` | `dimension_value: "${event.x}"` | Use `${}` for interpolation |
| `when: "${event.x}" > 100` | `when: event.x > 100` | Direct evaluation, no template |
| `reason: "Static only"` | `reason: "Score: ${total_score}"` | Can now interpolate |

---

## 8. Quick Decision Tree

```
Need to reference a value?
│
├─ Is it from config/server.yaml? → Use @{config.path}
│  └─ Examples: API keys, URLs, timeouts
│
├─ Is it inside a string that needs substitution?
│  └─ Use ${namespace.field}
│     └─ Examples: "User: ${event.user.id}", dimension_value: "${event.user_id}"
│
└─ Otherwise (direct evaluation/mapping)
   └─ Use namespace.field
      └─ Examples: event.amount > 100, params: { user_id: event.user.id }
```

---

## 9. Summary

CORINT uses **three syntax systems**:

1. **`namespace.field`** - Direct reference (runtime evaluation)
2. **`${namespace.field}`** - String interpolation (runtime substitution)
3. **`@{config.path}`** - Config reference (compile-time substitution)

The key distinction:
- `@{}` → **Compile-time** from config files (secrets, URLs)
- `${}` → **Runtime** from context namespaces (user data, computed values)
- No wrapper → **Direct** evaluation or mapping
