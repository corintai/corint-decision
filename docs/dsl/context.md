# CORINT Risk Definition Language (RDL)
## Context and Variable Management Specification (v0.1)

Context management is critical for passing data between pipeline steps, maintaining state, and enabling complex decision flows.  
This document defines how CORINT manages variables, context, and data flow throughout rule execution.

---

## 1. Overview

### 1.1 Context Layers

CORINT maintains multiple context layers:

| Layer | Scope | Mutability | Description |
|-------|-------|------------|-------------|
| `event` | Request | Read-only | Input event data |
| `vars` | Pipeline | Read-only | Pipeline-level variables |
| `context` | Pipeline | Read-write | Step outputs and intermediate results |
| `sys` | System | Read-only | System-provided variables |
| `env` | Environment | Read-only | Configuration and environment variables |

---

## 2. Event Context

### 2.1 Event Structure

The event is the primary input to a pipeline:

```yaml
# Event data is automatically available
event:
  type: string                  # Event type
  timestamp: datetime
  id: string
  
  # Event-specific fields
  user:
    id: string
    email: string
    profile: object
    
  transaction:
    id: string
    amount: number
    currency: string
    
  device:
    id: string
    type: string
    fingerprint: string
    
  geo:
    country: string
    ip: string
    city: string
```

### 2.2 Accessing Event Data

```yaml
# In conditions
conditions:
  - event.type == "login"
  - event.user.id exists
  - event.transaction.amount > 1000

# In prompts
prompt:
  template: |
    User {event.user.id} is attempting a {event.type} 
    from {event.geo.country}
```

---

## 3. Pipeline Variables

### 3.1 Defining Variables

```yaml
pipeline:
  # Define variables at pipeline start
  - vars:
      # Static values
      risk_threshold: 80
      max_transaction: 10000
      high_risk_countries: ["RU", "NG", "UA", "CN"]
      
      # Computed values
      user_age_days: days_between(event.user.created_at, now())
      is_new_user: days_between(event.user.created_at, now()) < 30
      
      # References to event data
      current_amount: event.transaction.amount
      user_tier: event.user.profile.tier
```

### 3.2 Using Variables

```yaml
# In conditions
conditions:
  - event.user.risk_score > vars.risk_threshold
  - event.transaction.amount <= vars.max_transaction
  - event.geo.country in vars.high_risk_countries
  - vars.is_new_user == true

# In expressions
score: vars.risk_threshold * 1.5

# In prompts
prompt:
  template: |
    Risk threshold: {vars.risk_threshold}
    User age: {vars.user_age_days} days
```

### 3.3 Variable Scope

```yaml
pipeline:
  # Global pipeline variables
  - vars:
      global_threshold: 80
      
  - branch:
      when:
        - condition: "event.type == 'login'"
          pipeline:
            # Branch-specific variables
            - vars:
                login_risk_weight: 1.5
                
            - conditions:
                # Can access both global and branch vars
                - vars.global_threshold exists
                - vars.login_risk_weight == 1.5
```

---

## 4. Context (Step Results)

### 4.1 Step Output Definition

```yaml
- type: extract
  id: extract_device_info
  
  # Define output structure
  output:
    device_info:
      fingerprint: string
      is_trusted: boolean
      risk_score: number
```

### 4.2 Storing Step Results

```yaml
- type: reason
  id: llm_behavior_analysis
  provider: openai
  model: gpt-4-turbo
  
  # Results automatically stored in context
  output_schema:
    risk_score: float
    reason: string
    tags: array<string>
    
# Results available as:
# context.llm_behavior_analysis.risk_score
# context.llm_behavior_analysis.reason
# context.llm_behavior_analysis.tags
```

### 4.3 Accessing Context Data

```yaml
# In later steps
- type: rules
  id: combined_check
  
  # Access previous step results
  conditions:
    - context.extract_device_info.risk_score > 70
    - context.llm_behavior_analysis.risk_score > 0.8
    - "suspicious" in context.llm_behavior_analysis.tags
    
# In aggregation
- aggregate:
    method: weighted
    weights:
      context.rules_engine.score: 0.5
      context.llm_analysis.score: 0.3
      context.api_check.score: 0.2
```

### 4.4 Explicit Context Updates

```yaml
- type: custom
  id: compute_derived_metrics
  
  # Explicitly set context values
  set_context:
    velocity_score: sum(event.user.transactions, last_24h) / vars.max_daily_transactions
    is_high_velocity: context.velocity_score > 0.8
    combined_risk: (context.rules_score + context.llm_score) / 2
```

---

## 5. System Context

### 5.1 System Variables

Built-in system variables are automatically available:

```yaml
sys:
  # Request metadata
  request_id: string            # Unique request ID
  timestamp: datetime           # Current timestamp
  
  # Execution context
  pipeline_id: string           # Current pipeline ID
  pipeline_version: string      # Pipeline version
  
  # Environment
  environment: string           # dev | staging | production
  region: string                # Deployment region
  
  # Performance
  execution_time_ms: number     # Current execution time
  
  # User context
  tenant_id: string             # Multi-tenant ID
```

### 5.2 Using System Variables

```yaml
conditions:
  # Time-based logic
  - hour(sys.timestamp) >= 22 || hour(sys.timestamp) <= 6
  - sys.execution_time_ms < 5000
  
  # Environment-specific rules
  - sys.environment == "production"
  
# In logging
observability:
  log:
    include:
      - sys.request_id
      - sys.pipeline_id
      - sys.execution_time_ms
```

---

## 6. Environment Variables

### 6.1 Configuration via Environment

```yaml
# Define environment-specific configuration
env:
  # Feature flags
  ENABLE_LLM_REASONING: boolean
  ENABLE_ADVANCED_ANALYTICS: boolean
  
  # Thresholds
  FRAUD_THRESHOLD: number
  REVIEW_THRESHOLD: number
  
  # API endpoints
  CHAINALYSIS_ENDPOINT: string
  LLM_SERVICE_URL: string
```

### 6.2 Using Environment Variables

```yaml
- type: reason
  id: optional_llm_check
  # Conditional execution based on feature flag
  if: "env.ENABLE_LLM_REASONING == true"
  
conditions:
  - event.risk_score > env.FRAUD_THRESHOLD
  
# In API configuration
- type: api
  id: external_check
  endpoint: env.CHAINALYSIS_ENDPOINT
```

---

## 7. Data Transformation

### 7.1 Transform Step

```yaml
- type: transform
  id: normalize_data
  
  transforms:
    # Create new fields
    - set: normalized_amount
      value: event.transaction.amount / event.transaction.currency_rate
      
    - set: risk_category
      value: |
        event.risk_score < 30 ? "low" :
        event.risk_score < 60 ? "medium" :
        event.risk_score < 85 ? "high" : "critical"
        
    # Compute aggregate values
    - set: velocity_24h
      value: count(event.user.transactions, last_24h)
      
    - set: avg_transaction_30d
      value: avg(event.user.transactions.amount, last_30d)
      
  # Results stored in context
  output_to: context.normalized_data
```

### 7.2 Enrichment Step

```yaml
- type: enrich
  id: add_user_features
  
  # Load data from external sources
  enrich:
    user_profile:
      source: database
      query: "SELECT * FROM users WHERE id = {event.user.id}"
      cache_ttl: 3600
      
    historical_behavior:
      source: analytics_service
      endpoint: /user/{event.user.id}/behavior
      cache_ttl: 300
      
  # Merge into context
  merge_into: event.user
```

---

## 8. Context Lifecycle

### 8.1 Context Evolution

```yaml
pipeline:
  # Step 1: Initial context = event only
  - type: extract
    id: step1
    # Now context includes: context.step1
    
  # Step 2: Context accumulates
  - type: reason
    id: step2
    # Now context includes: context.step1, context.step2
    
  # Step 3: Access all previous results
  - type: rules
    id: step3
    conditions:
      - context.step1.device_score > 50
      - context.step2.risk_score > 0.7
    # Now context includes: context.step1, context.step2, context.step3
```

### 8.2 Context Cleanup

```yaml
- type: cleanup
  id: remove_sensitive_data
  
  # Remove fields from context before logging
  remove:
    - event.user.ssn
    - event.user.password
    - context.api_response.raw_data
```

---

## 9. Conditional Context

### 9.1 Conditional Variable Definition

```yaml
- vars:
    # Different thresholds based on user tier
    risk_threshold: |
      event.user.tier == "premium" ? 90 :
      event.user.tier == "standard" ? 70 : 50
      
    # Conditional feature flags
    use_advanced_checks: |
      event.transaction.amount > 10000 && 
      env.ENABLE_ADVANCED_ANALYTICS == true
```

### 9.2 Conditional Context Updates

```yaml
- type: custom
  id: conditional_enrichment
  
  # Only set context if condition met
  set_context:
    - if: event.transaction.amount > 10000
      set:
        requires_enhanced_due_diligence: true
        edd_level: 2
        
    - if: event.user.risk_score > 80
      set:
        auto_review: true
        review_priority: "high"
```

---

## 10. Context Inheritance

### 10.1 Sub-Pipeline Context

```yaml
pipeline:
  - vars:
      parent_threshold: 80
      
  - branch:
      when:
        - condition: "event.type == 'login'"
          pipeline:
            # Sub-pipeline inherits parent context
            - conditions:
                - vars.parent_threshold exists     # ✓ Available
                - context.parent_step exists       # ✓ Available
                
            # Sub-pipeline adds to context
            - type: extract
              id: login_features
              
  # After branch, login_features is available
  - conditions:
      - context.login_features exists             # ✓ Available
```

### 10.2 Parallel Context Isolation

```yaml
- parallel:
    # Each parallel branch has isolated context
    - pipeline:
        - type: api
          id: check_a
          # Sets context.check_a
          
    - pipeline:
        - type: api
          id: check_b
          # Sets context.check_b
          # Cannot access context.check_a during execution
          
  merge:
    method: all
    
# After merge, both are available
- conditions:
    - context.check_a exists                      # ✓ Available
    - context.check_b exists                      # ✓ Available
```

---

## 11. Context Passing

### 11.1 Explicit Context Passing

```yaml
- type: custom
  id: process_data
  
  # Explicitly specify which context to pass
  inputs:
    user_data: event.user
    device_data: context.extract_device
    risk_data: context.llm_analysis
    
  # Access as function parameters
  function: |
    analyze_risk(inputs.user_data, inputs.device_data, inputs.risk_data)
```

### 11.2 Context Aliasing

```yaml
- type: alias
  id: create_shortcuts
  
  # Create aliases for complex paths
  aliases:
    current_risk: context.llm_analysis.risk_score
    device_trust: context.device_check.trust_level
    user_tier: event.user.profile.tier
    
# Use shorter names in subsequent steps
- conditions:
    - alias.current_risk > 0.8
    - alias.device_trust < 0.5
    - alias.user_tier == "premium"
```

---

## 12. Debugging Context

### 12.1 Context Inspection

```yaml
- type: debug
  id: inspect_context
  
  # Print current context state
  print_context:
    - event
    - vars
    - context
    
  # Filter what to print
  filter:
    - event.user
    - context.*.risk_score
    
  # Only in specific environments
  enabled: sys.environment != "production"
```

### 12.2 Context Snapshots

```yaml
observability:
  context_snapshots:
    enabled: true
    
    # Take snapshots at specific points
    capture_at:
      - after_step: extract_features
      - after_step: llm_analysis
      - after_step: final_decision
      
    # What to include
    include:
      - event.user.id
      - event.transaction.id
      - context.*.risk_score
      - vars.*
```

---

## 13. Context in Different Constructs

### 13.1 Context in Rules

```yaml
rule:
  id: context_aware_rule
  
  when:
    event.type: transaction
    conditions:
      # Rules can access event and vars only
      # (no context from other steps unless in pipeline)
      - event.amount > vars.threshold
      - event.user.risk_score > 70
```

### 13.2 Context in Rulesets

```yaml
ruleset:
  id: login_rules
  rules:
    - rule_1
    - rule_2
    
  # Ruleset-level variables
  vars:
    login_threshold: 75
    
# When included in pipeline
- include:
    ruleset: login_rules
    # Pass context to ruleset
    with_vars:
      login_threshold: vars.custom_threshold
```

### 13.3 Context in Aggregation

```yaml
- aggregate:
    method: weighted
    
    # Reference multiple context values
    sources:
      - path: context.rules_engine.score
        weight: 0.5
        
      - path: context.llm_analysis.risk_score
        weight: 0.3
        normalize: 0-1 to 0-100
        
      - path: context.api_check.score
        weight: 0.2
        
    # Store result in context
    output: context.final_risk_score
```

---

## 14. Memory and Performance

### 14.1 Context Size Management

```yaml
pipeline:
  # Limit context size
  context_config:
    max_size_mb: 10
    
    # What to do when limit exceeded
    on_overflow:
      action: prune
      keep_essential: true
      
    # Auto-cleanup
    auto_cleanup:
      - type: api
        keep_only: [score, status]
        discard: [raw_response]
```

### 14.2 Lazy Loading

```yaml
- type: enrich
  id: expensive_lookup
  
  # Load data only when accessed
  lazy_load: true
  
  enrich:
    detailed_profile:
      source: database
      query: expensive_query
      
# Data loaded only if accessed
- conditions:
    # This triggers the load
    - context.expensive_lookup.detailed_profile.score > 80
```

---

## 15. Best Practices

### 15.1 Naming Conventions

✅ **Good:**
```yaml
- vars:
    risk_threshold_high: 80
    is_new_user: true
    user_account_age_days: 30
    
- type: extract
  id: extract_device_features      # Descriptive
  output: device_features
```

❌ **Bad:**
```yaml
- vars:
    t: 80                           # Unclear
    flag: true                      # Ambiguous
    x: 30                           # Meaningless
    
- type: extract
  id: step1                         # Non-descriptive
  output: data
```

### 15.2 Context Organization

✅ **Good:** Organize related data
```yaml
set_context:
  risk:
    score: 85
    level: "high"
    factors:
      device: 0.7
      location: 0.9
      behavior: 0.6
```

❌ **Bad:** Flat structure
```yaml
set_context:
  risk_score: 85
  risk_level: "high"
  device_factor: 0.7
  location_factor: 0.9
  behavior_factor: 0.6
```

### 15.3 Avoid Context Pollution

```yaml
# Good: Clean, minimal context
- type: api
  id: ip_check
  output:
    ip_score: number
    ip_risk_level: string
    
# Bad: Storing unnecessary data
- type: api
  id: ip_check
  output:
    raw_response: object          # Large, unused
    debug_info: object            # Not needed in production
    internal_state: object        # Implementation detail
```

---

## 16. Context Schema

### 16.1 Full Context Structure

```yaml
# Complete context structure at runtime
{
  # Input event (read-only)
  "event": {
    "type": "login",
    "user": {...},
    "device": {...},
    "geo": {...}
  },
  
  # Pipeline variables (read-only)
  "vars": {
    "risk_threshold": 80,
    "is_new_user": true
  },
  
  # Step results (read-write)
  "context": {
    "extract_device": {
      "device_score": 75,
      "is_trusted": false
    },
    "llm_analysis": {
      "risk_score": 0.85,
      "reason": "...",
      "tags": ["suspicious"]
    }
  },
  
  # System context (read-only)
  "sys": {
    "request_id": "req_123",
    "timestamp": "2024-01-15T10:30:00Z",
    "environment": "production"
  },
  
  # Environment (read-only)
  "env": {
    "ENABLE_LLM": true,
    "FRAUD_THRESHOLD": 85
  }
}
```

---

## 17. Summary

CORINT's context management provides:

- **Layered context model** for clear data separation
- **Pipeline variables** for reusable values
- **Step result tracking** for data flow
- **System and environment integration** for dynamic behavior
- **Context transformation** for data enrichment
- **Scope management** for branches and sub-pipelines
- **Memory optimization** for large-scale processing

Proper context management enables:
- Complex multi-step decision flows
- Data sharing between pipeline components
- Maintainable and debuggable pipelines
- Efficient resource utilization

