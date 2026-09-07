use corint_decision_compiler::codegen::PipelineCompiler;
use corint_decision_model::ast::pipeline::{Pipeline, PipelineStep};
use corint_decision_model::ast::WhenBlock;
fn pipeline(step: PipelineStep) -> Pipeline {
    Pipeline::new("p".into(), "P".into(), "entry".into()).with_steps(vec![
        PipelineStep::ruleset("entry".into(), "Entry".into(), "rules".into()),
        step,
    ])
}
#[test]
fn unreachable_unsupported_steps_and_mismatched_details_are_rejected() {
    for kind in [
        "function", "trigger", "rule", "pipeline", "service", "extract", "typo", "ruleset",
    ] {
        let mut step = PipelineStep::router("unused".into(), "Unused".into());
        step.step_type = kind.into();
        let error = PipelineCompiler::compile(&pipeline(step))
            .unwrap_err()
            .to_string();
        assert!(error.contains("unused") && error.contains(kind), "{error}");
    }
}
#[test]
fn deferred_compatibility_rulesets_cannot_feed_a_router_condition() {
    use corint_decision_model::ast::{pipeline::Route, Expression};
    let mut step = PipelineStep::router("unused".into(), "Unused".into());
    step.routes = Some(vec![Route {
        next: "end".into(),
        when: WhenBlock {
            event_type: None,
            condition_group: None,
            conditions: Some(vec![Expression::field_access(vec![
                "results".into(),
                "risk".into(),
                "score".into(),
            ])]),
        },
    }]);
    assert!(PipelineCompiler::compile(&pipeline(step))
        .unwrap_err()
        .to_string()
        .contains("result-dependent"));
}

#[test]
fn invalid_unreachable_service_outputs_and_parameters_are_rejected() {
    use corint_decision_model::{
        ast::{pipeline::StepDetails, Expression},
        Value,
    };
    for invalid in ["event.amount", "api.risk", "service.", ""] {
        let mut step = PipelineStep::service(
            "unused".into(),
            "Unused".into(),
            "risk".into(),
            "score".into(),
        );
        if let StepDetails::Service { output, .. } = &mut step.details {
            *output = Some(invalid.into());
        }
        assert!(
            PipelineCompiler::compile(&pipeline(step)).is_err(),
            "{invalid}"
        );
    }
    let mut step = PipelineStep::service(
        "unused".into(),
        "Unused".into(),
        "risk".into(),
        "score".into(),
    );
    if let StepDetails::Service { params, .. } = &mut step.details {
        *params = Some(std::collections::HashMap::from([(
            "".into(),
            Expression::literal(Value::Null),
        )]));
    }
    assert!(PipelineCompiler::compile(&pipeline(step)).is_err());
}
