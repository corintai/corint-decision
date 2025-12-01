# CORINT Risk Definition Language (RDL)
## Error Handling Specification (v0.1)

Production risk engines must handle failures gracefully to maintain service availability and data integrity.  
This document defines CORINT's comprehensive error handling strategies for rules, rulesets, and pipelines.

---

## 1. Error Categories

### 1.1 Error Types

| Category | Description | Examples |
|----------|-------------|----------|
| **Syntax Error** | Invalid RDL syntax | Malformed YAML, undefined operators |
| **Validation Error** | Schema validation failure | Missing required fields, type mismatch |
| **Runtime Error** | Execution-time failure | Null pointer, division by zero |
| **External Error** | Third-party service failure | API timeout, LLM unavailable |
| **Data Error** | Invalid or missing input data | Required field missing, wrong type |
| **Timeout Error** | Operation exceeded time limit | Slow API, long computation |
| **Resource Error** | System resource exhaustion | Out of memory, rate limit exceeded |

---

## 2. Error Handling Strategies

### 2.1 Strategy Overview

| Strategy | Description | Use Case |
|----------|-------------|----------|
| `fail` | Stop execution, return error | Critical failures, validation errors |
| `skip` | Skip current step, continue pipeline | Optional enhancement steps |
| `fallback` | Use default/alternative value | External API failures |
| `retry` | Retry operation with backoff | Transient failures |
| `continue` | Log error, proceed with partial data | Non-critical enrichment |

---

## 3. Rule-Level Error Handling

### 3.1 Basic Error Handling

```yaml
rule:
  id: high_risk_check
  name: High Risk Detection
  
  when:
    event.type: login
    conditions:
      - user.risk_score > 80
      
  score: 100
  action: deny
  
  # Error handling configuration
  on_error:
    action: skip              # skip | fail | fallback
    log_level: warn           # debug | info | warn | error
    
    # Optional: custom error message
    message: "Risk score check failed, skipping rule"
```

### 3.2 Condition-Level Error Handling

```yaml
rule:
  id: complex_check
  
  when:
    event.type: payment
    conditions:
      # Individual condition error handling
      - expression: user.risk_score > threshold
        on_error:
          action: fallback
          fallback_value: false    # Treat as condition not met
          
      # Safe navigation for potentially null fields
      - user.profile?.kyc_level >= 2
      
      # Default values for missing fields
      - (user.transaction_count ?? 0) > 100
```

### 3.3 External Dependency Errors

```yaml
rule:
  id: api_based_check
  
  when:
    event.type: transaction
    conditions:
      - external_api.Chainalysis.risk_score > 80
        
  # Handle external API failures
  external_api_error:
    action: fallback
    fallback:
      Chainalysis.risk_score: 50    # Conservative default
      
    timeout: 3000                    # ms
    retry:
      max_attempts: 2
      backoff: exponential
```

---

## 4. Pipeline-Level Error Handling

### 4.1 Step Error Handling

```yaml
pipeline:
  - type: extract
    id: extract_device
    on_error:
      action: fail              # Critical step, must succeed
      message: "Device extraction is required"
      
  - type: api
    id: ip_reputation
    on_error:
      action: fallback
      fallback:
        ip_score: 50
        ip_risk_level: "medium"
      timeout: 3000
      
  - type: reason
    id: llm_analysis
    on_error:
      action: skip              # Optional enhancement
      log_level: warn
      
  - type: action
    id: final_decision
    # Actions should not fail silently
    on_error:
      action: fail
      alert: true               # Send alert on action failure
```

### 4.2 Branch Error Handling

```yaml
pipeline:
  - branch:
      when:
        - condition: "event.type == 'login'"
          pipeline:
            - login_flow_step1
            - login_flow_step2
          on_error:
            action: fallback
            fallback_pipeline:
              - minimal_login_check
              
        - condition: "event.type == 'payment'"
          pipeline:
            - payment_flow_step1
          on_error:
            action: fail        # Payment processing must succeed
            
      # Branch-level default error handler
      on_error:
        action: fail
        message: "No branch matched event type"
```

### 4.3 Parallel Execution Error Handling

```yaml
pipeline:
  - parallel:
      - device_fingerprint
      - ip_reputation
      - llm_reasoning
      
    merge:
      method: all
      
    # Error handling for parallel execution
    on_error:
      action: partial           # Continue if some succeed
      min_success: 2            # Require at least 2 of 3
      
    # Individual step errors
    steps_error_handling:
      device_fingerprint:
        on_error:
          action: fail          # Required
          
      ip_reputation:
        on_error:
          action: fallback
          fallback:
            score: 50
            
      llm_reasoning:
        on_error:
          action: skip          # Optional
```

---

## 5. Timeout Configuration

### 5.1 Global Timeouts

```yaml
pipeline:
  # Pipeline-level timeout
  timeout: 10000                # Total pipeline timeout (ms)
  
  # Step timeouts
  - type: api
    id: slow_service
    timeout: 5000               # Step-specific timeout
    
  - type: reason
    id: llm_call
    timeout: 8000
```

### 5.2 Timeout Handling

```yaml
- type: api
  id: external_check
  timeout: 3000
  
  on_timeout:
    action: fallback
    fallback:
      result: "timeout"
      risk_score: 50
      
  # Or retry on timeout
  on_timeout:
    action: retry
    max_retries: 2
    retry_timeout: 2000         # Shorter timeout for retries
```

---

## 6. Retry Logic

### 6.1 Basic Retry

```yaml
- type: api
  id: flaky_service
  
  retry:
    enabled: true
    max_attempts: 3
    backoff: exponential        # exponential | linear | fixed
    initial_delay: 1000         # ms
    max_delay: 10000            # ms
    multiplier: 2.0             # For exponential backoff
    
  on_retry_exhausted:
    action: fallback
    fallback:
      score: 50
```

### 6.2 Conditional Retry

```yaml
- type: api
  id: external_service
  
  retry:
    max_attempts: 3
    
    # Only retry on specific errors
    retry_on:
      - timeout
      - connection_error
      - rate_limit
      - server_error            # 5xx errors
      
    # Don't retry on
    do_not_retry_on:
      - authentication_error    # 401, 403
      - not_found               # 404
      - bad_request             # 400
      - validation_error
```

### 6.3 Retry with Circuit Breaker

```yaml
- type: api
  id: protected_service
  
  retry:
    max_attempts: 3
    backoff: exponential
    
  circuit_breaker:
    enabled: true
    
    # Open circuit after consecutive failures
    failure_threshold: 5
    
    # Keep circuit open for this duration
    open_timeout: 60000         # ms
    
    # Test with limited requests before closing
    half_open_max_calls: 3
    
    # Circuit open behavior
    on_circuit_open:
      action: fallback
      fallback:
        status: "service_unavailable"
        score: 50
```

---

## 7. Fallback Strategies

### 7.1 Static Fallback

```yaml
- type: api
  id: risk_api
  
  on_error:
    action: fallback
    fallback:
      risk_score: 50            # Conservative default
      risk_level: "medium"
      reason: "Service unavailable"
```

### 7.2 Fallback Chain

```yaml
- type: reason
  id: primary_llm
  provider: openai
  model: gpt-4-turbo
  
  fallback_chain:
    # Try faster model from same provider
    - provider: openai
      model: gpt-3.5-turbo
      timeout: 3000
      
    # Try different provider
    - provider: anthropic
      model: claude-3-haiku
      timeout: 3000
      
    # Use rule-based fallback
    - type: rules
      ruleset: fallback_rules
      
    # Final static fallback
    - static:
        risk_score: 0.5
        risk_level: "medium"
        reason: "All AI models unavailable, using default"
```

### 7.3 Degraded Mode

```yaml
pipeline:
  - type: extract
    id: full_feature_extraction
    
    on_error:
      action: fallback
      fallback_pipeline:
        # Use minimal feature extraction
        - type: extract
          id: basic_extraction
          features:
            - user.id
            - transaction.amount
            - event.timestamp
```

---

## 8. Validation and Schema Errors

### 8.1 Input Validation

```yaml
pipeline:
  # Validate input before processing
  - type: validate
    id: input_validation
    
    schema:
      required:
        - event.type
        - user.id
        - timestamp
        
      types:
        event.type: string
        user.id: string
        transaction.amount: number
        
    on_validation_error:
      action: fail
      return_error:
        code: "INVALID_INPUT"
        message: "Required fields missing or invalid types"
```

### 8.2 Output Validation

```yaml
- type: reason
  id: llm_risk_check
  
  output_schema:
    type: object
    required: [risk_score, reason]
    properties:
      risk_score:
        type: number
        minimum: 0.0
        maximum: 1.0
      risk_level:
        type: string
        enum: [low, medium, high, critical]
        
  output_validation:
    strict: true                # Fail if output doesn't match
    
    on_validation_error:
      action: retry
      max_retries: 2
      
      on_retry_exhausted:
        action: fallback
        fallback:
          risk_score: 0.5
          risk_level: "medium"
          reason: "Invalid LLM output"
```

---

## 9. Data Quality Errors

### 9.1 Missing Data Handling

```yaml
rule:
  id: data_dependent_rule
  
  when:
    event.type: transaction
    conditions:
      # Null-safe access
      - user.profile?.age >= 18
      
      # Default value for missing data
      - (user.kyc_level ?? 0) >= 2
      
      # Existence check
      - transaction.amount exists
      
  # Handle missing critical data
  on_missing_data:
    critical_fields:
      - user.id
      - transaction.amount
      
    action: fail
    message: "Critical data missing"
```

### 9.2 Data Type Coercion

```yaml
- type: transform
  id: normalize_data
  
  coercion:
    # Attempt to convert types
    - field: user.age
      from: string
      to: number
      on_error: fallback
      fallback_value: null
      
    - field: transaction.amount
      from: string
      to: number
      on_error: fail            # Critical field
```

---

## 10. Aggregate Error Handling

### 10.1 Partial Results

```yaml
- aggregate:
    method: weighted
    weights:
      rules_engine: 0.5
      llm_reasoning: 0.3
      external_api: 0.2
      
  # Handle partial results
  on_partial_data:
    action: continue            # Use available data
    min_required: 2             # Need at least 2 of 3 sources
    
    # Reweight if some sources missing
    reweight: true
    
  on_insufficient_data:
    action: fallback
    fallback:
      score: 50
      confidence: 0.0
```

---

## 11. Alerting and Monitoring

### 11.1 Error Alerts

```yaml
- type: api
  id: critical_check
  
  on_error:
    action: fail
    
    # Send alert
    alert:
      enabled: true
      severity: high            # low | medium | high | critical
      channels:
        - slack
        - pagerduty
        - email
        
      message: "Critical API check failed: {error.message}"
      
      # Alert only if error persists
      threshold:
        count: 3
        window: 300             # seconds
```

### 11.2 Error Metrics

```yaml
observability:
  metrics:
    errors:
      track:
        - error_count_total
        - error_rate
        - error_by_type
        - error_by_step
        - retry_count
        - fallback_count
        - circuit_breaker_opens
        
      labels:
        - step_id
        - error_type
        - error_severity
        - pipeline_id
```

### 11.3 Error Logging

```yaml
- type: api
  id: external_service
  
  on_error:
    log:
      level: error
      
      include:
        - error.type
        - error.message
        - error.stack_trace
        - request.id
        - step.id
        - timestamp
        - input_data           # Be careful with PII
        - retry_count
        
    # Sample error logs to reduce volume
    sample_rate: 1.0           # 100% = log all errors
```

---

## 12. Recovery Strategies

### 12.1 Checkpoint and Resume

```yaml
pipeline:
  # Enable checkpointing for long pipelines
  checkpoint:
    enabled: true
    interval: 5                # Checkpoint every 5 steps
    
  on_error:
    action: checkpoint_resume
    
    # Resume from last checkpoint
    resume_strategy: last_checkpoint
    
    # Or retry failed step only
    resume_strategy: failed_step
```

### 12.2 Compensating Actions

```yaml
- type: action
  id: approve_transaction
  
  on_error:
    compensate: true
    
    compensating_actions:
      # Reverse the action if it partially succeeded
      - type: action
        id: reverse_approval
        action: deny
        reason: "Original approval failed"
```

---

## 13. Testing Error Handling

### 13.1 Error Injection for Testing

```yaml
# Test configuration
test:
  error_injection:
    enabled: true
    
    scenarios:
      - step_id: external_api_check
        error_type: timeout
        probability: 0.1        # 10% of requests
        
      - step_id: llm_reasoning
        error_type: invalid_output
        probability: 0.05
        
      - step_id: device_fingerprint
        error_type: service_unavailable
        after_request: 5        # Fail after 5th request
```

### 13.2 Error Handling Tests

```yaml
test:
  - name: "Handle API timeout gracefully"
    inject_error:
      step: ip_reputation_check
      error: timeout
      
    expect:
      pipeline_status: success
      ip_reputation_result: fallback_value
      final_score: exists
      
  - name: "Fail on critical step error"
    inject_error:
      step: user_validation
      error: validation_error
      
    expect:
      pipeline_status: failed
      error_code: "VALIDATION_ERROR"
```

---

## 14. Best Practices

### 14.1 Error Handling Hierarchy

```yaml
# Good: Specific to general error handling
- type: api
  id: external_check
  
  # Specific error types
  on_timeout:
    action: retry
    
  on_rate_limit:
    action: backoff
    
  on_authentication_error:
    action: fail
    
  # General catch-all
  on_error:
    action: fallback
```

### 14.2 Fail Fast vs. Fail Safe

```yaml
# Fail Fast: For critical operations
- type: validate
  id: check_user_identity
  on_error:
    action: fail              # Don't proceed with invalid identity
    
# Fail Safe: For enhancements
- type: reason
  id: optional_llm_enhancement
  on_error:
    action: skip              # Continue without enhancement
```

### 14.3 Error Context

```yaml
on_error:
  action: fail
  
  # Provide rich error context
  error_context:
    step_id: "{step.id}"
    request_id: "{sys.request_id}"
    user_id: "{user.id}"
    timestamp: "{sys.timestamp}"
    
    # Sanitize sensitive data
    sanitize:
      - user.password
      - user.ssn
```

---

## 15. Error Response Format

### 15.1 Standard Error Response

```yaml
# Pipeline error response structure
error:
  code: string                # ERROR_CODE
  message: string             # Human-readable message
  type: string                # Error category
  step_id: string             # Where error occurred
  timestamp: datetime
  request_id: string
  
  # Optional details
  details:
    field: string             # Which field caused error
    expected: any             # Expected value/type
    actual: any               # Actual value/type
    
  # Stack trace (only in debug mode)
  stack_trace: string
  
  # Retry information
  retryable: boolean
  retry_after: number         # Seconds
```

### 15.2 Partial Success Response

```yaml
# When pipeline partially succeeds
response:
  status: partial_success
  
  completed_steps:
    - extract_device
    - ip_reputation
    
  failed_steps:
    - llm_reasoning:
        error: timeout
        fallback_used: true
        
  result:
    score: 65
    confidence: 0.7           # Lower due to missing LLM data
    warnings:
      - "LLM reasoning unavailable, used fallback"
```

---

## 16. Summary

CORINT's error handling framework provides:

- **Multiple strategies** (fail, skip, fallback, retry) for different scenarios
- **Granular control** at rule, step, and pipeline levels
- **Automatic retries** with configurable backoff strategies
- **Circuit breakers** for protecting external services
- **Fallback chains** for degraded operation
- **Comprehensive logging** and alerting
- **Testing support** for error scenarios

Production-ready error handling ensures:
- High availability through graceful degradation
- Data integrity through validation
- Operational visibility through logging and metrics
- Reliable recovery from transient failures

