//! Integration tests for decision logic
//!
//! Tests that decision_logic in rulesets works correctly,
//! including conditional actions, default actions, and termination.

mod common;

use corint_core::ast::Signal;
use corint_core::Value;
use common::{ResponseAssertions, TestEngine};
use std::collections::HashMap;

// ============================================================================
// Basic Decision Logic
// ============================================================================

#[tokio::test]
async fn test_decision_logic_approve() {
    let rule_yaml = r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - event.score > 0
  score: 10
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - test_rule
  conclusion:
    - default: true
      signal: approve
      reason: All checks passed
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("score".to_string(), Value::Number(5.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_action(Signal::Approve);
}

#[tokio::test]
async fn test_decision_logic_deny() {
    let rule_yaml = r#"
rule:
  id: high_risk
  name: High Risk Rule
  when:
    conditions:
      - event.risk_level > 80
  score: 100
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_risk
  conclusion:
    - when: total_score >= 100
      signal: decline
      reason: High risk detected
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("risk_level".to_string(), Value::Number(90.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(100);
    response.assert_action(Signal::Decline);
}

#[tokio::test]
async fn test_decision_logic_review() {
    let rule_yaml = r#"
rule:
  id: medium_risk
  name: Medium Risk Rule
  when:
    conditions:
      - event.risk_level > 50
  score: 60
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - medium_risk
  conclusion:
    - when: total_score >= 100
      signal: decline
    - when: total_score >= 50
      signal: review
      reason: Manual review required
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("risk_level".to_string(), Value::Number(60.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(60);
    response.assert_action(Signal::Review);
}

#[tokio::test]
async fn test_decision_logic_challenge() {
    let rule_yaml = r#"
rule:
  id: suspicious
  name: Suspicious Activity
  when:
    conditions:
      - event.suspicious_behavior == true
  score: 40
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - suspicious
  conclusion:
    - when: total_score >= 40
      signal: hold
      reason: Additional verification required
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("suspicious_behavior".to_string(), Value::Bool(true));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(40);
    response.assert_action(Signal::Hold);
}

// ============================================================================
// Score Thresholds
// ============================================================================

#[tokio::test]
async fn test_score_threshold_boundary_exact() {
    let rule_yaml = r#"
rule:
  id: exact_score
  name: Exact Score Rule
  when:
    conditions:
      - event.trigger == true
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - exact_score
  conclusion:
    - when: total_score >= 50
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("trigger".to_string(), Value::Bool(true));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(50);
    response.assert_action(Signal::Review); // Exactly at threshold
}

#[tokio::test]
async fn test_score_threshold_boundary_below() {
    let rule_yaml = r#"
rule:
  id: below_threshold
  name: Below Threshold Rule
  when:
    conditions:
      - event.trigger == true
  score: 49
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - below_threshold
  conclusion:
    - when: total_score >= 50
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("trigger".to_string(), Value::Bool(true));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(49);
    response.assert_action(Signal::Approve); // Below threshold
}

// ============================================================================
// Multiple Thresholds
// ============================================================================

#[tokio::test]
async fn test_multiple_thresholds() {
    let rule1 = r#"
rule:
  id: rule1
  name: Rule 1
  when:
    conditions:
      - event.factor1 == true
  score: 40
"#;

    let rule2 = r#"
rule:
  id: rule2
  name: Rule 2
  when:
    conditions:
      - event.factor2 == true
  score: 30
"#;

    let rule3 = r#"
rule:
  id: rule3
  name: Rule 3
  when:
    conditions:
      - event.factor3 == true
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - rule1
    - rule2
    - rule3
  conclusion:
    - when: total_score >= 100
      signal: decline
      reason: Critical risk level
    - when: total_score >= 70
      signal: review
      reason: High risk level
    - when: total_score >= 40
      signal: hold
      reason: Elevated risk level
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_rule(rule3)
        .with_ruleset(ruleset_yaml);

    // Test: score = 40 (challenge)
    let mut event1 = HashMap::new();
    event1.insert("factor1".to_string(), Value::Bool(true));
    event1.insert("factor2".to_string(), Value::Bool(false));
    event1.insert("factor3".to_string(), Value::Bool(false));

    let response1 = engine.execute_ruleset("test_ruleset", event1).await;
    response1.assert_score(40);
    response1.assert_action(Signal::Hold);

    // Test: score = 70 (review)
    let mut event2 = HashMap::new();
    event2.insert("factor1".to_string(), Value::Bool(true));
    event2.insert("factor2".to_string(), Value::Bool(true));
    event2.insert("factor3".to_string(), Value::Bool(false));

    let response2 = engine.execute_ruleset("test_ruleset", event2).await;
    response2.assert_score(70);
    response2.assert_action(Signal::Review);

    // Test: score = 120 (deny)
    let mut event3 = HashMap::new();
    event3.insert("factor1".to_string(), Value::Bool(true));
    event3.insert("factor2".to_string(), Value::Bool(true));
    event3.insert("factor3".to_string(), Value::Bool(true));

    let response3 = engine.execute_ruleset("test_ruleset", event3).await;
    response3.assert_score(120);
    response3.assert_action(Signal::Decline);
}

// ============================================================================
// Negative Scores
// ============================================================================

#[tokio::test]
async fn test_negative_score_reduces_total() {
    let rule1 = r#"
rule:
  id: risk_factor
  name: Risk Factor
  when:
    conditions:
      - event.risky == true
  score: 50
"#;

    let rule2 = r#"
rule:
  id: trust_factor
  name: Trust Factor
  when:
    conditions:
      - event.verified == true
  score: -30
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - risk_factor
    - trust_factor
  conclusion:
    - when: total_score >= 50
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_ruleset(ruleset_yaml);

    // Both triggers: 50 - 30 = 20
    let mut event = HashMap::new();
    event.insert("risky".to_string(), Value::Bool(true));
    event.insert("verified".to_string(), Value::Bool(true));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(20); // Net score
    response.assert_action(Signal::Approve); // Below threshold
}

// ============================================================================
// Triggered Rules Access
// ============================================================================

#[tokio::test]
async fn test_decision_based_on_triggered_rules_count() {
    let rule1 = r#"
rule:
  id: rule1
  name: Rule 1
  when:
    conditions:
      - event.cond1 == true
  score: 10
"#;

    let rule2 = r#"
rule:
  id: rule2
  name: Rule 2
  when:
    conditions:
      - event.cond2 == true
  score: 10
"#;

    let rule3 = r#"
rule:
  id: rule3
  name: Rule 3
  when:
    conditions:
      - event.cond3 == true
  score: 10
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - rule1
    - rule2
    - rule3
  conclusion:
    - when: triggered_count >= 3
      signal: decline
    - when: triggered_count >= 2
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_rule(rule3)
        .with_ruleset(ruleset_yaml);

    // 2 rules triggered
    let mut event = HashMap::new();
    event.insert("cond1".to_string(), Value::Bool(true));
    event.insert("cond2".to_string(), Value::Bool(true));
    event.insert("cond3".to_string(), Value::Bool(false));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_triggered_rules_count(2);
    response.assert_action(Signal::Review);
}

// ============================================================================
// No Rules Triggered
// ============================================================================

#[tokio::test]
async fn test_no_rules_triggered_default_action() {
    let rule_yaml = r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - event.trigger == true
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - test_rule
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
      reason: No risks detected
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("trigger".to_string(), Value::Bool(false));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(0);
    response.assert_action(Signal::Approve);
    response.assert_triggered_rules_count(0);
}

// ============================================================================
// Decision Logic with Complex Conditions
// ============================================================================

#[tokio::test]
async fn test_decision_logic_with_and_condition() {
    let rule1 = r#"
rule:
  id: high_amount
  name: High Amount
  when:
    conditions:
      - event.amount > 10000
  score: 50
"#;

    let rule2 = r#"
rule:
  id: new_user
  name: New User
  when:
    conditions:
      - event.is_new == true
  score: 30
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_amount
    - new_user
  conclusion:
    - when: total_score >= 50 && triggered_count >= 2
      signal: decline
      reason: Multiple risk factors
    - when: total_score >= 50
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_ruleset(ruleset_yaml);

    // Both rules triggered: 80 score, 2 rules -> deny
    let mut event1 = HashMap::new();
    event1.insert("amount".to_string(), Value::Number(15000.0));
    event1.insert("is_new".to_string(), Value::Bool(true));

    let response1 = engine.execute_ruleset("test_ruleset", event1).await;
    response1.assert_score(80);
    response1.assert_action(Signal::Decline);

    // Only high_amount triggered: 50 score, 1 rule -> review
    let mut event2 = HashMap::new();
    event2.insert("amount".to_string(), Value::Number(15000.0));
    event2.insert("is_new".to_string(), Value::Bool(false));

    let response2 = engine.execute_ruleset("test_ruleset", event2).await;
    response2.assert_score(50);
    response2.assert_action(Signal::Review);
}

// ============================================================================
// Empty Ruleset
// ============================================================================

#[tokio::test]
async fn test_empty_ruleset_default_action() {
    let ruleset_yaml = r#"
ruleset:
  id: empty_ruleset
  rules: []
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_ruleset(ruleset_yaml);

    let event = HashMap::new();

    let response = engine.execute_ruleset("empty_ruleset", event).await;
    response.assert_score(0);
    response.assert_action(Signal::Approve);
    response.assert_triggered_rules_count(0);
}

// ============================================================================
// Zero Score Rules
// ============================================================================

#[tokio::test]
async fn test_zero_score_rule() {
    let rule_yaml = r#"
rule:
  id: tracking_rule
  name: Tracking Rule
  when:
    conditions:
      - event.tracked == true
  score: 0
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - tracking_rule
  conclusion:
    - when: triggered_count > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("tracked".to_string(), Value::Bool(true));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(0);
    response.assert_triggered_rules(&["tracking_rule"]);
    response.assert_action(Signal::Review);
}
