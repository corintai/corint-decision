//! The root CLI routes command groups; CDL behavior belongs under `corint cdl`.
use serde_json::Value;
use std::process::{Command, Output};

const COMMANDS: &[&str] = &[
    "validate",
    "test",
    "build",
    "verify",
    "export",
    "import",
    "check-target",
    "resolve",
    "prepare-repository",
    "record",
    "replay",
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_corint"))
        .args(args)
        .output()
        .unwrap()
}

fn usage_report(args: &[&str]) -> Value {
    let output = run(args);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], false);
    assert_eq!(report["diagnostics"][0]["code"], "E_USAGE");
    assert_eq!(report["diagnostics"][0]["stage"], "usage");
    report
}

#[test]
fn root_help_lists_groups_and_cdl_help_lists_commands() {
    for args in [vec![], vec!["--help"], vec!["-h"]] {
        let output = run(&args);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("corint <COMMAND>"));
        assert!(help.contains("corint cdl --help"));
        assert!(!help.contains("--input-schema"));
    }
    for args in [vec!["cdl"], vec!["cdl", "--help"], vec!["cdl", "-h"]] {
        let output = run(&args);
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout).unwrap();
        for command in COMMANDS {
            assert!(
                help.contains(&format!("corint cdl {command} ")),
                "{command}: {help}"
            );
        }
        assert!(!help.contains("--profile"));
    }
}

#[test]
fn every_cdl_command_has_namespaced_long_and_short_help() {
    for command in COMMANDS {
        for flag in ["--help", "-h"] {
            let output = run(&["cdl", command, flag]);
            assert!(output.status.success(), "{command}: {output:?}");
            assert!(output.stderr.is_empty());
            let help = String::from_utf8(output.stdout).unwrap();
            assert!(
                help.contains(&format!("corint cdl {command} ")),
                "{command}: {help}"
            );
        }
    }
    let root_version = run(&["--version"]);
    let cdl_version = run(&["cdl", "--version"]);
    assert!(root_version.status.success());
    assert!(cdl_version.status.success());
    assert_eq!(root_version.stdout, cdl_version.stdout);
}

#[test]
fn old_top_level_commands_fail_with_migration_guidance() {
    for command in COMMANDS {
        let output = run(&[command]);
        assert_eq!(output.status.code(), Some(2));
        let expected = format!("Use corint cdl {command}");
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains(&expected));
        let report = usage_report(&[command, "--format", "json"]);
        assert_eq!(report["scope"], "usage");
        assert!(report["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains(&expected));
    }
}

#[test]
fn invalid_groups_and_subcommands_return_usage_errors() {
    for args in [
        vec!["unknown", "--format", "json"],
        vec!["cdl", "unknown", "--format", "json"],
        vec!["--help", "extra", "--format", "json"],
        vec!["--version", "extra", "--format", "json"],
        vec!["--", "cdl", "--format", "json"],
    ] {
        if args[0] == "--" {
            let output = run(&args);
            assert_eq!(output.status.code(), Some(2));
            assert!(String::from_utf8(output.stdout)
                .unwrap()
                .starts_with("E_USAGE:"));
        } else {
            usage_report(&args);
        }
    }
}

#[test]
fn capability_inventory_uses_the_cdl_namespace() {
    let capabilities: Value = serde_json::from_str(include_str!(
        "../../../docs/contracts/schema/capabilities.json"
    ))
    .unwrap();
    let mut count = 0;
    for tool in capabilities["tools"].as_object().unwrap().values() {
        let commands = tool["command"].as_str().into_iter().chain(
            tool["commands"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        );
        for command in commands.filter(|command| command.starts_with("corint ")) {
            assert!(command.starts_with("corint cdl "), "{command}");
            count += 1;
        }
    }
    assert_eq!(count, COMMANDS.len());
}
