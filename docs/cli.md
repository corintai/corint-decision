# Offline CDL CLI: validation

## Audience

For Agents, Skill authors and developers writing or modifying CDL files.

## Feature Overview

`corint validate` is the static syntax checker for Agent/Skill authoring. Its default
profile is `cdl-static-1`, covering **Rule, Ruleset, Pipeline, Registry, Feature,
List and Service**. It checks files without starting the decision engine, connecting
to a database, reading list data, making HTTP requests or executing actions.
The [authoring resource schema](../CDL/schema/authoring.json) defines accepted shapes.
Strict Core compilation remains available through an explicit profile below.

## Steps

```sh
cargo build -p corint-decision-cli --locked

# A single resource; no Registry or input Schema is required.
./target/debug/corint validate --format json \
  tests/conformance/cdl_authoring/features/payment.yaml

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

`corint validate [--profile cdl-static-1] [--root DIR] [--input-schema PATH] [--format text|json] [FILE...]`

| Input | Checks |
|---|---|
| Explicit files | YAML, closed resource structure, expressions, literal type errors, duplicate loaded IDs, local graph targets/cycles and declared dependency cycles. Missing cross-file resources do not fail this mode. |
| `--root DIR` with files | Load those root-relative files and transitive imports; require all resource references to resolve in the loaded collection. |
| `--root DIR` without files | Discover `.yaml`, `.yml`, `.json` recursively in `rules/`, `rulesets/`, `pipelines/`, `features/`, `lists/`, `services/`, plus root `registry.yaml`, `registry.yml`, `registry.json`; check the complete discovered collection. |
| `--input-schema PATH` | Also check declared event fields and their known expression types. The Schema path is relative to the current working directory, even with `--root`. |

Only resource files belong in resource directories. Schema, case inputs, backups and
reports are not resources and should be stored elsewhere. Supply explicit files for
other directory layouts. Files must be UTF-8 regular files no larger than 4 MiB.
Imports must stay within the canonical root, with at most 128 levels and 4096 files.
Discovery rejects symlinks; explicit files/imports are canonicalized and cannot escape
the root. Shared imports and repeated source paths are loaded once. Use `--` before
filenames beginning with a dash.

Static imports support `rules`, `rulesets`, `pipelines`, `features`, `lists`, `services`
arrays of root-relative file paths. Use inline `import:` or a version/import header
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
does not test their existence or connectivity. An external custom service without a
local HTTP definition can be syntax-checked alone; it cannot satisfy repository
reference checks without a declarative binding.

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
`references_checked`, `input_schema_checked`, `execution_checked: false`, `unchecked`
and `diagnostics`. Resource paths are canonical absolute paths; input Schema diagnostics use the supplied
Schema path. Text is the default.

| Exit | Meaning |
|---|---|
| `0` | The requested static checks passed. |
| `1` | CDL syntax, structure, expression, type or reference errors. |
| `2` | Invalid arguments or file-loading errors. |

Diagnostics contain `source`, `field_path` (JSON pointer), `stage`, `code`, severity
and message. YAML parse errors include line/column when available; semantic locations
are reported as field paths, without invented line numbers. Independent files are
checked in one run; shape diagnostics are capped at 32 per file. Invalid structures
are not fed into semantic checks. Message wording is explanatory; branch on codes
and paths, not English text. Service objects and credential values are not echoed
in schema diagnostics.

The default Skill loop is: **write or edit → validate → repair diagnostics → validate
again**. Read exit status and `valid`, and retain the reported `unchecked` scope.
Use repository mode when related resources are available; use standalone mode for
individual resources. Supplying an input Schema adds checks but is not a prerequisite.
The [authoring Skill](../skills/cdl-policy-authoring/SKILL.md) follows this workflow.

Static success does not prove execution-profile compatibility or expected business
behavior. When requested, use explicit Core compilation and
[`corint test`](testing.md) for supported Core resources and declared expectations.
Validation does not publish, activate or authorize a policy.

## Explicit Core compilation

`corint validate --profile cdl-core-risk-draft-1 --input-schema PATH [--format text|json] FILE...`

- Supply **all** resource files explicitly, including exactly one Registry.
  A lone syntactically valid Rule is not a valid complete bundle.
- Paths are relative to the current working directory, not the schema's directory.
  Use quoted paths for spaces; put `--` before dash-prefixed resource filenames.
  Prefix a dash-prefixed schema path with `./`.
- Files must be local, regular, UTF-8 text files. Symlinks to regular files are
  allowed; duplicate source paths, including canonical/symlink aliases, fail.
- Directories, stdin, URLs, import resolution, repository discovery, and implicit
  schema/field inference are not supported. Unknown and repeated options fail.
- Resource YAML uses the same strict [resource schema](../CDL/schema/core.json), parser,
  reference/type/control-flow checks and compiler as `DecisionEngine::from_core`.
  Registry selection conditions are also compiled by that shared gate.
- No input files are changed and no policy conditions or action intents are executed.

The input schema is YAML or JSON representing the existing model `Schema`, not
JSON Schema and not a new BusinessContext definition. See the runnable
[input-schema fixture](../tests/conformance/cdl_core/input-schema.yaml) and its
[file-format JSON Schema](../CDL/schema/input.json). Field map keys use `amount`, not
`event.amount`; expressions still use `event.amount`. Every field must specify its
matching `name`, a `field_type`, and an explicit boolean `required`. Scalar types are
`number`, `string` and `boolean`; closed objects use
`{"object":{"schema":{"name":"payment","fields":{...}}}}`. `required: false` permits
omission, including omission of an entire nested object; a present object must
satisfy its own child schema. Optional fields are not nullable. Arrays, open objects
and non-null defaults are not enabled. See the [nested input fixture](../tests/conformance/core_extensions/input-schema.yaml)
and [Core input rules](../CDL/context.md#strict-core-input-and-results). Descriptions are optional and have no
execution semantics. Duplicate keys, multiple YAML documents, unknown fields and
unsupported types are rejected rather than silently ignored.

`parse_core_input_schema` in the compiler owns this file gate. It checks the
embedded public schema, deserializes the existing model and applies the same
semantic input constraints as `compile_core`. Legacy model deserialization is unchanged.

### Core output and exit codes

`--format json` emits exactly one JSON object to stdout, including on usage or
file errors. No logs or banners are mixed in. Default text output also goes to
stdout; failure to write stdout is reported to stderr with exit code 2.

| Exit | Meaning |
|---|---|
| `0` | All supplied resources and Registry conditions compiled successfully. |
| `1` | Invalid CDL/input schema, duplicate source or incomplete/invalid resource closure. |
| `2` | Invalid command/options or file/output I/O failure; validation could not complete. |

The experimental JSON envelope has `report_version: "1"`, `tool_version`, `profile`,
`scope: "compile"`, `valid`, `input_schema`, `sources` and `diagnostics`. It always
includes `execution_checked: false` and `business_evaluation: "not_performed"`.
`valid: true` means only that this compile-time gate passed. Help/version exit 0
does not mean a policy was validated.

Diagnostics are fail-fast: an unsuccessful validation emits the first error,
not a complete list of every defect. Each diagnostic preserves the shared Core
`severity`, `code`, `message`, `stage`, `source`, and `field_path` (JSON pointer).
YAML errors include 1-based `line`/`column` when available. Locations not available
from the parser are not invented. Source labels use supplied file paths;
`<bundle>` denotes closure-wide errors and `<command-line>` denotes option errors.

The adapter adds `E_USAGE` (`usage`), `E_IO` (`load`) and `E_DUPLICATE_SOURCE`
(`resolve`). Input semantic errors point into the actual schema file, e.g.
`/fields/amount`. Resource diagnostics are identical to the library gate for
the same source labels/content and input schema. Treat error codes and pointers
as machine-readable; message wording is explanatory, not a stable parsing API.

This envelope is **not** a signed/version-bound ValidationReport or publication
authorization: it has no artifact hash, evaluation evidence, approval or business
data lineage. Do not reuse it as proof for a changed file or a different target.
Those cross-product contracts remain planned work.


## Evidence

[Static CLI tests](../crates/corint-decision-cli/tests/authoring.rs) cover all resource
kinds, public online examples, imports, diagnostics, type/reference failures and
absence of HTTP calls. [Core CLI tests](../crates/corint-decision-cli/tests/validate.rs)
continue to enforce compiler diagnostic parity under the explicit Core profile.

## FAQ

An individual resource can pass without its dependencies; inspect
`references_checked` before treating it as a checked collection. A static success
with `execution_checked: false` is expected. Use an explicit execution profile
and behavior tests when runtime compatibility or decisions need verification.

## Revision History

| Date | Changes |
|---|---|
| 2026-09-07 | Make full CDL static validation the default; retain explicit Core compilation. |
