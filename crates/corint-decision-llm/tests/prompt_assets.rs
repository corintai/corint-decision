#![cfg(feature = "core-generation")]
use corint_decision_compiler::core::{parse_core_input_schema, CoreSource};
use corint_decision_toolchain::behavior;
use std::path::PathBuf;

#[test]
fn pipeline_prompt_keeps_strict_references_without_legacy_registry_fallbacks() {
    let prompt = corint_decision_llm::generator::prompt_templates::build_pipeline_prompt(
        "Route a payment using declared amount",
    );
    assert!(prompt.contains("## Strict Core Registry"));
    assert!(prompt.contains("`E_NO_PIPELINE_MATCH`"));
    assert!(prompt.contains("`E_PIPELINE_SKIPPED`"));
    assert!(prompt.contains("`E_UNRESOLVED_REF`"));
    assert!(prompt.contains("Multiple entries may reference the **same Pipeline ID**"));
    assert!(prompt.contains("4096 pattern"));
    assert!(prompt.contains("Route a payment using declared amount"));
    assert!(!prompt.contains("Engine logs a warning at startup"));
    assert!(!prompt.contains("## Compatibility namespaces"));
    assert!(!prompt.contains("shadow_us"));
}

#[test]
fn generation_examples_execute_as_a_complete_core_closure() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance");
    let source = |path: &str| CoreSource {
        path: path.into(),
        yaml: std::fs::read_to_string(root.join(path)).unwrap(),
    };
    let sources: Vec<_> = ["rule", "ruleset", "pipeline", "registry"]
        .iter()
        .map(|name| source(&format!("generation/{name}.yaml")))
        .collect();
    let input = parse_core_input_schema(&source("cdl_core/input-schema.yaml")).unwrap();
    let cases = source("cdl_core/behavior.yaml");
    let result = behavior::test(&sources, input, &cases).unwrap();
    assert_eq!(
        result.failed,
        0,
        "{}",
        serde_json::to_string_pretty(&result).unwrap()
    );
    // Embedding is required: handwritten example copies cannot silently diverge.
    let prompts = include_str!("../src/generator/prompt_templates.rs");
    for resource in ["rule", "ruleset", "pipeline"] {
        assert!(prompts.contains(&format!(
            "include_str!(\"../../../../tests/conformance/generation/{resource}.yaml\")"
        )));
    }
    for obsolete in [
        "count(event.user.id",
        "strategy: first_match",
        "default_action:",
    ] {
        assert!(!prompts.contains(obsolete));
    }
}
