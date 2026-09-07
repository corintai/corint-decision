//! Content-bound decision records and offline replay through the real Core engine.
//! Hashes detect changes; records are unsigned and never publication authority.
use crate::{behavior, failure, package};
use corint_decision_compiler::{
    core::{parse_core_input_schema, CoreError, CoreSource, PROFILE},
    Diagnostic,
};
use corint_decision_engine::{DecisionEngine, DecisionRequest, EngineError, Value};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, HashMap};

pub const MAX_RECORD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRecord {
    pub format_version: String,
    pub profile: String,
    pub policy_sha256: String,
    pub engine_sha256: String,
    pub input_sha256: String,
    /// Omitted when any top-level field is redacted; never replay partial input.
    pub replay_input: Option<HashMap<String, Value>>,
    pub observed_input: BTreeMap<String, Json>,
    pub outcome: Json,
    pub diagnostic: Option<Diagnostic>,
    pub condition_trace: Option<Json>,
    pub call_trace: Option<Json>,
    pub record_sha256: String,
}

#[derive(Debug, Default)]
pub struct CaptureOptions {
    /// Explicit opt-in: empty allowlist emits only redaction markers.
    pub visible_fields: Vec<String>,
    pub retain_replay_input: bool,
    pub include_trace: bool,
}

fn error(code: &str, message: &str) -> CoreError {
    failure("<replay>", "replay", code, message)
}
fn identity(record: &DecisionRecord) -> String {
    let mut value = serde_json::to_value(record).expect("record serialization");
    value.as_object_mut().unwrap().remove("record_sha256");
    package::json_hash("decision-record-v1", value)
}
fn bounded(record: &DecisionRecord) -> Result<(), CoreError> {
    if serde_json::to_vec(record)
        .map_err(|_| error("E_REPLAY_FORMAT", "Invalid record"))?
        .len()
        > MAX_RECORD_BYTES
    {
        return Err(error("E_REPLAY_LIMIT", "Decision record exceeds 8 MiB"));
    }
    Ok(())
}
fn engine_error(error: EngineError) -> CoreError {
    match error {
        EngineError::Core(error) => error,
        _ => failure("<engine>", "compile", "E_ENGINE", error.to_string()),
    }
}

/// Records both successful decisions and structured execution/input failures.
/// Call from a blocking worker when the caller already owns a Tokio runtime.
pub fn capture(
    sources: &[CoreSource],
    input: &CoreSource,
    event: HashMap<String, Value>,
    options: &CaptureOptions,
) -> Result<DecisionRecord, CoreError> {
    // SDK values may be deeper than JSON's parser limit. Bound traversal before
    // serializing malformed requests for an error record.
    if event.len() > 10000 {
        return Err(error("E_REPLAY_LIMIT", "Too many input fields"));
    }
    let mut pending: Vec<_> = event.values().map(|v| (v, 1usize)).collect();
    let mut nodes = 0usize;
    let mut bytes: usize = event.keys().map(String::len).sum();
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        if depth > 64 || nodes > 10000 || bytes > MAX_RECORD_BYTES / 2 {
            return Err(error(
                "E_REPLAY_LIMIT",
                "Input structure exceeds capture limits",
            ));
        }
        match value {
            Value::Object(fields) => {
                if fields.len() > 10000 {
                    return Err(error("E_REPLAY_LIMIT", "Too many input fields"));
                }
                bytes = bytes.saturating_add(fields.keys().map(String::len).sum::<usize>());
                pending.extend(fields.values().map(|v| (v, depth + 1)));
            }
            Value::Array(values) => {
                if values.len() > 10000 {
                    return Err(error("E_REPLAY_LIMIT", "Too many input elements"));
                }
                pending.extend(values.iter().map(|v| (v, depth + 1)));
            }
            Value::String(s) => bytes = bytes.saturating_add(s.len()),
            Value::Number(n) if !n.is_finite() => {
                return Err(error(
                    "E_REPLAY_FORMAT",
                    "Nonfinite values cannot be recorded as JSON",
                ))
            }
            _ => (),
        }
        if pending.len() > 10000 {
            return Err(error(
                "E_REPLAY_LIMIT",
                "Input structure exceeds capture limits",
            ));
        }
    }
    let input_json =
        serde_json::to_value(&event).map_err(|_| error("E_REPLAY_FORMAT", "Invalid event"))?;
    if serde_json::to_vec(&input_json).unwrap().len() > MAX_RECORD_BYTES / 2 {
        return Err(error("E_REPLAY_LIMIT", "Capture input exceeds 4 MiB"));
    }
    let input_sha256 = package::json_hash("decision-input-v1", input_json);
    let all_visible = event.keys().all(|k| options.visible_fields.contains(k));
    if options.retain_replay_input && !all_visible {
        return Err(error(
            "E_REPLAY_REDACTION",
            "Replay input retention requires every input field to be explicitly visible",
        ));
    }
    let observed_input = event
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                if options.visible_fields.contains(k) {
                    serde_json::to_value(v).unwrap()
                } else {
                    json!({"redacted":true})
                },
            )
        })
        .collect();
    let engine = DecisionEngine::from_core(sources, parse_core_input_schema(input)?)
        .map_err(engine_error)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| failure("<replay>", "load", "E_IO", e.to_string()))?;
    let request = DecisionRequest::new(event.clone());
    let response = runtime.block_on(engine.decide(if options.include_trace {
        request.with_trace()
    } else {
        request
    }));
    let (conditions, calls) = response
        .as_ref()
        .ok()
        .and_then(|r| r.trace.as_ref())
        .map(|trace| {
            (
                trace
                    .core_conditions_v1
                    .as_ref()
                    .and_then(|v| serde_json::to_value(v).ok()),
                trace
                    .core_calls_v1
                    .as_ref()
                    .and_then(|v| serde_json::to_value(v).ok()),
            )
        })
        .unwrap_or_default();
    let (outcome, mut diagnostic) = behavior::outcome(response, options.include_trace);
    if let Some(diagnostic) = &mut diagnostic {
        // A Rule has one condition and one score. Locate these unambiguously;
        // never guess a clause inside a multi-branch Pipeline or Registry.
        if diagnostic.field_path.as_deref().is_none_or(str::is_empty) {
            if let Some(source) = sources
                .iter()
                .find(|s| Some(s.path.as_str()) == diagnostic.source.as_deref())
            {
                let document: Json = serde_yaml::from_str(&source.yaml).expect("compiled source");
                let field = match diagnostic.code.as_str() {
                    "E_DIVISION_BY_ZERO" | "E_NUMBER_OVERFLOW" => Some("when"),
                    "E_SCORE_OVERFLOW" => Some("score"),
                    _ => None,
                };
                if let Some(field) = field.filter(|_| document.get("rule").is_some()) {
                    diagnostic.field_path = Some(format!("/rule/{field}"));
                    let matches: Vec<_> = source
                        .yaml
                        .lines()
                        .enumerate()
                        .filter(|(_, line)| line.trim_start().starts_with(&format!("{field}:")))
                        .collect();
                    if let [(line, text)] = matches.as_slice() {
                        diagnostic.line = Some(line + 1);
                        diagnostic.column = Some(text.len() - text.trim_start().len() + 1);
                    }
                }
            }
        }
        // Runtime messages may contain values; preserve machine-readable location only.
        diagnostic.message = "Decision failed; inspect stage, code and source location".into();
        diagnostic.context = None;
    }
    let mut record = DecisionRecord {
        format_version: "1".into(),
        profile: PROFILE.into(),
        policy_sha256: package::policy_identity(sources, input)?,
        engine_sha256: package::checker_identity()?.1,
        input_sha256,
        replay_input: options.retain_replay_input.then_some(event),
        observed_input,
        outcome,
        diagnostic,
        condition_trace: conditions,
        call_trace: calls,
        record_sha256: String::new(),
    };
    record.record_sha256 = identity(&record);
    bounded(&record)?;
    Ok(record)
}

pub fn read(source: &CoreSource) -> Result<DecisionRecord, CoreError> {
    if source.yaml.len() > MAX_RECORD_BYTES {
        return Err(error("E_REPLAY_LIMIT", "Decision record exceeds 8 MiB"));
    }
    let record: DecisionRecord = serde_json::from_str(&source.yaml)
        .map_err(|_| error("E_REPLAY_FORMAT", "Invalid decision record JSON"))?;
    verify(&record)?;
    Ok(record)
}

pub fn write(record: &DecisionRecord, path: &std::path::Path) -> Result<(), CoreError> {
    verify(record)?;
    package::write_bytes(&serde_json::to_vec(record).expect("record JSON"), path)
}

fn verify(record: &DecisionRecord) -> Result<(), CoreError> {
    bounded(record)?;
    if record.format_version != "1" || record.profile != PROFILE {
        return Err(error(
            "E_REPLAY_VERSION",
            "Unsupported record version or profile",
        ));
    }

    let schema = jsonschema::JSONSchema::compile(
        &serde_json::from_str(include_str!(
            "../../../docs/contracts/schema/replay-record.json"
        ))
        .expect("record schema JSON"),
    )
    .expect("record schema");
    if !schema.is_valid(&serde_json::to_value(record).expect("record JSON")) {
        return Err(error(
            "E_REPLAY_FORMAT",
            "Record does not conform to the versioned schema",
        ));
    }
    if identity(record) != record.record_sha256 {
        return Err(error("E_REPLAY_DIGEST", "Record content changed"));
    }
    Ok(())
}

/// Re-evaluates recorded input without reading source files, databases or actions.
/// Missing input, changed policy, changed engine and changed results are distinct errors.
pub fn replay(
    sources: &[CoreSource],
    input: &CoreSource,
    record: &DecisionRecord,
) -> Result<DecisionRecord, CoreError> {
    verify(record)?;
    if record.policy_sha256 != package::policy_identity(sources, input)? {
        return Err(error("E_REPLAY_POLICY", "Policy or input schema changed"));
    }
    if record.engine_sha256 != package::checker_identity()?.1 {
        return Err(error(
            "E_REPLAY_ENGINE",
            "Replay requires the recorded executable",
        ));
    }
    let event = record.replay_input.clone().ok_or_else(|| {
        error(
            "E_REPLAY_INCOMPLETE",
            "Record has redacted or omitted input",
        )
    })?;
    if package::json_hash("decision-input-v1", serde_json::to_value(&event).unwrap())
        != record.input_sha256
    {
        return Err(error("E_REPLAY_INPUT", "Replay input changed"));
    }
    let options = CaptureOptions {
        visible_fields: event.keys().cloned().collect(),
        retain_replay_input: true,
        include_trace: record.condition_trace.is_some() || record.call_trace.is_some(),
    };
    let actual = capture(sources, input, event, &options)?;
    if actual.outcome != record.outcome
        || serde_json::to_value(&actual.diagnostic).unwrap()
            != serde_json::to_value(&record.diagnostic).unwrap()
        || actual.condition_trace != record.condition_trace
        || actual.call_trace != record.call_trace
    {
        return Err(error(
            "E_REPLAY_MISMATCH",
            "Decision, diagnostic or execution trace differs",
        ));
    }
    Ok(actual)
}
