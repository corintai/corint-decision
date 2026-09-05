# Offline CDL CLI: validation

Status: experimental; uses profile `cdl-core-risk-draft-1` and the shared
[CDL Core compiler](cdl-core.md). This page covers compile-time `corint validate`.
For real-engine example execution, see [`corint test`](testing.md); for source
packages and evidence binding, see [`corint build` / `corint verify`](packages.md).
These tools do not evaluate business effectiveness or authorize publication.

For a repository using file `import:` declarations, first use the explicit
[`corint resolve` authoring profile](resolution.md) to produce a frozen closure.
This does not broaden the accepted syntax of `validate` or the execution profile.

For opt-in declared-environment compatibility, see
[`corint check-target`](../contracts/README.md). It reuses this input model and
compiler; `validate` alone does not check a BusinessContext or target declaration.

## Build and run

From the repository root, with a Rust toolchain and cached dependencies:

```sh
cargo build -p corint-decision-cli --locked --offline
./target/debug/corint validate --format json \
  --input-schema tests/conformance/cdl_core/input-schema.yaml \
  tests/conformance/cdl_core/rule.yaml \
  tests/conformance/cdl_core/ruleset.yaml \
  tests/conformance/cdl_core/pipeline.yaml \
  tests/conformance/cdl_core/registry.yaml
```

If dependencies are not cached, omit `--offline` for the build. The resulting
binary itself does not need network access, a server, Work, a database or an LLM.
Alternatively, `cargo run -p corint-decision-cli --locked -- validate ...` uses
the same binary. Obtain help with `corint --help` and the tool/profile versions
with `corint --version` (both produce text, not validation reports).

An optional local installation is `cargo install --path crates/corint-decision-cli --locked`.
The executable is named `corint`. No prebuilt release/distribution is claimed here.

## Inputs and strictness

`corint validate --input-schema PATH [--format text|json] FILE...`

- Supply **all** resource files explicitly, including exactly one Registry.
  A lone syntactically valid Rule is not a valid complete bundle.
- Paths are relative to the current working directory, not the schema's directory.
  Use quoted paths for spaces; put `--` before dash-prefixed resource filenames.
  Prefix a dash-prefixed schema path with `./`.
- Files must be local, regular, UTF-8 text files. Symlinks to regular files are
  allowed; duplicate source paths, including canonical/symlink aliases, fail.
- Directories, stdin, URLs, import resolution, repository discovery, and implicit
  schema/field inference are not supported. Unknown and repeated options fail.
- Resource YAML uses the same strict [resource schema](schema/core.json), parser,
  reference/type/control-flow checks and compiler as `DecisionEngine::from_core`.
  Registry selection conditions are also compiled by that shared gate.
- No input files are changed and no policy conditions or action intents are executed.

The input schema is YAML or JSON representing the existing model `Schema`, not
JSON Schema and not a new BusinessContext definition. See the runnable
[input-schema fixture](../../tests/conformance/cdl_core/input-schema.yaml) and its
[file-format JSON Schema](schema/input.json). Field map keys use `amount`, not
`event.amount`; expressions still use `event.amount`. Every field must specify its
matching `name`, a `field_type` of `number`, `string` or `boolean`, and `required: true`.
Defaults other than null are not enabled. Descriptions are optional and have no
execution semantics. Duplicate keys, multiple YAML documents, unknown fields and
unsupported types are rejected rather than silently ignored.

`parse_core_input_schema` in the compiler owns this file gate. It checks the
embedded public schema, deserializes the existing model and applies the same
semantic input constraints as `compile_core`. Legacy model deserialization is unchanged.

## Output and exit codes

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

## Agent workflow and evidence

Give an Agent the [Core specification](cdl-core.md), [capability inventory](schema/capabilities.json),
resource/input schemas, explicit business requirements and a caller-supplied field schema.
After each YAML edit, run `corint validate --format json ...`, inspect the exit code
and diagnostics, correct the relevant file, then validate the **complete** bundle again.
Unknown fields should trigger clarification of the business context, not invented data.

Passing compilation is one gate. Behavior tests must still exercise representative
inputs and expected decisions in the real engine. Threshold quality, business
impact, publication approval and production safety require separate evidence.
Use [`corint test`](testing.md) for declared behavioral expectations and
[`corint build` / `corint verify`](packages.md) for the experimental source-package
workflow. Generator integration and production publication checks remain unimplemented.

The [CLI process tests](../../crates/corint-decision-cli/tests/validate.rs) reuse both
positive bundles and every negative mutation from the real-engine fixture manifest.
They check library diagnostic parity, strict input files, stable input-error paths,
exit codes, clean JSON output, explicit path handling and unchanged source files
from isolated temporary working directories. CI runs these alongside the
[real-engine conformance tests](../../crates/corint-decision-engine/tests/cdl_core_conformance.rs):

```sh
cargo test -p corint-decision-cli --locked --offline
cargo test -p corint-decision-engine --test cdl_core_conformance --locked --offline
```

All fixtures are synthetic. These tests do not claim real Work interoperability,
real-data evaluation, complete W01–W10 coverage or production readiness.
