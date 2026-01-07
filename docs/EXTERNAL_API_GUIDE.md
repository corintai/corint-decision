# External API Integration Guide

## Overview

CORINT Decision Engine provides a generic, configurable system for integrating with external APIs. This guide covers the complete workflow from API configuration to pipeline integration.

**Design Philosophy**: Start simple, extend gradually. The MVP focuses on essential features with clear upgrade paths.

---

## Quick Start

### 1. Create API Configuration

Create `repository/configs/apis/ipinfo.yaml`:

```yaml
name: ipinfo
base_url: "https://ipinfo.io"

# Optional: Authentication
auth:
  type: header
  name: "Authorization"
  value: "Bearer ${IPINFO_API_KEY}"  # Environment variable

endpoints:
  ip_lookup:
    method: GET
    path: "/{ip}"  # Path parameters in {brackets}
    params:
      ip: event.ip_address          # Read from event context
      token: "a63066c9a63590"        # Literal value
    query_params:
      - token       # Query parameter names
    response:
      mapping:
        is_vpn: privacy.vpn         # Extract nested field
      fallback:
        is_vpn: false
        error: true
```

### 2. Use in Pipeline

```yaml
pipeline:
  - type: api
    api: ipinfo
    endpoint: ip_lookup
    # Parameters automatically read from context (defined in API config)
    # Result automatically stored in: api.ipinfo.ip_lookup
```

### 3. Use API Response

```yaml
rule:
  id: block_vpn
  when:
    all:
      - event.type == "payment"
      - api.ipinfo.ip_lookup.is_vpn == true  # Default output location
  score: 100
```

---

## Parameter Mapping

### Defining Parameters in API Config

Parameters are defined in the API configuration with context paths or literal values:

```yaml
endpoints:
  check_user:
    method: GET
    path: "/users/{user_id}"
    params:
      # Context paths (automatically read at runtime)
      user_id: event.user.id           # Read from event data
      ip: event.ip_address             # Read from event
      session: context.session_id      # Read from context

      # Literal values (used as-is)
      api_version: "v2"                # String literal
      limit: 100                       # Number literal
      include_details: true            # Boolean literal
```

**Rules**:
- Context path (no quotes, contains `.`) → Automatically resolved from context at runtime
- Quoted string → String literal
- Unquoted number → Number literal
- Unquoted boolean → Boolean literal

**How It Works**:
1. You define parameter mapping in API config
2. Pipeline step automatically reads values from context when executing
3. No need to specify params in pipeline (unless overriding)

**Examples**:

```yaml
# API Config
params:
  transaction_id: event.transaction_id   # Context path
  amount: event.amount                   # Context path
  currency: "USD"                        # Literal

# Pipeline - no params needed!
- type: api
  api: fraud_api
  endpoint: check_transaction
  # Parameters automatically resolved from context
```

### Parameter Mapping Rules

Parameters are defined in API config and automatically resolved from context:

| Config Location | API Config Param Mapping | Usage |
|-----------------|--------------------------|-------|
| `path: "/{id}"` | `id: event.user_id` | Path replacement from context |
| `query_params: [token]` | `token: "abc123"` | Query string (literal value) |
| `request_body: "...${field}..."` | `field: event.data` | Body substitution from context |

**Parameter Resolution Priority** (highest to lowest):
1. Pipeline step `params` (override)
2. API config endpoint `params` (default mapping)
3. Error if required parameter missing

**How Parameters Work**:

- **Path Parameters**: Placeholders `{param_name}` in path are replaced with param values from context
- **Query Parameters**: Only params listed in `query_params` are appended as `?key=value` from context
- **Body Parameters**: Used in `request_body` template replacements from context

**Example**:
```yaml
# API Config
endpoints:
  get_user:
    method: GET
    path: "/users/{user_id}/posts/{post_id}"
    params:
      user_id: event.user.id        # Read from context
      post_id: event.post.id         # Read from context
      limit: 10                      # Literal value
      offset: 0                      # Literal value
    query_params:
      - limit
      - offset

# When event.user.id = "123" and event.post.id = "456"
# Result URL: https://api.example.com/users/123/posts/456?limit=10&offset=0
```

---

## API Configuration

### File Location

All API configurations must be placed in:
```
repository/configs/apis/
├── ipinfo.yaml
├── chainalysis.yaml
└── your_api.yaml
```

Files are automatically loaded on engine startup.

### Configuration Structure

```yaml
name: <api_identifier>              # Required: Unique ID
base_url: <base_url>                # Required: Base URL

# Optional: Authentication
auth:
  type: header                      # Only 'header' supported in MVP
  name: <header_name>               # e.g., "Authorization", "X-API-Key"
  value: <value>                    # Can use ${env.x.y.z}

# Optional: Default timeout for all endpoints
timeout_ms: 5000                    # Default: 10000ms

# Required: Endpoint definitions
endpoints:
  <endpoint_name>:
    method: <HTTP_method>           # GET, POST, PUT, DELETE, PATCH
    path: <url_path>                # Can include {placeholders}
    timeout_ms: <integer>           # Optional: Timeout for this endpoint (overrides API default)

    # Optional: Parameter mapping from context
    params:
      <param_name>: <context_path>  # e.g., user_id: event.user.id
      <param_name>: <literal>       # e.g., api_version: "v2"

    # Optional: Query parameter names
    query_params:
      - <param_name>
      - <param_name>

    # Optional: For POST/PUT/PATCH
    request_body: |
      {
        "field": "${field_name}"
      }

    # Optional: Response handling
    response:
      # Field mapping: rename/extract fields
      mapping:
        <output_field>: <response_field>

      # Fallback value on error
      fallback:
        <field>: <value>
```

### Configuration Fields Reference

#### API-Level Fields

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `name` | Yes | string | Unique identifier for this API |
| `base_url` | Yes | string | Base URL (e.g., "https://api.example.com") |
| `auth` | No | object | Authentication configuration |
| `auth.type` | If auth | string | Only "header" supported |
| `auth.name` | If auth | string | Header name |
| `auth.value` | If auth | string | Header value, can use `${env.x.y.z}` |
| `timeout_ms` | No | integer | Default timeout in milliseconds (default: 10000) |
| `endpoints` | Yes | map | Map of endpoint definitions |

#### Endpoint-Level Fields

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `method` | Yes | enum | HTTP method (GET/POST/PUT/PATCH/DELETE) |
| `path` | Yes | string | URL path, can include `{placeholder}` |
| `timeout_ms` | No | integer | Timeout for this endpoint (overrides API default) |
| `params` | No | map | Parameter mapping: `param_name: context_path` or literal |
| `query_params` | No | array | List of query parameter names |
| `request_body` | No | string | JSON template with `${placeholder}` for POST/PUT/PATCH |
| `response` | No | object | Response handling configuration |
| `response.mapping` | No | map | Field mapping: `output_field: response_field` |
| `response.fallback` | No | map | Fallback value on error (4xx, 5xx, timeout) |

### Examples

#### Simple GET API

```yaml
name: ipinfo
base_url: "https://ipinfo.io"

endpoints:
  ip_lookup:
    method: GET
    path: "/{ip}"
    params:
      ip: event.ip_address        # Read from event
      token: "your-token-here"    # Literal value
    query_params:
      - token
```

**Usage**: Generates `https://ipinfo.io/8.8.8.8?token=your-token-here` when `event.ip_address` is "8.8.8.8"

#### POST API with Body

```yaml
name: fraud_service
base_url: "https://api.fraud.example.com"

auth:
  type: header
  name: "X-API-Key"
  value: "${FRAUD_API_KEY}"

endpoints:
  check_transaction:
    method: POST
    path: "/v1/transactions/check"
    params:
      transaction_id: event.transaction_id   # Read from event
      amount: event.amount                   # Read from event
      currency: "USD"                        # Literal value
    request_body: |
      {
        "transaction_id": "${transaction_id}",
        "amount": ${amount},
        "currency": "${currency}"
      }
```

**Note**: In `request_body`, `${}` placeholders are replaced with param values from context.

#### API with Multiple Endpoints

```yaml
name: user_service
base_url: "https://api.example.com"

auth:
  type: header
  name: "Authorization"
  value: "Bearer ${USER_SERVICE_TOKEN}"

endpoints:
  get_user:
    method: GET
    path: "/users/{user_id}"
    params:
      user_id: event.user.id     # Read from event

  create_user:
    method: POST
    path: "/users"
    params:
      name: event.user.name      # Read from event
      email: event.user.email    # Read from event
    request_body: |
      {
        "name": "${name}",
        "email": "${email}"
      }

  update_user:
    method: PUT
    path: "/users/{user_id}"
    params:
      user_id: event.user.id     # Read from event
      status: event.new_status   # Read from event
    request_body: |
      {
        "status": "${status}"
      }
```

---

## Pipeline Integration

### Basic API Call

```yaml
pipeline:
  - type: api
    api: <api_name>             # Required: API identifier from config
    endpoint: <endpoint_name>   # Required: Endpoint name
    # Parameters automatically read from context (defined in API config)
    # Result automatically stored in: api.{api_name}.{endpoint_name}
```

### Automatic Parameter Resolution

**Parameters are defined in API config** with context paths:

```yaml
# API config
endpoints:
  get_user:
    method: GET
    path: "/users/{user_id}/posts/{post_id}"
    params:
      user_id: event.user.id      # Auto-read from context
      post_id: event.post.id      # Auto-read from context
```

**Pipeline automatically resolves them**:

```yaml
# Pipeline - no params needed!
- type: api
  api: user_service
  endpoint: get_user
  # Parameters automatically read from event.user.id and event.post.id
```

**How it works**:
- **Path Parameters**: `{placeholder}` in path are replaced with param values from context
- **Query Parameters**: Listed in `query_params` are appended as `?key=value` from context
- **Body Parameters**: Used in `request_body` replacements from context

### Automatic Output Storage

**Results automatically stored** at `api.{api_name}.{endpoint_name}`:

```yaml
# Pipeline step
- type: api
  id: ip_check
  api: ipinfo
  endpoint: ip_lookup
  # Result automatically stored in: api.ipinfo.ip_lookup
```

Access response fields directly:

```yaml
rule:
  conditions:
    - api.ipinfo.ip_lookup.country == "US"
    - api.ipinfo.ip_lookup.city == "San Francisco"
```

**Response Structure**: The entire JSON response is converted to CORINT Value type and stored at `api.{api_name}.{endpoint_name}`.

**Override when needed**:

```yaml
# Custom output location
- type: api
  api: ipinfo
  endpoint: ip_lookup
  output: api.ip_info  # Custom location

# Multiple calls to same endpoint
- type: api
  id: first_check
  api: fraud_api
  endpoint: check_transaction
  output: api.first_check

- type: api
  id: second_check
  api: fraud_api
  endpoint: check_transaction
  output: api.second_check
```

### Timeout Configuration

Timeouts can be configured at three levels with clear priority:

```yaml
# API Configuration
name: fraud_api
base_url: "https://api.example.com"
timeout_ms: 5000  # Level 3: API-wide default (5 seconds)

endpoints:
  quick_check:
    method: GET
    path: "/quick/{id}"
    # Uses API default: 5000ms

  deep_analysis:
    method: POST
    path: "/deep/analyze"
    timeout_ms: 30000  # Level 2: Endpoint-specific (30 seconds)

# Pipeline Step
- type: api
  api: fraud_api
  endpoint: deep_analysis
  timeout: 60000  # Level 1: Step-specific (60 seconds) - highest priority
```

**Timeout Priority** (highest to lowest):
1. **Pipeline step `timeout`** - Overrides everything
2. **Endpoint `timeout_ms`** - Overrides API default
3. **API config `timeout_ms`** - Overrides system default
4. **System default**: 10000ms (10 seconds)

**Use cases**:
- **API level**: Set reasonable default for all endpoints
- **Endpoint level**: Different timeouts for quick vs complex operations
- **Pipeline level**: Override for specific high-priority calls

**Timeout Hierarchy Details**:

```yaml
# API config - default for all endpoints
name: fraud_api
base_url: "https://api.example.com"
timeout_ms: 5000  # 5 seconds default

endpoints:
  # Quick endpoint - use API default (5000ms)
  quick_check:
    method: GET
    path: "/quick/{id}"

  # Slow endpoint - override with longer timeout
  deep_analysis:
    method: POST
    path: "/deep/analyze"
    timeout_ms: 30000  # 30 seconds for this endpoint

# Pipeline - can override both
- step:
    type: api
    api: fraud_api
    endpoint: deep_analysis
    timeout: 60000  # 60 seconds - highest priority
```

### Error Handling

```yaml
- type: api
  id: ip_check
  api: ipinfo
  endpoint: ip_lookup
  # Parameters automatically read from context
  on_error:
    fallback:
      country: "Unknown"
      city: "Unknown"
      error: true
```

**Behavior**:
- On success: Response stored in default location `api.{api_name}.{endpoint_name}`
- On error (timeout, 4xx, 5xx): Fallback value used instead
- If no fallback: `null` is stored

**Error Scenarios**:
- Network timeout
- Connection refused
- HTTP 4xx (client errors)
- HTTP 5xx (server errors)
- Invalid JSON response

---

## Advanced Usage

**Note**: Conditional execution of API calls should be implemented using pipeline routing logic (e.g., `when` conditions in pipeline steps or router configuration), not through API step-level `if` conditions.

### Chained API Calls

```yaml
# API configs define parameter mappings
# user_service.yaml
endpoints:
  get_user:
    params:
      user_id: event.user_id

# reputation_service.yaml
endpoints:
  check_email:
    params:
      email: api.user_service.get_user.email  # Chain from previous API result

# Pipeline - simple!
pipeline:
  # Step 1: Get user info
  - type: api
    id: get_user
    api: user_service
    endpoint: get_user
    # Result stored in: api.user_service.get_user
    next: check_reputation

  # Step 2: Automatically uses email from previous result
  - type: api
    id: check_reputation
    api: reputation_service
    endpoint: check_email
    # Reads email from: api.user_service.get_user.email
    # Result stored in: api.reputation_service.check_email
```

### Environment Variables

API configurations support environment variable substitution using `env.x.y.z` format:

**Syntax**: `env.x.y.z` (dot notation)

**Resolution**: At **compile time**, `env.x.y.z` is replaced with values from `config/server.yaml`

**Configuration Source**: Values are read from `config/server.yaml` and mapped to `env` namespace:
- `llm.openai.api_key` → `env.llm.openai.api_key`
- `llm.anthropic.api_key` → `env.llm.anthropic.api_key`
- `datasource.postgres_rules.connection_string` → `env.datasource.postgres_rules.connection_string`

**Example**:
```yaml
# config/server.yaml
llm:
  openai:
    api_key: "sk_live_abc123"

# API config (repository/configs/apis/fraud_api.yaml)
auth:
  type: header
  name: "Authorization"
  value: "Bearer ${env.llm.openai.api_key}"  # Compile-time replacement

# After compilation, becomes:
# value: "Bearer sk_live_abc123"
```

**Note**: The `${...}` syntax is used for string interpolation. The `env.x.y.z` path is resolved at compile time and replaced with the actual value from `config/server.yaml`.

**Legacy Support**: For backward compatibility, `${VAR_NAME}` format (without `env.` prefix) is still supported but deprecated. Use `env.x.y.z` format for new configurations.

**Security Best Practice**: Never hardcode secrets in YAML files. Always use environment variables via `env.x.y.z` format.

---

## Authentication

### Header Authentication

Most common authentication method:

```yaml
# API Key
auth:
  type: header
  name: "X-API-Key"
  value: "${API_KEY}"

# Bearer Token
auth:
  type: header
  name: "Authorization"
  value: "Bearer ${AUTH_TOKEN}"

# Custom Header
auth:
  type: header
  name: "X-Custom-Auth"
  value: "${CUSTOM_VALUE}"
```

### No Authentication

Simply omit the `auth` section:

```yaml
name: public_api
base_url: "https://api.example.com"
# No auth needed

endpoints:
  get_data:
    method: GET
    path: "/public/data"
```

---

## HTTP Methods

### GET

```yaml
endpoints:
  get_resource:
    method: GET
    path: "/resource/{id}"
    params:
      id: event.resource_id         # From context
      include: "metadata"            # Literal
      format: "json"                 # Literal
    query_params:
      - include
      - format
```

**Generates**: `GET /resource/123?include=metadata&format=json` when `event.resource_id` is "123"

### POST

```yaml
endpoints:
  create_resource:
    method: POST
    path: "/resources"
    params:
      name: "Transaction"            # Literal
      type: "payment"                # Literal
      amount: event.amount           # From context
    request_body: |
      {
        "name": "${name}",
        "type": "${type}",
        "amount": ${amount}
      }
```

**Generates**: `POST /resources` with JSON body containing values from context

### PUT / PATCH / DELETE

Similar to POST:

```yaml
endpoints:
  update_resource:
    method: PUT
    path: "/resources/{id}"
    params:
      id: event.resource_id
      status: event.new_status
    request_body: |
      {
        "status": "${status}"
      }

  partial_update:
    method: PATCH
    path: "/resources/{id}"
    params:
      id: event.resource_id
      value: event.field_value
    request_body: |
      {
        "field": "${value}"
      }

  delete_resource:
    method: DELETE
    path: "/resources/{id}"
    params:
      id: event.resource_id
```

---

## Response Handling

### Field Mapping

Field mapping allows you to rename or extract specific fields from the API response:

```yaml
# API Configuration
endpoints:
  check_risk:
    method: POST
    path: "/v1/check"
    request_body: |
      {
        "transaction_id": "${tx_id}"
      }
    response:
      mapping:
        risk_score: data.riskScore        # Extract nested field
        recommendation: data.action        # Rename field
        user_id: userId                    # Simple rename
```

**How it works**:

1. API returns:
```json
{
  "userId": "user_123",
  "data": {
    "riskScore": 85,
    "action": "REJECT"
  }
}
```

2. After mapping, stored as:
```json
{
  "risk_score": 85,
  "recommendation": "REJECT",
  "user_id": "user_123"
}
```

3. Access in rules:
```yaml
rule:
  conditions:
    - api.risk_data.risk_score > 80
    - api.risk_data.recommendation == "REJECT"
```

**Benefits**:
- Normalize different API formats to your schema
- Extract only needed fields
- Simplify complex nested structures
- Make rules more readable

### Fallback Values

Define default values to use when API calls fail:

```yaml
# API Configuration
endpoints:
  check_ip:
    method: GET
    path: "/{ip}"
    response:
      mapping:
        country_code: country
        city_name: city
      fallback:
        country_code: "Unknown"
        city_name: "Unknown"
        error: true              # Flag to detect fallback
```

**Fallback Priority** (highest to lowest):

1. **Pipeline step** (overrides all):
```yaml
- type: api
  api: ipinfo
  endpoint: ip_lookup
  on_error:
    fallback:
      country_code: "XX"         # This wins
      custom_field: "value"
```

2. **Endpoint config**:
```yaml
response:
  fallback:
    country_code: "Unknown"      # Used if pipeline has no fallback
```

3. **Null**: If no fallback defined anywhere

**Error Scenarios**:
- HTTP 4xx (client errors)
- HTTP 5xx (server errors)
- Network timeout
- Connection refused
- Invalid JSON response

**Best Practice**: Always include `error: true` in fallback to distinguish from real data:

```yaml
rule:
  conditions:
    - context.ip_info.country_code == "CN"
    - context.ip_info.error != true       # Ensure it's real data, not fallback
```

### Automatic JSON Parsing

All responses are automatically parsed as JSON:

```json
// API Response
{
  "country": "US",
  "city": "San Francisco",
  "postal": "94016",
  "location": {
    "lat": 37.4419,
    "lng": -122.1419
  }
}
```

Stored as CORINT Value and accessible via dot notation:

```yaml
rule:
  conditions:
    - context.ip_info.country == "US"
    - context.ip_info.location.lat > 37.0
```

### Response Type Mapping

| JSON Type | CORINT Value Type |
|-----------|------------------|
| `null` | `Value::Null` |
| `true`/`false` | `Value::Bool` |
| `123`, `45.67` | `Value::Number` |
| `"string"` | `Value::String` |
| `[...]` | `Value::Array` |
| `{...}` | `Value::Object` |

### Non-JSON Responses

If response is not valid JSON:
- Error is logged
- Fallback value is used (if configured)
- Otherwise `null` is stored

### Response Format Details

#### JSON Auto-Parsing

API responses are automatically parsed as JSON and converted to CORINT Value types:

```json
{
  "country": "US",
  "city": "San Francisco",
  "location": {
    "lat": 37.7749,
    "lng": -122.4194
  }
}
```

**Access in rules**:
```yaml
api.ip_info.country              # "US"
api.ip_info.location.lat         # 37.7749
```

#### Type Mapping

| JSON | CORINT Value |
|------|--------------|
| `null` | `Null` |
| `true`/`false` | `Bool` |
| Numbers | `Number` |
| Strings | `String` |
| Arrays | `Array` |
| Objects | `Object` |

### Request Body Template Syntax

For POST/PUT/PATCH requests, use `request_body` with `${placeholder}`:

```yaml
# Config
method: POST
path: "/transactions"
params:
  tx_id: event.transaction_id      # From context
  amount: event.amount              # From context
  currency: "USD"                   # Literal
request_body: |
  {
    "transaction_id": "${tx_id}",   # String: use "${param}"
    "amount": ${amount},            # Number: use ${param}
    "currency": "${currency}"        # String: use "${param}"
  }

# When event.transaction_id = "tx_12345" and event.amount = 1500.00
# Result body:
{
  "transaction_id": "tx_12345",
  "amount": 1500.00,
  "currency": "USD"
}
```

**Template syntax**:
- String values: `"${param}"` (with quotes in JSON)
- Numeric values: `${param}` (without quotes in JSON)
- Boolean values: `${param}` (without quotes in JSON)
- Param names refer to keys in `params` mapping

---

## Best Practices

### 1. Security

**✅ DO**:
```yaml
# Use environment variables for secrets
auth:
  value: "${API_KEY}"
```

**❌ DON'T**:
```yaml
# Never hardcode secrets
auth:
  value: "sk_live_abc123def456"
```

### 2. Timeout Configuration

**✅ DO**:
```yaml
# Set appropriate timeouts
- type: api
  api: slow_service
  timeout: 30000  # 30s for known slow APIs

- type: api
  api: fast_cache
  timeout: 1000   # 1s for cached data
```

**❌ DON'T**:
```yaml
# Don't use infinite timeout
timeout: 0

# Don't use too short timeout for complex queries
timeout: 100  # Too short!
```

### 3. Error Handling

**✅ DO**:
```yaml
# Always provide fallback for critical data
- type: api
  api: risk_service
  on_error:
    fallback:
      risk_score: 50  # Medium risk if service fails
      error: true
```

**❌ DON'T**:
```yaml
# Don't ignore errors for critical paths
- type: api
  api: critical_service
  # No error handling - fails silently!
```

### 4. API Configuration

**✅ DO**:
```yaml
# Use descriptive names
name: chainalysis_blockchain_risk
base_url: "https://api.chainalysis.com"

endpoints:
  wallet_risk_score:  # Clear purpose
    method: GET
    path: "/v2/wallets/{address}/risk"
```

**❌ DON'T**:
```yaml
# Vague names
name: api1
base_url: "https://api.example.com"

endpoints:
  check:  # What does this check?
    method: GET
    path: "/v1/check"
```

### 5. Parameter Naming

**✅ DO**:
```yaml
# Match API's expected parameter names
params:
  wallet_address: ${event.wallet.address}
  api_key: "${CHAINALYSIS_KEY}"
```

**❌ DON'T**:
```yaml
# Don't use misleading names
params:
  user_id: ${event.wallet.address}  # Confusing!
```

### 6. Design Principles

1. **Explicit over implicit**: Use `${}` to clearly mark expressions
2. **Fail-safe defaults**: Always provide fallback for critical data
3. **Security first**: Never hardcode secrets, use env vars via `env.x.y.z`
4. **Simple configuration**: Minimize boilerplate
5. **Clear errors**: Provide actionable error messages

---

## Troubleshooting

### API Not Found

```
ERROR: Unknown API: ipinfo
```

**Solutions**:
1. Check file exists: `repository/configs/apis/ipinfo.yaml`
2. Check `name` field matches API identifier in pipeline
3. Restart engine to reload configurations

### Parameter Not Found

```
ERROR: Field not found: event.ip_address
```

**Solutions**:
1. Verify field exists in event data
2. Check field path spelling
3. Use `${...}` syntax for expressions
4. Log event to see available fields

### Timeout Error

```
ERROR: External API call failed: operation timed out
```

**Solutions**:
1. Increase timeout: `timeout: 30000`
2. Check API availability
3. Verify network connectivity
4. Add fallback value

### Invalid JSON Response

```
ERROR: Failed to parse JSON response
```

**Solutions**:
1. Check API documentation for response format
2. Verify endpoint URL is correct
3. Check authentication credentials
4. Look at raw response in logs (INFO level)

### Authentication Failed

```
ERROR: HTTP 401 Unauthorized
```

**Solutions**:
1. Verify environment variable is set in `config/server.yaml`
2. Check token/key is valid
3. Verify header name matches API requirements
4. Check token format (e.g., "Bearer token" vs "token")
5. Ensure `env.x.y.z` path matches `config/server.yaml` structure

### Error Scenarios

| Error Type | HTTP Status | Behavior |
|------------|-------------|----------|
| Success | 200-299 | Parse and store response |
| Client error | 400-499 | Use fallback or `null` |
| Server error | 500-599 | Use fallback or `null` |
| Timeout | N/A | Use fallback or `null` |
| Network error | N/A | Use fallback or `null` |
| Invalid JSON | N/A | Use fallback or `null` |

**Logging**: All errors are logged at WARN level with details.

**Error Scenarios Using Fallback**:
- HTTP 4xx (client errors)
- HTTP 5xx (server errors)
- Network timeout
- Connection refused
- Invalid JSON response

---

## Examples

### Complete Example: Chainalysis API

**API Configuration** (`repository/configs/apis/chainalysis.yaml`):

```yaml
name: chainalysis
base_url: https://api.chainalysis.com/v2

auth:
  type: header
  name: X-API-Key
  value: ${env.chainalysis.api_key}

timeout_ms: 5000

endpoints:
  wallet_risk:
    method: GET
    path: /risk/wallet/{address}
    timeout_ms: 3000                     # Quick lookup
    params:
      address: event.wallet.address      # Read from context
      chain: event.chain
      details: "full"                     # Literal value
    query_params:
      - chain
      - details
    response:
      mapping:
        risk_level: riskScore           # Rename field
        is_sanctioned: sanctioned        # Rename field
      fallback:
        risk_level: 50
        is_sanctioned: false
        error: true

  transaction_risk:
    method: POST
    path: /risk/transaction
    timeout_ms: 10000                    # Longer for complex analysis
    params:
      tx_hash: event.transaction.hash    # Read from context
      chain: event.chain
      direction: event.direction
    request_body: |
      {
        "hash": "${tx_hash}",
        "chain": "${chain}",
        "direction": "${direction}"
      }
    response:
      mapping:
        risk: score                      # Rename field
      fallback:
        risk: 60
        error: true
```

**Pipeline Usage**:

```yaml
pipeline:
  steps:
    - step:
        type: api
        id: check_wallet
        api: chainalysis
        endpoint: wallet_risk
        # Parameters automatically read from context
        # Result stored in: api.chainalysis.wallet_risk
        next: check_transaction

    - step:
        type: api
        id: check_transaction
        api: chainalysis
        endpoint: transaction_risk
        # Result stored in: api.chainalysis.transaction_risk
```

### Complete Example: Payment Risk Check

**API Configuration** (`repository/configs/apis/fraud_detector.yaml`):

```yaml
name: fraud_detector
base_url: "https://api.frauddetector.example.com"

auth:
  type: header
  name: "X-API-Key"
  value: "${FRAUD_DETECTOR_KEY}"

timeout_ms: 5000

endpoints:
  check_transaction:
    method: POST
    path: "/v1/transactions/analyze"
    request_body: |
      {
        "transaction_id": "${transaction_id}",
        "amount": ${amount},
        "currency": "${currency}",
        "user_email": "${user_email}",
        "ip_address": "${ip_address}"
      }

  check_ip:
    method: GET
    path: "/v1/ip/{ip}"
    query_params:
      - include_history
```

**Pipeline** (`repository/pipelines/payment_fraud_check.yaml`):

```yaml
pipeline:
  id: payment_fraud_pipeline
  entry: check_ip

  when:
    all:
      - event.type == "payment"

  steps:
    # Step 1: IP reputation check
    - step:
        id: check_ip
        type: api
        api: fraud_detector
        endpoint: check_ip
        params:
          ip: ${event.ip_address}
          include_history: true
        timeout: 3000
        # Result automatically stored in: api.fraud_detector.check_ip
        on_error:
          fallback:
            risk_score: 50
            is_vpn: false
        next: check_transaction

    # Step 2: Transaction analysis
    - step:
        id: check_transaction
        type: api
        api: fraud_detector
        endpoint: check_transaction
        params:
          transaction_id: ${event.transaction_id}
          amount: ${event.amount}
          currency: ${event.currency}
          user_email: ${event.user.email}
          ip_address: ${event.ip_address}
        timeout: 5000
        # Result automatically stored in: api.fraud_detector.check_transaction
        on_error:
          fallback:
            risk_score: 60
            recommendation: "REVIEW"
        next: evaluate_risk

    # Step 3: Evaluate combined risk
    - step:
        id: evaluate_risk
        type: ruleset
        ruleset: fraud_rules
        next: end
```

**Rule** (`repository/library/rules/fraud/combined_risk.yaml`):

```yaml
rule:
  id: high_combined_risk
  name: High Combined Fraud Risk

  when:
    all:
      - event.type == "payment"
      - any:
          - api.ip_reputation.risk_score > 80
          - api.fraud_analysis.risk_score > 75
      - api.fraud_analysis.recommendation == "REJECT"

  score: 100
  actions:
    - block
    - notify_fraud_team
```

---

## Future Enhancements

The following features are planned for future releases:

### Phase 2: Enhanced Reliability
- **Retry mechanism** with exponential backoff
- **Circuit breaker** pattern
- **Request caching** with TTL

### Phase 3: Advanced Authentication
- OAuth 2.0 client credentials flow
- JWT token management
- API key rotation

### Phase 4: Advanced Features
- Response schema validation
- Response field mapping/transformation
- Batch request support
- Parallel API calls (any/all modes)
- Rate limiting integration
- Custom HTTP headers per request

### Phase 5: Observability
- Structured logging with request IDs
- Metrics collection (latency, success rate)
- Distributed tracing support

---

## Reference

### Configuration File Schema

```yaml
# API Configuration Schema
name: string                        # Required
base_url: string                    # Required
timeout_ms: integer                 # Optional, default: 10000

auth:                               # Optional
  type: "header"                    # Only "header" supported
  name: string                      # Header name
  value: string                     # Header value (supports ${ENV_VAR})

endpoints:                          # Required
  <endpoint_name>:                  # Key is endpoint identifier
    method: "GET"|"POST"|"PUT"|"PATCH"|"DELETE"  # Required
    path: string                    # Required, can include {placeholders}
    query_params: [string]          # Optional, list of param names
    request_body: string            # Optional, for POST/PUT/PATCH
```

### Pipeline Step Schema

```yaml
# API Step Schema
type: "api"                         # Required
id: string                          # Required
api: string                         # Required, API name
endpoint: string                    # Required, endpoint name

params:                             # Required
  <param_name>: value|${expression} # Key-value pairs

output: string                      # Optional, variable path
timeout: integer                    # Optional, milliseconds
next: string                        # Optional, next step ID


on_error:                           # Optional
  fallback: value                   # Fallback value on error
```

### Expression Syntax

```
expression := "${" field_path "}"
field_path := identifier ("." identifier)*
identifier := [a-zA-Z_][a-zA-Z0-9_]*

Examples:
  ${event.user.id}
  ${api.ip_info.country}
  ${payment.amount}
```

---

## Appendix

### HTTP Status Code Handling

| Status Code | Behavior |
|-------------|----------|
| 200-299 | Success, parse response |
| 400-499 | Client error, use fallback |
| 500-599 | Server error, use fallback |
| Timeout | Network timeout, use fallback |

### Environment Variable Resolution

Environment variables are resolved at **compile time** from `config/server.yaml`:

**Syntax**: `env.x.y.z` (dot notation)

**Resolution**: At **compile time**, `env.x.y.z` is replaced with values from `config/server.yaml`

**Location**: Can be used in API configuration files and pipeline YAML

**Configuration Source**: Values are read from `config/server.yaml` and mapped to `env` namespace:
- `llm.openai.api_key` → `env.llm.openai.api_key`
- `llm.anthropic.api_key` → `env.llm.anthropic.api_key`
- `datasource.postgres_rules.connection_string` → `env.datasource.postgres_rules.connection_string`

**Example**:
```yaml
# config/server.yaml
llm:
  openai:
    api_key: "sk-abc123"

# API config
auth:
  type: header
  name: "Authorization"
  value: "Bearer ${env.llm.openai.api_key}"  # Compile-time replacement

# After compilation, becomes:
# value: "Bearer sk-abc123"
```

**Note**: The `${...}` syntax is used for string interpolation. The `env.x.y.z` path is resolved at compile time and replaced with the actual value from `config/server.yaml`.

**Mapping Rules**:
- `config/server.yaml` structure maps directly to `env` namespace
- If variable doesn't exist, compilation will fail with clear error message

### Logging

Set log level to see API call details:

```bash
RUST_LOG=info cargo run
```

API calls log:
- Request URL (DEBUG level)
- Request parameters (TRACE level)
- Response body (INFO level)
- Errors (WARN/ERROR level)

---

**Version**: 0.2.0
**Last Updated**: 2025-12-30
**Status**: MVP Implementation Guide
