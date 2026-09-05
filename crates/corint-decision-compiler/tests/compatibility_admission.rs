use corint_decision_compiler::codegen::PipelineCompiler;
use corint_decision_model::ast::{
    pipeline::{ApiTarget, Pipeline, PipelineStep, StepDetails},
    WhenBlock,
};
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
fn guards_and_every_ignored_api_option_are_rejected_before_codegen() {
    let base = PipelineStep::api("unused".into(), "Unused".into(), "api".into());
    let mut guard = base.clone();
    guard.when = Some(WhenBlock::default());
    assert!(PipelineCompiler::compile(&pipeline(guard))
        .unwrap_err()
        .to_string()
        .contains("when"));
    for option in ["params", "on_error", "min_success", "any", "all"] {
        let mut step = base.clone();
        if let StepDetails::Api {
            api_target,
            params,
            on_error,
            min_success,
            ..
        } = &mut step.details
        {
            match option {
                "params" => *params = Some(Default::default()),
                "on_error" => *on_error = Some("ignore".into()),
                "min_success" => *min_success = Some(1),
                "any" => {
                    *api_target = ApiTarget::Any {
                        any: vec!["a".into(), "b".into()],
                    }
                }
                "all" => {
                    *api_target = ApiTarget::All {
                        all: vec!["a".into(), "b".into()],
                    }
                }
                _ => unreachable!(),
            }
        }
        assert!(PipelineCompiler::compile(&pipeline(step))
            .unwrap_err()
            .to_string()
            .contains(option));
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
