use corint_decision_dsl_parser::RuleParser;

#[test]
fn malformed_api_options_cannot_bypass_compiler_admission() {
    use corint_decision_dsl_parser::PipelineParser;
    for option in [
        "params: null",
        "params: []",
        "params: false",
        "on_error: {action: fallback}",
        "on_error: null",
        "on_error: false",
        "min_success: null",
        "min_success: -1",
        "min_success: '1'",
    ] {
        let yaml = format!("pipeline:\n  id: p\n  name: P\n  entry: lookup\n  steps:\n    - step:\n        id: lookup\n        name: Lookup\n        type: api\n        api: test_api\n        next: end\n        {option}\n");
        assert!(PipelineParser::parse(&yaml).is_err(), "{option}");
    }
}
#[test]
fn unknown_mixed_and_ill_typed_when_fields_are_not_silently_ignored() {
    for when in [
        "{typo: ['true']}",
        "{all: ['true'], any: ['false']}",
        "{conditions: 'false'}",
        "{event: {type: payment, typo: 1}}",
        "{event_type: payment, event.type: login}",
        "{all: [{any: ['true'], typo: 1}]}",
        "false",
    ] {
        let yaml = format!("rule:\n  id: r\n  name: R\n  score: 1\n  when: {when}\n");
        assert!(RuleParser::parse(&yaml).is_err(), "{when}");
    }
}
