//! Public Agent/human adapter for candidate repositories. No network or approval.
use super::{read_source, usage};
use corint_decision_compiler::core::CoreError;
use corint_decision_toolchain::{candidate, contracts};
use serde_json::json;
use std::{collections::BTreeMap, ffi::OsString, path::Path};

pub const HELP: &str = "corint prepare-repository --root DIR --input-schema LABEL --cases PATH --context PATH --target PATH --revision REV --output NEW_DIR [--format text|json] ENTRY...\n\nFreeze, strictly validate, check target declarations and execute behavior cases.\nWrite original sources, publication.json and published.json into a NEW candidate directory.\nNo active repository is modified, no server is contacted, no approval is granted.\nInput and entry labels are relative to --root; other paths are relative to cwd.\n";

pub fn run(args: &[OsString]) -> (u8, String) {
    if args == [OsString::from("--help")] {
        return (0, HELP.into());
    }
    let json_output = args
        .iter()
        .take_while(|arg| *arg != "--")
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json");
    let mut report = json!({"report_version":"1", "scope":"repository_candidate", "valid":false,
        "tool_version":env!("CARGO_PKG_VERSION"), "profile":corint_decision_compiler::core::PROFILE,
        "execution_checked":false, "business_evaluation":"not_performed",
        "publication_approval":"not_granted", "activated":false, "diagnostics":[]});
    let result = (|| -> Result<(), CoreError> {
        let mut options = BTreeMap::new();
        let mut entries = Vec::new();
        let mut rest = args.iter();
        let mut positional = false;
        while let Some(arg) = rest.next() {
            let arg = arg
                .to_str()
                .ok_or_else(|| usage("Arguments must be UTF-8"))?;
            if !positional && arg == "--" {
                positional = true;
                continue;
            }
            if !positional && arg.starts_with('-') {
                if ![
                    "--root",
                    "--input-schema",
                    "--cases",
                    "--context",
                    "--target",
                    "--revision",
                    "--output",
                    "--format",
                ]
                .contains(&arg)
                {
                    return Err(usage(format!("Unknown option: {arg}")));
                }
                let value = rest
                    .next()
                    .and_then(|arg| arg.to_str())
                    .ok_or_else(|| usage("Option requires a value"))?;
                if value.starts_with('-') || options.insert(arg, value).is_some() {
                    return Err(usage("Options require a value and may not repeat"));
                }
            } else {
                entries.push(arg.to_string());
            }
        }
        if options
            .get("--format")
            .is_some_and(|v| !["json", "text"].contains(v))
        {
            return Err(usage("--format requires text or json"));
        }
        for required in [
            "--root",
            "--input-schema",
            "--cases",
            "--context",
            "--target",
            "--revision",
            "--output",
        ] {
            if !options.contains_key(required) {
                return Err(usage(format!("Missing {required}")));
            }
        }
        if entries.is_empty() {
            return Err(usage("Supply at least one entry label"));
        }
        let output = Path::new(options["--output"]);
        if output.symlink_metadata().is_ok() {
            return Err(super::failure(
                &output.display().to_string(),
                "write",
                "E_IO",
                "Candidate output must not exist",
            ));
        }
        let (_, cases) = read_source(Path::new(options["--cases"]))?;
        let (_, context) = read_source(Path::new(options["--context"]))?;
        let (_, target) = read_source(Path::new(options["--target"]))?;
        let target = contracts::TargetContracts::load(&context, &target)?;
        let candidate = candidate::prepare(
            Path::new(options["--root"]),
            options["--input-schema"],
            &entries,
            options["--revision"],
            &cases,
            &target,
        )?;
        report["execution_checked"] = json!(candidate.tests().executed > 0);
        report["test_results"] = json!(candidate.tests());
        report["compatibility"] = json!(candidate.compatibility());
        report["evidence_subject"] = contracts::core_evidence_subject(candidate.compatibility())?;
        report["candidate"] = json!(candidate.write(output)?);
        report["valid"] = json!(true);
        Ok(())
    })();
    let exit = if let Err(error) = result {
        let exit = if matches!(
            error.diagnostic.stage.as_deref(),
            Some("usage" | "load" | "write")
        ) {
            2
        } else {
            1
        };
        if error.diagnostic.code == "E_CANDIDATE_BEHAVIOR" {
            report["execution_checked"] = json!(true);
        }
        report["diagnostics"] = json!([*error.diagnostic]);
        exit
    } else {
        0
    };
    let output = if json_output {
        serde_json::to_string(&report).expect("report JSON")
    } else if exit == 0 {
        "PASS: candidate repository prepared; publication approval not granted; not activated"
            .into()
    } else {
        format!("FAIL: {}", report["diagnostics"])
    };
    (exit, output + "\n")
}
