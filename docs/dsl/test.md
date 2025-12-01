# CORINT Risk Definition Language (RDL)
## Testing Framework Specification (v0.1)

Comprehensive testing is essential for reliable risk decisioning systems.  
This document defines CORINT's testing framework for rules, rulesets, and pipelines.

---

## 1. Overview

### 1.1 Testing Levels

| Level | Scope | Purpose |
|-------|-------|---------|
| **Unit Tests** | Individual rules | Verify rule logic |
| **Integration Tests** | Rulesets | Test rule combinations |
| **Pipeline Tests** | Full pipelines | End-to-end validation |
| **Regression Tests** | Historical cases | Prevent regressions |
| **Performance Tests** | Load testing | Verify scalability |

---

## 2. Rule Testing

### 2.1 Basic Rule Test

```yaml
test:
  rule_id: high_risk_login
  
  cases:
    - name: "Trigger on high-risk country with new device"
      input:
        event:
          type: login
          user:
            id: "user_123"
            login_failed_count: 5
          device:
            is_new: true
          geo:
            country: "RU"
            
      # Mock external dependencies
      mocks:
        llm:
          reason: "suspicious behavior detected"
          score: 0.85
          
      # Expected outcome
      expect:
        triggered: true
        score: 80
        action: review
```

### 2.2 Multiple Test Cases

```yaml
test:
  rule_id: high_value_transaction
  
  cases:
    - name: "Small transaction - should not trigger"
      input:
        event:
          type: payment
          transaction:
            amount: 100
      expect:
        triggered: false
        
    - name: "Large transaction - should trigger"
      input:
        event:
          type: payment
          transaction:
            amount: 50000
      expect:
        triggered: true
        score: 100
        action: review
        
    - name: "Medium transaction by premium user - should not trigger"
      input:
        event:
          type: payment
          transaction:
            amount: 5000
          user:
            tier: "premium"
      expect:
        triggered: false
```

### 2.3 Condition Testing

```yaml
test:
  rule_id: complex_logic_rule
  
  # Test individual conditions
  test_conditions:
    - condition: "user.age > 18"
      cases:
        - input: {user: {age: 25}}
          expect: true
        - input: {user: {age: 16}}
          expect: false
          
    - condition: "geo.country in ['RU', 'NG']"
      cases:
        - input: {geo: {country: "RU"}}
          expect: true
        - input: {geo: {country: "US"}}
          expect: false
```

---

## 3. Ruleset Testing

### 3.1 Ruleset Test

```yaml
test:
  ruleset_id: login_risk_rules
  
  input:
    event:
      type: login
      user:
        id: "user_123"
        login_failed_count: 5
      device:
        is_new: true
        type: "mobile"
      geo:
        country: "RU"
        ip: "192.0.2.1"
        
  mocks:
    llm:
      behavior_analysis:
        risk_score: 0.85
        tags: ["device_mismatch", "geo_anomaly"]
        
  expect:
    triggered_rules:
      - high_risk_login
      - device_risk_check
      - ip_reputation_check
      
    total_score: 240
    
    rule_results:
      high_risk_login:
        triggered: true
        score: 80
      device_risk_check:
        triggered: true
        score: 60
      ip_reputation_check:
        triggered: true
        score: 100
```

---

## 4. Pipeline Testing

### 4.1 End-to-End Pipeline Test

```yaml
test:
  pipeline_id: login_risk_pipeline
  
  scenario: "High-risk login attempt from new device in suspicious country"
  
  input:
    event:
      type: login
      timestamp: "2024-01-15T10:30:00Z"
      user:
        id: "user_123"
        email: "user@example.com"
        last_login_country: "US"
      device:
        id: "device_456"
        type: "mobile"
        is_new: true
      geo:
        country: "RU"
        ip: "192.0.2.1"
        city: "Moscow"
        
  mocks:
    # Mock LLM responses
    llm:
      login_risk_analysis:
        risk_score: 0.85
        risk_level: "high"
        reason: "Suspicious login pattern detected"
        tags: ["geo_anomaly", "device_mismatch"]
        confidence: 0.9
        
    # Mock external API responses
    external_api:
      ip_reputation:
        score: 75
        risk_level: "high"
      device_fingerprint:
        trust_score: 0.3
        is_bot: false
        
  expect:
    # Pipeline execution
    status: success
    execution_time_ms: { max: 5000 }
    
    # Step results
    steps:
      extract_device:
        status: completed
        output:
          device_score: { gte: 70 }
          
      llm_analysis:
        status: completed
        output:
          risk_score: 0.85
          
      login_risk_rules:
        status: completed
        triggered_rules: { count: { gte: 2 } }
        
    # Final decision
    final_decision:
      action: review
      score: { gte: 80, lte: 100 }
      confidence: { gte: 0.8 }
```

### 4.2 Branch Testing

```yaml
test:
  pipeline_id: multi_event_pipeline
  
  cases:
    - name: "Login event routing"
      input:
        event:
          type: login
          user: {...}
      expect:
        branch_taken: login_flow
        steps_executed:
          - extract_login
          - login_risk_rules
          
    - name: "Payment event routing"
      input:
        event:
          type: payment
          transaction: {...}
      expect:
        branch_taken: payment_flow
        steps_executed:
          - extract_payment
          - payment_risk_rules
```

---

## 5. Mocking

### 5.1 LLM Mocking

```yaml
mocks:
  llm:
    # Mock specific LLM step
    fraud_analysis:
      risk_score: 0.75
      reason: "Mocked reason"
      tags: ["test_tag"]
      confidence: 0.9
      
    # Mock all LLM calls with same response
    default:
      risk_score: 0.5
      reason: "Default mock"
      
    # Mock with function
    dynamic:
      function: |
        (input) => {
          if (input.transaction.amount > 10000) {
            return { risk_score: 0.9, reason: "Large amount" };
          }
          return { risk_score: 0.3, reason: "Normal" };
        }
```

### 5.2 External API Mocking

```yaml
mocks:
  external_api:
    Chainalysis:
      risk_score: 85
      risk_category: "high"
      
    DeviceFingerprint:
      trust_score: 0.7
      device_age_days: 30
      
    # Mock with delay to simulate latency
    SlowService:
      response: { score: 50 }
      delay_ms: 2000
      
    # Mock with error
    FailingService:
      error: "Service unavailable"
      error_code: 503
```

### 5.3 Database Mocking

```yaml
mocks:
  database:
    users:
      - id: "user_123"
        email: "user@example.com"
        tier: "premium"
        risk_score: 45
        
    transactions:
      - id: "tx_1"
        user_id: "user_123"
        amount: 1000
        timestamp: "2024-01-15T09:00:00Z"
```

---

## 6. Assertions

### 6.1 Exact Match

```yaml
expect:
  action: deny
  score: 85
  triggered: true
```

### 6.2 Comparison Operators

```yaml
expect:
  score:
    gte: 80                       # Greater than or equal
    lte: 100                      # Less than or equal
    gt: 79                        # Greater than
    lt: 101                       # Less than
    
  execution_time_ms:
    max: 5000
```

### 6.3 Array Assertions

```yaml
expect:
  triggered_rules:
    contains: ["high_risk_login", "device_check"]
    length: 3
    
  tags:
    includes: "suspicious"
    not_includes: "trusted"
```

### 6.4 Object Assertions

```yaml
expect:
  output:
    fields:
      risk_score: { exists: true }
      reason: { not_null: true }
      tags: { type: "array" }
      
    partial_match:
      risk_level: "high"
      confidence: { gte: 0.8 }
```

### 6.5 Regular Expression

```yaml
expect:
  user_id:
    regex: "^user_[0-9]+$"
    
  reason:
    contains_regex: "suspicious|anomaly|fraud"
```

---

## 7. Test Data Management

### 7.1 Test Fixtures

```yaml
fixtures:
  users:
    normal_user:
      id: "user_normal"
      tier: "basic"
      risk_score: 30
      created_at: "2023-01-01T00:00:00Z"
      
    premium_user:
      id: "user_premium"
      tier: "premium"
      risk_score: 20
      
    high_risk_user:
      id: "user_risky"
      tier: "basic"
      risk_score: 85
      flagged: true
      
  events:
    normal_login:
      type: login
      user: $ref:users.normal_user
      device: { is_new: false }
      geo: { country: "US" }
      
    suspicious_login:
      type: login
      user: $ref:users.high_risk_user
      device: { is_new: true }
      geo: { country: "RU" }
```

### 7.2 Using Fixtures

```yaml
test:
  rule_id: login_check
  
  cases:
    - name: "Normal user login"
      input:
        event: $ref:fixtures.events.normal_login
      expect:
        triggered: false
        
    - name: "Suspicious login"
      input:
        event: $ref:fixtures.events.suspicious_login
      expect:
        triggered: true
```

---

## 8. Regression Testing

### 8.1 Historical Case Testing

```yaml
regression_tests:
  suite: "Q4_2024_fraud_cases"
  
  source:
    type: database
    query: |
      SELECT * FROM fraud_cases 
      WHERE quarter = 'Q4_2024' 
      AND outcome IS NOT NULL
      
  cases:
    auto_generate: true
    
    # Each historical case becomes a test
    test_template:
      input: { event: $row.event_data }
      expect:
        action: $row.outcome
        score: 
          # Allow some variance
          gte: $row.score - 10
          lte: $row.score + 10
```

### 8.2 Regression Test Suite

```yaml
test_suite:
  name: "Critical Regression Tests"
  
  cases:
    - name: "CVE-2024-001 - Bypass attempt"
      description: "Ensure vulnerability fix still works"
      input: {...}
      expect: {...}
      
    - name: "False positive case from ticket #1234"
      description: "User complained, fixed in v1.2.0"
      input: {...}
      expect:
        action: approve              # Should not deny
```

---

## 9. Performance Testing

### 9.1 Load Testing

```yaml
performance_test:
  type: load
  
  config:
    # Ramp up
    duration: 300                    # seconds
    users: 1000                      # concurrent users
    ramp_up_time: 60                 # seconds
    
  scenario:
    pipeline_id: login_risk_pipeline
    
    input_generator:
      type: random
      template:
        event:
          type: login
          user:
            id: $random.uuid()
          device:
            is_new: $random.boolean(0.1)    # 10% new devices
          geo:
            country: $random.choice(["US", "UK", "RU", "CN"])
            
  expect:
    # Performance SLOs
    p50_latency_ms: { max: 100 }
    p95_latency_ms: { max: 500 }
    p99_latency_ms: { max: 1000 }
    error_rate: { max: 0.01 }        # 1%
    throughput_rps: { min: 100 }     # Requests per second
```

### 9.2 Stress Testing

```yaml
performance_test:
  type: stress
  
  config:
    # Push to limits
    initial_users: 100
    max_users: 10000
    increment: 100
    increment_interval: 30           # seconds
    
    stop_on:
      - error_rate: { gt: 0.05 }     # 5%
      - p95_latency_ms: { gt: 5000 }
      
  expect:
    max_users_handled: { gte: 5000 }
```

### 9.3 Latency Testing

```yaml
test:
  pipeline_id: high_performance_pipeline
  
  iterations: 1000
  
  expect:
    # Strict latency requirements
    execution_time_ms:
      p50: { max: 50 }
      p95: { max: 200 }
      p99: { max: 500 }
      max: { max: 1000 }
      
    # Step-level latency
    steps:
      llm_analysis:
        duration_ms: { max: 3000 }
      external_api:
        duration_ms: { max: 1000 }
```

---

## 10. Integration Testing

### 10.1 External Service Integration

```yaml
integration_test:
  name: "LLM Service Integration"
  
  # Use real services (no mocks)
  use_real_services: true
  
  services:
    - llm_service
    
  test:
    pipeline_id: llm_enabled_pipeline
    
    input:
      event: {...}
      
    expect:
      # Verify real LLM response format
      steps:
        llm_analysis:
          output:
            risk_score: { type: "number", gte: 0, lte: 1 }
            reason: { type: "string", min_length: 10 }
            confidence: { type: "number" }
```

### 10.2 Database Integration

```yaml
integration_test:
  name: "User Profile Enrichment"
  
  setup:
    # Setup test data in database
    database:
      insert:
        users:
          - id: "test_user_1"
            email: "test@example.com"
            tier: "premium"
            
  test:
    pipeline_id: enrichment_pipeline
    input:
      event:
        user:
          id: "test_user_1"
          
  expect:
    context:
      user_profile:
        tier: "premium"
        
  teardown:
    # Clean up test data
    database:
      delete:
        users:
          - id: "test_user_1"
```

---

## 11. Property-Based Testing

### 11.1 Invariant Testing

```yaml
property_test:
  name: "Risk score bounds"
  
  property: "Risk score must always be between 0 and 100"
  
  generator:
    # Generate random inputs
    event:
      type: $random.choice(["login", "payment", "transfer"])
      user:
        age: $random.int(0, 100)
        risk_score: $random.float(0, 100)
      transaction:
        amount: $random.float(0, 1000000)
        
  iterations: 1000
  
  invariant: |
    output.score >= 0 && output.score <= 100
```

### 11.2 Idempotency Testing

```yaml
property_test:
  name: "Pipeline idempotency"
  
  property: "Same input should produce same output"
  
  test:
    # Run pipeline twice with same input
    run1:
      pipeline_id: deterministic_pipeline
      input: {...}
      
    run2:
      pipeline_id: deterministic_pipeline
      input: {...}  # Same input
      
  expect:
    run1.output == run2.output
```

---

## 12. Test Organization

### 12.1 Test Suites

```yaml
test_suite:
  name: "Login Risk Detection"
  
  setup:
    # Suite-level setup
    mocks: {...}
    fixtures: {...}
    
  tests:
    - $ref: tests/login/high_risk_login_test.yml
    - $ref: tests/login/device_check_test.yml
    - $ref: tests/login/geo_anomaly_test.yml
    
  teardown:
    # Suite-level cleanup
    clear_cache: true
```

### 12.2 Test Tags

```yaml
test:
  rule_id: high_value_transaction
  
  tags:
    - unit
    - payment
    - critical
    - fast
    
  # Run only tests with specific tags
  # pytest -m "critical and not slow"
```

---

## 13. Coverage Analysis

### 13.1 Coverage Configuration

```yaml
coverage:
  enabled: true
  
  targets:
    rules: 90%                      # 90% rule coverage
    conditions: 85%                 # 85% condition coverage
    branches: 80%                   # 80% branch coverage
    
  report:
    format: html
    output: coverage_report/
    
  fail_under: 80                    # Fail if < 80% overall
```

### 13.2 Coverage Report

```yaml
# Coverage report output
coverage_report:
  overall: 87%
  
  by_component:
    rules:
      covered: 45
      total: 50
      percentage: 90%
      
    conditions:
      covered: 340
      total: 400
      percentage: 85%
      
  uncovered:
    - rule: edge_case_rule
      conditions: ["condition_3", "condition_5"]
      
    - rule: deprecated_rule
      reason: "Marked for deprecation"
```

---

## 14. Continuous Integration

### 14.1 CI Configuration

```yaml
ci:
  # Run tests on every commit
  on: [push, pull_request]
  
  jobs:
    unit_tests:
      run: corint test --suite unit
      timeout: 5m
      
    integration_tests:
      run: corint test --suite integration
      timeout: 15m
      requires: [unit_tests]
      
    performance_tests:
      run: corint test --suite performance
      timeout: 30m
      on_branch: [main]
      
  # Quality gates
  gates:
    - test_pass_rate: { min: 100% }
    - coverage: { min: 80% }
    - no_critical_failures: true
```

---

## 15. Test Utilities

### 15.1 Test Helpers

```yaml
test_helpers:
  # Generate test user
  create_user:
    function: |
      (tier = "basic") => ({
        id: $random.uuid(),
        email: $random.email(),
        tier: tier,
        created_at: $now()
      })
      
  # Generate transaction
  create_transaction:
    function: |
      (amount, user_id) => ({
        id: $random.uuid(),
        amount: amount,
        user_id: user_id,
        timestamp: $now()
      })
```

### 15.2 Custom Matchers

```yaml
custom_matchers:
  # Check if score is in risk category
  to_be_high_risk:
    condition: value >= 80
    
  to_be_medium_risk:
    condition: value >= 50 && value < 80
    
  to_be_low_risk:
    condition: value < 50

# Usage
expect:
  score: { to_be_high_risk: true }
```

---

## 16. Best Practices

### 16.1 Test Design

✅ **Good Practices:**
- Test one thing per test case
- Use descriptive test names
- Test edge cases and boundaries
- Mock external dependencies
- Use fixtures for reusable data
- Maintain test independence

❌ **Avoid:**
- Testing multiple scenarios in one test
- Vague test names like "test1"
- Ignoring edge cases
- Tests dependent on external services
- Hard-coded test data
- Tests depending on execution order

### 16.2 Example: Well-Structured Test

```yaml
test_suite:
  name: "High Value Transaction Detection"
  
  fixtures:
    users:
      basic_user: {...}
      premium_user: {...}
      
  tests:
    - name: "Should trigger for large transaction by basic user"
      input:
        event:
          transaction: { amount: 50000 }
          user: $ref:fixtures.users.basic_user
      expect:
        triggered: true
        score: 100
        
    - name: "Should not trigger for large transaction by premium user"
      input:
        event:
          transaction: { amount: 50000 }
          user: $ref:fixtures.users.premium_user
      expect:
        triggered: false
        
    - name: "Should handle boundary at exactly threshold"
      input:
        event:
          transaction: { amount: 10000 }
          user: $ref:fixtures.users.basic_user
      expect:
        triggered: true
```

---

## 17. Backtesting Framework

### 17.1 Overview

Backtesting enables evaluation of rule changes against historical data to predict impact before deployment.

### 17.2 Backtest Configuration

```yaml
backtest:
  id: "backtest-2024-02-fraud-rules-v2"
  name: "Fraud Rules V2 Impact Analysis"
  description: "Evaluate new ML-based fraud rules against historical data"

  # Dataset configuration
  dataset:
    source: historical_decisions
    filters:
      date_range:
        start: "2024-01-01"
        end: "2024-01-31"
      event_types: [transaction, transfer]
      sample_size: 100000
      sampling_method: stratified       # random | stratified | all

    # Stratification for balanced testing
    stratification:
      - field: outcome
        distribution:
          fraud_confirmed: 0.05
          legitimate: 0.95
      - field: transaction.amount
        buckets: [0-1000, 1000-10000, 10000+]

  # What to compare
  comparison:
    baseline:
      type: production
      version: "2.0.0"

    candidate:
      type: ruleset
      id: fraud_detection_v2
      version: "2.1.0"

  # Metrics to evaluate
  metrics:
    primary:
      - id: precision
        description: "True positives / (True positives + False positives)"
        goal: maximize
        minimum: 0.90

      - id: recall
        description: "True positives / (True positives + False negatives)"
        goal: maximize
        minimum: 0.85

      - id: f1_score
        description: "Harmonic mean of precision and recall"
        goal: maximize

    secondary:
      - id: latency_p99
        description: "99th percentile latency"
        goal: minimize
        maximum: 200ms

      - id: false_positive_rate
        description: "False positives / Total negatives"
        goal: minimize
        maximum: 0.02

      - id: decision_change_rate
        description: "How many decisions would change"
        threshold: 0.10                  # Alert if > 10% change

  # Ground truth configuration
  ground_truth:
    source: fraud_outcomes
    label_field: is_fraud
    delay: 30d                          # Wait 30 days for chargebacks
```

### 17.3 Backtest Execution

```yaml
backtest_execution:
  # Parallel processing
  parallelism: 10

  # Progress tracking
  checkpoint_interval: 1000             # Save progress every 1000 events

  # Error handling
  on_error:
    strategy: skip                      # skip | fail | retry
    max_errors: 100
    log_errors: true

  # Resource limits
  limits:
    max_memory_gb: 16
    max_duration: 4h
    timeout_per_event: 5s
```

### 17.4 Backtest Results

```yaml
backtest_results:
  id: "backtest-2024-02-fraud-rules-v2"
  status: completed
  executed_at: "2024-02-15T10:00:00Z"
  duration: 45m

  summary:
    total_events: 100000
    processed: 99850
    errors: 150

    # Overall comparison
    baseline_vs_candidate:
      decisions_changed: 3250
      change_rate: 3.25%

      improvements:
        - metric: precision
          baseline: 0.88
          candidate: 0.93
          change: "+5.7%"

        - metric: recall
          baseline: 0.82
          candidate: 0.87
          change: "+6.1%"

      regressions:
        - metric: latency_p99
          baseline: 120ms
          candidate: 145ms
          change: "+20.8%"
          within_threshold: true

  # Confusion matrix
  confusion_matrix:
    baseline:
      true_positive: 4200
      false_positive: 550
      true_negative: 94500
      false_negative: 750

    candidate:
      true_positive: 4450
      false_positive: 380
      true_negative: 94670
      false_negative: 500

  # Decision changes breakdown
  decision_changes:
    deny_to_approve: 180
    approve_to_deny: 420
    review_to_deny: 150
    deny_to_review: 200

    # High-impact changes
    high_value_changes:
      - event_id: "evt-123"
        amount: 50000
        baseline_decision: approve
        candidate_decision: deny
        ground_truth: fraud
        analysis: "Correctly caught by new ML rule"

  # Rule-level analysis
  rule_analysis:
    new_rules:
      - rule_id: ml_behavior_pattern
        triggers: 2500
        true_positives: 2100
        false_positives: 400
        precision: 0.84

    removed_rules:
      - rule_id: legacy_geo_check
        would_have_triggered: 800
        impact: "15 frauds would be missed"

    modified_rules:
      - rule_id: velocity_check
        baseline_triggers: 3000
        candidate_triggers: 3500
        precision_change: "+3%"

  # Recommendations
  recommendations:
    - type: approve
      confidence: high
      reason: "Significant improvement in precision and recall"

    - type: monitor
      metric: latency_p99
      reason: "25% latency increase, may need optimization"

    - type: investigate
      cases: 50
      reason: "Review false positive cases for pattern analysis"
```

### 17.5 A/B Backtest

```yaml
ab_backtest:
  id: "ab-backtest-threshold-optimization"
  name: "Threshold Optimization Study"

  # Test multiple variants
  variants:
    - id: threshold_70
      ruleset: fraud_detection
      vars:
        risk_threshold: 70

    - id: threshold_80
      ruleset: fraud_detection
      vars:
        risk_threshold: 80

    - id: threshold_90
      ruleset: fraud_detection
      vars:
        risk_threshold: 90

  dataset:
    source: historical_decisions
    date_range:
      start: "2024-01-01"
      end: "2024-01-31"
    sample_size: 50000

  metrics:
    - precision
    - recall
    - false_positive_rate

  # Find optimal variant
  optimization:
    objective: maximize_f1
    constraints:
      - false_positive_rate < 0.03
```

### 17.6 Time Series Backtest

```yaml
timeseries_backtest:
  id: "rolling-window-backtest"
  name: "Rolling Window Performance Analysis"

  # Simulate production rollout over time
  windows:
    - start: "2024-01-01"
      end: "2024-01-07"
      label: "Week 1"

    - start: "2024-01-08"
      end: "2024-01-14"
      label: "Week 2"

    - start: "2024-01-15"
      end: "2024-01-21"
      label: "Week 3"

    - start: "2024-01-22"
      end: "2024-01-31"
      label: "Week 4"

  # Track metrics over time
  track:
    - precision
    - recall
    - volume
    - fraud_rate

  # Detect performance drift
  drift_detection:
    enabled: true
    threshold: 0.10                     # 10% change triggers alert
    baseline: first_window
```

### 17.7 Backtest Comparison Report

```yaml
backtest_report:
  format: html                          # html | pdf | json

  sections:
    - executive_summary
    - metric_comparison
    - confusion_matrix
    - decision_changes
    - rule_analysis
    - case_studies
    - recommendations

  visualizations:
    - type: precision_recall_curve
    - type: roc_curve
    - type: decision_distribution
    - type: score_histogram
    - type: time_series_metrics

  export:
    path: reports/backtest-{id}-{date}.html
    notify:
      - risk-team@company.com
```

### 17.8 Continuous Backtesting

```yaml
continuous_backtest:
  enabled: true

  # Run backtest on every rule change
  trigger:
    on: [rule_change, ruleset_change]
    branch: [main, develop]

  # Quick backtest for PRs
  quick_backtest:
    sample_size: 10000
    max_duration: 10m
    metrics: [precision, recall]

  # Full backtest for releases
  full_backtest:
    sample_size: 100000
    max_duration: 2h
    all_metrics: true

  # Quality gates
  gates:
    - metric: precision
      change: { max_decrease: 0.02 }

    - metric: recall
      change: { max_decrease: 0.02 }

    - metric: false_positive_rate
      change: { max_increase: 0.01 }

  # Auto-approve if gates pass
  auto_approve:
    enabled: true
    requires: all_gates_pass
```

### 17.9 Backtest Best Practices

**Good:**
```yaml
backtest:
  # Sufficient sample size
  dataset:
    sample_size: 100000
    sampling_method: stratified

  # Clear ground truth
  ground_truth:
    source: confirmed_outcomes
    delay: 30d                          # Wait for chargebacks

  # Multiple metrics
  metrics:
    primary: [precision, recall, f1_score]
    secondary: [latency, volume]

  # Statistical rigor
  confidence_level: 0.95
  min_samples_per_segment: 1000
```

**Avoid:**
```yaml
backtest:
  # Too small sample
  dataset:
    sample_size: 100                    # Not statistically significant

  # No ground truth verification
  ground_truth: null                    # Can't validate results

  # Single metric
  metrics: [precision]                  # Missing important trade-offs
```

---

## 18. Summary

CORINT's testing framework provides:

- **Multi-level testing** from units to end-to-end
- **Comprehensive mocking** for external dependencies
- **Flexible assertions** for validation
- **Performance testing** for scalability
- **Regression testing** for stability
- **Property-based testing** for invariants
- **Coverage analysis** for completeness
- **CI integration** for automation
- **Backtesting framework** for historical data analysis

Proper testing ensures:
- Correct rule logic
- Reliable pipelines
- Performance at scale
- Prevention of regressions
- Confidence in deployments
- Data-driven rule optimization

---

## 19. Related Documentation

- `rule.md` - Rule specification and testing
- `deployment.md` - A/B testing in production
- `observability.md` - Monitoring test results
- `compliance.md` - Compliance testing requirements
