# Source exchange: export and import (experimental)

Use source exchange to move a strategy between the strict generator, an external
editor/Agent, and the standalone CLI. The portable object is an **editable source
bundle**, not historical validation evidence or publication authority.

## CLI round trip

Starting with a [source package](packages.md) produced by `corint build` or the
[strict generator](generation.md):

```bash
corint export --package policy.json --output editable.json --format json
# Review/edit YAML source strings inside editable.json.
corint import --bundle editable.json --cases behavior.yaml --output rebuilt.json --format json
corint verify --package rebuilt.json --cases behavior.yaml --format json
```

Every output path must be new. Existing files, directories, symlinks and
concurrent writers are never overwritten. These commands produce **one JSON
file**, not a directory of extracted YAML files. Source labels are not read or
written as filesystem paths; imports, directory scanning, network access and
deployment are not performed. The CLI `import` command does **not** enable CDL's
currently unsupported YAML `import` language feature.

## Editable bundle contract

The [source-bundle schema](schema/source-bundle.json) defines:

| Field | Meaning |
|---|---|
| `format` | `corint-core-source-bundle` |
| `format_version` | `1` |
| `profile` / `language_version` | `cdl-core-risk-draft-1` / `0.1` |
| `input_schema` | One `{path, yaml}` input-contract source |
| `sources` | 1–10000 `{path, yaml}` resources forming a complete Core closure |

YAML strings retain their original bytes, including comments. JSON escaping is
only transport encoding. Exported labels use the package's canonical resource
kind/ID paths; import also accepts other unique logical `.yaml`/`.yml` labels.
No filesystem path-length limit is imposed on these in-memory labels.
Slash-separated label segments permit ASCII letters,
digits, underscores and hyphens. Absolute paths, dot segments, URLs and
backslashes are rejected. The input-schema label cannot collide with a resource
label.

There are no hashes to manually update when editing the bundle, and no embedded
test suite or evidence. Unknown fields, duplicate keys, incompatible versions,
missing dependencies, duplicate resource IDs and unsupported semantics are
rejected by the shared schema/compiler. Do not place secrets or unneeded business
data in source comments: export preserves source text rather than redacting it.

## Two different meanings of success

**Export** checks the original package's strict shape, source hashes, policy
bindings, canonical labels and compilation under the current Core compiler.
It deliberately does **not** verify the original tool fingerprint, historical
test report or original suite. This allows source recovery from a different host
without falsely accepting its test evidence.

An export JSON report has `scope: export`, `execution_checked: false`, and an
`exported_bundle` receipt containing the written bundle's SHA-256,
`source_package_evidence: not_verified`, `evidence_exported: false` and
`publication_approval: not_granted`. It has no test results or package artifact.
Export can succeed with stale/fabricated historical test hashes; it cannot
transfer them as trusted claims. A broken source hash or policy binding fails.

**Import** parses the entire bundle, recompiles the full closure, and executes
the caller's explicit [behavior suite](testing.md) with Trace off/on. It builds a
new source package only if every case passes. An import report has
`scope: import`, `test_results` when tests ran, and an `artifact` only after
successful output creation. Assertion failures retain results but write no
package. Invalid suites are rejected before any case runs.

The supplied suite may intentionally differ from the original one: import
creates **new evidence**, while `verify` checks existing evidence and still
requires the exact original suite and host binary. If policy changes invalidate
old expected results, the caller must review and explicitly update the suite;
the tools never modify expectations automatically.

The imported input contract comes from the bundle. Review changes to that
contract too: import does not compare it with a production target's schema or
provide tenant/target authorization. Business evaluation remains
`not_performed`, authenticity `unsigned`, and approval `not_granted`.

## Identity and cross-host behavior

Without source changes, export/import preserves policy identity and execution
results. Reordering or relabelling source entries does not change the rebuilt
policy hash because package construction uses canonical resource kind/ID labels.
Changing YAML bytes (including comments) or the input schema changes policy
identity; changing the suite changes evidence. Same source bytes, suite bytes
and host binary produce the same source-package bytes.

A generator-host package still fails direct CLI `verify` with
`E_TOOL_MISMATCH`. Export its sources, then import with the CLI and a reviewed
suite to produce a CLI-bound package. This is fresh validation, **not** portable
attestation or historical report authentication.

## Shared Rust API and regression gate

`corint_decision_toolchain::transfer` exposes `SourceBundle::new`,
`export_sources`, `read_bundle`, `import_sources` and `export`. The CLI calls
these same functions. `import_sources` uses the synchronous test/build runtime;
async callers should use a blocking worker, as the strict generator does.

[Process tests](../../crates/corint-decision-cli/tests/transfer.rs) cover actual
generator-host → CLI → library round trips using a fixed model response, byte
preservation, source edits, independently updated cases, relocation, all 24
Core negative mutations, stale evidence, unsafe labels, no-overwrite behavior,
invalid suites and machine-readable failures. They do not establish real Work
integration, model quality, business effectiveness or a safe production rollout.

```bash
cargo nextest run -p corint-decision-cli --test transfer --locked
```
