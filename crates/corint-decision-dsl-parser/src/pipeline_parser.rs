//! Backwards-compatible module path for the shared pipeline parser.
pub use crate::pipeline::PipelineParser;

#[cfg(test)]
mod tests {
    use super::*;
    use corint_decision_model::ast::pipeline::{ApiTarget, StepDetails, StepNext};

    #[test]
    fn test_parse_new_format_pipeline() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: step1
  metadata:
    version: "1.0"
    author: "Test Team"
  steps:
    - step:
        id: step1
        name: Router Step
        type: router
        routes:
          - next: step2
            when:
              all:
                - event.amount > 100
        default: step3
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        assert_eq!(pipeline.id, "test_pipeline");
        assert_eq!(pipeline.name, "Test Pipeline");
        assert_eq!(pipeline.entry, "step1");
        assert!(pipeline.metadata.is_some());
        let metadata = pipeline.metadata.unwrap();
        assert_eq!(metadata.get("version").unwrap(), &serde_json::json!("1.0"));
        assert_eq!(pipeline.steps.len(), 1);

        let step = &pipeline.steps[0];
        assert_eq!(step.id, "step1");
        assert_eq!(step.name, "Router Step");
        assert_eq!(step.step_type, "router");
        assert!(step.routes.is_some());
        assert_eq!(step.default, Some("step3".to_string()));
    }

    #[test]
    fn test_parse_ruleset_step() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: ruleset_step
  steps:
    - step:
        id: ruleset_step
        name: Execute Ruleset
        type: ruleset
        ruleset: fraud_detection
        next: end
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        assert_eq!(pipeline.steps.len(), 1);
        let step = &pipeline.steps[0];
        assert_eq!(step.id, "ruleset_step");
        assert_eq!(step.step_type, "ruleset");
        assert!(matches!(
            &step.details,
            StepDetails::Ruleset { ruleset } if ruleset == "fraud_detection"
        ));
        // StepNext::End was removed - "end" is now represented as StepNext::StepId("end".to_string())
        assert_eq!(step.next, Some(StepNext::StepId("end".to_string())));
    }

    #[test]
    fn test_parse_api_single() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: api_step
  steps:
    - step:
        id: api_step
        name: Call Single API
        type: api
        api: geolocation_service
        endpoint: /check
        output: api.geo
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        let step = &pipeline.steps[0];
        assert!(matches!(
            &step.details,
            StepDetails::Api { api_target: ApiTarget::Single { api }, .. } if api == "geolocation_service"
        ));
    }

    #[test]
    fn test_parse_api_any_mode() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: api_step
  steps:
    - step:
        id: api_step
        name: Call Any API
        type: api
        any: [primary_api, backup_api, fallback_api]
        output: api.result
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        let step = &pipeline.steps[0];
        assert!(matches!(
            &step.details,
            StepDetails::Api { api_target: ApiTarget::Any { any }, .. } if any.len() == 3
        ));
    }

    #[test]
    fn test_parse_api_all_mode() {
        let yaml = r#"
pipeline:
  id: test_pipeline
  name: Test Pipeline
  entry: api_step
  steps:
    - step:
        id: api_step
        name: Call All APIs
        type: api
        all: [api1, api2, api3]
        timeout: 5000
        min_success: 2
        on_error: continue
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        let step = &pipeline.steps[0];
        assert!(matches!(
            &step.details,
            StepDetails::Api {
                api_target: ApiTarget::All { all },
                timeout: Some(5000),
                min_success: Some(2),
                ..
            } if all.len() == 3
        ));
    }

    #[test]
    fn test_parse_extract_step() {
        let yaml = r#"
pipeline:
  steps:
    - type: extract
      id: extract_features
      features:
        - name: login_count
          value: user.login_count
        - name: device_count
          value: user.device_count
"#;

        let pipeline = PipelineParser::parse(yaml).unwrap();

        // Legacy format now converts to new PipelineStep format
        assert_eq!(pipeline.steps.len(), 1);
        assert_eq!(pipeline.steps[0].step_type, "extract");
    }
}
