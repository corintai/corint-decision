# CORINT Risk Definition Language (RDL)
## Observability and Monitoring Specification (v0.1)

Observability is essential for operating production risk engines.  
This document defines comprehensive logging, metrics, tracing, and debugging capabilities for CORINT.

---

## 1. Overview

### 1.1 Observability Pillars

| Pillar | Purpose | Use Cases |
|--------|---------|-----------|
| **Logging** | Record events and errors | Debugging, audit trails |
| **Metrics** | Measure performance and health | Monitoring, alerting, SLOs |
| **Tracing** | Track request flow | Performance analysis, debugging |
| **Debugging** | Inspect runtime state | Development, troubleshooting |

---

## 2. Logging

### 2.1 Log Levels

```yaml
observability:
  logging:
    # Global log level
    level: info                   # debug | info | warn | error
    
    # Per-step log levels
    steps:
      extract_device:
        level: debug
        
      llm_reasoning:
        level: info
        
      critical_check:
        level: error
```

### 2.2 Structured Logging

```yaml
- type: extract
  id: device_extraction
  
  observability:
    log:
      enabled: true
      level: info
      
      # Structured log fields
      include:
        - step.id
        - step.type
        - request.id
        - user.id
        - timestamp
        - duration_ms
        - input_size
        - output_size
        
      # Exclude sensitive data
      exclude:
        - user.password
        - user.ssn
        - credit_card
```

### 2.3 Log Sampling

```yaml
observability:
  logging:
    # Sample logs to reduce volume
    sampling:
      enabled: true
      rate: 0.1                   # Log 10% of requests
      
      # Always log errors
      always_log_errors: true
      
      # Sample based on conditions
      rules:
        - condition: event.transaction.amount > 10000
          rate: 1.0               # Log 100% of large transactions
          
        - condition: event.type == "login"
          rate: 0.05              # Log 5% of logins
```

### 2.4 Log Formats

```yaml
observability:
  logging:
    format: json                  # json | text | structured
    
    # JSON format
    json:
      pretty: false
      include_timestamp: true
      include_caller: true
      
    # Custom fields
    fields:
      service: corint-risk-engine
      version: 1.0.0
      environment: ${env.ENVIRONMENT}
```

### 2.5 Log Output

```yaml
observability:
  logging:
    outputs:
      # Console output
      - type: console
        enabled: true
        level: info
        
      # File output
      - type: file
        enabled: true
        path: /var/log/corint/decisions.log
        rotation:
          max_size_mb: 100
          max_files: 10
          compress: true
          
      # External logging service
      - type: remote
        enabled: true
        endpoint: https://logs.company.com/api/v1/logs
        batch_size: 100
        flush_interval: 5000      # ms
```

---

## 3. Metrics

### 3.1 Built-in Metrics

```yaml
observability:
  metrics:
    enabled: true
    
    # System metrics
    system:
      - request_count_total
      - request_duration_seconds
      - request_errors_total
      - pipeline_execution_time_ms
      
    # Business metrics
    business:
      - decisions_by_action           # approve, deny, review
      - risk_score_distribution
      - rule_trigger_count
      - llm_invocation_count
      
    # Performance metrics
    performance:
      - step_duration_ms
      - cache_hit_rate
      - external_api_latency_ms
      - llm_token_usage
```

### 3.2 Custom Metrics

```yaml
- type: metric
  id: track_high_risk_users
  
  metric:
    name: high_risk_users_total
    type: counter
    description: "Number of high-risk user events"
    
    labels:
      event_type: event.type
      country: event.geo.country
      risk_level: context.risk_assessment.risk_level
      
    # Increment when condition met
    increment_when: context.risk_assessment.risk_score > 0.8
```

### 3.3 Metric Types

#### 3.3.1 Counter

```yaml
- metric:
    name: fraud_attempts_total
    type: counter
    description: "Total fraud attempts detected"
    
    labels:
      - fraud_type
      - country
      
    increment: 1
```

#### 3.3.2 Gauge

```yaml
- metric:
    name: active_sessions
    type: gauge
    description: "Current number of active user sessions"
    
    value: count(active_sessions)
```

#### 3.3.3 Histogram

```yaml
- metric:
    name: transaction_amount_usd
    type: histogram
    description: "Distribution of transaction amounts"
    
    value: event.transaction.amount
    
    buckets: [10, 50, 100, 500, 1000, 5000, 10000, 50000]
```

#### 3.3.4 Summary

```yaml
- metric:
    name: risk_score
    type: summary
    description: "Risk score quantiles"
    
    value: context.final_risk_score
    
    quantiles: [0.5, 0.9, 0.95, 0.99]
```

### 3.4 Metric Export

```yaml
observability:
  metrics:
    export:
      # Prometheus
      - type: prometheus
        enabled: true
        endpoint: /metrics
        port: 9090
        
      # StatsD
      - type: statsd
        enabled: true
        host: localhost
        port: 8125
        prefix: corint.
        
      # CloudWatch
      - type: cloudwatch
        enabled: true
        namespace: CORINT/RiskEngine
        region: us-east-1
        
      # Custom endpoint
      - type: http
        enabled: true
        endpoint: https://metrics.company.com/api/v1/metrics
        interval: 60000           # Push every 60s
```

---

## 4. Tracing

### 4.1 Distributed Tracing

```yaml
observability:
  tracing:
    enabled: true
    provider: opentelemetry        # opentelemetry | datadog | jaeger
    
    # Sampling strategy
    sampling:
      strategy: probability
      rate: 0.1                    # Trace 10% of requests
      
      # Always trace specific conditions
      always_trace:
        - condition: event.transaction.amount > 10000
        - condition: context.risk_score > 0.9
```

### 4.2 Trace Context

```yaml
- type: extract
  id: device_check
  
  observability:
    trace:
      enabled: true
      
      # Span attributes
      attributes:
        step.id: device_check
        step.type: extract
        user.id: event.user.id
        device.type: event.device.type
        
      # Span events
      events:
        - name: device_fingerprint_computed
          timestamp: now()
          attributes:
            fingerprint: device.fingerprint
```

### 4.3 Trace Propagation

```yaml
- type: api
  id: external_check
  
  observability:
    trace:
      # Propagate trace context to external APIs
      propagate: true
      
      # Trace headers
      headers:
        traceparent: true
        tracestate: true
```

### 4.4 Trace Export

```yaml
observability:
  tracing:
    export:
      # OpenTelemetry
      - type: otlp
        endpoint: http://otel-collector:4318
        protocol: http            # http | grpc
        
      # Jaeger
      - type: jaeger
        endpoint: http://jaeger:14268/api/traces
        
      # Datadog
      - type: datadog
        agent_host: localhost
        agent_port: 8126
```

---

## 5. Debugging

### 5.1 Debug Mode

```yaml
pipeline:
  # Enable debug mode
  debug:
    enabled: true
    
    # Only in specific environments
    environments: [development, staging]
    
    # Debug-specific logging
    log_level: debug
    log_all_variables: true
    log_all_context: true
```

### 5.2 Step Inspection

```yaml
- type: reason
  id: llm_check
  
  debug:
    enabled: true
    
    # Inspect step execution
    inspect:
      - input                      # Log input data
      - output                     # Log output data
      - duration                   # Log execution time
      - prompt                     # Log LLM prompt
      - response                   # Log LLM response
      - tokens                     # Log token usage
```

### 5.3 Breakpoints

```yaml
- type: debug_breakpoint
  id: check_intermediate_state
  
  # Pause execution and capture state
  capture:
    - event
    - vars
    - context
    
  # Conditional breakpoint
  when: context.risk_score > 0.8
  
  # Action
  action: capture                  # capture | halt | alert
```

### 5.4 Variable Watching

```yaml
debug:
  watch:
    # Watch specific variables
    - path: context.risk_score
      log_on_change: true
      alert_on_condition: value > 0.9
      
    - path: event.user.id
      log_always: true
```

---

## 6. Audit Trail

### 6.1 Decision Audit

```yaml
observability:
  audit:
    enabled: true
    retention_days: 90
    
    capture:
      # Request information
      - request_id
      - timestamp
      - event_type
      
      # User information
      - user_id
      - user_tier
      
      # Decision information
      - final_action
      - final_score
      - triggered_rules
      - llm_outputs
      
      # Execution metadata
      - pipeline_id
      - pipeline_version
      - execution_time_ms
```

### 6.2 Rule Execution Audit

```yaml
rule:
  id: high_value_transaction
  
  observability:
    audit:
      enabled: true
      
      # What to log when rule triggers
      on_trigger:
        log:
          - rule.id
          - rule.name
          - matched_conditions
          - score_added
          - action_taken
          
      # What to log when rule doesn't trigger
      on_skip:
        log: false                # Don't log non-triggers
```

### 6.3 Change Audit

```yaml
observability:
  audit:
    # Track pipeline/rule changes
    changes:
      enabled: true
      
      track:
        - pipeline_updates
        - rule_modifications
        - threshold_changes
        - model_versions
        
      storage:
        type: database
        table: audit_trail
```

---

## 7. Alerting

### 7.1 Alert Configuration

```yaml
observability:
  alerts:
    # Error rate alert
    - name: high_error_rate
      condition: error_rate > 0.05
      window: 5m
      severity: high
      channels: [slack, pagerduty]
      
    # Latency alert
    - name: slow_pipeline
      condition: p95_latency_ms > 5000
      window: 10m
      severity: medium
      channels: [slack]
      
    # Business metric alert
    - name: unusual_deny_rate
      condition: deny_rate > 0.3
      window: 15m
      severity: medium
      channels: [slack, email]
```

### 7.2 Step-Level Alerts

```yaml
- type: api
  id: critical_service
  
  observability:
    alert:
      # Alert on step failure
      on_error:
        enabled: true
        severity: critical
        channels: [pagerduty]
        message: "Critical service failure: {step.id}"
        
      # Alert on timeout
      on_timeout:
        enabled: true
        severity: high
        channels: [slack]
```

### 7.3 Alert Channels

```yaml
observability:
  alert_channels:
    slack:
      webhook_url: ${SLACK_WEBHOOK_URL}
      channel: "#alerts-risk-engine"
      username: "CORINT Alerts"
      
    pagerduty:
      integration_key: ${PAGERDUTY_KEY}
      
    email:
      smtp_host: smtp.company.com
      from: alerts@company.com
      to: [oncall@company.com]
      
    webhook:
      url: https://alerts.company.com/webhook
      headers:
        Authorization: "Bearer ${ALERT_TOKEN}"
```

---

## 8. Dashboards

### 8.1 Dashboard Definition

```yaml
observability:
  dashboards:
    - name: "Risk Engine Overview"
      panels:
        # Request rate
        - type: graph
          metric: request_count_total
          aggregation: rate
          interval: 1m
          
        # Latency percentiles
        - type: graph
          metric: request_duration_seconds
          percentiles: [50, 95, 99]
          
        # Decision distribution
        - type: pie
          metric: decisions_by_action
          
        # Error rate
        - type: single_stat
          metric: error_rate
          threshold_warning: 0.01
          threshold_critical: 0.05
```

### 8.2 Custom Dashboard Widgets

```yaml
dashboard_widget:
  - name: "LLM Performance"
    type: custom
    queries:
      - metric: llm_invocation_count
        label: "Total Invocations"
        
      - metric: llm_latency_ms
        aggregation: avg
        label: "Avg Latency"
        
      - metric: llm_token_usage
        aggregation: sum
        label: "Total Tokens"
        
      - metric: llm_error_rate
        label: "Error Rate"
```

---

## 9. Performance Profiling

### 9.1 Profiling Configuration

```yaml
observability:
  profiling:
    enabled: true
    
    # CPU profiling
    cpu:
      enabled: true
      sample_rate: 100            # Hz
      
    # Memory profiling
    memory:
      enabled: true
      sample_interval: 1000       # ms
      
    # Output
    output:
      format: pprof               # pprof | flamegraph
      path: /var/log/corint/profiles/
```

### 9.2 Step Performance Tracking

```yaml
- type: reason
  id: llm_analysis
  
  observability:
    profile:
      enabled: true
      
      track:
        - execution_time
        - memory_usage
        - cpu_usage
        - token_count
        - api_calls
```

---

## 10. Health Checks

### 10.1 Health Endpoint

```yaml
observability:
  health:
    enabled: true
    endpoint: /health
    
    checks:
      # Basic liveness
      - name: liveness
        type: ping
        
      # Readiness checks
      - name: database
        type: connection
        timeout: 1000
        
      - name: redis
        type: connection
        timeout: 500
        
      - name: llm_service
        type: http
        endpoint: ${LLM_SERVICE_URL}/health
        timeout: 2000
```

### 10.2 Health Check Response

```yaml
# Health check response format
{
  "status": "healthy",          # healthy | degraded | unhealthy
  "timestamp": "2024-01-15T10:30:00Z",
  "version": "1.0.0",
  
  "checks": {
    "database": {
      "status": "healthy",
      "latency_ms": 5
    },
    "llm_service": {
      "status": "degraded",
      "latency_ms": 4500,
      "message": "High latency detected"
    }
  }
}
```

---

## 11. Explainability

### 11.1 Decision Explanation

```yaml
observability:
  explainability:
    enabled: true
    
    capture:
      # Which rules triggered
      - triggered_rules:
          - rule_id
          - rule_name
          - score_contribution
          - matched_conditions
          
      # LLM reasoning
      - llm_reasoning:
          - model_used
          - risk_score
          - explanation_text
          - confidence
          
      # Factor weights
      - decision_factors:
          - factor_name
          - weight
          - value
          - contribution
```

### 11.2 Explanation Format

```yaml
# Decision explanation output
explanation:
  final_decision: deny
  final_score: 92
  confidence: 0.85
  
  contributing_factors:
    - factor: high_risk_country
      weight: 0.3
      score: 100
      contribution: 30
      reason: "Login from high-risk country: RU"
      
    - factor: new_device
      weight: 0.2
      score: 80
      contribution: 16
      reason: "First time seeing this device"
      
    - factor: llm_behavior_analysis
      weight: 0.5
      score: 92
      contribution: 46
      reason: "AI detected suspicious behavior patterns"
      
  triggered_rules:
    - id: high_risk_login
      name: "High Risk Login Detection"
      score: 80
      matched_conditions:
        - "device.is_new == true"
        - "geo.country in high_risk_list"
        - "LLM.score > 0.7"
```

---

## 12. Cost Tracking

### 12.1 LLM Cost Tracking

```yaml
observability:
  cost_tracking:
    enabled: true
    
    llm:
      track:
        - token_usage
        - api_calls
        - model_type
        
      # Cost per model
      pricing:
        gpt-4-turbo:
          input_per_1k_tokens: 0.01
          output_per_1k_tokens: 0.03
          
        gpt-3.5-turbo:
          input_per_1k_tokens: 0.0005
          output_per_1k_tokens: 0.0015
```

### 12.2 External API Cost Tracking

```yaml
observability:
  cost_tracking:
    external_apis:
      chainalysis:
        cost_per_request: 0.10
        
      device_fingerprint:
        cost_per_request: 0.02
        
      ip_reputation:
        cost_per_request: 0.01
```

---

## 13. Privacy and Compliance

### 13.1 PII Handling in Logs

```yaml
observability:
  privacy:
    # Automatic PII detection
    pii_detection: true
    
    # PII handling strategy
    pii_handling:
      action: mask                # mask | redact | hash | tokenize
      
    # Field-level control
    fields:
      - field: user.email
        action: hash
        
      - field: user.ssn
        action: redact
        
      - field: user.phone
        action: mask
        visible_chars: 4
```

### 13.2 Data Retention

```yaml
observability:
  retention:
    # Log retention
    logs:
      default: 30                 # days
      audit: 90
      error: 180
      
    # Metric retention
    metrics:
      raw: 7                      # days
      rollup_1h: 30
      rollup_1d: 365
      
    # Trace retention
    traces:
      default: 7
      sampled: 30
```

---

## 14. Integration Examples

### 14.1 Complete Observability Setup

```yaml
version: "0.1"

pipeline:
  # Global observability config
  observability:
    logging:
      level: info
      format: json
      sampling_rate: 0.1
      
    metrics:
      enabled: true
      export:
        - type: prometheus
          endpoint: /metrics
          
    tracing:
      enabled: true
      provider: opentelemetry
      sampling_rate: 0.1
      
    audit:
      enabled: true
      retention_days: 90
      
  # Pipeline steps
  - type: extract
    id: extract_features
    observability:
      log:
        level: debug
      trace:
        enabled: true
        
  - type: reason
    id: llm_analysis
    observability:
      log:
        level: info
        include: [prompt, response, tokens]
      metric:
        - name: llm_tokens_used
          value: llm.tokens.total
          
  - metric:
      name: high_risk_decisions
      type: counter
      increment_when: context.risk_score > 0.8
      labels:
        event_type: event.type
        country: event.geo.country
```

---

## 15. Best Practices

### 15.1 Logging Best Practices

✅ **Good:**
- Use structured logging (JSON)
- Include correlation IDs
- Log at appropriate levels
- Sample high-volume logs
- Sanitize PII

❌ **Avoid:**
- Logging sensitive data
- Excessive debug logging in production
- Unstructured log messages
- Missing context in logs

### 15.2 Metrics Best Practices

✅ **Good:**
- Use appropriate metric types
- Add meaningful labels
- Track business metrics
- Set up alerts on SLOs
- Monitor costs

❌ **Avoid:**
- Too many labels (high cardinality)
- Redundant metrics
- Missing units in metric names
- Unbounded label values

### 15.3 Tracing Best Practices

✅ **Good:**
- Use consistent trace propagation
- Sample appropriately
- Add meaningful span attributes
- Trace critical paths
- Link traces to logs

❌ **Avoid:**
- Tracing everything (100%)
- Missing span context
- Overly granular spans
- Not propagating trace context

---

## 16. Summary

CORINT's observability framework provides:

- **Comprehensive logging** with structured format and sampling
- **Rich metrics** for monitoring and alerting
- **Distributed tracing** for performance analysis
- **Audit trails** for compliance and debugging
- **Alerting** for proactive incident response
- **Dashboards** for visualization
- **Explainability** for transparent decisions
- **Cost tracking** for budget management
- **Privacy controls** for compliance

Proper observability enables:
- Quick incident detection and resolution
- Performance optimization
- Regulatory compliance
- Cost management
- Continuous improvement

