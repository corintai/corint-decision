use corint_decision_dsl_parser::RuleParser;
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
