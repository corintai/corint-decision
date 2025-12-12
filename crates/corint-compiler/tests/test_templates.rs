//! Integration tests for decision logic templates

use corint_compiler::ImportResolver;
use corint_parser::RulesetParser;

#[test]
fn test_template_loading_and_application() {
    // Load ruleset that uses a template
    let content = std::fs::read_to_string(
        "../../repository/library/rulesets/payment_with_template.yaml"
    ).expect("Failed to read payment_with_template.yaml");

    let doc = RulesetParser::parse_with_imports(&content)
        .expect("Failed to parse ruleset");

    // Verify template reference exists in parsed document
    assert!(doc.definition.decision_template.is_some());
    let template_ref = doc.definition.decision_template.as_ref().unwrap();
    assert_eq!(template_ref.template, "score_based_decision");

    // Verify parameter overrides
    assert!(template_ref.params.is_some());
    let params = template_ref.params.as_ref().unwrap();
    assert_eq!(params.get("critical_threshold"), Some(&serde_json::json!(150)));
    assert_eq!(params.get("high_threshold"), Some(&serde_json::json!(80)));

    // Resolve imports (this should load the template and apply it)
    let mut resolver = ImportResolver::new("../../repository");
    let resolved = resolver.resolve_ruleset_imports(&doc)
        .expect("Failed to resolve imports");

    // Verify rules were loaded
    assert_eq!(resolved.rules.len(), 3);

    // Verify ruleset was processed
    assert_eq!(resolved.rulesets.len(), 1);
    let ruleset = &resolved.rulesets[0];

    // After template application, decision_logic should be populated
    assert!(!ruleset.decision_logic.is_empty());
    println!("Decision logic rules count: {}", ruleset.decision_logic.len());

    // Template reference should be cleared after resolution
    assert!(ruleset.decision_template.is_none());

    // Verify decision logic structure (should have 5 rules from template)
    assert_eq!(ruleset.decision_logic.len(), 5);

    // Verify first rule (critical threshold)
    let first_rule = &ruleset.decision_logic[0];
    assert!(first_rule.condition.is_some());
    assert!(!first_rule.default);
    assert!(matches!(first_rule.action, corint_core::ast::Action::Deny));
    assert!(first_rule.terminate);
}

#[test]
fn test_template_param_substitution() {
    let content = std::fs::read_to_string(
        "../../repository/library/rulesets/payment_with_template.yaml"
    ).expect("Failed to read payment_with_template.yaml");

    let doc = RulesetParser::parse_with_imports(&content).unwrap();
    let mut resolver = ImportResolver::new("../../repository");
    let resolved = resolver.resolve_ruleset_imports(&doc).unwrap();

    let ruleset = &resolved.rulesets[0];

    // Check that parameters were applied (via reason string substitution)
    // The template uses params in reason strings, which should be substituted
    for rule in &ruleset.decision_logic {
        if let Some(reason) = &rule.reason {
            // Reasons should not contain "params." placeholders after substitution
            assert!(!reason.contains("params."),
                "Reason still contains unsubstituted params: {}", reason);
        }
    }
}

#[test]
fn test_multiple_template_types() {
    // Test that we can load different template types
    let mut resolver = ImportResolver::new("../../repository");

    // Load score-based template
    let score_template = resolver.load_template("library/templates/score_based_decision.yaml");
    if let Err(e) = &score_template {
        panic!("Failed to load score template: {:?}", e);
    }
    let (template, _) = score_template.unwrap();
    assert_eq!(template.id, "score_based_decision");
    assert!(template.params.is_some());
    assert_eq!(template.decision_logic.len(), 5);

    // Load pattern-based template
    let pattern_template = resolver.load_template("library/templates/pattern_based_decision.yaml");
    assert!(pattern_template.is_ok());
    let (template, _) = pattern_template.unwrap();
    assert_eq!(template.id, "pattern_based_decision");

    // Load hybrid template
    let hybrid_template = resolver.load_template("library/templates/hybrid_decision.yaml");
    assert!(hybrid_template.is_ok());
    let (template, _) = hybrid_template.unwrap();
    assert_eq!(template.id, "hybrid_decision");
}
