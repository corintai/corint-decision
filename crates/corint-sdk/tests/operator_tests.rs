//! Integration tests for operators
//!
//! Tests that operators work correctly end-to-end from YAML parsing
//! through compilation and execution.

mod common;

use corint_core::ast::Signal;
use corint_core::Value;
use common::{ResponseAssertions, TestEngine};
use std::collections::HashMap;

// ============================================================================
// Comparison Operators
// ============================================================================

#[tokio::test]
async fn test_greater_than_operator() {
    let rule_yaml = r#"
rule:
  id: high_amount
  name: High Amount Rule
  when:
    conditions:
      - event.amount > 1000
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_amount
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Test: amount > 1000 should trigger
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(1500.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(50);
    response.assert_action(Signal::Review);
    response.assert_triggered_rules(&["high_amount"]);
}

#[tokio::test]
async fn test_greater_than_operator_not_triggered() {
    let rule_yaml = r#"
rule:
  id: high_amount
  name: High Amount Rule
  when:
    conditions:
      - event.amount > 1000
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_amount
  conclusion:
    - when: total_score > 0
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Test: amount <= 1000 should not trigger
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(500.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(0);
    response.assert_action(Signal::Approve);
    response.assert_triggered_rules_count(0);
}

#[tokio::test]
async fn test_less_than_operator() {
    let rule_yaml = r#"
rule:
  id: low_balance
  name: Low Balance Rule
  when:
    conditions:
      - event.balance < 100
  score: 30
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - low_balance
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("balance".to_string(), Value::Number(50.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(30);
    response.assert_triggered_rules(&["low_balance"]);
}

#[tokio::test]
async fn test_equality_operator() {
    let rule_yaml = r#"
rule:
  id: country_us
  name: US Country Rule
  when:
    conditions:
      - event.country == "US"
  score: 10
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - country_us
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("country".to_string(), Value::String("US".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(10);
    response.assert_triggered_rules(&["country_us"]);
}

#[tokio::test]
async fn test_not_equal_operator() {
    let rule_yaml = r#"
rule:
  id: non_premium
  name: Non Premium User
  when:
    conditions:
      - event.tier != "premium"
  score: 20
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - non_premium
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("tier".to_string(), Value::String("basic".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(20);
    response.assert_triggered_rules(&["non_premium"]);
}

#[tokio::test]
async fn test_greater_or_equal_operator() {
    let rule_yaml = r#"
rule:
  id: adult
  name: Adult User
  when:
    conditions:
      - event.age >= 18
  score: 5
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - adult
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Test exact boundary
    let mut event = HashMap::new();
    event.insert("age".to_string(), Value::Number(18.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(5);
    response.assert_triggered_rules(&["adult"]);
}

#[tokio::test]
async fn test_less_or_equal_operator() {
    let rule_yaml = r#"
rule:
  id: low_risk
  name: Low Risk Score
  when:
    conditions:
      - event.risk_score <= 30
  score: -10
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - low_risk
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("risk_score".to_string(), Value::Number(30.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(-10);
}

// ============================================================================
// String Operators
// ============================================================================

#[tokio::test]
async fn test_contains_operator() {
    let rule_yaml = r#"
rule:
  id: suspicious_email
  name: Suspicious Email Domain
  when:
    conditions:
      - event.email contains "tempmail"
  score: 40
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - suspicious_email
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("email".to_string(), Value::String("user@tempmail.com".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(40);
    response.assert_triggered_rules(&["suspicious_email"]);
}

#[tokio::test]
async fn test_starts_with_operator() {
    let rule_yaml = r#"
rule:
  id: internal_ip
  name: Internal IP Address
  when:
    conditions:
      - event.ip starts_with "192.168"
  score: -20
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - internal_ip
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("ip".to_string(), Value::String("192.168.1.100".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(-20);
    response.assert_triggered_rules(&["internal_ip"]);
}

#[tokio::test]
async fn test_ends_with_operator() {
    let rule_yaml = r#"
rule:
  id: gov_email
  name: Government Email
  when:
    conditions:
      - event.email ends_with ".gov"
  score: -30
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - gov_email
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("email".to_string(), Value::String("admin@whitehouse.gov".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(-30);
    response.assert_triggered_rules(&["gov_email"]);
}

// ============================================================================
// In Operator
// ============================================================================

#[tokio::test]
async fn test_in_operator_with_array() {
    let rule_yaml = r#"
rule:
  id: high_risk_country
  name: High Risk Country
  when:
    conditions:
      - event.country in ["RU", "CN", "NK", "IR"]
  score: 80
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_risk_country
  conclusion:
    - when: total_score >= 80
      signal: decline
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("country".to_string(), Value::String("RU".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(80);
    response.assert_action(Signal::Decline);
    response.assert_triggered_rules(&["high_risk_country"]);
}

#[tokio::test]
async fn test_in_operator_not_in_array() {
    let rule_yaml = r#"
rule:
  id: high_risk_country
  name: High Risk Country
  when:
    conditions:
      - event.country in ["RU", "CN", "NK", "IR"]
  score: 80
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_risk_country
  conclusion:
    - when: total_score >= 80
      signal: decline
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("country".to_string(), Value::String("US".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(0);
    response.assert_action(Signal::Approve);
    response.assert_triggered_rules_count(0);
}

// ============================================================================
// Logical Operators
// ============================================================================

#[tokio::test]
async fn test_and_operator() {
    let rule_yaml = r#"
rule:
  id: high_value_new_user
  name: High Value New User
  when:
    conditions:
      - event.amount > 5000 && event.account_age_days < 30
  score: 60
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_value_new_user
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(10000.0));
    event.insert("account_age_days".to_string(), Value::Number(5.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(60);
    response.assert_triggered_rules(&["high_value_new_user"]);
}

#[tokio::test]
async fn test_and_operator_partial_match() {
    let rule_yaml = r#"
rule:
  id: high_value_new_user
  name: High Value New User
  when:
    conditions:
      - event.amount > 5000 && event.account_age_days < 30
  score: 60
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_value_new_user
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // High amount but old account - should NOT trigger
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(10000.0));
    event.insert("account_age_days".to_string(), Value::Number(365.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(0);
    response.assert_triggered_rules_count(0);
}

#[tokio::test]
async fn test_or_operator() {
    let rule_yaml = r#"
rule:
  id: suspicious_activity
  name: Suspicious Activity
  when:
    conditions:
      - event.failed_logins > 5 || event.password_reset_count > 3
  score: 45
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - suspicious_activity
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Only one condition true
    let mut event = HashMap::new();
    event.insert("failed_logins".to_string(), Value::Number(10.0));
    event.insert("password_reset_count".to_string(), Value::Number(1.0));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(45);
    response.assert_triggered_rules(&["suspicious_activity"]);
}

// ============================================================================
// Boolean Values
// ============================================================================

#[tokio::test]
async fn test_boolean_true() {
    let rule_yaml = r#"
rule:
  id: verified_user
  name: Verified User
  when:
    conditions:
      - event.is_verified == true
  score: -25
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - verified_user
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("is_verified".to_string(), Value::Bool(true));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(-25);
    response.assert_triggered_rules(&["verified_user"]);
}

#[tokio::test]
async fn test_boolean_false() {
    let rule_yaml = r#"
rule:
  id: unverified_user
  name: Unverified User
  when:
    conditions:
      - event.is_verified == false
  score: 35
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - unverified_user
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("is_verified".to_string(), Value::Bool(false));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(35);
    response.assert_triggered_rules(&["unverified_user"]);
}

// ============================================================================
// Multiple Conditions
// ============================================================================

#[tokio::test]
async fn test_multiple_conditions_all_match() {
    let rule_yaml = r#"
rule:
  id: complex_rule
  name: Complex Rule
  when:
    all:
      - event.amount > 1000
      - event.country == "US"
      - event.is_verified == true
  score: 100
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - complex_rule
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(5000.0));
    event.insert("country".to_string(), Value::String("US".to_string()));
    event.insert("is_verified".to_string(), Value::Bool(true));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(100);
    response.assert_triggered_rules(&["complex_rule"]);
}

#[tokio::test]
async fn test_multiple_conditions_any_match() {
    let rule_yaml = r#"
rule:
  id: any_risk
  name: Any Risk Factor
  when:
    any:
      - event.amount > 10000
      - event.is_new_device == true
      - event.country in ["RU", "CN"]
  score: 50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - any_risk
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    // Only new device is true
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(500.0));
    event.insert("is_new_device".to_string(), Value::Bool(true));
    event.insert("country".to_string(), Value::String("US".to_string()));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(50);
    response.assert_triggered_rules(&["any_risk"]);
}

// ============================================================================
// Multiple Rules
// ============================================================================

#[tokio::test]
async fn test_multiple_rules_accumulate_score() {
    let rule1 = r#"
rule:
  id: high_amount
  name: High Amount
  when:
    conditions:
      - event.amount > 1000
  score: 30
"#;

    let rule2 = r#"
rule:
  id: new_user
  name: New User
  when:
    conditions:
      - event.account_age_days < 30
  score: 20
"#;

    let rule3 = r#"
rule:
  id: new_device
  name: New Device
  when:
    conditions:
      - event.is_new_device == true
  score: 25
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_amount
    - new_user
    - new_device
  conclusion:
    - when: total_score >= 75
      signal: decline
    - when: total_score >= 50
      signal: review
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule1)
        .with_rule(rule2)
        .with_rule(rule3)
        .with_ruleset(ruleset_yaml);

    // All three rules should trigger
    let mut event = HashMap::new();
    event.insert("amount".to_string(), Value::Number(5000.0));
    event.insert("account_age_days".to_string(), Value::Number(5.0));
    event.insert("is_new_device".to_string(), Value::Bool(true));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(75); // 30 + 20 + 25
    response.assert_action(Signal::Decline);
    response.assert_triggered_rules(&["high_amount", "new_user", "new_device"]);
}

// ============================================================================
// Nested Field Access
// ============================================================================

#[tokio::test]
async fn test_nested_field_access() {
    let rule_yaml = r#"
rule:
  id: high_transaction
  name: High Transaction Amount
  when:
    conditions:
      - event.transaction.amount > 10000
  score: 70
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - high_transaction
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut transaction = HashMap::new();
    transaction.insert("amount".to_string(), Value::Number(50000.0));
    transaction.insert("currency".to_string(), Value::String("USD".to_string()));

    let mut event = HashMap::new();
    event.insert("transaction".to_string(), Value::Object(transaction));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(70);
    response.assert_triggered_rules(&["high_transaction"]);
}

#[tokio::test]
async fn test_deeply_nested_field_access() {
    let rule_yaml = r#"
rule:
  id: risky_location
  name: Risky Location
  when:
    conditions:
      - event.user.location.country == "RU"
  score: 60
"#;

    let ruleset_yaml = r#"
ruleset:
  id: test_ruleset
  rules:
    - risky_location
  conclusion:
    - default: true
      signal: approve
"#;

    let engine = TestEngine::new()
        .with_rule(rule_yaml)
        .with_ruleset(ruleset_yaml);

    let mut location = HashMap::new();
    location.insert("country".to_string(), Value::String("RU".to_string()));
    location.insert("city".to_string(), Value::String("Moscow".to_string()));

    let mut user = HashMap::new();
    user.insert("location".to_string(), Value::Object(location));
    user.insert("id".to_string(), Value::String("user123".to_string()));

    let mut event = HashMap::new();
    event.insert("user".to_string(), Value::Object(user));

    let response = engine.execute_ruleset("test_ruleset", event).await;
    response.assert_score(60);
    response.assert_triggered_rules(&["risky_location"]);
}
