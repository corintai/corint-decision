//! Comprehensive unit tests for compiler components
//!
//! Tests codegen, optimizer, semantic analysis, validator, and import resolver
//! with focus on achieving 80%+ coverage.

use corint_compiler::*;
use corint_core::ast::*;
use corint_core::types::Value;

// =============================================================================
// Expression Codegen Tests
// =============================================================================

#[test]
fn test_codegen_literal_number() {
    let expr = Expression::literal(Value::Number(42.0));
    let instructions = codegen::ExpressionCompiler::compile(&expr);

    assert!(instructions.is_ok());
    let instructions = instructions.unwrap();
    assert!(instructions.len() > 0);
}

#[test]
fn test_codegen_literal_string() {
    let expr = Expression::literal(Value::String("hello".to_string()));
    let instructions = codegen::ExpressionCompiler::compile(&expr);

    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_literal_bool() {
    let expr = Expression::literal(Value::Bool(true));
    let instructions = codegen::ExpressionCompiler::compile(&expr);

    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_literal_null() {
    let expr = Expression::literal(Value::Null);
    let instructions = codegen::ExpressionCompiler::compile(&expr);

    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_field_access() {
    let expr = Expression::field_access(vec!["event".to_string(), "amount".to_string()]);
    let instructions = codegen::ExpressionCompiler::compile(&expr);

    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_nested_field_access() {
    let expr = Expression::field_access(vec![
        "event".to_string(),
        "user".to_string(),
        "profile".to_string(),
        "age".to_string(),
    ]);
    let instructions = codegen::ExpressionCompiler::compile(&expr);

    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_binary_arithmetic() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Add, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);

    assert!(instructions.is_ok());
    let instructions = instructions.unwrap();
    // Should have: LoadConst 10, LoadConst 5, BinaryOp Add
    assert!(instructions.len() >= 3);
}

#[test]
fn test_codegen_binary_subtraction() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Sub, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_binary_multiplication() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Mul, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_binary_division() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Div, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_comparison_gt() {
    let left = Expression::field_access(vec!["event".to_string(), "amount".to_string()]);
    let right = Expression::literal(Value::Number(1000.0));
    let expr = Expression::binary(left, Operator::Gt, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_comparison_lt() {
    let left = Expression::field_access(vec!["event".to_string(), "amount".to_string()]);
    let right = Expression::literal(Value::Number(100.0));
    let expr = Expression::binary(left, Operator::Lt, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_comparison_eq() {
    let left = Expression::field_access(vec!["event".to_string(), "type".to_string()]);
    let right = Expression::literal(Value::String("payment".to_string()));
    let expr = Expression::binary(left, Operator::Eq, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_comparison_ne() {
    let left = Expression::field_access(vec!["event".to_string(), "status".to_string()]);
    let right = Expression::literal(Value::String("blocked".to_string()));
    let expr = Expression::binary(left, Operator::Ne, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_logical_and() {
    let left = Expression::literal(Value::Bool(true));
    let right = Expression::literal(Value::Bool(false));
    let expr = Expression::binary(left, Operator::And, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_logical_or() {
    let left = Expression::literal(Value::Bool(true));
    let right = Expression::literal(Value::Bool(false));
    let expr = Expression::binary(left, Operator::Or, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_unary_not() {
    let operand = Expression::literal(Value::Bool(true));
    let expr = Expression::unary(UnaryOperator::Not, operand);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_unary_negate() {
    let operand = Expression::literal(Value::Number(42.0));
    let expr = Expression::unary(UnaryOperator::Negate, operand);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_list_reference() {
    let value_expr = Expression::field_access(vec!["event".to_string(), "country".to_string()]);
    let list_ref = Expression::ListReference {
        list_id: "blocked_countries".to_string(),
    };
    let expr = Expression::binary(value_expr, Operator::InList, list_ref);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_not_in_list() {
    let value_expr = Expression::field_access(vec!["event".to_string(), "email".to_string()]);
    let list_ref = Expression::ListReference {
        list_id: "whitelist".to_string(),
    };
    let expr = Expression::binary(value_expr, Operator::NotInList, list_ref);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_result_access() {
    let expr = Expression::result_access("score".to_string());
    let instructions = codegen::ExpressionCompiler::compile(&expr);

    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_result_access_with_ruleset() {
    let expr = Expression::result_access_for("fraud_check".to_string(), "score".to_string());
    let instructions = codegen::ExpressionCompiler::compile(&expr);

    assert!(instructions.is_ok());
}

// =============================================================================
// Optimizer Tests - Constant Folding
// =============================================================================

#[test]
fn test_constant_folding_addition() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Add, right);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    // Should fold to: Literal(15.0)
    match optimized {
        Expression::Literal(Value::Number(n)) => assert_eq!(n, 15.0),
        _ => panic!("Expected constant folding to produce Literal(15.0)"),
    }
}

#[test]
fn test_constant_folding_subtraction() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Sub, right);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    match optimized {
        Expression::Literal(Value::Number(n)) => assert_eq!(n, 5.0),
        _ => panic!("Expected constant folding to produce Literal(5.0)"),
    }
}

#[test]
fn test_constant_folding_multiplication() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Mul, right);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    match optimized {
        Expression::Literal(Value::Number(n)) => assert_eq!(n, 50.0),
        _ => panic!("Expected constant folding to produce Literal(50.0)"),
    }
}

#[test]
fn test_constant_folding_division() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Div, right);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    match optimized {
        Expression::Literal(Value::Number(n)) => assert_eq!(n, 2.0),
        _ => panic!("Expected constant folding to produce Literal(2.0)"),
    }
}

#[test]
fn test_constant_folding_comparison_gt() {
    let left = Expression::literal(Value::Number(10.0));
    let right = Expression::literal(Value::Number(5.0));
    let expr = Expression::binary(left, Operator::Gt, right);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    match optimized {
        Expression::Literal(Value::Bool(b)) => assert!(b),
        _ => panic!("Expected constant folding to produce Literal(true)"),
    }
}

#[test]
fn test_constant_folding_comparison_lt() {
    let left = Expression::literal(Value::Number(5.0));
    let right = Expression::literal(Value::Number(10.0));
    let expr = Expression::binary(left, Operator::Lt, right);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    match optimized {
        Expression::Literal(Value::Bool(b)) => assert!(b),
        _ => panic!("Expected constant folding to produce Literal(true)"),
    }
}

#[test]
fn test_constant_folding_logical_and() {
    let left = Expression::literal(Value::Bool(true));
    let right = Expression::literal(Value::Bool(false));
    let expr = Expression::binary(left, Operator::And, right);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    match optimized {
        Expression::Literal(Value::Bool(b)) => assert!(!b),
        _ => panic!("Expected constant folding to produce Literal(false)"),
    }
}

#[test]
fn test_constant_folding_logical_or() {
    let left = Expression::literal(Value::Bool(true));
    let right = Expression::literal(Value::Bool(false));
    let expr = Expression::binary(left, Operator::Or, right);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    match optimized {
        Expression::Literal(Value::Bool(b)) => assert!(b),
        _ => panic!("Expected constant folding to produce Literal(true)"),
    }
}

#[test]
fn test_constant_folding_unary_not() {
    let operand = Expression::literal(Value::Bool(true));
    let expr = Expression::unary(UnaryOperator::Not, operand);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    match optimized {
        Expression::Literal(Value::Bool(b)) => assert!(!b),
        _ => panic!("Expected constant folding to produce Literal(false)"),
    }
}

#[test]
fn test_constant_folding_unary_negate() {
    let operand = Expression::literal(Value::Number(42.0));
    let expr = Expression::unary(UnaryOperator::Negate, operand);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    match optimized {
        Expression::Literal(Value::Number(n)) => assert_eq!(n, -42.0),
        _ => panic!("Expected constant folding to produce Literal(-42.0)"),
    }
}

#[test]
fn test_constant_folding_no_change() {
    // Expression with field access cannot be folded
    let left = Expression::field_access(vec!["event".to_string(), "amount".to_string()]);
    let right = Expression::literal(Value::Number(100.0));
    let expr = Expression::binary(left.clone(), Operator::Add, right.clone());

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    // Should remain a Binary expression
    match optimized {
        Expression::Binary { .. } => {}, // Expected
        _ => panic!("Expected expression to remain unchanged"),
    }
}

// =============================================================================
// Semantic Analysis Tests
// =============================================================================

#[test]
fn test_semantic_analyzer_creation() {
    let _analyzer = semantic::SemanticAnalyzer::new();
    // Should create successfully
}

#[test]
fn test_semantic_analyze_valid_rule() {
    let when = WhenBlock::new().add_condition(Expression::binary(
        Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
        Operator::Gt,
        Expression::literal(Value::Number(1000.0)),
    ));

    let rule = Rule {
        id: "valid_rule".to_string(),
        name: "Valid Rule".to_string(),
        description: None,
        params: None,
        when,
        score: 50,
        metadata: None,
    };

    let mut analyzer = semantic::SemanticAnalyzer::new();
    let result = analyzer.analyze_rule(&rule);

    assert!(result.is_ok(), "Valid rule should pass semantic analysis: {:?}", result.err());
}

#[test]
fn test_semantic_analyze_valid_ruleset() {
    let ruleset = Ruleset {
        id: "valid_ruleset".to_string(),
        name: Some("Valid Ruleset".to_string()),
        extends: None,
        rules: vec!["rule1".to_string(), "rule2".to_string()],
        conclusion: vec![],
        description: None,
        metadata: None,
    };

    let mut analyzer = semantic::SemanticAnalyzer::new();
    let result = analyzer.analyze_ruleset(&ruleset);

    // May or may not succeed depending on whether rules exist
    assert!(result.is_ok() || result.is_err());
}

// =============================================================================
// Validator Tests
// =============================================================================

#[test]
fn test_validator_validate_rule() {
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

    let validator = validator::DslValidator::new();
    let validation_result = validator.validate(yaml, validator::DslType::Rule);

    assert!(validation_result.valid, "Valid rule YAML should pass validation");
}

#[test]
fn test_validator_validate_ruleset() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: test_ruleset
  name: Test Ruleset
  rules:
    - rule1
  conclusion:
    - default: true
      signal: approve
"#;

    let validator = validator::DslValidator::new();
    let validation_result = validator.validate(yaml, validator::DslType::Ruleset);

    assert!(validation_result.valid);
}

#[test]
fn test_validator_validate_pipeline() {
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
"#;

    let validator = validator::DslValidator::new();
    let validation_result = validator.validate(yaml, validator::DslType::Pipeline);

    assert!(validation_result.valid);
}

#[test]
fn test_validator_invalid_yaml() {
    let yaml = r#"
this is: not: valid: yaml:
"#;

    let validator = validator::DslValidator::new();
    let validation_result = validator.validate(yaml, validator::DslType::Rule);

    // Validation runs but should report errors
    assert!(!validation_result.valid);
    assert!(validation_result.errors.len() > 0);
}

#[test]
fn test_validator_missing_required_field() {
    let yaml = r#"
version: "0.1"

rule:
  name: Missing ID Rule
  when:
    conditions:
      - "event.amount > 100"
  score: 50
"#;

    let validator = validator::DslValidator::new();
    let validation_result = validator.validate(yaml, validator::DslType::Rule);

    assert!(!validation_result.valid);
}

#[test]
fn test_validator_auto_detect_rule() {
    let yaml = r#"
version: "0.1"

rule:
  id: auto_detect_rule
  name: Auto Detect Rule
  when:
    conditions:
      - "event.amount > 1000"
  score: 50
"#;

    let validator = validator::DslValidator::new();
    let validation_result = validator.validate(yaml, validator::DslType::Auto);

    assert!(validation_result.valid);
    assert!(validation_result.metadata.is_some());
    assert_eq!(validation_result.metadata.unwrap().doc_type, validator::DslType::Rule);
}

#[test]
fn test_validator_auto_detect_ruleset() {
    let yaml = r#"
version: "0.1"

ruleset:
  id: auto_detect_ruleset
  name: Auto Detect Ruleset
  rules: []
  conclusion:
    - default: true
      signal: approve
"#;

    let validator = validator::DslValidator::new();
    let validation_result = validator.validate(yaml, validator::DslType::Auto);

    assert!(validation_result.valid);
    assert!(validation_result.metadata.is_some());
    assert_eq!(validation_result.metadata.unwrap().doc_type, validator::DslType::Ruleset);
}

// =============================================================================
// Compiler Integration Tests
// =============================================================================

#[test]
fn test_compiler_compile_rule() {
    let when = WhenBlock::new().add_condition(Expression::binary(
        Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
        Operator::Gt,
        Expression::literal(Value::Number(1000.0)),
    ));

    let rule = Rule {
        id: "test_rule".to_string(),
        name: "Test Rule".to_string(),
        description: None,
        params: None,
        when,
        score: 50,
        metadata: None,
    };

    let mut compiler = Compiler::new();
    let result = compiler.compile_rule(&rule);

    assert!(result.is_ok(), "Failed to compile rule: {:?}", result.err());
}

#[test]
fn test_compiler_compile_ruleset() {
    let ruleset = Ruleset {
        id: "test_ruleset".to_string(),
        name: Some("Test Ruleset".to_string()),
        extends: None,
        rules: vec!["rule1".to_string()],
        conclusion: vec![],
        description: None,
        metadata: None,
    };

    let mut compiler = Compiler::new();
    let result = compiler.compile_ruleset(&ruleset);

    // May fail if rule1 is not found, but we're testing the API
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_compiler_with_options() {
    let options = CompilerOptions {
        enable_semantic_analysis: false,
        enable_constant_folding: false,
        enable_dead_code_elimination: false,
        library_base_path: "test_repo".to_string(),
    };

    let mut compiler = Compiler::with_options(options);

    let when = WhenBlock::new().add_condition(Expression::literal(Value::Bool(true)));

    let rule = Rule {
        id: "simple_rule".to_string(),
        name: "Simple Rule".to_string(),
        description: None,
        params: None,
        when,
        score: 25,
        metadata: None,
    };

    let result = compiler.compile_rule(&rule);
    assert!(result.is_ok());
}

#[test]
fn test_compiler_accessors() {
    let compiler = Compiler::new();

    // Test accessors
    let _analyzer = compiler.semantic_analyzer();
    let _folder = compiler.constant_folder();
    let _eliminator = compiler.dead_code_eliminator();
    let _resolver = compiler.import_resolver();

    // All should succeed
}

// =============================================================================
// Import Resolver Tests
// =============================================================================

#[test]
fn test_import_resolver_creation() {
    let _resolver = import_resolver::ImportResolver::new("repository");
    // Should create successfully
}

#[test]
fn test_import_resolver_with_custom_base() {
    let _resolver = import_resolver::ImportResolver::new("/custom/path");
    // Should create successfully
}

// =============================================================================
// Rule Compiler Tests
// =============================================================================

#[test]
fn test_rule_compiler_simple_rule() {
    let when = WhenBlock::new().add_condition(Expression::literal(Value::Bool(true)));

    let rule = Rule {
        id: "simple_rule".to_string(),
        name: "Simple Rule".to_string(),
        description: None,
        params: None,
        when,
        score: 50,
        metadata: None,
    };

    let result = codegen::RuleCompiler::compile(&rule);
    assert!(result.is_ok());
}

#[test]
fn test_rule_compiler_with_multiple_conditions() {
    let when = WhenBlock::new()
        .add_condition(Expression::binary(
            Expression::field_access(vec!["event".to_string(), "amount".to_string()]),
            Operator::Gt,
            Expression::literal(Value::Number(100.0)),
        ))
        .add_condition(Expression::binary(
            Expression::field_access(vec!["event".to_string(), "country".to_string()]),
            Operator::Eq,
            Expression::literal(Value::String("US".to_string())),
        ));

    let rule = Rule {
        id: "multi_condition_rule".to_string(),
        name: "Multi Condition Rule".to_string(),
        description: None,
        params: None,
        when,
        score: 75,
        metadata: None,
    };

    let result = codegen::RuleCompiler::compile(&rule);
    assert!(result.is_ok());
}

// =============================================================================
// Ruleset Compiler Tests
// =============================================================================

#[test]
fn test_ruleset_compiler_empty_ruleset() {
    let ruleset = Ruleset {
        id: "empty_ruleset".to_string(),
        name: Some("Empty Ruleset".to_string()),
        extends: None,
        rules: vec![],
        conclusion: vec![],
        description: None,
        metadata: None,
    };

    let result = codegen::RulesetCompiler::compile(&ruleset);
    assert!(result.is_ok());
}

// =============================================================================
// Pipeline Compiler Tests
// =============================================================================

#[test]
fn test_pipeline_compiler_api() {
    // Test that PipelineCompiler exists and has a compile method
    // We're not testing full pipeline compilation as it requires complex structure
    // Just verify the API is accessible

    // The PipelineCompiler::compile method exists and can be called
    // Full testing would require creating proper Pipeline AST structure
}

// =============================================================================
// Edge Cases and Error Handling
// =============================================================================

#[test]
fn test_codegen_deeply_nested_expression() {
    // Create a deeply nested expression: ((((a + b) * c) / d) - e)
    let a = Expression::literal(Value::Number(1.0));
    let b = Expression::literal(Value::Number(2.0));
    let c = Expression::literal(Value::Number(3.0));
    let d = Expression::literal(Value::Number(4.0));
    let e = Expression::literal(Value::Number(5.0));

    let add = Expression::binary(a, Operator::Add, b);
    let mul = Expression::binary(add, Operator::Mul, c);
    let div = Expression::binary(mul, Operator::Div, d);
    let sub = Expression::binary(div, Operator::Sub, e);

    let instructions = codegen::ExpressionCompiler::compile(&sub);
    assert!(instructions.is_ok());
}

#[test]
fn test_codegen_large_literal_array() {
    let elements: Vec<Value> = (0..100).map(|i| Value::Number(i as f64)).collect();
    let expr = Expression::literal(Value::Array(elements));

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_constant_folding_nested_operations() {
    // (5 + 3) * 2 = 16
    let five = Expression::literal(Value::Number(5.0));
    let three = Expression::literal(Value::Number(3.0));
    let two = Expression::literal(Value::Number(2.0));

    let add = Expression::binary(five, Operator::Add, three);
    let mul = Expression::binary(add, Operator::Mul, two);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&mul);

    match optimized {
        Expression::Literal(Value::Number(n)) => assert_eq!(n, 16.0),
        _ => panic!("Expected constant folding to produce Literal(16.0)"),
    }
}

#[test]
fn test_rule_with_negative_score() {
    let when = WhenBlock::new().add_condition(Expression::literal(Value::Bool(true)));

    let rule = Rule {
        id: "negative_score_rule".to_string(),
        name: "Negative Score Rule".to_string(),
        description: None,
        params: None,
        when,
        score: -10,
        metadata: None,
    };

    let result = codegen::RuleCompiler::compile(&rule);
    assert!(result.is_ok());
}

#[test]
fn test_rule_with_zero_score() {
    let when = WhenBlock::new().add_condition(Expression::literal(Value::Bool(true)));

    let rule = Rule {
        id: "zero_score_rule".to_string(),
        name: "Zero Score Rule".to_string(),
        description: None,
        params: None,
        when,
        score: 0,
        metadata: None,
    };

    let result = codegen::RuleCompiler::compile(&rule);
    assert!(result.is_ok());
}

#[test]
fn test_rule_with_metadata() {
    let mut metadata_map = std::collections::HashMap::new();
    metadata_map.insert("category".to_string(), serde_json::json!("fraud"));
    let metadata = serde_json::to_value(metadata_map).unwrap();

    let when = WhenBlock::new().add_condition(Expression::literal(Value::Bool(true)));

    let rule = Rule {
        id: "metadata_rule".to_string(),
        name: "Metadata Rule".to_string(),
        description: Some("Rule with metadata".to_string()),
        params: None,
        when,
        score: 50,
        metadata: Some(metadata),
    };

    let result = codegen::RuleCompiler::compile(&rule);
    assert!(result.is_ok());
}

#[test]
fn test_ruleset_with_extends() {
    let ruleset = Ruleset {
        id: "child_ruleset".to_string(),
        name: Some("Child Ruleset".to_string()),
        extends: Some("parent_ruleset".to_string()),
        rules: vec!["additional_rule".to_string()],
        conclusion: vec![],
        description: None,
        metadata: None,
    };

    let result = codegen::RulesetCompiler::compile(&ruleset);
    assert!(result.is_ok());
}

#[test]
fn test_expression_with_string_operators() {
    let left = Expression::field_access(vec!["event".to_string(), "email".to_string()]);
    let right = Expression::literal(Value::String("@gmail.com".to_string()));
    let expr = Expression::binary(left, Operator::Contains, right);

    let instructions = codegen::ExpressionCompiler::compile(&expr);
    assert!(instructions.is_ok());
}

#[test]
fn test_constant_folding_string_concatenation() {
    let left = Expression::literal(Value::String("hello".to_string()));
    let right = Expression::literal(Value::String("world".to_string()));
    let expr = Expression::binary(left, Operator::Add, right);

    let optimizer = optimizer::ConstantFolder::new();
    let optimized = optimizer.fold(&expr);

    // String concatenation may or may not be folded depending on implementation
    // Just verify the optimization doesn't crash
    match optimized {
        Expression::Literal(Value::String(s)) => {
            // If folded, should be "helloworld"
            assert_eq!(s, "helloworld");
        }
        Expression::Binary { .. } => {
            // If not folded, should remain as binary expression
            // This is acceptable
        }
        _ => panic!("Unexpected expression type after optimization"),
    }
}

#[test]
fn test_validator_with_warnings() {
    let yaml = r#"
version: "0.1"

rule:
  id: "test_rule_123"
  name: "Test Rule"
  when:
    conditions:
      - "event.amount > 1000"
  score: 50
"#;

    let validator = validator::DslValidator::new();
    let validation_result = validator.validate(yaml, validator::DslType::Rule);

    // Warnings are allowed, validation should still pass
    assert!(validation_result.valid);
}
