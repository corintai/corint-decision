# Offline CDL CLI: validation

## Audience

For Agents, Skill authors and developers writing or modifying CDL files.

## Feature Overview

`corint validate` is the full CDL static syntax checker for Agent/Skill authoring,
covering **Rule, Ruleset, Pipeline, Registry, Feature,
List and Service**. It checks files without starting the decision engine, connecting
to a database, reading list data, making HTTP requests or executing actions.
The [authoring resource schema](../CDL/schema/authoring.json) defines accepted shapes.
Validation has one static path and accepts no `--profile` option. The JSON report
keeps `profile: "cdl-static-1"` as a machine-readable format identifier.

## Steps

```sh
cargo build -p corint-decision-cli --locked

# A single resource; no Registry or input Schema is required.
./target/debug/corint validate --format json \
  tests/conformance/cdl_authoring/features/payment.yaml

# Multiple files/directories: include all referenced definitions.
./target/debug/corint validate --format json \
  tests/conformance/cdl_authoring/rules/blocked.yaml \
  tests/conformance/cdl_authoring/features \
  tests/conformance/cdl_authoring/lists

# A repository: all seven resource kinds, references and optional input types.
./target/debug/corint validate --format json \
  --root tests/conformance/cdl_authoring \
  --input-schema tests/conformance/cdl_authoring/input-schema.yaml
```

With cached dependencies, add `--offline` to the build. To install the executable,
use `cargo install --path crates/corint-decision-cli --locked`. The command is named
`corint`. `corint --help` describes all commands; help/version output is not a
validation report. Existing `test`, `build`, `verify`, `resolve`, package and replay
commands retain their execution profiles and contracts.

## Inputs and Outputs

### Static inputs and scope

`corint validate [--root DIR] [--input-schema PATH] [--format text|json] [PATH...]`

| Input | Checks |
|---|---|
| One explicit file | Check its YAML, structure, expressions and resource references. Infer a policy root (or use `--root`), load imports and referenced objects by ID, and recursively validate their defining files. Missing references and invalid dependencies fail. |
| Multiple distinct files | The same static checks plus resource references across the selected files. A missing Rule/Ruleset/Pipeline/Feature/List/Service definition fails with `E_UNRESOLVED_REFERENCE`. |
| Directories or mixed paths | Recursively inspect every `.yaml`, `.yml`, `.json` file in the supplied directories, regardless of directory names or depth; suffix matching is case-insensitive. Check references across the entire selected collection, even when a directory contains only one resource. Explicit files are checked regardless of suffix. Overlapping paths load each file once. |
| `--root DIR` with paths | Expand the supplied root-relative files/directories, load transitive imports and referenced resource definitions from that root; require all references to resolve. |
| `--root DIR` without files | Discover `.yaml`, `.yml`, `.json` recursively in `rules/`, `rulesets/`, `pipelines/`, `features/`, `lists/`, `services/`, plus root `registry.yaml`, `registry.yml`, `registry.json`; check the complete discovered collection. |
| `--input-schema PATH` | Also check declared event fields and their known expression types. The Schema path is relative to the current working directory, even with `--root`. |

Pass files to validate precisely those files, or directories to discover their CDL
resources. Discovery recognizes auxiliary documents by content: input Schemas
(`name`/`fields`), behavior suites (`version`/`profile`/`cases`), validation reports
(`report_version`/`profile`/`diagnostics`), and analysis reports carrying provenance
with metrics or row/column summaries. These are listed in `skipped_sources`, not
counted as validated resources. Unknown shapes, YAML parse failures and any document
declaring CDL resource fields still undergo validation. File/directory names alone
never cause a document to be skipped. Explicit files and imports are always strict;
explicit auxiliary files return `E_NOT_CDL` with guidance on the appropriate input.

The root-only option retains its existing repository-layout shortcut; it is not
required for directory validation. Other file suffixes are skipped during scanning.
Single-file validation also checks dependencies. Without `--root`, it uses the
nearest ancestor containing `registry.yaml`, `registry.yml` or `registry.json`,
stopping the ancestor search at a Git root. If none exists, it uses the source
file's directory (or the parent of a conventional `rules/`, `rulesets/`,
`pipelines/`, `features/`, `lists/` or `services/` directory). The report exposes
this directory as `reference_root`; use `--root` for a different layout.

Within this root, definitions are indexed by resource kind and declared ID, not
filename. Only the selected sources, their imports and transitively referenced
files undergo full validation. Unrelated malformed files do not fail a single-file
check. Duplicate definitions of a referenced ID, missing IDs, invalid dependencies,
unknown service operations and dependency cycles fail validation. All loaded
files appear in `sources`. Definitions must be local; no services are contacted.
Directory/multiple-file selection without `--root` checks references within the
selected collection. It does not infer extra search roots.

For example, `corint validate repo2/ruleset.yaml` finds the rules under `repo2/rules`
but rejects `customer_amount_spike_7` when the definition declares
`customer_amount_spike_7d`. There is no syntax-only exception for single files.
Discovering an input Schema does not enable input checks: supply `--input-schema`
explicitly. Save redirected reports outside scanned directories because a shell
creates an empty output file before validation, which is not a recognizable report.
An empty or auxiliary-only selection is an error, not a successful validation. Files must be UTF-8
regular files no larger than 4 MiB.
Imports must stay within the canonical root, with at most 128 levels and 4096 files.
Discovery rejects symlinks; explicit files/imports are canonicalized and cannot escape
the root. Shared imports and repeated source paths are loaded once. Use `--` before
filenames beginning with a dash.

Static imports support `rules`, `rulesets`, `pipelines`, `features`, `lists`, `services`
arrays of root-relative file paths. Without an explicit or inferred root, import declarations are checked
for syntax but not followed; only supplied files and directory contents are loaded.
Use inline `import:` or a version/import header
followed by `---` and one resource body. Duplicate keys across header and body fail.
This is authoring composition; it does not extend the runtime or the separate
[Core import profile](resolution.md).

The static schema includes compatibility annotations, Rule parameters, Ruleset
inheritance, condition aliases and Service steps alongside the Core resource shapes.
Feature collections support aggregation, state (`time_since`), expression and lookup
as specified in [Feature](../CDL/feature.md). Lists use a flat binding or `lists`
collection; HTTP services use the flat [Service](../CDL/service.md) configuration.
Legacy, unimplemented operators such as Feature graph/sequence are not enabled by
this checker. Unknown fields, invalid versions and malformed structures fail.

Feature checks reuse the runtime's pure window/filter validation and mathematical
expression parser. Service checks reuse HTTP configuration and JSON body-template
validation; operation references are checked when the service definition is loaded.
Datasource names and SDK-provided bindings are external declarations: this command
does not test their existence or connectivity. An external custom service requires a local declarative binding to satisfy
resource reference checks, including when validating one file.

The optional input file is the existing model `Schema` serialization, **not JSON
Schema**. Its [format schema](../CDL/schema/authoring-input.json) supports number,
string, boolean, null, any, arrays and objects. Map keys omit the `event.` prefix.
See the [runnable input fixture](../tests/conformance/cdl_authoring/input-schema.yaml).
No input Schema is inferred from example values or invented to make validation pass.
External Feature/Service response types and runtime result availability are not
proven by static validation.

### Static output and Agent workflow

`--format json` writes one JSON report to stdout on success and failure. It includes
`report_version`, `profile: "cdl-static-1"`, `scope: "static"`, `valid`, `sources`,
`references_checked`, `reference_root`, `input_schema_checked`, `execution_checked: false`, `unchecked`,
`skipped_sources` (each entry has `source` and `reason`), and `diagnostics`. Text output
lists each passed resource as `[PASS] <path>` when the input includes a directory
(including the root-only repository shortcut), followed by skipped auxiliary files
and failure diagnostics. File-local failures still allow other files to be listed
as passed. Collection reference failures or invalid global inputs withhold per-file
pass claims. JSON output retains its existing `sources` and `diagnostics` fields.
Resource paths are canonical absolute paths; input Schema diagnostics use the supplied
Schema path. Text is the default.

| Exit | Meaning |
|---|---|
| `0` | The requested static checks passed. |
| `1` | CDL syntax, structure, expression, type or reference errors. |
| `2` | Invalid arguments or file-loading errors. |

Diagnostics contain `source`, `field_path` (JSON pointer), `stage`, `code`, severity
and message. YAML parse errors include line/column when available; semantic locations
are reported as field paths, without invented line numbers. Independent files are
checked in one run; shape diagnostics are capped at 32 per resource. Invalid structures
are not fed into semantic checks. Message wording is explanatory; branch on codes
and paths, not English text. Service objects and credential values are not echoed
in schema diagnostics.

The default Skill loop is: **write or edit → validate → repair diagnostics → validate
again**. Read exit status and `valid`, and retain the reported `unchecked` scope.
Resources may share one source file (`rule` / `ruleset` object lists or `---` documents),
use separate files, or mix both layouts; see [authoring source layout](../CDL/overall.md#authoring-source-layout).
All declarations are checked and indexed for imports and ID references. File paths
appear once in `sources`, even when a file contains several resources.

Both single-file and repository validation check referenced resources. Supplying an input Schema adds checks but is not a prerequisite.
The [authoring Skill](../skills/cdl-policy-authoring/SKILL.md) follows this workflow.

Static success does not prove target compatibility or expected business behavior.
Use [`corint check-target`](contracts/README.md) to check declared target compatibility
and [`corint test`](testing.md) to compile and execute supported Core resources
against declared expectations.
Validation does not publish, activate or authorize a policy.

## Evidence

[Static CLI tests](../crates/corint-decision-cli/tests/authoring.rs) cover all resource
kinds, public online examples, imports, diagnostics, type/reference failures and
absence of HTTP calls. [CLI contract tests](../crates/corint-decision-cli/tests/validate.rs)
cover the single static path and reject removed profile options.
[Execution preflight tests](../crates/corint-decision-cli/tests/core_preflight.rs)
continue to enforce compiler diagnostic parity through `corint test`.

## FAQ

Single-file validation loads and checks referenced dependencies; missing references
fail. Inspect `references_checked` and `input_schema_checked` for the reported scope.
A static success with `execution_checked: false` is expected. Use `check-target`
for declared target compatibility and `test` for expected decisions.

## Revision History

| Date | Changes |
|---|---|
| 2026-09-22 | Remove validation profile selection; all validation uses the full CDL static checker. |
| 2026-09-07 | List passed resource paths in directory text output alongside skip reasons and errors. |
| 2026-09-07 | Identify auxiliary documents during directory discovery and report skipped files; explicit inputs remain strict. |
| 2026-09-07 | Accept files, recursive directories and mixed paths; keep imports opt-in with `--root`. |
| 2026-09-07 | Make full CDL static validation the default; retain explicit Core compilation. |
