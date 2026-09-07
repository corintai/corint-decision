use super::{read_source, usage};
use corint_decision_compiler::core::CoreError;
use corint_decision_toolchain::{replay, transfer};
use serde_json::json;
use std::{collections::BTreeMap, ffi::OsString, path::Path};

pub const HELP: &str = "corint record --bundle PATH --event PATH --output NEW_PATH [--visible-fields a,b] [--retain-input] [--trace] [--format text|json]\ncorint replay --bundle PATH --record PATH [--format text|json]\nEvent is a JSON object of event fields, without an event wrapper.\nRecords are redacted by default. Retention requires all fields to be explicitly visible.\nReplay runs the same executable and frozen Core sources without external I/O or actions.\n";

pub fn run(args: &[OsString]) -> (u8, String) {
    if args.get(1).is_some_and(|a| a == "--help") && args.len() == 2 {
        return (0, HELP.into());
    }
    let mut report = json!({"report_version":"1", "scope":"offline_decision_replay", "valid":false, "diagnostics":[], "authenticity":"unsigned", "publication_approval":"not_granted"});
    let json_output = args
        .windows(2)
        .any(|p| p[0] == "--format" && p[1] == "json");
    let result = (|| -> Result<(), CoreError> {
        let recording = args[0] == "record";
        let mut values = BTreeMap::new();
        let mut retain = false;
        let mut trace = false;
        let mut rest = args[1..].iter();
        while let Some(arg) = rest.next() {
            let key = arg
                .to_str()
                .ok_or_else(|| usage("Arguments must be UTF-8"))?;
            if recording && ["--retain-input", "--trace"].contains(&key) {
                let flag = if key == "--trace" {
                    &mut trace
                } else {
                    &mut retain
                };
                if *flag {
                    return Err(usage("Duplicate flag"));
                }
                *flag = true;
                continue;
            }
            let allowed = if recording {
                vec![
                    "--bundle",
                    "--event",
                    "--output",
                    "--visible-fields",
                    "--format",
                ]
            } else {
                vec!["--bundle", "--record", "--format"]
            };
            if !allowed.contains(&key) {
                return Err(usage(format!("Unknown option: {key}")));
            }
            let value = rest
                .next()
                .and_then(|s| s.to_str())
                .ok_or_else(|| usage("Missing option value"))?;
            if value.starts_with('-') || values.insert(key, value).is_some() {
                return Err(usage("Invalid or duplicate option"));
            }
        }
        if values
            .get("--format")
            .is_some_and(|v| !["text", "json"].contains(v))
        {
            return Err(usage("Invalid format"));
        }
        let required = |key: &str| {
            values
                .get(key)
                .copied()
                .ok_or_else(|| usage(format!("Missing {key}")))
        };
        let bundle = transfer::read_bundle(&read_source(Path::new(required("--bundle")?))?.1)?;
        if recording {
            let event = read_source(Path::new(required("--event")?))?.1;
            let event = serde_json::from_str(&event.yaml)
                .map_err(|_| usage("Event must be a JSON object"))?;
            let fields: Vec<String> = values
                .get("--visible-fields")
                .map(|v| v.split(',').map(str::to_string).collect())
                .unwrap_or_default();
            if fields.iter().any(|s| s.is_empty()) {
                return Err(usage("Empty visible field"));
            }
            let options = replay::CaptureOptions {
                visible_fields: fields,
                retain_replay_input: retain,
                include_trace: trace,
            };
            let record = replay::capture(&bundle.sources, &bundle.input_schema, event, &options)?;
            replay::write(&record, Path::new(required("--output")?))?;
            report["record_sha256"] = json!(record.record_sha256);
            report["decision_succeeded"] = json!(record.diagnostic.is_none());
            report["replayable"] = json!(record.replay_input.is_some());
        } else {
            let record = replay::read(&read_source(Path::new(required("--record")?))?.1)?;
            replay::replay(&bundle.sources, &bundle.input_schema, &record)?;
            report["matched"] = json!(true);
        }
        report["valid"] = json!(true);
        Ok(())
    })();
    let code = match result {
        Ok(()) => 0,
        Err(e) => {
            let code = if matches!(
                e.diagnostic.code.as_str(),
                "E_USAGE" | "E_IO" | "E_OUTPUT_EXISTS"
            ) {
                2
            } else {
                1
            };
            report["diagnostics"] = json!([e.diagnostic]);
            code
        }
    };
    let rendered = if json_output {
        serde_json::to_string_pretty(&report).unwrap()
    } else {
        format!(
            "{}\n{}",
            if code == 0 { "PASS" } else { "FAIL" },
            serde_json::to_string_pretty(&report).unwrap()
        )
    };
    (code, rendered + "\n")
}
