//! Agent-friendly static authoring adapter; all checks live in the shared toolchain.
use corint_decision_toolchain::authoring::{self, Options, Report};
use std::ffi::OsString;
use std::path::PathBuf;

pub fn run(args: &[OsString]) -> (u8, String) {
    let json = args
        .iter()
        .take_while(|arg| *arg != "--")
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json");
    let mut options = Options::default();
    let mut report = Report::empty();
    let mut iter = args.iter();
    let mut positional = false;
    let mut seen = std::collections::BTreeSet::new();
    while let Some(arg) = iter.next() {
        if positional {
            options.files.push(PathBuf::from(arg));
            continue;
        }
        match arg.to_str() {
            Some("--") => positional = true,
            Some("--root" | "--input-schema" | "--format" | "--profile") => {
                if !seen.insert(arg.clone()) {
                    report.error(
                        "",
                        "",
                        "usage",
                        "E_ARGUMENT",
                        format!("Repeated {}", arg.to_string_lossy()),
                    );
                }
                let Some(value) = iter.next() else {
                    report.error(
                        "",
                        "",
                        "usage",
                        "E_ARGUMENT",
                        format!("Missing value for {}", arg.to_string_lossy()),
                    );
                    break;
                };
                match arg.to_str().unwrap() {
                    "--root" => options.root = Some(value.into()),
                    "--input-schema" => options.input_schema = Some(value.into()),
                    "--format" if value != "text" && value != "json" => {
                        report.error("", "", "usage", "E_ARGUMENT", "Format must be text or json")
                    }
                    "--profile" if value != authoring::PROFILE => report.error(
                        "",
                        "",
                        "usage",
                        "E_ARGUMENT",
                        "Profile must be cdl-static-1 or cdl-core-risk-draft-1",
                    ),
                    _ => (),
                }
            }
            Some(text) if text.starts_with('-') => report.error(
                "",
                "",
                "usage",
                "E_ARGUMENT",
                format!("Unknown option: {text}"),
            ),
            _ => options.files.push(arg.into()),
        }
    }
    if report.diagnostics.is_empty() {
        report = authoring::validate(&options);
    }
    let code = report.exit_code();
    let output = if json {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report).expect("serializable static report")
        )
    } else {
        let mut text = format!(
            "CDL static validation: {} ({} files)\n",
            if report.valid { "PASS" } else { "FAIL" },
            report.sources.len()
        );
        for diagnostic in &report.diagnostics {
            text.push_str(&format!(
                "{} {}{} [{}]: {}\n",
                diagnostic.source.as_deref().unwrap_or(""),
                diagnostic.field_path.as_deref().unwrap_or(""),
                diagnostic
                    .line
                    .map(|line| format!(" (line {line})"))
                    .unwrap_or_default(),
                diagnostic.code,
                diagnostic.message
            ));
        }
        text.push_str(&format!(
            "References checked: {}; input Schema checked: {}; execution checked: false\n",
            report.references_checked, report.input_schema_checked
        ));
        text
    };
    (code, output)
}
