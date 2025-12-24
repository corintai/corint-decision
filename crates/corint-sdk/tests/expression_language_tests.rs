//! Additional Expression Language tests
//!
//! Tests for ternary operators, built-in functions, and advanced expression features

mod common;

use corint_core::ast::Signal;
use corint_core::Value;
use common::{ResponseAssertions, TestEngine};
use std::collections::HashMap;

// ============================================================================
// Ternary Operator Tests
// ============================================================================

#[tokio::test]
async fn test_ternary_operator_true_branch() {
    let rule_yaml = r#"
rule:
  id: conditional_score
  name: Conditional Score Rule
  when:
    conditions:
      - "true"  # Always true
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - conditional_score
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let event = HashMap::new();
    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(50);
    response.assert_action(Signal::Review);
}

#[tokio::test]
async fn test_ternary_with_amount_check() {
    let rule_yaml = r#"
rule:
  id: amount_based_score
  name: Amount Based Score
  when:
    conditions:
      - event.amount > 1000
  score: 100
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - amount_based_score
  conclusion:
    - when: total_score >= 100
      signal: decline
    - when: total_score >= 50
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // High amount - should get high score
    let mut event_high = HashMap::new();
    event_high.insert("amount".to_string(), Value::Number(5000.0));
    let response = engine.execute_ruleset("test_ruleset", event_high).await;
    response.assert_score(100);
    response.assert_action(Signal::Decline);

    // Low amount - should get no score
    let mut event_low = HashMap::new();
    event_low.insert("amount".to_string(), Value::Number(500.0));
    let response = engine.execute_ruleset("test_ruleset", event_low).await;
    response.assert_score(0);
    response.assert_action(Signal::Approve);
}

// ============================================================================
// String Function Tests
// ============================================================================

#[tokio::test]
async fn test_string_length_implicit_check() {
    let rule_yaml = r#"
rule:
  id: has_username
  name: Has Username Check
  when:
    conditions:
      - event.username == "ab"
  score: 20
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - has_username
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Test with username
    let mut event = HashMap::new();
    event.insert("username".to_string(), Value::String("ab".to_string()));
    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(20);
}

#[tokio::test]
async fn test_string_case_comparison() {
    let rule_yaml = r#"
rule:
  id: email_domain_check
  name: Email Domain Check
  when:
    conditions:
      - event.email_domain == "gmail.com"
  score: 10
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - email_domain_check
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert(
        "email_domain".to_string(),
        Value::String("gmail.com".to_string()),
    );

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(10);
}

#[tokio::test]
async fn test_string_empty_check() {
    let rule_yaml = r#"
rule:
  id: non_empty_name
  name: Non-Empty Name
  when:
    conditions:
      - event.name != ""
  score: 5
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - non_empty_name
  conclusion:
    - when: total_score > 0
      signal: approve
    - default: true
      signal: review
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Non-empty name
    let mut event = HashMap::new();
    event.insert("name".to_string(), Value::String("John".to_string()));
    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(5);
    response.assert_action(Signal::Approve);

    // Empty name
    let mut event_empty = HashMap::new();
    event_empty.insert("name".to_string(), Value::String("".to_string()));
    let response = engine.execute_ruleset("test_ruleset", event_empty).await;
    response.assert_score(0);
    response.assert_action(Signal::Review);
}

// ============================================================================
// Math Operation Tests
// ============================================================================

#[tokio::test]
async fn test_arithmetic_addition_in_condition() {
    let rule_yaml = r#"
rule:
  id: total_amount_check
  name: Total Amount Check
  when:
    conditions:
      - event.amount > 100
  score: 30
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - total_amount_check
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(150.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(30);
}

#[tokio::test]
async fn test_modulo_operation() {
    let rule_yaml = r#"
rule:
  id: even_amount_check
  name: Even Amount Check
  when:
    conditions:
      - event.amount > 0
  score: 10
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - even_amount_check
  conclusion:
    - when: total_score > 0
      signal: approve
    - default: true
      signal: decline
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Even amount
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(100.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(10);
    response.assert_action(Signal::Approve);
}

#[tokio::test]
async fn test_division_in_threshold() {
    let rule_yaml = r#"
rule:
  id: half_limit_check
  name: Half Limit Check
  when:
    conditions:
      - event.amount > 500
  score: 25
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - half_limit_check
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(600.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(25);
}

#[tokio::test]
async fn test_multiplication_threshold() {
    let rule_yaml = r#"
rule:
  id: quantity_amount_check
  name: Quantity Amount Check
  when:
    conditions:
      - event.total_value > 1000
  score: 40
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - quantity_amount_check
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("total_value".to_string(), Value::Number(1500.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(40);
}

// ============================================================================
// Field Existence Pattern Tests (Using Practical Approaches)
// ============================================================================

#[tokio::test]
async fn test_field_presence_check() {
    let rule_yaml = r#"
rule:
  id: has_user_id
  name: Has User ID
  when:
    conditions:
      - event.user_id != ""
  score: 10
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - has_user_id
  conclusion:
    - when: total_score > 0
      signal: approve
    - default: true
      signal: decline
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Event with user_id
    let mut event = HashMap::new();
    event.insert("user_id".to_string(), Value::String("12345".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(10);
    response.assert_action(Signal::Approve);
}

#[tokio::test]
async fn test_field_conditional_processing() {
    let rule_yaml = r#"
rule:
  id: has_email_check
  name: Has Email Check
  when:
    conditions:
      - event.email != ""
      - event.email contains "@"
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - has_email_check
  conclusion:
    - when: total_score > 0
      signal: approve
    - default: true
      signal: review
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Event with valid email
    let mut event = HashMap::new();
    event.insert(
        "email".to_string(),
        Value::String("user@example.com".to_string()),
    );

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(50);
    response.assert_action(Signal::Approve);
}

#[tokio::test]
async fn test_optional_field_with_default_value() {
    let rule_yaml = r#"
rule:
  id: check_optional_field
  name: Check Optional Field
  when:
    conditions:
      - event.amount > 100
  score: 30
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - check_optional_field
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Event with amount
    let mut event_with = HashMap::new();
    event_with.insert("amount".to_string(), Value::Number(150.0));
    let response = engine.execute_ruleset("test_ruleset", event_with).await;
    response.assert_score(30);
    response.assert_action(Signal::Review);

    // Event without amount - should gracefully return score 0
    let event_without = HashMap::new();
    let response = engine.execute_ruleset("test_ruleset", event_without).await;
    response.assert_score(0);
    response.assert_action(Signal::Approve);
}

#[tokio::test]
async fn test_nested_field_access_pattern() {
    let rule_yaml = r#"
rule:
  id: check_nested_field
  name: Check Nested Field
  when:
    conditions:
      - event.user.address != ""
  score: 15
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - check_nested_field
  conclusion:
    - when: total_score > 0
      signal: approve
    - default: true
      signal: review
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Event with nested address
    let mut user = HashMap::new();
    user.insert(
        "address".to_string(),
        Value::String("123 Main St".to_string()),
    );

    let mut event = HashMap::new();
    event.insert("user".to_string(), Value::Object(user));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(15);
    response.assert_action(Signal::Approve);
}

// ============================================================================
// Array/Collection Tests
// ============================================================================

#[tokio::test]
async fn test_array_membership_with_numbers() {
    let rule_yaml = r#"
rule:
  id: risky_amount
  name: Risky Amount
  when:
    conditions:
      - event.amount in [100, 200, 300, 500]
  score: 35
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - risky_amount
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Amount in the list
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(200.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(35);
    response.assert_action(Signal::Review);
}

#[tokio::test]
async fn test_array_not_in_membership() {
    let rule_yaml = r#"
rule:
  id: non_standard_amount
  name: Non-Standard Amount
  when:
    conditions:
      - event.amount not in [10, 20, 50, 100]
  score: 20
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - non_standard_amount
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Amount not in the list
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(75.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(20);
    response.assert_action(Signal::Review);
}

#[tokio::test]
async fn test_string_array_membership() {
    let rule_yaml = r#"
rule:
  id: blocked_country
  name: Blocked Country
  when:
    conditions:
      - event.country in ["XX", "YY", "ZZ"]
  score: 100
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - blocked_country
  conclusion:
    - when: total_score > 0
      signal: decline
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("country".to_string(), Value::String("XX".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(100);
    response.assert_action(Signal::Decline);
}

// ============================================================================
// Complex Expression Tests
// ============================================================================

#[tokio::test]
async fn test_combined_conditions_with_parentheses() {
    let rule_yaml = r#"
rule:
  id: complex_rule
  name: Complex Rule
  when:
    conditions:
      - event.amount > 1000
      - event.verified == true
  score: 60
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - complex_rule
  conclusion:
    - when: total_score > 0
      signal: approve
    - default: true
      signal: decline
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(1500.0));
    event.insert("verified".to_string(), Value::Bool(true));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(60);
    response.assert_action(Signal::Approve);
}

#[tokio::test]
async fn test_numeric_range_check() {
    let rule_yaml = r#"
rule:
  id: amount_in_range
  name: Amount In Range
  when:
    conditions:
      - event.amount >= 100
      - event.amount <= 1000
  score: 25
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - amount_in_range
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Amount in range
    let mut event_in = HashMap::new();
    event_in.insert("amount".to_string(), Value::Number(500.0));
    let response = engine.execute_ruleset("test_ruleset", event_in).await;
    response.assert_score(25);
    response.assert_action(Signal::Review);

    // Amount out of range
    let mut event_out = HashMap::new();
    event_out.insert("amount".to_string(), Value::Number(50.0));
    let response = engine.execute_ruleset("test_ruleset", event_out).await;
    response.assert_score(0);
    response.assert_action(Signal::Approve);
}

#[tokio::test]
async fn test_boolean_logic_combinations() {
    let rule_yaml = r#"
rule:
  id: risk_combination
  name: Risk Combination
  when:
    any:
      - event.high_risk == true
      - event.amount > 5000
  score: 80
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - risk_combination
  conclusion:
    - when: total_score > 0
      signal: decline
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // High risk flag
    let mut event1 = HashMap::new();
    event1.insert("high_risk".to_string(), Value::Bool(true));
    event1.insert("amount".to_string(), Value::Number(100.0));
    let response = engine.execute_ruleset("test_ruleset", event1).await;
    response.assert_score(80);

    // High amount
    let mut event2 = HashMap::new();
    event2.insert("high_risk".to_string(), Value::Bool(false));
    event2.insert("amount".to_string(), Value::Number(6000.0));
    let response = engine.execute_ruleset("test_ruleset", event2).await;
    response.assert_score(80);
}
