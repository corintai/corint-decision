# External API Integration Guide

## Overview

CORINT Decision Engine provides a generic, configurable system for integrating with external APIs. Instead of hardcoding API URLs and parameters, you can define APIs through YAML configuration files.

## How It Works

1. **API Configuration Files**: Define external APIs in YAML files
2. **Automatic Loading**: The engine loads all API configs from `repository/configs/apis/`
3. **Generic HTTP Client**: A unified client handles all API calls based on the configuration
4. **Flexible URL Building**: Supports path parameters, query parameters, and multiple HTTP methods

## Creating an API Configuration

### Basic Structure

```yaml
name: api_identifier          # Unique identifier for this API
base_url: "https://api.example.com"  # Base URL for all endpoints

endpoints:
  endpoint_name:              # Name used in pipeline YAML
    method: GET               # HTTP method (GET, POST, PUT, DELETE, PATCH)
    path: "/resource"         # URL path (can include placeholders)
    path_params:              # Map placeholders to parameter names
      param_name: "source_field"
    query_params:             # Map query parameters to source fields
      key: "source_field"
```

### Example: IPInfo API

```yaml
# File: repository/configs/apis/ipinfo.yaml

name: ipinfo
base_url: "https://ipinfo.io"

endpoints:
  ip_lookup:
    method: GET
    path: "/{ip}"             # {ip} will be replaced with actual value
    path_params:
      ip: "ip_address"        # Uses value from ip_address field/param
    query_params:
      token: "token"          # Uses value from token parameter
```

This configuration generates URLs like: `https://ipinfo.io/8.8.8.8?token=abc123`

### Example: REST API with POST

```yaml
name: fraud_service
base_url: "https://fraud-api.example.com"

endpoints:
  check_transaction:
    method: POST
    path: "/v1/check"
    query_params:
      api_key: "api_key"
    body_template: |
      {
        "transaction_id": "{{transaction_id}}",
        "amount": {{amount}}
      }
```

## Using External APIs in Pipelines

Once configured, use the API in your pipeline YAML:

```yaml
pipeline:
  - type: api
    id: ip_check
    api: ipinfo                # References the API name
    endpoint: ip_lookup        # References the endpoint name
    params:
      ip: event.ip_address     # Field access (with dot)
      token: "your_token_here" # String literal (no dot)
    output: context.ip_info    # Where to store the result
    timeout: 3000              # Optional timeout in ms
    on_error:                  # Optional error handling
      action: fallback
      fallback:
        country: "Unknown"
```

### Parameter Resolution

Parameters can come from:
1. **Literal values**: Strings, numbers, booleans passed directly
   ```yaml
   params:
     token: "abc123"          # String literal
     limit: 100               # Number literal
   ```

2. **Event data fields**: Access event data using dot notation
   ```yaml
   params:
     ip: event.ip_address     # Loads from event_data["ip_address"]
   ```

3. **Context variables**: Access stored context data
   ```yaml
   params:
     user_id: context.user.id # Loads from context.user.id
   ```

## URL Construction

The system builds URLs following this pattern:

```
{base_url}{path_with_replaced_placeholders}?{query_string}
```

### Path Parameters

Path parameters use `{placeholder}` syntax:

```yaml
path: "/users/{user_id}/posts/{post_id}"
path_params:
  user_id: "uid"
  post_id: "pid"
```

With `uid=123` and `pid=456`, this produces: `/users/123/posts/456`

### Query Parameters

Query parameters are appended to the URL:

```yaml
query_params:
  token: "api_token"
  format: "response_format"
```

With appropriate values, this produces: `?token=abc123&format=json`

## Response Handling

API responses are:
1. **Parsed as JSON**: Automatically converted to CORINT Value types
2. **Logged**: Raw response is logged at INFO level
3. **Stored**: Result is saved to the specified output variable

### Accessing Response Data

After storing to `context.ip_info`, you can access fields in rules:

```yaml
rule:
  id: check_country
  when:
    event.type: payment
    conditions:
      - context.ip_info.country == "US"
```

## Error Handling

### Fallback Strategy

Provide default values when API calls fail:

```yaml
on_error:
  action: fallback
  fallback:
    country: "US"
    city: "Unknown"
```

### Timeout Configuration

Set timeouts to prevent slow APIs from blocking:

```yaml
timeout: 5000  # 5 seconds
```

## Adding a New API

### Step 1: Create Configuration File

Create `repository/configs/apis/your_api.yaml`:

```yaml
name: your_api
base_url: "https://your-api.example.com"

endpoints:
  your_endpoint:
    method: GET
    path: "/api/v1/resource/{id}"
    path_params:
      id: "resource_id"
    query_params:
      api_key: "api_key"
```

### Step 2: Use in Pipeline

Reference it in your pipeline:

```yaml
pipeline:
  - type: api
    id: call_your_api
    api: your_api
    endpoint: your_endpoint
    params:
      resource_id: event.item_id
      api_key: "your_key_here"
    output: context.api_result
```

### Step 3: Test

The configuration is loaded automatically on engine startup. You'll see:

```
INFO corint_sdk::decision_engine: Loaded API config: your_api
```

## Best Practices

1. **Use Descriptive Names**: Name your APIs and endpoints clearly
2. **Handle Errors**: Always provide fallback values for critical data
3. **Set Timeouts**: Don't let slow APIs block your pipeline
4. **Secure Credentials**: Store API keys securely, not in YAML files (use environment variables)
5. **Log Responses**: Keep INFO logging enabled during development to see API responses
6. **Test Thoroughly**: Test with both successful and failed API calls

## Advanced Features

### Multiple Endpoints per API

You can define multiple endpoints for the same API:

```yaml
name: user_service
base_url: "https://api.example.com"

endpoints:
  get_user:
    method: GET
    path: "/users/{id}"
    path_params:
      id: "user_id"

  create_user:
    method: POST
    path: "/users"
    body_template: |
      {"name": "{{name}}", "email": "{{email}}"}
```

### Complex URL Patterns

Combine path and query parameters:

```yaml
endpoints:
  search:
    method: GET
    path: "/v2/search/{category}"
    path_params:
      category: "search_category"
    query_params:
      q: "search_term"
      limit: "max_results"
      offset: "page_offset"
```

This produces URLs like: `https://api.example.com/v2/search/products?q=laptop&limit=20&offset=0`

## Troubleshooting

### API Not Found Error

```
RuntimeError: Unknown API: your_api
```

**Solution**: Check that your YAML file is in `repository/configs/apis/` and has `.yaml` extension.

### Missing Parameter Error

```
RuntimeError: Field not found: ip_address
```

**Solution**: Ensure the parameter exists in event data or context variables.

### Timeout Errors

```
External API call failed: HTTP request failed: operation timed out
```

**Solution**: Increase timeout or check API availability.

## Security Considerations

1. **Never Hardcode Secrets**: Use environment variables for API keys
2. **Validate Responses**: Add validation rules for critical API responses
3. **Rate Limiting**: Be aware of API rate limits
4. **HTTPS Only**: Always use HTTPS for external APIs
5. **Error Messages**: Don't expose sensitive data in error messages

## Future Enhancements

Planned features:
- Environment variable substitution for API keys
- Request/response transformation hooks
- Retry strategies with exponential backoff
- Response caching
- Request batching
- Authentication header templates
