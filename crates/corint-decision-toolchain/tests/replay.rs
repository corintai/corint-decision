use corint_decision_compiler::core::CoreSource;
use corint_decision_engine::Value;
use corint_decision_toolchain::replay::{self, CaptureOptions};
use std::{collections::HashMap, path::PathBuf};

fn fixtures() -> (Vec<CoreSource>, CoreSource) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/cdl_core");
    let read = |name: &str| CoreSource {
        path: name.into(),
        yaml: std::fs::read_to_string(root.join(name)).unwrap(),
    };
    (
        [
            "rule.yaml",
            "ruleset.yaml",
            "pipeline.yaml",
            "registry.yaml",
        ]
        .iter()
        .map(|name| read(name))
        .collect(),
        read("input-schema.yaml"),
    )
}
fn options() -> CaptureOptions {
    CaptureOptions {
        visible_fields: vec!["amount".into()],
        retain_replay_input: true,
        include_trace: true,
    }
}

#[test]
fn records_replay_decisions_and_failures_and_reject_incomplete_or_changed_evidence() {
    let (mut sources, input) = fixtures();
    let event = HashMap::from([("amount".into(), Value::Number(1001.0))]);
    let record = replay::capture(&sources, &input, event.clone(), &options()).unwrap();
    let actual = replay::replay(&sources, &input, &record).unwrap();
    assert_eq!(actual.outcome, record.outcome);
    assert_eq!(record.outcome["result"]["score"], 60);
    let encoded = serde_json::to_string(&record).unwrap();
    let decoded = replay::read(&CoreSource {
        path: "record.json".into(),
        yaml: encoded,
    })
    .unwrap();
    replay::replay(&sources, &input, &decoded).unwrap();
    let mut damaged = record.clone();
    damaged.outcome["result"]["score"] = serde_json::json!(61);
    assert_eq!(
        replay::replay(&sources, &input, &damaged)
            .unwrap_err()
            .diagnostic
            .code,
        "E_REPLAY_DIGEST"
    );
    damaged.outcome = serde_json::json!({});
    assert_eq!(
        replay::replay(&sources, &input, &damaged)
            .unwrap_err()
            .diagnostic
            .code,
        "E_REPLAY_FORMAT"
    );
    let redacted = replay::capture(&sources, &input, event, &CaptureOptions::default()).unwrap();
    assert!(redacted.replay_input.is_none());
    assert_eq!(
        redacted.observed_input["amount"],
        serde_json::json!({"redacted":true})
    );
    assert_eq!(
        replay::replay(&sources, &input, &redacted)
            .unwrap_err()
            .diagnostic
            .code,
        "E_REPLAY_INCOMPLETE"
    );
    let failed = replay::capture(&sources, &input, HashMap::new(), &options()).unwrap();
    assert_eq!(failed.diagnostic.as_ref().unwrap().code, "E_INPUT_SCHEMA");
    replay::replay(&sources, &input, &failed).unwrap();
    let mut arithmetic = sources.clone();
    arithmetic[0].yaml = arithmetic[0]
        .yaml
        .replace("event.amount > 1000", "event.amount / 0 > 1");
    let failed = replay::capture(
        &arithmetic,
        &input,
        HashMap::from([("amount".into(), Value::Number(1001.0))]),
        &options(),
    )
    .unwrap();
    let diagnostic = failed.diagnostic.as_ref().unwrap();
    assert_eq!(diagnostic.code, "E_DIVISION_BY_ZERO");
    assert_eq!(diagnostic.field_path.as_deref(), Some("/rule/when"));
    assert!(diagnostic.line.is_some());
    replay::replay(&arithmetic, &input, &failed).unwrap();
    sources[0].yaml = sources[0].yaml.replace("score: 60", "score: 61");
    assert_eq!(
        replay::replay(&sources, &input, &record)
            .unwrap_err()
            .diagnostic
            .code,
        "E_REPLAY_POLICY"
    );
}

#[test]
fn sdk_capture_rejects_excessive_structure_and_nonfinite_numbers() {
    let (sources, input) = fixtures();
    let mut deep = Value::Null;
    for _ in 0..65 {
        deep = Value::Array(vec![deep]);
    }
    for (value, expected) in [
        (deep, "E_REPLAY_LIMIT"),
        (Value::Array(vec![Value::Null; 10001]), "E_REPLAY_LIMIT"),
        (Value::Number(f64::NAN), "E_REPLAY_FORMAT"),
        (Value::Number(f64::INFINITY), "E_REPLAY_FORMAT"),
    ] {
        assert_eq!(
            replay::capture(
                &sources,
                &input,
                HashMap::from([("amount".into(), value)]),
                &options(),
            )
            .unwrap_err()
            .diagnostic
            .code,
            expected
        );
    }
}

#[test]
fn records_do_not_overwrite_and_cannot_retain_redacted_input() {
    let (sources, input) = fixtures();
    let event = HashMap::from([("amount".into(), Value::Number(1001.0))]);
    let invalid = CaptureOptions {
        retain_replay_input: true,
        ..Default::default()
    };
    assert_eq!(
        replay::capture(&sources, &input, event.clone(), &invalid)
            .unwrap_err()
            .diagnostic
            .code,
        "E_REPLAY_REDACTION"
    );
    let record = replay::capture(&sources, &input, event, &options()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("record.json");
    replay::write(&record, &path).unwrap();
    assert!(replay::write(&record, &path).is_err());
    let bytes = std::fs::read_to_string(path).unwrap();
    let damaged = bytes.replacen(
        "\"format_version\":\"1\"",
        "\"format_version\":\"1\",\"format_version\":\"1\"",
        1,
    );
    assert_eq!(
        replay::read(&CoreSource {
            path: "record.json".into(),
            yaml: damaged
        })
        .unwrap_err()
        .diagnostic
        .code,
        "E_REPLAY_FORMAT"
    );
}
