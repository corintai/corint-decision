//! Integration tests for pipeline execution
//!
//! Tests pipeline features including steps, routers, branches, and when conditions.
//!
//! NOTE: These tests are currently ignored because they use a pipeline DSL format
//! that is not yet supported by the SDK. The tests use features like:
//! - `entry:` field for pipeline entry point
//! - `steps:` with `- step:` wrapper
//! - `type: router` for routing steps
//! - `next:` for step transitions
//!
//! The SDK currently only supports the shorthand format with `- include: ruleset: xxx`

mod common;

use corint_core::ast::Action;
use corint_core::Value;
use common::{ResponseAssertions, TestEngine};
use std::collections::HashMap;

// ============================================================================
// Basic Pipeline Execution
// ============================================================================

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_simple_pipeline_with_single_ruleset() {
    let rule_yaml = r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - event.amount > 100
  score: 25
"#;

    let ruleset_yaml = r#"
ruleset:
  id: fraud_rules
  rules:
    - test_rule
  decision_logic:
    - condition: total_score > 0
      action: review
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: check_fraud
  steps:
    - step:
        id: check_fraud
        type: ruleset
        ruleset: fraud_rules
        next: end
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml)
        .with_pipeline(pipeline_yaml);

    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(150.0));

    let response = engine.execute_pipeline("test_pipeline", event).await;
    response.assert_score(25);
    response.assert_action(Action::Review);
}

// ============================================================================
// Pipeline with Multiple Steps
// ============================================================================

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_pipeline_with_sequential_rulesets() {
    let rule1 = r#"
rule:
  id: amount_check
  name: Amount Check
  when:
    conditions:
      - event.amount > 1000
  score: 30
"#;

    let rule2 = r#"
rule:
  id: user_check
  name: User Check
  when:
    conditions:
      - event.is_new_user == true
  score: 20
"#;

    let ruleset1 = r#"
ruleset:
  id: amount_rules
  rules:
    - amount_check
  decision_logic:
    - default: true
      action: approve
"#;

    let ruleset2 = r#"
ruleset:
  id: user_rules
  rules:
    - user_check
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: multi_step_pipeline
  name: Multi Step Pipeline
  entry: step1
  steps:
    - step:
        id: step1
        type: ruleset
        ruleset: amount_rules
        next: step2
    - step:
        id: step2
        type: ruleset
        ruleset: user_rules
        next: end
  decision_logic:
    - condition: total_score >= 50
      action: review
    - default: true
      action: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_ruleset(ruleset1)
        .with_ruleset(ruleset2)
        .with_pipeline(pipeline_yaml);

    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(2000.0));
    event.insert("is_new_user".to_string(), Value::Bool(true));

    let response = engine.execute_pipeline("multi_step_pipeline", event).await;
    response.assert_score(50); // 30 + 20
    response.assert_action(Action::Review);
}

// ============================================================================
// Pipeline with Router
// ============================================================================

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_pipeline_with_router_first_route() {
    let rule1 = r#"
rule:
  id: high_value_rule
  name: High Value Rule
  when:
    conditions:
      - event.amount > 0
  score: 100
"#;

    let rule2 = r#"
rule:
  id: low_value_rule
  name: Low Value Rule
  when:
    conditions:
      - event.amount > 0
  score: 10
"#;

    let ruleset1 = r#"
ruleset:
  id: high_value_rules
  rules:
    - high_value_rule
  decision_logic:
    - default: true
      action: review
"#;

    let ruleset2 = r#"
ruleset:
  id: low_value_rules
  rules:
    - low_value_rule
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: routed_pipeline
  name: Routed Pipeline
  entry: router
  steps:
    - step:
        id: router
        type: router
        routes:
          - condition: event.amount > 10000
            next: high_value
          - condition: event.amount <= 10000
            next: low_value
        default: low_value
    - step:
        id: high_value
        type: ruleset
        ruleset: high_value_rules
        next: end
    - step:
        id: low_value
        type: ruleset
        ruleset: low_value_rules
        next: end
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_ruleset(ruleset1)
        .with_ruleset(ruleset2)
        .with_pipeline(pipeline_yaml);

    // High value path
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(50000.0));

    let response = engine.execute_pipeline("routed_pipeline", event).await;
    response.assert_score(100);
    response.assert_action(Action::Review);
}

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_pipeline_with_router_second_route() {
    let rule1 = r#"
rule:
  id: high_value_rule
  name: High Value Rule
  when:
    conditions:
      - event.amount > 0
  score: 100
"#;

    let rule2 = r#"
rule:
  id: low_value_rule
  name: Low Value Rule
  when:
    conditions:
      - event.amount > 0
  score: 10
"#;

    let ruleset1 = r#"
ruleset:
  id: high_value_rules
  rules:
    - high_value_rule
  decision_logic:
    - default: true
      action: review
"#;

    let ruleset2 = r#"
ruleset:
  id: low_value_rules
  rules:
    - low_value_rule
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: routed_pipeline
  name: Routed Pipeline
  entry: router
  steps:
    - step:
        id: router
        type: router
        routes:
          - condition: event.amount > 10000
            next: high_value
          - condition: event.amount <= 10000
            next: low_value
        default: low_value
    - step:
        id: high_value
        type: ruleset
        ruleset: high_value_rules
        next: end
    - step:
        id: low_value
        type: ruleset
        ruleset: low_value_rules
        next: end
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_ruleset(ruleset1)
        .with_ruleset(ruleset2)
        .with_pipeline(pipeline_yaml);

    // Low value path
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(500.0));

    let response = engine.execute_pipeline("routed_pipeline", event).await;
    response.assert_score(10);
    response.assert_action(Action::Approve);
}

// ============================================================================
// Pipeline with When Conditions
// ============================================================================

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_pipeline_with_when_condition_matches() {
    let rule_yaml = r#"
rule:
  id: payment_rule
  name: Payment Rule
  when:
    conditions:
      - event.amount > 100
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: payment_rules
  rules:
    - payment_rule
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: payment_pipeline
  name: Payment Pipeline
  when:
    all:
      - event.type == "payment"
  entry: check
  steps:
    - step:
        id: check
        type: ruleset
        ruleset: payment_rules
        next: end
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml)
        .with_pipeline(pipeline_yaml);

    let mut event = HashMap::new();
    event.insert("type".to_string(), Value::String("payment".to_string()));
    event.insert("amount".to_string(), Value::Number(500.0));

    let response = engine.execute_pipeline("payment_pipeline", event).await;
    response.assert_score(50);
    response.assert_triggered_rules(&["payment_rule"]);
}

// ============================================================================
// Pipeline with Step When Conditions
// ============================================================================

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_step_when_condition_skips_step() {
    let rule1 = r#"
rule:
  id: always_trigger
  name: Always Trigger
  when:
    conditions:
      - event.amount > 0
  score: 10
"#;

    let rule2 = r#"
rule:
  id: premium_rule
  name: Premium Rule
  when:
    conditions:
      - event.amount > 0
  score: 100
"#;

    let ruleset1 = r#"
ruleset:
  id: basic_rules
  rules:
    - always_trigger
  decision_logic:
    - default: true
      action: approve
"#;

    let ruleset2 = r#"
ruleset:
  id: premium_rules
  rules:
    - premium_rule
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: conditional_pipeline
  name: Conditional Pipeline
  entry: basic_step
  steps:
    - step:
        id: basic_step
        type: ruleset
        ruleset: basic_rules
        next: premium_step
    - step:
        id: premium_step
        type: ruleset
        ruleset: premium_rules
        when:
          all:
            - event.is_premium == true
        next: end
  decision_logic:
    - default: true
      action: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_ruleset(ruleset1)
        .with_ruleset(ruleset2)
        .with_pipeline(pipeline_yaml);

    // Non-premium user - premium step should be skipped
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(100.0));
    event.insert("is_premium".to_string(), Value::Bool(false));

    let response = engine.execute_pipeline("conditional_pipeline", event).await;
    response.assert_score(10); // Only basic rule triggered
    response.assert_triggered_rules(&["always_trigger"]);
}

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_step_when_condition_executes_step() {
    let rule1 = r#"
rule:
  id: always_trigger
  name: Always Trigger
  when:
    conditions:
      - event.amount > 0
  score: 10
"#;

    let rule2 = r#"
rule:
  id: premium_rule
  name: Premium Rule
  when:
    conditions:
      - event.amount > 0
  score: 100
"#;

    let ruleset1 = r#"
ruleset:
  id: basic_rules
  rules:
    - always_trigger
  decision_logic:
    - default: true
      action: approve
"#;

    let ruleset2 = r#"
ruleset:
  id: premium_rules
  rules:
    - premium_rule
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: conditional_pipeline
  name: Conditional Pipeline
  entry: basic_step
  steps:
    - step:
        id: basic_step
        type: ruleset
        ruleset: basic_rules
        next: premium_step
    - step:
        id: premium_step
        type: ruleset
        ruleset: premium_rules
        when:
          all:
            - event.is_premium == true
        next: end
  decision_logic:
    - default: true
      action: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_ruleset(ruleset1)
        .with_ruleset(ruleset2)
        .with_pipeline(pipeline_yaml);

    // Premium user - both steps should execute
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(100.0));
    event.insert("is_premium".to_string(), Value::Bool(true));

    let response = engine.execute_pipeline("conditional_pipeline", event).await;
    response.assert_score(110); // 10 + 100
    response.assert_triggered_rules(&["always_trigger", "premium_rule"]);
}

// ============================================================================
// Pipeline Decision Logic
// ============================================================================

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_pipeline_level_decision_logic() {
    let rule1 = r#"
rule:
  id: rule_a
  name: Rule A
  when:
    conditions:
      - event.a == true
  score: 40
"#;

    let rule2 = r#"
rule:
  id: rule_b
  name: Rule B
  when:
    conditions:
      - event.b == true
  score: 40
"#;

    let ruleset1 = r#"
ruleset:
  id: ruleset_a
  rules:
    - rule_a
  decision_logic:
    - default: true
      action: approve
"#;

    let ruleset2 = r#"
ruleset:
  id: ruleset_b
  rules:
    - rule_b
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: step_a
  steps:
    - step:
        id: step_a
        type: ruleset
        ruleset: ruleset_a
        next: step_b
    - step:
        id: step_b
        type: ruleset
        ruleset: ruleset_b
        next: end
  decision_logic:
    - condition: total_score >= 80
      action: deny
      reason: Combined high risk
    - condition: total_score >= 40
      action: review
    - default: true
      action: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_ruleset(ruleset1)
        .with_ruleset(ruleset2)
        .with_pipeline(pipeline_yaml);

    // Both rules trigger -> 80 -> deny
    let mut event = HashMap::new();
    event.insert("a".to_string(), Value::Bool(true));
    event.insert("b".to_string(), Value::Bool(true));

    let response = engine.execute_pipeline("test_pipeline", event).await;
    response.assert_score(80);
    response.assert_action(Action::Deny);
}

// ============================================================================
// Trace Verification
// ============================================================================

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_pipeline_trace_contains_steps() {
    let rule_yaml = r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - event.value > 10
  score: 25
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - test_rule
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: traced_pipeline
  name: Traced Pipeline
  entry: main_step
  steps:
    - step:
        id: main_step
        type: ruleset
        ruleset: test_ruleset
        next: end
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml)
        .with_pipeline(pipeline_yaml);

    let mut event = HashMap::new();
    event.insert("value".to_string(), Value::Number(100.0));

    let response = engine.execute_pipeline("traced_pipeline", event).await;

    // Verify trace is present
    assert!(response.trace.is_some(), "Trace should be present");

    let trace = response.trace.as_ref().unwrap();
    assert!(trace.pipeline.is_some(), "Pipeline trace should be present");

    let pipeline_trace = trace.pipeline.as_ref().unwrap();
    assert!(!pipeline_trace.steps.is_empty(), "Steps should be traced");
    assert!(!pipeline_trace.rulesets.is_empty(), "Rulesets should be traced");
}

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_trace_contains_condition_values() {
    let rule_yaml = r#"
rule:
  id: amount_check
  name: Amount Check
  when:
    conditions:
      - event.amount > 1000
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - amount_check
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: check
  steps:
    - step:
        id: check
        type: ruleset
        ruleset: test_ruleset
        next: end
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml)
        .with_pipeline(pipeline_yaml);

    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(5000.0));

    let response = engine.execute_pipeline("test_pipeline", event).await;

    let trace = response.trace.as_ref().unwrap();
    let pipeline_trace = trace.pipeline.as_ref().unwrap();

    // Find the ruleset trace
    assert!(!pipeline_trace.rulesets.is_empty());
    let ruleset_trace = &pipeline_trace.rulesets[0];

    // Find the rule trace
    assert!(!ruleset_trace.rules.is_empty());
    let rule_trace = &ruleset_trace.rules[0];

    assert!(rule_trace.triggered, "Rule should be triggered");
    assert_eq!(rule_trace.score, Some(50));

    // Verify condition trace has values
    assert!(!rule_trace.conditions.is_empty());
    let condition_trace = &rule_trace.conditions[0];
    assert!(condition_trace.left_value.is_some(), "Left value should be present");
    assert!(condition_trace.right_value.is_some(), "Right value should be present");
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_empty_pipeline() {
    let pipeline_yaml = r#"
pipeline:
  id: empty_pipeline
  name: Empty Pipeline
  entry: end
  steps: []
  decision_logic:
    - default: true
      action: approve
"#;

    let engine = TestEngine::new()
        .with_pipeline(pipeline_yaml);

    let event = HashMap::new();

    let response = engine.execute_pipeline("empty_pipeline", event).await;
    response.assert_score(0);
    response.assert_action(Action::Approve);
}

#[tokio::test]
#[ignore = "Pipeline DSL format not yet supported by SDK"]
async fn test_pipeline_with_missing_field_in_event() {
    let rule_yaml = r#"
rule:
  id: optional_field
  name: Optional Field Check
  when:
    conditions:
      - event.optional_field > 100
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - optional_field
  decision_logic:
    - default: true
      action: approve
"#;

    let pipeline_yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: check
  steps:
    - step:
        id: check
        type: ruleset
        ruleset: test_ruleset
        next: end
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml)
        .with_pipeline(pipeline_yaml);

    // Event without the optional_field - should not crash
    let event = HashMap::new();

    let response = engine.execute_pipeline("test_pipeline", event).await;
    response.assert_score(0); // Rule should not trigger due to missing field
    response.assert_action(Action::Approve);
}
