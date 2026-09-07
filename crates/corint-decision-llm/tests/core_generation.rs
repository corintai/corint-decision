#![cfg(feature = "core-generation")]
use async_trait::async_trait;
use corint_decision_compiler::core::{compile_core, parse_core_input_schema, CoreSource, PROFILE};
use corint_decision_llm::{
    CoreGeneration, CoreGenerationError, CoreGenerator, LLMClient, LLMError, LLMRequest,
    LLMResponse, RuleGeneratorConfig,
};
use corint_decision_toolchain::{contracts::TargetContracts, package};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[test]
fn generation_capability_contract_points_to_real_gate() {
    let capabilities: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/capabilities.json"
    ))
    .unwrap();
    let tool = &capabilities["tools"]["core_generator"];
    assert_eq!(
        tool["entry_points"],
        json!(["CoreGenerator::generate", "CoreGenerator::revise"])
    );
    assert_eq!(tool["acceptance_cases_sent_to_model"], false);
    assert_eq!(tool["automatic_retries"], 0);
    assert_eq!(tool["business_evaluation"], "not_performed");
    assert_eq!(tool["publication_approval"], "not_granted");
    assert_eq!(
        tool["targeted_entry_points"],
        json!([
            "CoreGenerator::generate_for_target",
            "CoreGenerator::revise_for_target"
        ])
    );
    assert_eq!(
        tool["evidence"],
        "../../../crates/corint-decision-llm/tests/core_generation.rs"
    );
    let schema: Value =
        serde_json::from_str(corint_decision_llm::generator::core_generator::RESPONSE_SCHEMA)
            .unwrap();
    let compiled = jsonschema::JSONSchema::compile(&schema).unwrap();
    assert!(compiled.is_valid(&serde_json::from_str::<Value>(&envelope(&sources())).unwrap()));
    assert_eq!(schema["properties"]["profile"]["const"], PROFILE);
}

#[tokio::test]
async fn revision_passes_after_caller_explicitly_updates_acceptance_boundaries() {
    let original = sources();
    let mut revised = sources();
    revised[0].yaml = revised[0].yaml.replace("> 1000", "> 500");
    let mut cases = source("behavior.yaml");
    cases.yaml = cases
        .yaml
        .replace("1001", "501")
        .replace("1000", "500")
        .replace("999", "499");
    let result = generator(envelope(&revised), "stop", false)
        .0
        .revise(
            "Lower threshold to 500",
            &original,
            &source("input-schema.yaml"),
            &cases,
        )
        .await
        .unwrap();
    assert!(result.accepted());
    assert_eq!(result.tests.passed, 5);
    assert!(original[0].yaml.contains("> 1000"));
}

const FILES: &[&str] = &[
    "rule.yaml",
    "ruleset.yaml",
    "pipeline.yaml",
    "registry.yaml",
];
fn source(file: &str) -> CoreSource {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/cdl_core");
    CoreSource {
        path: file.into(),
        yaml: std::fs::read_to_string(root.join(file)).unwrap(),
    }
}
fn sources() -> Vec<CoreSource> {
    FILES.iter().map(|f| source(f)).collect()
}

fn contracts(allow_review: bool) -> TargetContracts {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/contracts");
    let context = CoreSource {
        path: "business-context.yaml".into(),
        yaml: std::fs::read_to_string(root.join("business-context.yaml")).unwrap(),
    };
    let mut target: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("target-capabilities.json")).unwrap(),
    )
    .unwrap();
    if !allow_review {
        target["actions"] = json!(["BLOCK"]);
    }
    TargetContracts::load(
        &context,
        &CoreSource {
            path: "target-capabilities.json".into(),
            yaml: target.to_string(),
        },
    )
    .unwrap()
}

#[tokio::test]
async fn targeted_generation_binds_package_and_sends_context_but_not_acceptance_cases() {
    let (generator, recorder) = generator(envelope(&sources()), "stop", false);
    let mut cases = source("behavior.yaml");
    cases.yaml.push_str("\n# private-target-test-5739\n");
    let result = generator
        .generate_for_target(
            "Threshold 1000",
            &source("input-schema.yaml"),
            &cases,
            &contracts(true),
        )
        .await
        .unwrap();
    assert!(result.accepted());
    let report = result.compatibility.unwrap();
    let package = serde_json::to_value(result.package.unwrap()).unwrap();
    assert_eq!(report.policy_sha256, package["policy"]["sha256"]);
    assert!(!report.execution_checked);
    assert!(!report.live_target_verified);
    assert!(!report.business_semantics_checked);
    assert_eq!(report.publication_approval, "not_granted");
    let requests = recorder.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].prompt.contains("CNY_yuan"));
    assert!(requests[0].prompt.contains("target_capabilities"));
    assert!(!requests[0].prompt.contains("private-target-test-5739"));
    assert!(!requests[0].prompt.contains("above_threshold"));
}

#[tokio::test]
async fn targeted_generation_rejects_input_before_provider_and_actions_after_provider() {
    let (generator, recorder) = generator(envelope(&sources()), "stop", false);
    let mut input = source("input-schema.yaml");
    input.yaml = input.yaml.replace("number", "string");
    error(
        generator
            .generate_for_target(
                "Threshold 1000",
                &input,
                &source("behavior.yaml"),
                &contracts(true),
            )
            .await,
        "E_CONTEXT_INPUT",
    );
    assert!(recorder.requests.lock().unwrap().is_empty());
    error(
        generator
            .generate_for_target(
                "Threshold 1000",
                &source("input-schema.yaml"),
                &source("behavior.yaml"),
                &contracts(false),
            )
            .await,
        "E_ACTION_UNAVAILABLE",
    );
    assert_eq!(recorder.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn targeted_revision_still_requires_independent_behavior_acceptance() {
    let mut revised = sources();
    revised[0].yaml = revised[0].yaml.replace("> 1000", ">= 1000");
    let (generator, recorder) = generator(envelope(&revised), "stop", false);
    let result = generator
        .revise_for_target(
            "Include boundary",
            &sources(),
            &source("input-schema.yaml"),
            &source("behavior.yaml"),
            &contracts(true),
        )
        .await
        .unwrap();
    assert!(result.compatibility.is_some());
    assert!(!result.accepted());
    assert!(result.package.is_none());
    assert_eq!(result.tests.failed, 1);
    error(
        generator
            .revise_for_target(
                "Include boundary",
                &[],
                &source("input-schema.yaml"),
                &source("behavior.yaml"),
                &contracts(true),
            )
            .await,
        "E_GENERATION_REQUEST",
    );
    assert_eq!(recorder.requests.lock().unwrap().len(), 1);
}
fn envelope(sources: &[CoreSource]) -> String {
    json!({"profile":PROFILE,"sources":sources}).to_string()
}

struct Recorder {
    requests: Mutex<Vec<LLMRequest>>,
    response: LLMResponse,
    fail: bool,
}
#[async_trait]
impl LLMClient for Recorder {
    async fn call(&self, request: LLMRequest) -> corint_decision_llm::Result<LLMResponse> {
        self.requests.lock().unwrap().push(request);
        if self.fail {
            Err(LLMError::ApiCallFailed("offline mock failure".into()))
        } else {
            Ok(self.response.clone())
        }
    }
    fn name(&self) -> &str {
        "recorded-offline"
    }
}
fn generator(content: String, finish: &str, fail: bool) -> (CoreGenerator, Arc<Recorder>) {
    let recorder = Arc::new(Recorder {
        requests: Mutex::new(vec![]),
        response: LLMResponse::new(content, "test-model".into()).with_finish_reason(finish.into()),
        fail,
    });
    let generator = CoreGenerator::new(
        recorder.clone(),
        RuleGeneratorConfig::new("test-model").with_max_tokens(8192),
    );
    (generator, recorder)
}
async fn run(content: String) -> Result<CoreGeneration, CoreGenerationError> {
    generator(content, "stop", false)
        .0
        .generate(
            "Decline amounts above 1000, score 60",
            &source("input-schema.yaml"),
            &source("behavior.yaml"),
        )
        .await
}
fn error(result: Result<CoreGeneration, CoreGenerationError>, code: &str) {
    match result {
        Err(CoreGenerationError::Core(e)) => assert_eq!(e.diagnostic.code, code, "{e}"),
        Err(e) => panic!("wrong error: {e}"),
        Ok(_) => panic!("expected {code}, got a candidate"),
    }
}

#[tokio::test]
async fn generates_tests_and_reverifies_with_shared_toolchain_inside_async_runtime() {
    let (generator, recorder) = generator(envelope(&sources()), "stop", false);
    let mut cases = source("behavior.yaml");
    cases.yaml.push_str("\n# private-heldout-marker-83456\n");
    let result = generator
        .generate("Threshold 1000", &source("input-schema.yaml"), &cases)
        .await
        .unwrap();
    assert!(result.accepted());
    assert_eq!(
        (
            result.tests.total,
            result.tests.executed,
            result.tests.passed
        ),
        (5, 5, 5)
    );
    assert!(result.tests.cases.iter().all(|c| c.trace_parity));
    {
        let requests = recorder.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.model, "test-model");
        assert_eq!(request.max_tokens, Some(8192));
        assert!(!request.prompt.contains("private-heldout-marker-83456"));
        assert!(!request.prompt.contains("above_threshold"));
        assert!(request.prompt.contains("## Strict Core Registry"));
        assert!(request.prompt.contains("`E_UNRESOLVED_REF`"));
        assert!(request.prompt.contains("4096 pattern"));
        assert!(!request.prompt.contains("Engine logs a warning at startup"));
        assert!(!request.prompt.contains("## Compatibility namespaces"));
        assert!(request
            .prompt
            .contains(corint_decision_compiler::core::CORE_SCHEMA));
        assert!(request
            .prompt
            .contains(corint_decision_llm::generator::core_generator::RESPONSE_SCHEMA));
    }
    let stored = CoreSource {
        path: "generated-package.json".into(),
        yaml: serde_json::to_string(&result.package.unwrap()).unwrap(),
    };
    let encoded: Value = serde_json::from_str(&stored.yaml).unwrap();
    assert_eq!(encoded["evidence"]["business_evaluation"], "not_performed");
    assert_eq!(encoded["evidence"]["publication_approval"], "not_granted");
    assert_eq!(encoded["evidence"]["authenticity"], "unsigned");
    assert!(!stored.yaml.contains("private-heldout-marker-83456"));
    let verified = tokio::task::spawn_blocking(move || package::verify(&stored, &cases))
        .await
        .unwrap()
        .unwrap();
    assert!(verified.error.is_none());
    assert_eq!(verified.tests.passed, 5);
}

#[tokio::test]
async fn complete_but_wrong_behavior_has_results_and_no_package() {
    let mut docs = sources();
    docs[0].yaml = docs[0].yaml.replace("> 1000", ">= 1000");
    let result = run(envelope(&docs)).await.unwrap();
    assert!(!result.accepted());
    assert!(result.package.is_none());
    assert_eq!(result.tests.executed, 5);
    assert_eq!(result.tests.failed, 1);
    assert_eq!(result.tests.cases[1].id, "equal_threshold");
    assert!(result.tests.cases[1]
        .diagnostics
        .iter()
        .any(|d| d.code == "E_TEST_MISMATCH"));
}

#[tokio::test]
async fn all_manifest_failures_keep_compiler_diagnostics() {
    let manifest: Value = serde_yaml::from_str(&source("manifest.yaml").yaml).unwrap();
    // Preserve the original 24 rejections plus five new Pipeline boundary cases;
    // future manifest additions must also run through this adapter.
    assert!(manifest["invalid"].as_array().unwrap().len() >= 29);
    for case in manifest["invalid"].as_array().unwrap() {
        let mut docs = sources();
        let doc = docs
            .iter_mut()
            .find(|d| d.path == case["document"].as_str().unwrap())
            .unwrap();
        let find = case["find"].as_str().unwrap();
        assert_eq!(doc.yaml.matches(find).count(), 1);
        doc.yaml = doc
            .yaml
            .replacen(find, case["replace"].as_str().unwrap(), 1);
        let expected = compile_core(
            &docs,
            parse_core_input_schema(&source("input-schema.yaml")).unwrap(),
        )
        .unwrap_err();
        match run(envelope(&docs)).await {
            Err(CoreGenerationError::Core(actual)) => assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(expected).unwrap(),
                "{case}"
            ),
            _ => panic!("expected shared compiler error: {case}"),
        }
    }
}

#[tokio::test]
async fn missing_registry_or_dependency_fails_closed() {
    let docs = sources();
    error(run(envelope(&docs[..3])).await, "E_MISSING_FIELD");
    error(run(envelope(&docs[1..])).await, "E_UNRESOLVED_REF");
}

#[tokio::test]
async fn model_cannot_supply_tests_contracts_or_validation_claims() {
    for key in [
        "cases",
        "input_schema",
        "evidence",
        "package",
        "validated",
        "business_evaluation",
    ] {
        let mut value: Value = serde_json::from_str(&envelope(&sources())).unwrap();
        value[key] = json!({"approved":true});
        error(run(value.to_string()).await, "E_GENERATION_FORMAT");
    }
}

#[tokio::test]
async fn malformed_fenced_duplicate_or_trailing_responses_are_not_salvaged() {
    let good = envelope(&sources());
    for bad in [
        format!("\x60\x60\x60json\n{good}\n\x60\x60\x60"),
        format!("{good} explanatory prose"), format!("{good}{good}"),
        format!("{{\"profile\":\"{PROFILE}\",\"profile\":\"{PROFILE}\",\"sources\":[]}}"),
        format!("{{\"profile\":\"{PROFILE}\",\"sources\":[{{\"path\":\"a.yaml\",\"path\":\"b.yaml\",\"yaml\":\"x\"}}]}}"),
        "rule: {id: yaml-is-not-the-envelope}".into(),
    ] { error(run(bad).await, "E_GENERATION_FORMAT"); }
}

#[tokio::test]
async fn public_response_schema_and_limits_are_enforced() {
    for (field, value) in [
        ("profile", json!("cdl-next")),
        ("sources", json!([])),
        ("sources", json!(vec![source("rule.yaml"); 257])),
    ] {
        let mut bad: Value = serde_json::from_str(&envelope(&sources())).unwrap();
        bad[field] = value;
        error(run(bad.to_string()).await, "E_GENERATION_FORMAT");
    }
    let mut docs = sources();
    docs[0].yaml = "x".repeat(1048577);
    error(run(envelope(&docs)).await, "E_GENERATION_FORMAT");
    error(
        run(" ".repeat(4 * 1024 * 1024 + 1)).await,
        "E_GENERATION_FORMAT",
    );
}

#[tokio::test]
async fn labels_are_unique_logical_names_not_filesystem_targets() {
    for label in [
        "../escape.yaml",
        "/tmp/escape.yaml",
        "a/../b.yaml",
        "https://x/a.yaml",
        "a\\b.yaml",
        "x.json",
    ] {
        let mut docs = sources();
        docs[0].path = label.into();
        error(run(envelope(&docs)).await, "E_GENERATION_FORMAT");
    }
    let mut docs = sources();
    docs[1].path = docs[0].path.clone();
    error(run(envelope(&docs)).await, "E_DUPLICATE_SOURCE");
}

#[tokio::test]
async fn incomplete_or_refused_output_never_becomes_an_artifact() {
    for finish in [
        "length",
        "max_tokens",
        "content_filter",
        "tool_calls",
        "unknown",
        "",
    ] {
        let (generator, recorder) = generator(envelope(&sources()), finish, false);
        error(
            generator
                .generate(
                    "policy",
                    &source("input-schema.yaml"),
                    &source("behavior.yaml"),
                )
                .await,
            "E_GENERATION_INCOMPLETE",
        );
        assert_eq!(recorder.requests.lock().unwrap().len(), 1);
    }
    for finish in ["end_turn", "STOP"] {
        assert!(generator(envelope(&sources()), finish, false)
            .0
            .generate(
                "policy",
                &source("input-schema.yaml"),
                &source("behavior.yaml")
            )
            .await
            .unwrap()
            .accepted());
    }
}

#[tokio::test]
async fn invalid_caller_contracts_fail_before_model_call() {
    let (generator, recorder) = generator(envelope(&sources()), "stop", false);
    let bad = CoreSource {
        path: "bad.yaml".into(),
        yaml: "broken: true".into(),
    };
    assert!(generator
        .generate("policy", &bad, &source("behavior.yaml"))
        .await
        .is_err());
    error(
        generator
            .generate("policy", &source("input-schema.yaml"), &bad)
            .await,
        "E_TEST_SUITE",
    );
    error(
        generator
            .generate("", &source("input-schema.yaml"), &source("behavior.yaml"))
            .await,
        "E_GENERATION_REQUEST",
    );
    error(
        generator
            .revise(
                "policy",
                &[],
                &source("input-schema.yaml"),
                &source("behavior.yaml"),
            )
            .await,
        "E_GENERATION_REQUEST",
    );
    assert!(generator
        .revise(
            "policy",
            &sources()[1..],
            &source("input-schema.yaml"),
            &source("behavior.yaml")
        )
        .await
        .is_err());
    assert!(recorder.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn provider_error_is_propagated_without_retry_or_fallback() {
    let (generator, recorder) = generator(String::new(), "stop", true);
    assert!(matches!(
        generator
            .generate(
                "policy",
                &source("input-schema.yaml"),
                &source("behavior.yaml")
            )
            .await,
        Err(CoreGenerationError::Provider(LLMError::ApiCallFailed(_)))
    ));
    assert_eq!(recorder.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn revision_returns_a_new_closure_and_uses_unchanged_caller_contracts() {
    let original = sources();
    let before = envelope(&original);
    let mut revised = sources();
    revised[0].yaml = revised[0].yaml.replace("> 1000", "> 500");
    let (generator, recorder) = generator(envelope(&revised), "stop", false);
    let result = generator
        .revise(
            "Lower threshold to 500",
            &original,
            &source("input-schema.yaml"),
            &source("behavior.yaml"),
        )
        .await
        .unwrap();
    assert_eq!(envelope(&original), before);
    assert!(!result.accepted()); // Existing independent boundaries deliberately reject the change.
    assert!(recorder.requests.lock().unwrap()[0]
        .prompt
        .contains("existing_sources"));
    assert_eq!(result.sources[0].path, original[0].path);
    assert!(result.sources[0].yaml.contains("> 500"));
}
