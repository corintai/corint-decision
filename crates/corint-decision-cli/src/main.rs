//! Thin, offline adapter over the shared Core compiler, never a second validator.
use corint_decision_toolchain::{behavior, contracts, package, resolve, transfer};

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
  corint export --package PATH --output PATH [--format text|json]
  corint import --bundle PATH --cases PATH --output PATH [--format text|json]
  corint check-target --input-schema PATH --context PATH --target PATH [--expected-binding SHA256] [--format text|json] FILE...
  corint resolve --source-profile cdl-core-import-draft-1 --root DIR --input-schema LABEL --output PATH [--format text|json] ENTRY...
  corint --help
  corint --version

Supply the complete resource closure, including exactly one Registry.
PATH is the strict model Schema in YAML or JSON; FILEs are CDL YAML resources.
Paths are relative to the current directory. Use -- before dash-prefixed FILEs.
Except resolve, only explicit local regular files are read. No imports, discovery, network access,
business evaluation or publication is performed. Validate compiles only; test
executes declared cases through the real engine, with trace off/on. Build writes
a new source package after tests pass (never overwrites); verify checks bindings
and reruns the supplied cases. Packages do not embed case inputs or authorization.
Export writes an editable JSON source bundle without historical evidence; it
checks content bindings and compilation only. Import retests that bundle with
caller-owned cases and builds fresh evidence under this host. Neither command
extracts embedded paths to disk, trusts old evidence or activates a policy.
Check-target checks source compatibility with explicit context/target declarations.
It does not execute cases, contact the target, authenticate declarations or approve
publication. --expected-binding rejects a stale policy/contract/checker binding.
Resolve is an opt-in authoring step: it reads root-relative imports on Unix,
rejects symlinks and escapes, and writes a frozen v1 source bundle without tests.
It does not enable imports in existing draft-1 validators, generators or servers.

JSON reports go to stdout for success and failure; text is the default format.
Exit codes: 0 = command passed, 1 = validation/test failure, 2 = usage or I/O error.
Success is not evidence of business effectiveness or publication approval.
";

#[derive(Default)]
struct Options {
    test: bool,
    build: bool,
    verify: bool,
    export: bool,
    import: bool,
    check_target: bool,
    resolve: bool,
    root: Option<PathBuf>,
    source_profile: Option<String>,
    context: Option<PathBuf>,
    target: Option<PathBuf>,
    expected_binding: Option<String>,
    output: Option<PathBuf>,
    package: Option<PathBuf>,
    bundle: Option<PathBuf>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    exported_bundle: Option<transfer::ExportReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<contracts::CompatibilityReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolution: Option<resolve::ResolutionReceipt>,
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
        Some("resolve") => options.resolve = true,
        Some("check-target") => options.check_target = true,
        Some("test") => options.test = true,
        Some("build") => {
            options.test = true;
            options.build = true;
        }
        Some("verify") => {
            options.test = true;
            options.verify = true;
        }
        Some("export") => options.export = true,
        Some("import") => {
            options.test = true;
            options.import = true;
        }
        _ => {
            return Err(usage(
                "Expected validate, test, build, verify, export, import, check-target or resolve; use corint --help",
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
                if options.verify
                    || options.export
                    || options.import
                    || options.input_schema.is_some()
                {
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
                if (output
                    && (!(options.build || options.export || options.import || options.resolve)
                        || options.output.is_some()))
                    || (!output
                        && (!(options.verify || options.export) || options.package.is_some()))
                {
                    return Err(usage("--output is only for build/export/import/resolve; --package is only for verify/export; neither may repeat"));
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
            Some("--context" | "--target" | "--expected-binding") => {
                if !options.check_target {
                    return Err(usage(
                        "Context, target and expected binding are only accepted by check-target",
                    ));
                }
                let value = args
                    .next()
                    .ok_or_else(|| usage("Option requires a value"))?;
                if value.to_string_lossy().starts_with('-') {
                    return Err(usage("Option requires a value; prefix dash paths with ./"));
                }
                match arg.to_str().expect("known option") {
                    "--context" if options.context.is_none() => {
                        options.context = Some(value.into())
                    }
                    "--target" if options.target.is_none() => options.target = Some(value.into()),
                    "--expected-binding" if options.expected_binding.is_none() => {
                        let hash = value
                            .to_str()
                            .ok_or_else(|| usage("Binding must be hexadecimal"))?;
                        if hash.len() != 64
                            || !hash
                                .bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                        {
                            return Err(usage(
                                "Binding must be 64 lowercase hexadecimal characters",
                            ));
                        }
                        options.expected_binding = Some(hash.into());
                    }
                    _ => {
                        return Err(usage(
                            "Context, target and expected binding options may not repeat",
                        ))
                    }
                }
            }
            Some("--root" | "--source-profile") => {
                if !options.resolve {
                    return Err(usage("--root and --source-profile are only for resolve"));
                }
                let value = args
                    .next()
                    .ok_or_else(|| usage("Option requires a value"))?;
                if value.to_string_lossy().starts_with('-') {
                    return Err(usage("Option requires a value"));
                }
                if arg == "--root" && options.root.is_none() {
                    options.root = Some(value.into());
                } else if arg == "--source-profile" && options.source_profile.is_none() {
                    if value != resolve::SOURCE_PROFILE {
                        return Err(usage("Unsupported source profile"));
                    }
                    options.source_profile = Some(resolve::SOURCE_PROFILE.into());
                } else {
                    return Err(usage("Resolver options may not repeat"));
                }
            }
            Some("--bundle") => {
                if !options.import || options.bundle.is_some() {
                    return Err(usage("--bundle is only for import and may not repeat"));
                }
                let value = args.next().ok_or_else(|| usage("--bundle needs a path"))?;
                if value.to_string_lossy().starts_with('-') {
                    return Err(usage("--bundle needs a path; prefix dash paths with ./"));
                }
                options.bundle = Some(value.into());
            }
            Some("--cases") => {
                if !options.test || options.cases.is_some() {
                    return Err(usage(
                        "--cases is required once for test/build/verify/import, not allowed for validate/export",
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
    if options.export || options.import {
        let input_present = if options.export {
            options.package.is_some()
        } else {
            options.bundle.is_some()
        };
        if !input_present
            || options.output.is_none()
            || !options.sources.is_empty()
            || (options.import && options.cases.is_none())
        {
            return Err(usage("export requires --package and --output; import requires --bundle, --cases and --output; neither accepts source files"));
        }
        return Ok(());
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
    if options.resolve
        && (options.root.is_none() || options.source_profile.is_none() || options.output.is_none())
    {
        return Err(usage(
            "resolve requires --root, --source-profile and --output",
        ));
    }
    if options.check_target && (options.context.is_none() || options.target.is_none()) {
        return Err(usage("check-target requires --context and --target"));
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
    if report.scope == "export" {
        if let Some(bundle) = &report.exported_bundle {
            return format!("EXPORTED: {} ({} sources)\nBundle SHA-256: {}\nSource bindings and compilation checked; execution and historical evidence NOT checked.\nNo evidence or publication approval exported.\n",
                bundle.path, bundle.source_count, bundle.bundle_sha256);
        }
        // Failures use the normal diagnostic renderer below, not a success banner.
    }
    if report.scope == "build" || report.scope == "verify" || report.scope == "import" {
        let mut text = render_behavior(report);
        if let Some(artifact) = &report.artifact {
            text.push_str(&format!(
                "{} package: {}\nPolicy SHA-256: {}\nPackage SHA-256: {}\n",
                if report.scope == "verify" {
                    "Verified"
                } else {
                    "Built"
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
        if let Some(resolution) = &report.resolution {
            return format!("RESOLVED: {} files into frozen Core source bundle.\nResolution SHA-256: {}\nExecution not checked; no publication approval.\n", resolution.manifest.sources.len(), resolution.resolution_sha256);
        }
        if let Some(check) = &report.compatibility {
            return format!("COMPATIBLE with declared target {}\nBinding SHA-256: {}\nExecution, business semantics, live target and authorization NOT verified; not a publication approval.\n",
                check.target.id, check.binding_sha256);
        }
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
        || args == [OsString::from("export"), OsString::from("--help")]
        || args == [OsString::from("import"), OsString::from("--help")]
        || args == [OsString::from("check-target"), OsString::from("--help")]
        || args == [OsString::from("resolve"), OsString::from("--help")]
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
    let mut exported_bundle = None;
    let mut compatibility = None;
    let mut resolution = None;
    let result = parse_args(&args, &mut options).and_then(|()| {
        if options.resolve {
            let output = options.output.as_ref().expect("checked args");
            package::check_output(output)?;
            let resolved = resolve::resolve(
                options.root.as_deref().expect("checked args"),
                options
                    .input_schema
                    .as_ref()
                    .and_then(|p| p.to_str())
                    .ok_or_else(|| usage("Input label must be UTF-8"))?,
                &options
                    .sources
                    .iter()
                    .map(|p| {
                        p.to_str()
                            .map(str::to_owned)
                            .ok_or_else(|| usage("Entry labels must be UTF-8"))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )?;
            resolved.write(output)?;
            resolution = Some(resolved.into_receipt());
            return Ok(());
        }
        if options.export {
            let (_, stored) = read_source(options.package.as_ref().expect("checked args"))?;
            exported_bundle = Some(transfer::export(
                &stored,
                options.output.as_ref().expect("checked args"),
            )?);
            return Ok(());
        }
        if options.import {
            let output = options.output.as_ref().expect("checked args");
            package::check_output(output)?;
            let (_, stored) = read_source(options.bundle.as_ref().expect("checked args"))?;
            let (_, suite) = read_source(options.cases.as_ref().expect("checked args"))?;
            let (package, tests) = transfer::import_sources(&stored, &suite)?;
            test_results = Some(tests);
            if let Some(package) = package {
                artifact = Some(package::write(&package, output)?);
            }
            return Ok(());
        }
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
        if options.check_target {
            let (_, context) = read_source(options.context.as_ref().expect("checked args"))?;
            let (_, target) = read_source(options.target.as_ref().expect("checked args"))?;
            compatibility = Some(contracts::TargetContracts::load(&context, &target)?.check(
                &sources,
                &input,
                options.expected_binding.as_deref(),
            )?);
            return Ok(());
        }
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
        scope: if options.resolve {
            "resolve"
        } else if options.check_target {
            "compatibility"
        } else if options.export {
            "export"
        } else if options.import {
            "import"
        } else if options.build {
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
        exported_bundle,
        package_file: options.package.as_deref().map(label),
        bundle_file: options.bundle.as_deref().map(label),
        compatibility,
        resolution,
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
