//! Comprehensive unit tests for YAML parsers
//!
//! Tests the YAML parsing functionality for rules, rulesets, pipelines, expressions,
//! imports, and registry with focus on achieving 80%+ coverage.

use corint_parser::*;

// =============================================================================
// Rule Parser Tests
// =============================================================================

#[test]
fn test_parse_simple_rule() {
    let yaml = r#"
version: "0.1"

rule:
  id: test_rule
  name: Test Rule
  when:
    conditions:
      - "event.amount > 1000"
  score: 50
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok(), "Failed to parse simple rule: {:?}", result.err());

    let rule = result.unwrap();
    assert_eq!(rule.id, "test_rule");
    assert_eq!(rule.name, "Test Rule");
    assert_eq!(rule.score, 50);
}

#[test]
fn test_parse_rule_with_metadata() {
    let yaml = r#"
version: "0.1"

rule:
  id: metadata_rule
  name: Rule with Metadata
  when:
    conditions:
      - "event.amount > 100"
  score: 25
  metadata:
    category: fraud
    severity: high
    version: "1.0.0"
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());

    let rule = result.unwrap();
    assert_eq!(rule.id, "metadata_rule");
    assert!(rule.metadata.is_some());
}

#[test]
fn test_parse_rule_missing_id() {
    let yaml = r#"
version: "0.1"

rule:
  name: Missing ID Rule
  when:
    conditions:
      - "event.amount > 100"
  score: 50
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_err(), "Should fail without rule ID");
}

#[test]
fn test_parse_rule_with_description() {
    let yaml = r#"
version: "0.1"

rule:
  id: desc_rule
  name: Rule with Description
  description: This rule checks for high-value transactions
  when:
    conditions:
      - "event.amount > 1000"
  score: 50
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());

    let rule = result.unwrap();
    assert_eq!(rule.description, Some("This rule checks for high-value transactions".to_string()));
}

#[test]
fn test_parse_rule_with_params() {
    let yaml = r#"
version: "0.1"

rule:
  id: param_rule
  name: Rule with Parameters
  params:
    threshold: 1000
    country: "US"
  when:
    conditions:
      - "event.amount > params.threshold"
  score: 50
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());

    let rule = result.unwrap();
    assert!(rule.params.is_some());
}

#[test]
fn test_parse_rule_with_all_condition_group() {
    let yaml = r#"
version: "0.1"

rule:
  id: all_rule
  name: Rule with All Conditions
  when:
    all:
      - "event.amount > 1000"
      - "event.country == \"US\""
  score: 75
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok(), "Failed to parse rule with all conditions: {:?}", result.err());

    let rule = result.unwrap();
    assert!(rule.when.condition_group.is_some());
}

#[test]
fn test_parse_rule_with_any_condition_group() {
    let yaml = r#"
version: "0.1"

rule:
  id: any_rule
  name: Rule with Any Conditions
  when:
    any:
      - "event.country == \"US\""
      - "event.country == \"CA\""
  score: 50
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());

    let rule = result.unwrap();
    assert!(rule.when.condition_group.is_some());
}

#[test]
fn test_parse_rule_with_not_condition_group() {
    let yaml = r#"
version: "0.1"

rule:
  id: not_rule
  name: Rule with Not Conditions
  when:
    not:
      - "event.is_verified == true"
  score: 60
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());

    let rule = result.unwrap();
    assert!(rule.when.condition_group.is_some());
}

#[test]
fn test_parse_rule_with_nested_condition_groups() {
    let yaml = r#"
version: "0.1"

rule:
  id: nested_rule
  name: Rule with Nested Conditions
  when:
    all:
      - "event.amount > 100"
      - any:
          - "event.country == \"US\""
          - "event.country == \"CA\""
  score: 80
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok(), "Failed to parse nested condition groups: {:?}", result.err());
}

#[test]
fn test_parse_rule_missing_when_block() {
    let yaml = r#"
version: "0.1"

rule:
  id: no_when
  name: Missing When Block
  score: 50
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_err(), "Should fail without when block");
}

#[test]
fn test_parse_rule_missing_score() {
    let yaml = r#"
version: "0.1"

rule:
  id: no_score
  name: Missing Score
  when:
    conditions:
      - "event.amount > 100"
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_err(), "Should fail without score");
}

// =============================================================================
// Ruleset Parser Tests
// =============================================================================

#[test]
fn test_parse_simple_ruleset() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: test_ruleset
  name: Test Ruleset
  rules:
    - rule1
    - rule2
  decision_logic:
    - condition: total_score >= 100
      action: deny
    - default: true
      action: approve
"#;

    let result = RulesetParser::parse(yaml);
    assert!(result.is_ok(), "Failed to parse ruleset: {:?}", result.err());

    let ruleset = result.unwrap();
    assert_eq!(ruleset.id, "test_ruleset");
    assert_eq!(ruleset.rules.len(), 2);
    assert!(ruleset.decision_logic.len() > 0);
}

#[test]
fn test_parse_ruleset_with_extends() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: child_ruleset
  extends: parent_ruleset
  rules:
    - additional_rule
  decision_logic:
    - condition: total_score >= 50
      action: review
    - default: true
      action: approve
"#;

    let result = RulesetParser::parse(yaml);
    assert!(result.is_ok());

    let ruleset = result.unwrap();
    assert_eq!(ruleset.id, "child_ruleset");
    assert_eq!(ruleset.extends, Some("parent_ruleset".to_string()));
}

#[test]
fn test_parse_ruleset_with_empty_rules() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: empty_ruleset
  rules: []
  decision_logic:
    - default: true
      action: approve
"#;

    let result = RulesetParser::parse(yaml);
    assert!(result.is_ok());

    let ruleset = result.unwrap();
    assert_eq!(ruleset.rules.len(), 0);
}

#[test]
fn test_parse_ruleset_with_description() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: desc_ruleset
  name: Ruleset with Description
  description: This ruleset handles fraud detection
  rules:
    - rule1
  decision_logic:
    - default: true
      action: approve
"#;

    let result = RulesetParser::parse(yaml);
    assert!(result.is_ok());

    let ruleset = result.unwrap();
    assert_eq!(ruleset.description, Some("This ruleset handles fraud detection".to_string()));
}

#[test]
fn test_parse_ruleset_with_all_actions() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: action_ruleset
  rules: []
  decision_logic:
    - condition: score > 300
      action: deny
      reason: Very high risk
    - condition: score > 200
      action: review
      reason: High risk
    - condition: score > 100
      action: challenge
      reason: Medium risk
    - default: true
      action: approve
      reason: Low risk
"#;

    let result = RulesetParser::parse(yaml);
    assert!(result.is_ok());

    let ruleset = result.unwrap();
    assert_eq!(ruleset.decision_logic.len(), 4);
}

#[test]
fn test_parse_ruleset_with_terminate() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: terminate_ruleset
  rules: []
  decision_logic:
    - condition: critical_error == true
      action: deny
      terminate: true
      reason: Critical condition met
"#;

    let result = RulesetParser::parse(yaml);
    assert!(result.is_ok());

    let ruleset = result.unwrap();
    assert!(ruleset.decision_logic[0].terminate);
}

#[test]
fn test_parse_ruleset_missing_id() {
    let yaml = r#"
version: "0.1"

ruleset:
  name: Missing ID
  rules: []
  decision_logic: []
"#;

    let result = RulesetParser::parse(yaml);
    assert!(result.is_err(), "Should fail without ruleset ID");
}

#[test]
fn test_parse_ruleset_with_invalid_action() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: invalid_action_ruleset
  rules: []
  decision_logic:
    - condition: score > 100
      action: invalid_action
"#;

    let result = RulesetParser::parse(yaml);
    assert!(result.is_err(), "Should fail with invalid action");
}

#[test]
fn test_parse_ruleset_with_metadata() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: meta_ruleset
  name: Ruleset with Metadata
  rules: []
  metadata:
    category: fraud
    version: "1.0.0"
  decision_logic:
    - default: true
      action: approve
"#;

    let result = RulesetParser::parse(yaml);
    assert!(result.is_ok());

    let ruleset = result.unwrap();
    assert!(ruleset.metadata.is_some());
}

// =============================================================================
// Pipeline Parser Tests
// =============================================================================

#[test]
fn test_parse_new_format_pipeline() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: first_step
  steps:
    - step:
        id: first_step
        name: First Step
        type: ruleset
        ruleset: fraud_detection
        next: end
"#;

    let result = PipelineParser::parse(yaml);
    assert!(result.is_ok(), "Failed to parse pipeline: {:?}", result.err());

    let pipeline = result.unwrap();
    assert_eq!(pipeline.id, "test_pipeline");
    assert_eq!(pipeline.entry, "first_step");
    assert_eq!(pipeline.steps.len(), 1);
}

#[test]
fn test_parse_pipeline_with_router_step() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: router_pipeline
  name: Router Pipeline
  entry: router_step
  steps:
    - step:
        id: router_step
        name: Route Based on Score
        type: router
        routes:
          - when: result.action == "deny"
            next: deny_step
          - when: result.action == "review"
            next: review_step
        default: approve_step

    - step:
        id: deny_step
        name: Deny Action
        type: ruleset
        ruleset: deny_handler

    - step:
        id: review_step
        name: Review Action
        type: ruleset
        ruleset: review_handler

    - step:
        id: approve_step
        name: Approve Action
        type: ruleset
        ruleset: approve_handler
"#;

    let result = PipelineParser::parse(yaml);
    assert!(result.is_ok(), "Failed to parse router pipeline: {:?}", result.err());

    let pipeline = result.unwrap();
    assert_eq!(pipeline.steps.len(), 4);
    assert_eq!(pipeline.steps[0].step_type, "router");
}

#[test]
fn test_parse_pipeline_with_api_step() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: api_pipeline
  name: API Pipeline
  entry: api_step
  steps:
    - step:
        id: api_step
        name: Call Geolocation API
        type: api
        api: geo_service
        endpoint: /lookup
        params:
          ip: event.ip_address
        output: api.geo
        timeout: 5000
"#;

    let result = PipelineParser::parse(yaml);
    assert!(result.is_ok());

    let pipeline = result.unwrap();
    assert_eq!(pipeline.steps[0].step_type, "api");
}

#[test]
fn test_parse_pipeline_with_service_step() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: service_pipeline
  name: Service Pipeline
  entry: service_step
  steps:
    - step:
        id: service_step
        name: Query Database
        type: service
        service: postgres
        query: SELECT * FROM users WHERE id = ?
        params:
          user_id: event.user_id
"#;

    let result = PipelineParser::parse(yaml);
    assert!(result.is_ok(), "Failed to parse service pipeline: {:?}", result.err());

    let pipeline = result.unwrap();
    assert_eq!(pipeline.steps[0].step_type, "service");
}

#[test]
fn test_parse_pipeline_with_when_condition() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: conditional_pipeline
  name: Conditional Pipeline
  when:
    event.type: transaction
  entry: first_step
  steps:
    - step:
        id: first_step
        name: First Step
        type: ruleset
        ruleset: transaction_checks
"#;

    let result = PipelineParser::parse(yaml);
    assert!(result.is_ok());

    let pipeline = result.unwrap();
    assert!(pipeline.when.is_some());
}

#[test]
fn test_parse_pipeline_with_metadata() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: meta_pipeline
  name: Pipeline with Metadata
  entry: first_step
  metadata:
    version: "2.0"
    author: "Test Team"
  steps:
    - step:
        id: first_step
        name: First Step
        type: ruleset
        ruleset: checks
"#;

    let result = PipelineParser::parse(yaml);
    assert!(result.is_ok());

    let pipeline = result.unwrap();
    assert!(pipeline.metadata.is_some());
}

#[test]
fn test_parse_pipeline_missing_entry() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: no_entry_pipeline
  name: Missing Entry
  steps:
    - step:
        id: first_step
        name: First Step
        type: ruleset
        ruleset: checks
"#;

    let result = PipelineParser::parse(yaml);
    // Should fail with new format missing entry
    assert!(result.is_err());
}

#[test]
fn test_parse_pipeline_missing_steps() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: no_steps_pipeline
  name: Missing Steps
  entry: first_step
"#;

    let result = PipelineParser::parse(yaml);
    assert!(result.is_err(), "Should fail without steps");
}

#[test]
fn test_parse_pipeline_with_description() {
    let yaml = r#"
version: "0.1"

pipeline:
  id: desc_pipeline
  name: Pipeline with Description
  description: This pipeline handles payment processing
  entry: first_step
  steps:
    - step:
        id: first_step
        name: First Step
        type: ruleset
        ruleset: payment_checks
"#;

    let result = PipelineParser::parse(yaml);
    assert!(result.is_ok());

    let pipeline = result.unwrap();
    assert_eq!(pipeline.description, Some("This pipeline handles payment processing".to_string()));
}

// =============================================================================
// Expression Parser Tests
// =============================================================================

#[test]
fn test_parse_simple_comparison() {
    let expr = "event.amount > 1000";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok(), "Failed to parse comparison: {:?}", result.err());
}

#[test]
fn test_parse_complex_logical_expression() {
    let expr = "(event.amount > 1000) && (event.country == \"US\")";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok(), "Failed to parse complex expression: {:?}", result.err());
}

#[test]
fn test_parse_field_access() {
    let expr = "event.user.profile.age";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_function_call() {
    let expr = "len(event.items)";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_function_with_multiple_args() {
    let expr = "count(transactions, user.id, last_7d)";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_in_operator() {
    let expr = "event.country in [\"US\", \"CA\", \"GB\"]";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_not_in_operator() {
    let expr = "event.country not in [\"CN\", \"RU\"]";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_contains_operator() {
    let expr = "event.email contains \"@gmail.com\"";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_starts_with_operator() {
    let expr = "event.url starts_with \"https://\"";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_ends_with_operator() {
    let expr = "event.file ends_with \".pdf\"";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_arithmetic_expression() {
    let test_cases = vec![
        "event.amount + 100",
        "event.total - event.discount",
        "event.price * event.quantity",
        "event.total / event.count",
        "event.value % 10",
    ];

    for expr in test_cases {
        let result = ExpressionParser::parse(expr);
        assert!(result.is_ok(), "Failed to parse arithmetic: {}", expr);
    }
}

#[test]
fn test_parse_unary_operators() {
    let test_cases = vec![
        "!event.is_verified",
        "-event.amount",
    ];

    for expr in test_cases {
        let result = ExpressionParser::parse(expr);
        assert!(result.is_ok(), "Failed to parse unary: {}", expr);
    }
}

#[test]
fn test_parse_boolean_literals() {
    assert!(ExpressionParser::parse("true").is_ok());
    assert!(ExpressionParser::parse("false").is_ok());
}

#[test]
fn test_parse_null_literal() {
    assert!(ExpressionParser::parse("null").is_ok());
}

#[test]
fn test_parse_number_literals() {
    let test_cases = vec!["42", "3.14", "-10", "0"];

    for expr in test_cases {
        let result = ExpressionParser::parse(expr);
        assert!(result.is_ok(), "Failed to parse number: {}", expr);
    }
}

#[test]
fn test_parse_string_literal() {
    let expr = "\"hello world\"";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_array_literal() {
    let expr = "[\"US\", \"CA\", \"GB\"]";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_empty_expression() {
    let result = ExpressionParser::parse("");
    assert!(result.is_err(), "Should fail on empty expression");
}

#[test]
fn test_parse_nested_parentheses() {
    let expr = "((event.amount > 100) && (event.country == \"US\")) || (event.vip == true)";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_result_access() {
    let expr = "result.action";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_result_with_ruleset_id() {
    let expr = "result.fraud_check.score";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

// =============================================================================
// Import Parser Tests
// =============================================================================

#[test]
fn test_parse_imports() {
    let yaml = r#"
version: "0.1"

imports:
  rules:
    - library/rules/fraud/velocity_check.yaml
    - library/rules/fraud/geo_check.yaml
  rulesets:
    - library/rulesets/fraud_detection.yaml
"#;

    let yaml_value = corint_parser::yaml_parser::YamlParser::parse(yaml).unwrap();
    let result = ImportParser::parse_from_yaml(&yaml_value);
    assert!(result.is_ok(), "Failed to parse imports: {:?}", result.err());

    let imports_opt = result.unwrap();
    assert!(imports_opt.is_some());

    let imports = imports_opt.unwrap();
    assert_eq!(imports.rules.len(), 2);
    assert_eq!(imports.rulesets.len(), 1);
}

#[test]
fn test_parse_no_imports() {
    let yaml = r#"
version: "0.1"

rule:
  id: test_rule
  name: Test
  when:
    conditions: []
  score: 50
"#;

    let yaml_value = corint_parser::yaml_parser::YamlParser::parse(yaml).unwrap();
    let result = ImportParser::parse_from_yaml(&yaml_value);
    assert!(result.is_ok());

    let imports_opt = result.unwrap();
    assert!(imports_opt.is_none());
}

#[test]
fn test_parse_imports_with_pipelines() {
    let yaml = r#"
version: "0.1"

imports:
  rules:
    - library/rules/rule1.yaml
  rulesets:
    - library/rulesets/ruleset1.yaml
  pipelines:
    - library/pipelines/pipeline1.yaml
"#;

    let yaml_value = corint_parser::yaml_parser::YamlParser::parse(yaml).unwrap();
    let result = ImportParser::parse_from_yaml(&yaml_value);
    assert!(result.is_ok());

    let imports = result.unwrap().unwrap();
    assert_eq!(imports.rules.len(), 1);
    assert_eq!(imports.rulesets.len(), 1);
    assert_eq!(imports.pipelines.len(), 1);
}

// =============================================================================
// Registry Parser Tests
// =============================================================================

#[test]
fn test_parse_registry() {
    let yaml = r#"
version: "0.1"

registry:
  - pipeline: payment_pipeline
    when:
      event.type: payment

  - pipeline: login_pipeline
    when:
      event.type: login
"#;

    let result = RegistryParser::parse(yaml);
    assert!(result.is_ok(), "Failed to parse registry: {:?}", result.err());

    let registry = result.unwrap();
    assert_eq!(registry.registry.len(), 2);
}

#[test]
fn test_parse_registry_with_conditions() {
    let yaml = r#"
version: "0.1"

registry:
  - pipeline: high_value_pipeline
    when:
      all:
        - "event.type == \"transaction\""
        - "event.amount > 10000"

  - pipeline: default_pipeline
    when:
      event.type: transaction
"#;

    let result = RegistryParser::parse(yaml);
    assert!(result.is_ok());

    let registry = result.unwrap();
    assert_eq!(registry.registry.len(), 2);
}

#[test]
fn test_parse_registry_missing_when() {
    let yaml = r#"
version: "0.1"

registry:
  - pipeline: test_pipeline
"#;

    let result = RegistryParser::parse(yaml);
    assert!(result.is_err(), "Should fail without when block");
}

#[test]
fn test_parse_registry_missing_pipeline() {
    let yaml = r#"
version: "0.1"

registry:
  - when:
      event.type: test
"#;

    let result = RegistryParser::parse(yaml);
    assert!(result.is_err(), "Should fail without pipeline ID");
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn test_parse_invalid_yaml() {
    let yaml = r#"
this is not: valid: yaml: syntax:
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_err());
}

#[test]
fn test_parse_empty_document() {
    let yaml = "";

    let result = RuleParser::parse(yaml);
    assert!(result.is_err());
}

#[test]
fn test_parse_incomplete_yaml() {
    let yaml = r#"
rule:
  id: incomplete
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_err(), "Should fail with incomplete YAML");
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_parse_rule_with_special_characters_in_id() {
    let yaml = r#"
version: "0.1"

rule:
  id: rule_with_underscores_123
  name: Special ID Rule
  when:
    conditions:
      - "event.amount > 100"
  score: 50
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_parse_rule_with_unicode() {
    let yaml = r#"
version: "0.1"

rule:
  id: unicode_rule
  name: "规则名称 - Rule with Chinese"
  when:
    conditions:
      - "event.amount > 100"
  score: 50
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_parse_multiline_conditions() {
    let yaml = r#"
version: "0.1"

rule:
  id: multiline_rule
  name: Multiline Conditions
  when:
    conditions:
      - "event.amount > 1000"
      - "event.country == \"US\""
      - "event.user.is_verified == true"
  score: 75
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());

    let rule = result.unwrap();
    assert!(rule.when.conditions.is_some());
    assert_eq!(rule.when.conditions.unwrap().len(), 3);
}

#[test]
fn test_parse_deeply_nested_expressions() {
    let expr = "((((event.a + event.b) * event.c) - event.d) / event.e) > 100";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_expression_with_whitespace() {
    let expr = "  event.amount   >   1000  ";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_large_array_literal() {
    let expr = "[\"A\", \"B\", \"C\", \"D\", \"E\", \"F\", \"G\", \"H\", \"I\", \"J\"]";
    let result = ExpressionParser::parse(expr);
    assert!(result.is_ok());
}

#[test]
fn test_parse_zero_score_rule() {
    let yaml = r#"
version: "0.1"

rule:
  id: zero_score
  name: Zero Score Rule
  when:
    conditions:
      - "event.test == true"
  score: 0
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());

    let rule = result.unwrap();
    assert_eq!(rule.score, 0);
}

#[test]
fn test_parse_negative_score_rule() {
    let yaml = r#"
version: "0.1"

rule:
  id: negative_score
  name: Negative Score Rule
  when:
    conditions:
      - "event.good_signal == true"
  score: -50
"#;

    let result = RuleParser::parse(yaml);
    assert!(result.is_ok());

    let rule = result.unwrap();
    assert_eq!(rule.score, -50);
}

// =============================================================================
// YAML Parser Utility Tests
// =============================================================================

#[test]
fn test_yaml_parser_multi_document() {
    let yaml = r#"
version: "0.1"
imports:
  rules:
    - library/test.yaml
---
rule:
  id: test_rule
  name: Test
  when:
    conditions:
      - "event.amount > 100"
  score: 50
"#;

    let docs = corint_parser::yaml_parser::YamlParser::parse_multi_document(yaml).unwrap();
    assert_eq!(docs.len(), 2);
}

#[test]
fn test_yaml_parser_get_string() {
    let yaml = r#"
name: "Test Name"
value: 42
"#;

    let yaml_value = corint_parser::yaml_parser::YamlParser::parse(yaml).unwrap();
    let name = corint_parser::yaml_parser::YamlParser::get_string(&yaml_value, "name");
    assert!(name.is_ok());
    assert_eq!(name.unwrap(), "Test Name");
}

#[test]
fn test_yaml_parser_get_optional_string() {
    let yaml = r#"
name: "Test"
"#;

    let yaml_value = corint_parser::yaml_parser::YamlParser::parse(yaml).unwrap();
    let name = corint_parser::yaml_parser::YamlParser::get_optional_string(&yaml_value, "name");
    assert_eq!(name, Some("Test".to_string()));

    let missing = corint_parser::yaml_parser::YamlParser::get_optional_string(&yaml_value, "missing");
    assert_eq!(missing, None);
}

#[test]
fn test_yaml_parser_has_field() {
    let yaml = r#"
name: "Test"
value: 42
"#;

    let yaml_value = corint_parser::yaml_parser::YamlParser::parse(yaml).unwrap();
    assert!(corint_parser::yaml_parser::YamlParser::has_field(&yaml_value, "name"));
    assert!(corint_parser::yaml_parser::YamlParser::has_field(&yaml_value, "value"));
    assert!(!corint_parser::yaml_parser::YamlParser::has_field(&yaml_value, "missing"));
}
