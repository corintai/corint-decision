# External API Integration DSL

## Overview

External API integration allows CORINT pipelines to call third-party HTTP APIs with automatic parameter resolution, error handling, and response parsing.

---

## 1. Basic Structure

### API Configuration

```yaml
name: <string>                      # Required: Unique API identifier
base_url: <string>                  # Required: Base URL for all endpoints

# Optional: Authentication
auth:
  type: header                      # Only 'header' supported in MVP
  name: <string>                    # Header name (e.g., "Authorization")
  value: <string>                   # Header value, supports ${env.x.y.z}

# Optional: Default timeout for all endpoints
timeout_ms: <integer>               # Milliseconds, default: 10000

# Required: Endpoint definitions
endpoints:
  <endpoint_name>:                  # Unique endpoint identifier
    method: <http_method>           # GET | POST | PUT | PATCH | DELETE
    path: <string>                  # URL path, can include {placeholders}
    timeout_ms: <integer>           # Optional: Timeout for this endpoint (overrides API default)
    params:                         # Optional: Parameter mapping from context
      <param_name>: <context_path>  # e.g., transaction_id: event.transaction_id
      <param_name>: <literal>       # e.g., api_version: "v2"
    query_params:                   # Optional: Query parameter names
      - <param_name>
    request_body: <string>          # Optional: JSON template for POST/PUT/PATCH
                                    # Use ${param_name} for substitution
    response:                       # Optional: Response handling
      mapping:                      # Optional: Field mapping/renaming
        <output_field>: <response_field>
      fallback:                     # Optional: Default value on error
        <field>: <value>
```

### Pipeline API Step

```yaml
- step:
    type: api
    id: <identifier>                # Required: Step identifier
    api: <identifier>                # Required: API name from config
    endpoint: <identifier>           # Required: Endpoint name
    params:                         # Optional: Override default params
      <param_name>: <context_path>
 
    next: <identifier>              # Optional: Next step ID
```

---

## 2. BNF Grammar

### 2.1 API Configuration

```bnf
<api_config>       ::= <api_metadata> <auth_config>? <timeout_config>? <endpoints>

<api_metadata>     ::= "name:" <identifier>
                       "base_url:" <url>

<auth_config>      ::= "auth:"
                         "type: header"
                         "name:" <string>
                         "value:" <env_var_or_string>

<timeout_config>   ::= "timeout_ms:" <integer>

<endpoints>        ::= "endpoints:" <endpoint>+

<endpoint>         ::= <endpoint_name> ":"
                         "method:" <http_method>
                         "path:" <url_path>
                         ( "timeout_ms:" <integer> )?
                         ( "params:" <param_mapping> )?
                         ( "query_params:" <param_list> )?
                         ( "request_body:" <json_template> )?
                         ( "response:" <response_config> )?

<param_mapping>    ::= ( <param_name> ":" <template_value> )+

<template_value>   ::= "${" <context_path> "}"  // e.g., ${event.user.id}
                     | <literal>                // e.g., "v2" or 123 or true

<context_path>     ::= <identifier> ( "." <identifier> )*

<literal>          ::= <string_literal> | <number_literal> | <boolean_literal>
<string_literal>   ::= "\"" <chars> "\""
<number_literal>   ::= <integer> | <float>
<boolean_literal>  ::= "true" | "false"

<response_config>  ::= ( "mapping:" <field_mapping> )?
                         ( "fallback:" <fallback_value> )?

<field_mapping>    ::= ( <output_field> ":" <response_field> )+

<fallback_value>   ::= ( <field> ":" <value> )+

<http_method>      ::= "GET" | "POST" | "PUT" | "PATCH" | "DELETE"

<url_path>         ::= <string>  // Can contain {placeholder}

<param_list>       ::= "- " <identifier> ( "\n- " <identifier> )*

<json_template>    ::= <string>  // JSON with ${param_name} for substitution
                                  // Placeholders refer to keys in `params`, not context paths
                                  // String values: "${param_name}" (with quotes)
                                  // Number/boolean values: ${param_name} (without quotes)

<env_var_or_string> ::= "${" <env_path> "}"  // env.x.y.z format
                      | <string>              // Literal string

<env_path>         ::= "env." <identifier> ( "." <identifier> )*
```

### 2.2 Pipeline API Step

```bnf
<api_step>         ::= "type: api"
                       "id:" <identifier>
                       "api:" <identifier>
                       "endpoint:" <identifier>
                       ( "params:" <param_override> )?     // Optional: override default
                       ( "output:" <variable_path> )?      // Optional: default api.{api}.{endpoint}
                       ( "timeout:" <integer> )?
                       ( "on_error:" <error_handler> )?
                       ( "next:" <identifier> )?

<param_override>   ::= ( <param_entry> )+

<param_entry>      ::= <identifier> ":" <param_value>

<param_value>      ::= <expression> | <literal>
<expression>       ::= "${" <context_path> "}"  // e.g., ${event.user.id}
<context_path>     ::= <identifier> ( "." <identifier> )*

<variable_path>    ::= <identifier> ( "." <identifier> )*

<error_handler>    ::= "fallback:" <value_object>

<value_object>     ::= ( <key> ":" <value> )+
```

---

## 3. Core Concepts

### Overview: Expression Resolution Timing

CORINT external API integration resolves values at different times:

**1. Compile Time** (when loading configurations):
- Environment variables (`${env.x.y.z}`) are resolved from `config/server.yaml`
- API config `params` mappings are registered (not resolved, just recorded)

**2. Runtime** (when executing API call):
- Pipeline step `params` override API config defaults (if provided)
- Context paths are resolved to actual values from execution context
- Request body template placeholders are substituted with param values

**Example**:
```yaml
# API config (compile time)
auth:
  value: "Bearer ${env.llm.openai.api_key}"    # ← Resolved at compile time
params:
  user_id: ${event.user.id}                    # ← Mapping recorded at compile time

# Runtime execution
request_body: |
  {"user_id": "${user_id}"}                    # ← Substituted at runtime
```

### 3.1 Parameter Resolution

Parameters can be defined in two places with the same consistent `${}` syntax:

#### API Configuration `params` (Default Mapping)

Defines default parameter mappings using `${}` syntax for context references:

```yaml
# In API config file
endpoints:
  get_user:
    params:
      user_id: ${event.user.id}      # Context path (with ${})
      api_version: "v2"               # String literal
      limit: 100                      # Number literal
      include_details: true           # Boolean literal
```

**Rules**:
- Context paths use `${}`: `${event.user.id}`
- Literals are written as-is: `"string"`, `123`, `true`
- These define the default mapping relationship at compile time

#### Pipeline Step `params` (Override)

Overrides default mappings using the same `${}` syntax:

```yaml
# In pipeline step
- step:
    type: api
    params:
      user_id: ${event.user.id}      # Context path (with ${})
      api_version: "v3"               # String literal
      limit: 200                      # Number literal
      include_details: false          # Boolean literal
```

**Rules**:
- Context paths use `${}`: `${event.user.id}` (same as API config)
- Literals are written as-is: `"string"`, `123`, `true`
- These override API config params at runtime

#### Resolution Process

1. **Compile time**: API config `params` define default mappings
2. **Runtime**: Pipeline step `params` override defaults (if provided)
3. **Runtime**: Values are resolved from execution context
4. **Priority**: Pipeline step `params` > API config `params`

**Example**:
```yaml
# API config defines defaults
params:
  user_id: ${event.user.id}
  limit: 100

# Pipeline overrides only user_id
params:
  user_id: ${event.different_user}
  # limit still uses default (100)
```

### 3.2 URL Construction

```
{base_url} + {path with {placeholders} replaced} + {query string from query_params}
```

- Path placeholders: `{param_name}` replaced with param value
- Query params: Only params listed in `query_params` appended as `?key=value`

### 3.3 Request Body Template

For POST/PUT/PATCH requests, `request_body` uses template substitution with `${param_name}`:

**Important**: Placeholders refer to **keys in `params`**, not context paths.

#### Type-Sensitive Substitution

```yaml
# API config
params:
  transaction_id: ${event.transaction_id}    # Context path (value: "tx_12345")
  amount: ${event.amount}                    # Context path (value: 1500.00)
  currency: "USD"                            # Literal string
  verified: true                             # Literal boolean

request_body: |
  {
    "transaction_id": "${transaction_id}",    // String: use "${}" with quotes
    "amount": ${amount},                      // Number: use ${} WITHOUT quotes
    "currency": "${currency}",                // String: use "${}" with quotes
    "verified": ${verified}                   // Boolean: use ${} WITHOUT quotes
  }
```

**Result** (when resolved at runtime):
```json
{
  "transaction_id": "tx_12345",
  "amount": 1500.00,
  "currency": "USD",
  "verified": true
}
```

**Rules**:
1. Placeholders use `${param_name}` where `param_name` is a key in `params`
2. String values: `"${param_name}"` (include JSON quotes)
3. Number values: `${param_name}` (no JSON quotes)
4. Boolean values: `${param_name}` (no JSON quotes)
5. Substitution happens at runtime after resolving param values

### 3.4 Timeout Configuration

Timeouts can be configured at three levels with clear priority:

**Priority** (highest to lowest):
1. **Pipeline step `timeout`** - Overrides everything
2. **Endpoint `timeout_ms`** - Overrides API default
3. **API config `timeout_ms`** - Overrides system default
4. **System default**: 10000ms (10 seconds)

**Example**:
```yaml
# API config
name: fraud_api
timeout_ms: 5000           # Level 3: API-wide default (5 seconds)

endpoints:
  quick_check:
    method: GET
    # Uses API default: 5000ms

  deep_analysis:
    method: POST
    timeout_ms: 30000      # Level 2: Endpoint-specific (30 seconds)

# Pipeline step
- step:
    type: api
    api: fraud_api
    endpoint: deep_analysis
    timeout: 60000         # Level 1: Step-specific (60 seconds) - highest priority
```

**Use cases**:
- **API level**: Set reasonable default for all endpoints
- **Endpoint level**: Different timeouts for quick vs complex operations
- **Pipeline level**: Override for specific high-priority calls

### 3.5 Output Storage

Results are automatically stored at `api.{api_name}.{endpoint_name}` unless `output` is specified.

**Example**:
```yaml
# API config
name: ipinfo
endpoints:
  ip_lookup:
    # ...

# Pipeline step
- step:
    type: api
    api: ipinfo
    endpoint: ip_lookup
    # Result automatically stored at: api.ipinfo.ip_lookup

# Access in rules
rule:
  when:
    - api.ipinfo.ip_lookup.country == "US"
```

**Custom output location**:
```yaml
- step:
    type: api
    api: ipinfo
    endpoint: ip_lookup
    output: custom.location
    # Result stored at: custom.location instead
```

### 3.6 Error Handling

Fallback priority (highest to lowest):
1. Pipeline step `on_error.fallback`
2. API config endpoint `response.fallback`
3. `null` if no fallback defined

---

## 4. Minimal Example

### API Configuration

```yaml
name: ipinfo
base_url: "https://ipinfo.io"

endpoints:
  ip_lookup:
    method: GET
    path: "/{ip}"
    params:
      ip: ${event.ip_address}        # Read from event
      token: "abc123"                # Literal value
    query_params:
      - token
    response:
      mapping:
        country_code: country
        city_name: city
      fallback:
        country_code: "Unknown"
        city_name: "Unknown"
        error: true
```

### Pipeline Usage

```yaml
pipeline:
  steps:
    - step:
        type: api
        id: check_ip
        api: ipinfo
        endpoint: ip_lookup
        # Parameters automatically read from context
        # Result stored in: api.ipinfo.ip_lookup
```

### Using Response

```yaml
rule:
  id: high_risk_country
  when:
    all:
      - event.type == "payment"
      - api.ipinfo.ip_lookup.country_code == "XX"
      - api.ipinfo.ip_lookup.error != true
  score: 80
```

---

## 5. Environment Variables

**Syntax**: `env.x.y.z` (dot notation)

**Resolution**: At **compile time**, `env.x.y.z` is replaced with values from `config/server.yaml`

**Example**:
```yaml
# config/server.yaml
llm:
  openai:
    api_key: "sk_live_abc123"

# API config
auth:
  type: header
  name: "Authorization"
  value: "Bearer ${env.llm.openai.api_key}"  # Compile-time replacement
```

**Mapping**: `config/server.yaml` structure maps directly to `env` namespace:
- `llm.openai.api_key` → `env.llm.openai.api_key`
- `datasources.postgres_rules.connection_string` → `env.datasources.postgres_rules.connection_string`

---

## 6. Grammar Summary

```
API Config:
  name + base_url + [auth] + [timeout_ms] + endpoints

Endpoint:
  method + path + [timeout_ms] + [params] + [query_params] + [request_body] + [response]

Response:
  [mapping: {output_field: response_field}] + [fallback: {field: value}]

Pipeline Step:
  type: api + id + api + endpoint + [params] + [output] + [timeout] + [on_error] + [next]

Parameter Value:
  ${expression}  |  "string"  |  number  |  boolean

Error Handling:
  fallback: { key: value, ... }
```

---

**Version**: 0.2.0 (MVP)
**Target**: LLM-readable DSL specification
**Status**: Simplified design for initial implementation
