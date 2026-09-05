//! Thin, offline adapter over the shared Core compiler, never a second validator.
use corint_decision_toolchain::{behavior, package};

use corint_decision_compiler::core::{
    compile_core, diagnostic, parse_core_input_schema, CoreError, CoreSource, PROFILE,
};
use corint_decision_compiler::Diagnostic;
use serde::Serialize;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "corint — offline strict CDL Core validator

Usage:
  corint validate --input-schema PATH [--format text|json] FILE...
  corint test --input-schema PATH --cases PATH [--format text|json] FILE...
  corint build --input-schema PATH --cases PATH --output PATH [--format text|json] FILE...
  corint verify --package PATH --cases PATH [--format text|json]
  corint --help
  corint --version

Supply the complete resource closure, including exactly one Registry.
PATH is the strict model Schema in YAML or JSON; FILEs are CDL YAML resources.
Paths are relative to the current directory. Use -- before dash-prefixed FILEs.
Only explicit local regular files are read. No imports, discovery, network access,
business evaluation or publication is performed. Validate compiles only; test
executes declared cases through the real engine, with trace off/on. Build writes
a new source package after tests pass (never overwrites); verify checks bindings
and reruns the supplied cases. Packages do not embed case inputs or authorization.

JSON reports go to stdout for success and failure; text is the default format.
Exit codes: 0 = command passed, 1 = validation/test failure, 2 = usage or I/O error.
Success is not evidence of business effectiveness or publication approval.
";

#[derive(Default)]
struct Options {
    test: bool,
    build: bool,
    verify: bool,
    output: Option<PathBuf>,
    package: Option<PathBuf>,
    cases: Option<PathBuf>,
    input_schema: Option<PathBuf>,
    sources: Vec<PathBuf>,
}

#[derive(Serialize)]
struct Report {
    report_version: &'static str,
    tool_version: &'static str,
    profile: &'static str,
    scope: &'static str,
    valid: bool,
    execution_checked: bool,
    business_evaluation: &'static str,
    input_schema: Option<String>,
    sources: Vec<String>,
    diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cases_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    test_results: Option<behavior::TestResults>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact: Option<package::Receipt>,
}

fn render_behavior(report: &Report) -> String {
    let mut text = if let Some(results) = &report.test_results {
        format!(
            "{}: {}/{} cases passed (trace off/on); {} failed.\n",
            if report.valid { "PASS" } else { "FAIL" },
            results.passed,
            results.total,
            results.failed
        )
    } else {
        "FAIL: no behavior cases executed.\n".into()
    };
    let diagnostics = report
        .diagnostics
        .iter()
        .chain(report.test_results.iter().flat_map(|r| {
            r.cases
                .iter()
                .filter(|c| !c.passed)
                .flat_map(|c| c.diagnostics.iter())
        }));
    for d in diagnostics {
        text.push_str(&format!(
            "{} [{}] {} {}: {}\n",
            d.code,
            d.stage.as_deref().unwrap_or("unknown"),
            d.source.as_deref().unwrap_or("<unknown>"),
            d.field_path.as_deref().unwrap_or(""),
            d.message
        ));
    }
    text.push_str("Only declared examples were tested; business effectiveness not evaluated; not a publication approval.\n");
    text
}

fn failure(source: &str, stage: &str, code: &str, message: impl Into<String>) -> CoreError {
    diagnostic(source, "", stage, code, message)
}

fn usage(message: impl Into<String>) -> CoreError {
    failure("<command-line>", "usage", "E_USAGE", message)
}

// Keep this bounded grammar dependency-free; reject unknown/duplicate flags.
fn parse_args(args: &[OsString], options: &mut Options) -> Result<(), CoreError> {
    match args.first().and_then(|arg| arg.to_str()) {
        Some("validate") => (),
        Some("test") => options.test = true,
        Some("build") => {
            options.test = true;
            options.build = true;
        }
        Some("verify") => {
            options.test = true;
            options.verify = true;
        }
        _ => {
            return Err(usage(
                "Expected 'validate', 'test', 'build' or 'verify'; use corint --help",
            ))
        }
    };
    let mut args = args[1..].iter();
    let mut format_seen = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--") => {
                options.sources.extend(args.map(PathBuf::from));
                break;
            }
            Some("--input-schema") => {
                if options.verify || options.input_schema.is_some() {
                    return Err(usage("--input-schema may only be supplied once"));
                }
                let value = args
                    .next()
                    .ok_or_else(|| usage("--input-schema needs a path"))?;
                if value.to_string_lossy().starts_with('-') {
                    return Err(usage(
                        "--input-schema needs a path; prefix dash paths with ./",
                    ));
                }
                options.input_schema = Some(value.into());
            }
            Some("--output" | "--package") => {
                let output = arg == "--output";
                if (output && (!options.build || options.output.is_some()))
                    || (!output && (!options.verify || options.package.is_some()))
                {
                    return Err(usage("--output is only for build; --package is only for verify; neither may repeat"));
                }
                let value = args.next().ok_or_else(|| usage("Option needs a path"))?;
                if value.to_string_lossy().starts_with('-') {
                    return Err(usage("Option needs a path; prefix dash paths with ./"));
                }
                if output {
                    options.output = Some(value.into());
                } else {
                    options.package = Some(value.into());
                }
            }
            Some("--cases") => {
                if !options.test || options.cases.is_some() {
                    return Err(usage(
                        "--cases is required once for test/build/verify, not allowed for validate",
                    ));
                }
                let value = args.next().ok_or_else(|| usage("--cases needs a path"))?;
                if value.to_string_lossy().starts_with('-') {
                    return Err(usage("--cases needs a path; prefix dash paths with ./"));
                }
                options.cases = Some(value.into());
            }
            Some("--format") => {
                if format_seen {
                    return Err(usage("--format may only be supplied once"));
                }
                format_seen = true;
                match args.next().and_then(|arg| arg.to_str()) {
                    Some("text" | "json") => (),
                    _ => return Err(usage("--format requires 'text' or 'json'")),
                }
            }
            _ if arg.to_string_lossy().starts_with('-') => {
                return Err(usage(format!("Unknown option: {}", arg.to_string_lossy())));
            }
            _ => options.sources.push(arg.into()),
        }
    }
    if options.verify {
        if options.package.is_none() || options.cases.is_none() || !options.sources.is_empty() {
            return Err(usage(
                "verify requires --package and --cases, with no source files or input schema",
            ));
        }
        return Ok(());
    }
    if options.build && options.output.is_none() {
        return Err(usage("build requires --output for a new package file"));
    }
    if options.input_schema.is_none() {
        return Err(usage(
            "--input-schema is required; fields are never inferred",
        ));
    }
    if options.sources.is_empty() {
        return Err(usage(
            "Supply the complete CDL resource closure as explicit files",
        ));
    }
    if options.test && options.cases.is_none() {
        return Err(usage(
            "--cases is required for test/build; expected results are never inferred",
        ));
    }
    Ok(())
}

fn label(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn read_source(path: &Path) -> Result<(PathBuf, CoreSource), CoreError> {
    let source = label(path);
    let io_error = |e: io::Error| failure(&source, "load", "E_IO", e.to_string());
    let canonical = path.canonicalize().map_err(io_error)?;
    if !canonical.metadata().map_err(io_error)?.is_file() {
        return Err(failure(&source, "load", "E_IO", "Expected a regular file"));
    }
    let yaml = std::fs::read_to_string(&canonical).map_err(io_error)?;
    Ok((canonical, CoreSource { path: source, yaml }))
}

fn load_bundle(options: &Options) -> Result<(Vec<CoreSource>, CoreSource), CoreError> {
    let (_, input) = read_source(options.input_schema.as_ref().expect("checked args"))?;
    parse_core_input_schema(&input)?;
    let mut seen = BTreeSet::new();
    let mut sources = Vec::new();
    for path in &options.sources {
        let (canonical, source) = read_source(path)?;
        if !seen.insert(canonical) {
            return Err(failure(
                &source.path,
                "resolve",
                "E_DUPLICATE_SOURCE",
                "The same source file was supplied more than once (possibly through an alias)",
            ));
        }
        sources.push(source);
    }
    Ok((sources, input))
}

fn render(report: &Report, json: bool) -> String {
    if json {
        return serde_json::to_string_pretty(report).expect("serializable report") + "\n";
    }
    if report.scope == "build" || report.scope == "verify" {
        let mut text = render_behavior(report);
        if let Some(artifact) = &report.artifact {
            text.push_str(&format!(
                "{} package: {}\nPolicy SHA-256: {}\nPackage SHA-256: {}\n",
                if report.scope == "build" {
                    "Built"
                } else {
                    "Verified"
                },
                artifact.path,
                artifact.policy_sha256,
                artifact.package_sha256
            ));
        }
        return text;
    }
    if report.scope == "behavior" {
        return render_behavior(report);
    }
    if report.valid {
        return format!(
            "VALID ({PROFILE}): {} source files compiled.\nExecution not checked; business effectiveness not evaluated; not a publication approval.\n",
            report.sources.len()
        );
    }
    let mut output = format!("INVALID ({PROFILE})\n");
    for d in &report.diagnostics {
        let location = match (d.line, d.column) {
            (Some(line), Some(column)) => format!(":{line}:{column}"),
            _ => String::new(),
        };
        output.push_str(&format!(
            "{} [{}] {}{} {}: {}\n",
            d.code,
            d.stage.as_deref().unwrap_or("unknown"),
            d.source.as_deref().unwrap_or("<unknown>"),
            location,
            d.field_path.as_deref().unwrap_or(""),
            d.message
        ));
    }
    output
}

fn run(args: Vec<OsString>) -> (u8, String) {
    if args == [OsString::from("--help")]
        || args == [OsString::from("-h")]
        || args == [OsString::from("validate"), OsString::from("--help")]
        || args == [OsString::from("test"), OsString::from("--help")]
        || args == [OsString::from("build"), OsString::from("--help")]
        || args == [OsString::from("verify"), OsString::from("--help")]
    {
        return (0, HELP.into());
    }
    if args == [OsString::from("--version")] {
        return (
            0,
            format!("corint {} ({PROFILE})\n", env!("CARGO_PKG_VERSION")),
        );
    }
    // Honor an explicit JSON request even when another argument is invalid.
    // Do not interpret filenames after the option terminator as flags.
    let option_args: Vec<_> = args.iter().take_while(|arg| *arg != "--").collect();
    let json = option_args
        .windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json");
    let mut options = Options::default();
    let mut test_results = None;
    let mut artifact = None;
    let result = parse_args(&args, &mut options).and_then(|()| {
        if options.verify {
            let path = options.package.as_ref().expect("checked args");
            let (_, stored) = read_source(path)?;
            let (_, suite) = read_source(options.cases.as_ref().expect("checked args"))?;
            let checked = package::verify(&stored, &suite)?;
            test_results = Some(checked.tests);
            artifact = checked.receipt;
            if let Some(error) = checked.error {
                return Err(error);
            }
            return Ok(());
        }
        if options.build {
            package::check_output(options.output.as_ref().expect("checked args"))?;
        }
        let (sources, input) = load_bundle(&options)?;
        let schema = parse_core_input_schema(&input)?;
        if options.test {
            let (_, suite) = read_source(options.cases.as_ref().expect("checked args"))?;
            if options.build {
                let (package, tests) = package::prepare(&sources, &input, &suite)?;
                test_results = Some(tests);
                if let Some(package) = package {
                    artifact = Some(package::write(
                        &package,
                        options.output.as_ref().expect("checked args"),
                    )?);
                }
            } else {
                test_results = Some(behavior::test(&sources, schema, &suite)?);
            }
        } else {
            compile_core(&sources, schema)?;
        }
        Ok(())
    });
    let mut report = Report {
        report_version: "1",
        tool_version: env!("CARGO_PKG_VERSION"),
        profile: PROFILE,
        scope: if options.build {
            "build"
        } else if options.verify {
            "verify"
        } else if options.test {
            "behavior"
        } else {
            "compile"
        },
        valid: result.is_ok() && test_results.as_ref().is_none_or(|r| r.failed == 0),
        execution_checked: test_results.as_ref().is_some_and(|r| r.executed > 0),
        business_evaluation: "not_performed",
        input_schema: options.input_schema.as_deref().map(label),
        sources: options.sources.iter().map(|p| label(p)).collect(),
        diagnostics: Vec::new(),
        cases_file: options.cases.as_deref().map(label),
        test_results,
        artifact,
    };
    let exit = match result {
        Ok(()) => {
            if report.valid {
                0
            } else {
                1
            }
        }
        Err(error) => {
            let exit = if matches!(
                error.diagnostic.stage.as_deref(),
                Some("usage" | "load" | "write")
            ) {
                2
            } else {
                1
            };
            report.diagnostics.push(*error.diagnostic);
            exit
        }
    };
    (exit, render(&report, json))
}

fn main() -> ExitCode {
    let (code, output) = run(std::env::args_os().skip(1).collect());
    if let Err(error) = io::stdout().lock().write_all(output.as_bytes()) {
        let _ = writeln!(io::stderr().lock(), "E_IO [output]: {error}");
        return ExitCode::from(2);
    }
    ExitCode::from(code)
}
