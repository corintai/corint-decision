mod common;

use common::{ResponseAssertions, TestEngine};
use corint_core::ast::Signal;
use corint_core::Value;
use std::collections::HashMap;

fn build_engine() -> TestEngine {
    let high_credit_rule = r#"
rule:
  id: high_credit_amount_with_long_duration_applicants_requesting_credit_8000_with_durati_486fbad6b03f
  name: high credit
  when: event.credit_amount > 8000
  score: 85
"#;

    let low_reserves_rule = r#"
rule:
  id: low_financial_reserves_applicants_with_little_or_no_savings_and_little_checking_bal_6e59b7f65e13
  name: low reserves
  when:
    all:
      - event.saving_accounts in ["little", "NA"]
      - event.checking_account == "little"
  score: 70
"#;

    let young_rule = r#"
rule:
  id: young_applicant_with_high_credit_request_applicants_under_25_years_requesting_credi_faf731a21b21
  name: young applicant
  when:
    all:
      - event.age < 25
      - event.credit_amount > 3000
  score: 65
"#;

    let very_long_rule = r#"
rule:
  id: very_long_loan_duration_credit_terms_exceeding_36_months_have_higher_default_probab_660348d19f4a
  name: very long duration
  when: event.duration > 36
  score: 60
"#;

    let stable_rule = r#"
rule:
  id: stable_applicant_profile_applicants_with_own_housing_and_rich_savings_demonstrate_s_f12ddba82678
  name: stable profile
  when:
    all:
      - event.housing == "own"
      - event.saving_accounts == "rich"
  score: -50
"#;

    let ruleset_yaml = r#"
ruleset:
  id: german_credit_ruleset
  rules:
    - high_credit_amount_with_long_duration_applicants_requesting_credit_8000_with_durati_486fbad6b03f
    - low_financial_reserves_applicants_with_little_or_no_savings_and_little_checking_bal_6e59b7f65e13
    - young_applicant_with_high_credit_request_applicants_under_25_years_requesting_credi_faf731a21b21
    - very_long_loan_duration_credit_terms_exceeding_36_months_have_higher_default_probab_660348d19f4a
    - stable_applicant_profile_applicants_with_own_housing_and_rich_savings_demonstrate_s_f12ddba82678
  conclusion:
    - when: total_score >= 100
      signal: decline
    - when: total_score >= 50
      signal: review
    - default: true
      signal: approve
"#;

    TestEngine::new()
        .with_rule(high_credit_rule)
        .with_rule(low_reserves_rule)
        .with_rule(young_rule)
        .with_rule(very_long_rule)
        .with_rule(stable_rule)
        .with_ruleset(ruleset_yaml)
}

fn event(fields: &[(&str, Value)]) -> HashMap<String, Value> {
    fields
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

#[tokio::test]
async fn german_credit_neutral_profile_should_not_trigger_very_long_duration() {
    let engine = build_engine();

    let response = engine
        .execute_ruleset(
            "german_credit_ruleset",
            event(&[
                ("age", Value::Number(33.0)),
                ("credit_amount", Value::Number(2800.0)),
                ("duration", Value::Number(12.0)),
                ("housing", Value::String("rent".to_string())),
                ("saving_accounts", Value::String("moderate".to_string())),
                ("checking_account", Value::String("moderate".to_string())),
            ]),
        )
        .await;

    response.assert_action(Signal::Approve);
    response.assert_score(0);
    response.assert_triggered_rules_count(0);
}

#[tokio::test]
async fn german_credit_very_long_duration_only_triggers_above_36() {
    let engine = build_engine();

    let response_36 = engine
        .execute_ruleset(
            "german_credit_ruleset",
            event(&[
                ("age", Value::Number(33.0)),
                ("credit_amount", Value::Number(2800.0)),
                ("duration", Value::Number(36.0)),
                ("housing", Value::String("rent".to_string())),
                ("saving_accounts", Value::String("moderate".to_string())),
                ("checking_account", Value::String("moderate".to_string())),
            ]),
        )
        .await;
    response_36.assert_action(Signal::Approve);
    response_36.assert_triggered_rules_count(0);

    let response_37 = engine
        .execute_ruleset(
            "german_credit_ruleset",
            event(&[
                ("age", Value::Number(33.0)),
                ("credit_amount", Value::Number(2800.0)),
                ("duration", Value::Number(37.0)),
                ("housing", Value::String("rent".to_string())),
                ("saving_accounts", Value::String("moderate".to_string())),
                ("checking_account", Value::String("moderate".to_string())),
            ]),
        )
        .await;
    response_37.assert_action(Signal::Review);
    response_37.assert_triggered_rules(&[
        "very_long_loan_duration_credit_terms_exceeding_36_months_have_higher_default_probab_660348d19f4a",
    ]);
}
