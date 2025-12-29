# CORINT Risk Definition Language (RDL)
## LLM Integration Specification (v0.1)

This document defines how Large Language Models (LLMs) integrate into CORINT's Risk Definition Language to enable **cognitive risk reasoning**.

LLMs augment traditional rule-based systems by providing:
- Natural language understanding of user behavior
- Pattern recognition across unstructured data
- Contextual anomaly detection
- Explainable risk assessments

---

## 1. LLM Step Definition

### 1.1 Basic Structure

```yaml
- type: reason
  id: user_behavior_analysis
  provider: openai
  model: gpt-4-turbo
  prompt: <prompt-definition>
  output: <output-schema>
  config: <llm-configuration>
```

### 1.2 Complete Example

```yaml
- type: reason
  id: login_risk_analysis
  
  provider: openai
  model: gpt-4-turbo
  
  prompt:
    template: |
      Analyze this login attempt for potential security risks:
      
      User Info:
      - User ID: {user.id}
      - Account Age: {user.account_age_days} days
      - Previous Login Country: {user.last_login_country}
      
      Current Event:
      - Login Country: {geo.country}
      - Device: {device.type} ({device.is_new})
      - Failed Attempts: {user.login_failed_count}
      - Time: {event.timestamp}
      
      Assess the risk level and provide reasoning.
      
    input_fields:
      - user.id
      - user.account_age_days
      - user.last_login_country
      - geo.country
      - device.type
      - device.is_new
      - user.login_failed_count
      - event.timestamp
      
  output_schema:
    risk_score: float          # 0.0 - 1.0
    risk_level: string         # low | medium | high | critical
    reason: string             # Human-readable explanation
    tags: array<string>        # Categorical flags
    confidence: float          # Model confidence 0.0 - 1.0
    
  config:
    temperature: 0.3
    max_tokens: 500
    timeout: 5000              # milliseconds
    cache_ttl: 300             # seconds
```

---

## 2. Intelligent Inference Action

### 2.1 Overview

The `infer` action marks cases for asynchronous AI-powered cognition analysis. Unlike synchronous actions (`approve`/`deny`/`review`), `infer` triggers async processing by an external cognition AI service.

**Key Design:**
- ⚡ **Real-time path remains fast** (<100ms)
- 🤖 **AI analysis happens asynchronously** (2-5s)
- 🔄 **Decision can be updated** when AI completes

### 2.2 When to Use `infer`

Use `infer` action when:
- ✅ Rules alone are insufficient
- ✅ Case complexity requires deep reasoning
- ✅ Evidence is ambiguous or conflicting
- ✅ Pattern requires contextual understanding
- ❌ **NOT** when immediate decision is critical

### 2.3 Simple `infer` Action

```yaml
rule:
  id: complex_fraud_case
  action: infer
  
  infer:
    # Only specify what data to send
    data_snapshot:
      - event.transaction
      - event.user
      - event.device
      - context.rules_score
      - context.triggered_rules
```

**That's it!** No need to configure:
- ❌ LLM provider/model
- ❌ Prompts
- ❌ Output schemas
- ❌ Temperature, tokens, etc.

All LLM configuration is managed by the cognition AI service.

### 2.4 How It Works

```
┌─────────────────────────────────────────────────────────────┐
│ Real-time Decision Pipeline (<100ms)                        │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  1. Event arrives                                            │
│  2. Rules engine evaluates (score: 65)                      │
│  3. Action: infer                                            │
│  4. User gets response ✓                                     │
│                                                               │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ Push to cognition queue
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ Async Cognition AI Service (2-5s)                           │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  1. Receive data snapshot                                    │
│  2. LLM intelligent analysis                                 │
│  3. Infer: legitimate/fraudulent                            │
│  4. Confidence: 0.87                                         │
│  5. Recommended action: approve                              │
│  6. Callback to decision engine                              │
│  7. Update decision: infer → approve                        │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### 2.5 Complete Example

```yaml
rule:
  id: ambiguous_transaction
  name: Ambiguous Transaction Pattern
  
  when:
    event.type: transaction
    conditions:
      - context.rules_score >= 40
      - context.rules_score <= 85
      
  score: 65
  action: infer
  
  infer:
    # Specify data to send to cognition AI
    data_snapshot:
      # Event data
      - event.transaction.amount
      - event.transaction.merchant
      - event.transaction.category
      - event.user.id
      - event.user.tier
      - event.device.type
      - event.geo.country
      
      # Context data
      - context.rules_score
      - context.triggered_rules
      - context.behavior_analysis
      - context.device_fingerprint
      - context.ip_reputation
```

### 2.6 Data Snapshot Patterns

**Include everything:**
```yaml
infer:
  data_snapshot:
    - event.*
    - context.*
```

**Include specific fields:**
```yaml
infer:
  data_snapshot:
    - event.transaction
    - event.user.id
    - event.user.tier
    - context.rules_score
```

**Exclude sensitive data:**
```yaml
infer:
  data_snapshot:
    - event.*
    - context.*
  exclude:
    - event.user.password
    - event.user.ssn
    - event.payment.cvv
```

### 2.7 `infer` vs Other Actions

| Action | Decision Maker | Timing | Latency | Use Case |
|--------|---------------|--------|---------|----------|
| `approve` | Rules | Sync | <100ms | Clear low risk |
| `deny` | Rules | Sync | <100ms | Clear high risk |
| `review` | Human | Sync | Hours | Need human judgment |
| `infer` | **AI** | **Async** | **2-5s** | **Complex reasoning** |

---

## 3. LLM Providers

### 3.1 Supported Providers

| Provider | Models | Features |
|----------|--------|----------|
| `openai` | gpt-4, gpt-4-turbo, gpt-3.5-turbo | Function calling, JSON mode |
| `anthropic` | claude-3-5-sonnet, claude-3-opus | Long context, structured output |
| `azure_openai` | gpt-4, gpt-35-turbo | Enterprise compliance |
| `bedrock` | claude-3, titan | AWS native |
| `vertex` | gemini-pro, palm-2 | GCP native |
| `custom` | custom endpoint | Self-hosted models |

### 3.2 Provider Configuration

```yaml
llm_providers:
  openai:
    api_key: ${OPENAI_API_KEY}
    organization: ${OPENAI_ORG_ID}
    base_url: https://api.openai.com/v1
    
  anthropic:
    api_key: ${ANTHROPIC_API_KEY}
    
  azure_openai:
    api_key: ${AZURE_OPENAI_KEY}
    endpoint: https://{resource}.openai.azure.com
    api_version: "2024-02-01"
    
  custom:
    endpoint: https://custom-llm.company.com/v1/chat
    auth:
      type: bearer
      token: ${CUSTOM_LLM_TOKEN}
```

---

## 3. Prompt Engineering

### 3.1 Prompt Templates

#### 3.1.1 String Interpolation

```yaml
prompt:
  template: |
    User {user.id} from {geo.country} is attempting to transfer ${transaction.amount}.
    Their average transaction is ${avg(transaction.amounts, last_30d)}.
    
    Is this suspicious?
```

#### 3.1.2 Conditional Sections

```yaml
prompt:
  template: |
    Analyze this transaction:
    Amount: {transaction.amount}
    
    {% if device.is_new %}
    ⚠️ This is a NEW DEVICE for this user.
    {% endif %}
    
    {% if geo.country != user.home_country %}
    ⚠️ Login from different country than usual: {geo.country} vs {user.home_country}
    {% endif %}
    
    Assess risk level.
```

#### 3.1.3 Loops and Arrays

```yaml
prompt:
  template: |
    Recent transaction history:
    {% for tx in user.recent_transactions %}
    - {tx.date}: {tx.amount} to {tx.recipient}
    {% endfor %}
    
    Current transaction: {transaction.amount} to {transaction.recipient}
    
    Does this match the user's normal behavior?
```

### 3.2 System Prompts

```yaml
- type: reason
  id: fraud_detection
  
  system_prompt: |
    You are an expert fraud detection system for a financial institution.
    
    Your role:
    - Analyze transactions for fraud patterns
    - Provide risk scores between 0.0 (safe) and 1.0 (fraudulent)
    - Give clear, actionable explanations
    - Tag specific fraud categories
    
    Always respond in valid JSON format.
    
  prompt:
    template: |
      Analyze this transaction:
      {transaction_data}
```

### 3.3 Few-Shot Examples

```yaml
- type: reason
  id: account_takeover_detection
  
  few_shot_examples:
    - input: |
        User normally logs in from US, now logging from Russia.
        Device is new. Multiple failed password attempts.
      output:
        risk_score: 0.95
        risk_level: critical
        reason: Strong indicators of account takeover
        tags: [account_takeover, geo_anomaly, device_mismatch]
        
    - input: |
        User logs in from nearby city, same device, normal time.
      output:
        risk_score: 0.05
        risk_level: low
        reason: Consistent with normal user behavior
        tags: []
        
  prompt:
    template: |
      {current_event}
```

---

## 4. Output Schema

### 4.1 Structured Output

#### 4.1.1 JSON Schema Definition

```yaml
output_schema:
  type: object
  required: [risk_score, reason]
  properties:
    risk_score:
      type: number
      minimum: 0.0
      maximum: 1.0
      description: Overall risk score
      
    risk_level:
      type: string
      enum: [low, medium, high, critical]
      
    reason:
      type: string
      maxLength: 500
      description: Human-readable explanation
      
    tags:
      type: array
      items:
        type: string
      description: Categorical risk flags
      
    confidence:
      type: number
      minimum: 0.0
      maximum: 1.0
      
    factors:
      type: object
      properties:
        device_risk: number
        location_risk: number
        behavior_risk: number
```

#### 4.1.2 Output Validation

```yaml
output_validation:
  strict: true                    # Reject invalid outputs
  retry_on_failure: true          # Retry if output doesn't match schema
  max_retries: 2
  
  fallback:                       # Use if all retries fail
    risk_score: 0.5
    risk_level: medium
    reason: "LLM output validation failed"
    confidence: 0.0
```

### 4.2 Using LLM Outputs in Conditions

```yaml
# In rule conditions
conditions:
  - LLM.login_risk_analysis.risk_score > 0.7
  - LLM.login_risk_analysis.risk_level in ["high", "critical"]
  - LLM.login_risk_analysis.tags contains "account_takeover"
  - LLM.login_risk_analysis.confidence >= 0.8
  
  # Access nested fields
  - LLM.fraud_check.factors.device_risk > 0.6
  - LLM.user_analysis.behavior.consistency_score < 0.3
```

---

## 5. LLM Configuration

### 5.1 Model Parameters

```yaml
config:
  # Sampling parameters
  temperature: 0.3              # 0.0 = deterministic, 2.0 = very random
  top_p: 0.9                    # Nucleus sampling
  frequency_penalty: 0.0        # -2.0 to 2.0
  presence_penalty: 0.0         # -2.0 to 2.0
  
  # Generation limits
  max_tokens: 500               # Maximum response length
  
  # Format control (OpenAI)
  response_format:
    type: json_object           # Force JSON output
    
  # Function calling (if applicable)
  functions: [...]
  function_call: auto
```

### 5.2 Timeout and Retries

```yaml
config:
  # Timeouts
  timeout: 5000                 # Total timeout (ms)
  connect_timeout: 2000         # Connection timeout (ms)
  read_timeout: 30000           # Read timeout (ms)
  
  # Retry logic
  retry:
    max_attempts: 3
    backoff: exponential        # exponential | linear | fixed
    initial_delay: 1000         # ms
    max_delay: 10000            # ms
    retry_on:
      - timeout
      - rate_limit
      - server_error
```

### 5.3 Caching

```yaml
config:
  cache:
    enabled: true
    ttl: 3600                   # seconds
    
    # Cache key generation
    key_fields:
      - user.id
      - event.type
      - device.fingerprint
      
    # Or custom cache key
    key_template: "llm:{user.id}:{event.type}:{date}"
    
    # Cache backend
    backend: redis              # redis | memcached | memory
```

---

## 6. Error Handling

### 6.1 Error Handling Strategies

```yaml
- type: reason
  id: risk_assessment
  
  on_error:
    action: fallback            # fallback | fail | skip | retry
    
    # Fallback response
    fallback:
      risk_score: 0.5           # Conservative default
      risk_level: medium
      reason: "LLM service unavailable"
      confidence: 0.0
      
  on_timeout:
    action: skip                # Skip this step and continue
    log_level: warn
    
  on_invalid_output:
    action: retry
    max_retries: 2
```

### 6.2 Circuit Breaker

```yaml
config:
  circuit_breaker:
    enabled: true
    failure_threshold: 5        # Open circuit after N failures
    timeout: 60000              # Keep circuit open for 60s
    half_open_max_calls: 3      # Test with 3 calls before fully closing
```

### 6.3 Fallback Chain

```yaml
- type: reason
  id: primary_llm_check
  provider: openai
  model: gpt-4-turbo
  
  fallback:
    - provider: openai
      model: gpt-3.5-turbo      # Faster fallback model
      
    - provider: anthropic
      model: claude-3-haiku     # Different provider
      
    - static:                   # Static fallback
        risk_score: 0.5
        reason: "All LLM services unavailable"
```

---

## 7. Multi-Model Ensemble

### 7.1 Parallel Execution

```yaml
- parallel:
    - type: reason
      id: llm_openai_gpt4
      provider: openai
      model: gpt-4-turbo
      
    - type: reason
      id: llm_anthropic_claude
      provider: anthropic
      model: claude-3-5-sonnet
      
    - type: reason
      id: llm_custom
      provider: custom
      
  merge:
    method: all
    
# Aggregate results
- aggregate:
    method: weighted
    weights:
      llm_openai_gpt4: 0.5
      llm_anthropic_claude: 0.3
      llm_custom: 0.2
```

### 7.2 Consensus-Based Decision

```yaml
- type: consensus
  llm_steps:
    - llm_model_a
    - llm_model_b
    - llm_model_c
    
  strategy: majority            # majority | unanimous | weighted
  
  output:
    risk_score: avg([model_a.risk_score, model_b.risk_score, model_c.risk_score])
    risk_level: majority([model_a.risk_level, model_b.risk_level, model_c.risk_level])
```

---

## 8. Cost Optimization

### 8.1 Token Usage Limits

```yaml
config:
  cost_control:
    max_prompt_tokens: 2000
    max_completion_tokens: 500
    
    # Truncation strategy
    truncate:
      enabled: true
      strategy: smart           # smart | tail | head
      preserve_fields:          # Always include these
        - user.id
        - transaction.amount
```

### 8.2 Conditional LLM Execution

```yaml
# Only use expensive LLM for high-value transactions
- type: reason
  id: deep_analysis
  if: "transaction.amount > 10000"
  provider: openai
  model: gpt-4-turbo
  
# Use cheaper model for routine checks  
- type: reason
  id: basic_check
  if: "transaction.amount <= 10000"
  provider: openai
  model: gpt-3.5-turbo
```

### 8.3 Request Batching

```yaml
config:
  batching:
    enabled: true
    max_batch_size: 10
    max_wait_time: 100          # ms
```

---

## 9. Observability

### 9.1 Logging

```yaml
- type: reason
  id: fraud_check
  
  observability:
    log:
      level: info               # debug | info | warn | error
      
      include:
        - prompt                # Log full prompt
        - response              # Log full response
        - tokens                # Log token usage
        - latency               # Log response time
        - cost                  # Estimated cost
        
    log_sampling: 0.1           # Log 10% of requests
```

### 9.2 Metrics

```yaml
observability:
  metrics:
    enabled: true
    
    track:
      - llm_requests_total
      - llm_request_duration_seconds
      - llm_tokens_used_total
      - llm_errors_total
      - llm_cache_hit_rate
      - llm_cost_usd_total
      
    labels:
      - provider
      - model
      - step_id
      - outcome              # success | error | timeout
```

### 9.3 Tracing

```yaml
observability:
  tracing:
    enabled: true
    # Basic tracing support (no external provider integration)
    
    capture:
      - prompt_template
      - rendered_prompt
      - model_response
      - processing_time
      - token_usage
```

---

## 10. Security and Privacy

### 10.1 Data Masking

```yaml
- type: reason
  id: user_analysis
  
  privacy:
    mask_fields:
      - field: user.email
        method: hash            # hash | mask | redact | tokenize
        
      - field: user.ssn
        method: redact
        replacement: "***-**-****"
        
      - field: user.phone
        method: mask
        visible_chars: 4        # Show last 4 digits
```

### 10.2 PII Protection

```yaml
privacy:
  pii_detection: true           # Auto-detect PII in prompts
  
  pii_handling:
    action: mask                # mask | reject | anonymize
    
  allowed_pii_types:            # Explicitly allow certain PII
    - user_id
    - transaction_id
    
  blocked_pii_types:
    - ssn
    - credit_card
    - password
```

### 10.3 Audit Trail

```yaml
observability:
  audit:
    enabled: true
    retention_days: 90
    
    capture:
      - request_id
      - timestamp
      - user_id
      - prompt_hash           # Don't log full prompt for privacy
      - model_used
      - decision_influenced   # Did LLM affect final decision?
```

---

## 11. Testing and Validation

### 11.1 LLM Mocking for Tests

```yaml
# In test configuration
test:
  llm_mocks:
    - step_id: fraud_check
      mock_response:
        risk_score: 0.85
        risk_level: high
        reason: "Mocked response for testing"
        tags: [test_tag]
        confidence: 1.0
```

### 11.2 Prompt Testing

```yaml
prompt_tests:
  - name: "High risk scenario"
    input:
      user.id: "test_123"
      geo.country: "RU"
      device.is_new: true
      
    expected_output:
      risk_level: high
      tags: [geo_anomaly, device_mismatch]
      
  - name: "Normal scenario"
    input:
      user.id: "test_456"
      geo.country: "US"
      device.is_new: false
      
    expected_output:
      risk_level: low
```

---

## 12. Best Practices

### 12.1 Prompt Design

✅ **Good Practices:**
- Be specific and clear in instructions
- Provide structured output format
- Include relevant context only
- Use few-shot examples for consistency
- Set appropriate temperature (0.0-0.5 for deterministic tasks)

❌ **Avoid:**
- Vague or ambiguous instructions
- Too much irrelevant context
- Asking for creative/subjective assessments in risk contexts
- High temperature settings for risk evaluation

### 12.2 Output Handling

✅ **Good:**
```yaml
# Always validate LLM outputs
conditions:
  - LLM.risk_check.risk_score exists
  - LLM.risk_check.risk_score >= 0.0
  - LLM.risk_check.risk_score <= 1.0
  - LLM.risk_check.confidence > 0.6
```

❌ **Bad:**
```yaml
# Blindly trust LLM output
conditions:
  - LLM.risk_check.risk_score > 0.7
```

### 12.3 Performance

✅ **Good:**
- Use caching for repeated queries
- Implement circuit breakers
- Set appropriate timeouts
- Use cheaper models for low-risk decisions

---

## 13. Migration from Legacy Systems

### 13.1 Hybrid Approach

```yaml
# Start with rules, enhance with LLM
conditions:
  # Traditional rule checks
  - all:
      - transaction.amount > 10000
      - geo.country in high_risk_countries
      - device.is_new == true
      
  # Add LLM reasoning as additional signal
  - or:
      - user.manual_review_flag == true
      - LLM.deep_analysis.risk_score > 0.8
```

### 13.2 A/B Testing

```yaml
# Shadow mode: Run LLM but don't use results yet
- type: reason
  id: experimental_llm_check
  shadow_mode: true             # Log results but don't affect decisions
  
observability:
  compare_with: traditional_rule_result
```

---

## 14. Summary

CORINT's LLM integration provides:

- **Multiple provider support** for flexibility and redundancy
- **Structured output schemas** for type-safe integration
- **Comprehensive error handling** for production reliability
- **Cost optimization** through caching and conditional execution
- **Privacy protection** with built-in PII masking
- **Full observability** for monitoring and debugging
- **Testing support** for validation and CI/CD

LLMs enhance traditional rules with cognitive reasoning while maintaining the determinism and auditability required for risk decisioning.

