//! Test assertions over the real engine. No expression evaluation lives here.
use crate::failure;
use corint_decision_compiler::core::{diagnostic, CoreError, CoreSource};
use corint_decision_compiler::Diagnostic;
use corint_decision_engine::{
    DecisionEngine, DecisionRequest, DecisionResponse, EngineError, RuntimeError, Schema, Value,
};
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::collections::{BTreeSet, HashMap};

const SUITE_SCHEMA: &str = include_str!("../../../docs/cdl/schema/test-suite.json");

#[derive(Deserialize)]
struct Suite {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    id: String,
    input: Input,
    expect: Option<Json>,
    expect_error: Option<Json>,
}
#[derive(Deserialize)]
struct Input {
    event: HashMap<String, Value>,
}

#[derive(Serialize)]
pub struct TestResults {
    pub total: usize,
    pub executed: usize,
    pub passed: usize,
    pub failed: usize,
    pub cases: Vec<CaseResult>,
}
#[derive(Serialize)]
pub struct CaseResult {
    pub id: String,
    pub passed: bool,
    pub trace_parity: bool,
    pub expected: Json,
    pub actual: Json,
    pub trace_actual: Json,
    pub diagnostics: Vec<Diagnostic>,
}

fn parse_suite(source: &CoreSource) -> Result<Suite, CoreError> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(&source.yaml).map_err(|e| {
        let mut err = diagnostic(&source.path, "", "parse", "E_TEST_SUITE", e.to_string());
        if let Some(p) = e.location() {
            err.diagnostic.line = Some(p.line());
            err.diagnostic.column = Some(p.column());
        }
        err
    })?;
    let value = serde_json::to_value(yaml)
        .map_err(|e| failure(&source.path, "validate", "E_TEST_SUITE", e.to_string()))?;
    let schema =
        JSONSchema::compile(&serde_json::from_str(SUITE_SCHEMA).expect("suite schema JSON"))
            .expect("embedded suite schema");
    if let Err(mut errors) = schema.validate(&value) {
        if let Some(error) = errors.next() {
            return Err(diagnostic(
                &source.path,
                &error.instance_path.to_string(),
                "validate",
                "E_TEST_SUITE",
                error.to_string(),
            ));
        }
    }
    let suite: Suite = serde_json::from_value(value)
        .map_err(|e| failure(&source.path, "validate", "E_TEST_SUITE", e.to_string()))?;
    let mut ids = BTreeSet::new();
    for (i, case) in suite.cases.iter().enumerate() {
        if !ids.insert(&case.id) {
            return Err(diagnostic(
                &source.path,
                &format!("/cases/{i}/id"),
                "validate",
                "E_DUPLICATE_ID",
                "Duplicate test case ID",
            ));
        }
    }
    Ok(suite)
}

/// Validate caller-owned cases before any model request or execution.
pub fn validate_suite(source: &CoreSource) -> Result<(), CoreError> {
    parse_suite(source).map(|_| ())
}

fn engine_error(error: EngineError) -> CoreError {
    match error {
        EngineError::Core(error) => error,
        EngineError::RuntimeError(RuntimeError::InvalidOperation(ref message))
            if message.starts_with("E_SCORE_OVERFLOW:") =>
        {
            failure("<engine>", "execute", "E_SCORE_OVERFLOW", message)
        }
        other => failure("<engine>", "execute", "E_ENGINE", other.to_string()),
    }
}

// Project only deterministic evidence, excluding request IDs, timing and raw inputs.
// Missing internal evidence is an adapter failure, never silently an empty array.
fn snapshot(response: DecisionResponse, traced: bool) -> Result<Json, CoreError> {
    let bad = || {
        failure(
            "<engine>",
            "execute",
            "E_TEST_EVIDENCE",
            "Missing or malformed Core execution evidence",
        )
    };
    let context = serde_json::to_value(&response.result.context).map_err(|_| bad())?;
    if response.trace.is_some() != traced {
        return Err(bad());
    }
    let steps = context["__executed_steps__"]
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|step| {
            let step: Json =
                serde_json::from_str(step.as_str().ok_or_else(bad)?).map_err(|_| bad())?;
            Ok(Json::String(
                step["step_id"].as_str().ok_or_else(bad)?.into(),
            ))
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    // A router-only path can legitimately execute no rulesets.
    let calls = context
        .get("__core_rule_executions__")
        .cloned()
        .unwrap_or_else(|| json!([]));
    if !calls.is_array() {
        return Err(bad());
    }
    let mut local = serde_json::Map::new();
    for (key, value) in context.as_object().ok_or_else(bad)? {
        if let Some(id) = key.strip_prefix("__ruleset_result__.") {
            let output = if value["status"] == "skipped" {
                json!({"status":"skipped"})
            } else if value["score"].is_number() && value["matched"].is_boolean() {
                json!({"score":value["score"],"matched":value["matched"]})
            } else if value["score"].is_number() && value["signal"].is_string() {
                json!({"score":value["score"],"signal":value["signal"]})
            } else {
                return Err(bad());
            };
            local.insert(id.into(), output);
        }
    }
    if let Some(trace) = &response.trace {
        let pipeline = trace.pipeline.as_ref().ok_or_else(bad)?;
        if trace.rules_evaluated != calls.as_array().ok_or_else(bad)?.len()
            || pipeline
                .steps
                .iter()
                .any(|step| step.executed != steps.contains(&json!(step.step_id)))
        {
            return Err(bad());
        }
    }
    Ok(json!({
        "pipeline_id": response.pipeline_id, "score": response.result.score,
        "signal": serde_json::to_value(response.result.signal).map_err(|_| bad())?["type"],
        "actions": response.result.actions, "triggered_rules": response.result.triggered_rules,
        "explanation": response.result.explanation, "steps": steps,
        "calls": calls, "local_results": local
    }))
}

fn outcome(
    result: Result<DecisionResponse, EngineError>,
    traced: bool,
) -> (Json, Option<Diagnostic>) {
    let snapshot = match result {
        Ok(response) => snapshot(response, traced),
        Err(error) => Err(engine_error(error)),
    };
    match snapshot {
        Ok(value) => (json!({"result": value}), None),
        Err(error) => (
            json!({"error": {"stage": error.diagnostic.stage, "code": error.diagnostic.code}}),
            Some(*error.diagnostic),
        ),
    }
}

// JSON Schema integer includes 60.0. Compare numeric values without changing strings,
// array order or object keys. Scores are schema-bounded i32 so this is lossless.
fn equal(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(a), Json::Number(b)) => a.as_f64() == b.as_f64(),
        (Json::Array(a), Json::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (Json::Object(a), Json::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|b| equal(v, b)))
        }
        _ => a == b,
    }
}

pub fn test(
    sources: &[CoreSource],
    schema: Schema,
    source: &CoreSource,
) -> Result<TestResults, CoreError> {
    // Validate the whole suite and compile the whole bundle before any case runs.
    let suite = parse_suite(source)?;
    let engine = DecisionEngine::from_core(sources, schema).map_err(engine_error)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| failure("<runtime>", "load", "E_IO", e.to_string()))?;
    runtime.block_on(async {
        let mut report = TestResults {
            total: suite.cases.len(),
            executed: 0,
            passed: 0,
            failed: 0,
            cases: Vec::new(),
        };
        for (i, case) in suite.cases.into_iter().enumerate() {
            let request = DecisionRequest::new(case.input.event);
            let (actual, plain_error) = outcome(engine.decide(request.clone()).await, false);
            let (trace_actual, trace_error) =
                outcome(engine.decide(request.with_trace()).await, true);
            let parity = equal(&actual, &trace_actual);
            let expected = match case.expect {
                Some(expect) => json!({"result": expect}),
                None => json!({"error": case.expect_error.expect("validated expectation")}),
            };
            let mut diagnostics = Vec::new();
            if !parity {
                diagnostics.push(
                    *diagnostic(
                        &source.path,
                        &format!("/cases/{i}"),
                        "test",
                        "E_TRACE_PARITY",
                        "Trace on/off outcomes differ",
                    )
                    .diagnostic,
                );
            }
            if let Some(result) = expected.get("result") {
                for (field, expected) in result.as_object().expect("validated expected result") {
                    if !equal(expected, &actual["result"][field]) {
                        diagnostics.push(
                            *diagnostic(
                                &source.path,
                                &format!("/cases/{i}/expect/{field}"),
                                "test",
                                "E_TEST_MISMATCH",
                                format!("Expected {field} differs from actual result"),
                            )
                            .diagnostic,
                        );
                    }
                }
            } else if !equal(&expected, &actual) {
                diagnostics.push(
                    *diagnostic(
                        &source.path,
                        &format!("/cases/{i}/expect_error"),
                        "test",
                        "E_TEST_MISMATCH",
                        "Expected error differs from actual outcome",
                    )
                    .diagnostic,
                );
            }
            // Preserve engine diagnostics as evidence, including expected input failures.
            let passed = diagnostics.is_empty();
            for error in [plain_error, trace_error].into_iter().flatten() {
                if !diagnostics
                    .iter()
                    .any(|d| serde_json::to_value(d).ok() == serde_json::to_value(&error).ok())
                {
                    diagnostics.push(error);
                }
            }
            report.executed += 1;
            if passed {
                report.passed += 1;
            } else {
                report.failed += 1;
            }
            report.cases.push(CaseResult {
                id: case.id,
                passed,
                trace_parity: parity,
                expected,
                actual,
                trace_actual,
                diagnostics,
            });
        }
        Ok(report)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_trace_or_execution_evidence_is_not_a_pass() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/conformance/cdl_core");
        let source = |file: &str| CoreSource {
            path: file.into(),
            yaml: std::fs::read_to_string(root.join(file)).unwrap(),
        };
        let schema =
            corint_decision_compiler::core::parse_core_input_schema(&source("input-schema.yaml"))
                .unwrap();
        let files = [
            "rule.yaml",
            "ruleset.yaml",
            "pipeline.yaml",
            "registry.yaml",
        ]
        .map(source);
        let engine = DecisionEngine::from_core(&files, schema).unwrap();
        let response = engine
            .decide(
                DecisionRequest::new(HashMap::from([("amount".into(), Value::Number(1001.0))]))
                    .with_trace(),
            )
            .await
            .unwrap();
        assert!(snapshot(response.clone(), true).is_ok());
        assert!(snapshot(response.clone(), false).is_err());
        for field in ["__executed_steps__", "__core_rule_executions__"] {
            let mut broken = response.clone();
            broken.result.context.remove(field);
            assert_eq!(
                snapshot(broken, true).unwrap_err().diagnostic.code,
                "E_TEST_EVIDENCE"
            );
        }
        let mut broken = response;
        broken.trace.as_mut().unwrap().rules_evaluated += 1;
        assert!(snapshot(broken, true).is_err());
    }

    #[test]
    fn unexpected_engine_errors_cannot_impersonate_score_overflow() {
        let error = EngineError::RuntimeError(RuntimeError::InvalidOperation(
            "other error mentioning E_SCORE_OVERFLOW: not an overflow".into(),
        ));
        assert_eq!(engine_error(error).diagnostic.code, "E_ENGINE");
    }
}
