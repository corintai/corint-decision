# Source packages and exchange (experimental)

`corint build` / `corint verify` bind Core sources to fresh test evidence.
`corint export` / `corint import` exchange editable sources and rebuild that evidence.
This is a bounded source-package increment, **not** the full cross-product
PolicyPackage, portable IR, trusted historical report or deployment system.
No new CDL top-level keyword is introduced.

## Build and verify

From the repository root, choose a **new** output filename:

```sh
cargo build -p corint-decision-cli --locked --offline
./target/debug/corint build --format json \
  --input-schema tests/conformance/cdl_core/input-schema.yaml \
  --cases tests/conformance/cdl_core/behavior.yaml \
  --output payment.core-package.json \
  tests/conformance/cdl_core/rule.yaml \
  tests/conformance/cdl_core/ruleset.yaml \
  tests/conformance/cdl_core/pipeline.yaml \
  tests/conformance/cdl_core/registry.yaml

./target/debug/corint verify --format json \
  --package payment.core-package.json \
  --cases tests/conformance/cdl_core/behavior.yaml
```

Omit `--offline` for compilation if dependencies are not cached. The CLI needs no
Work account, database or network. Build always recompiles and runs supplied cases
with Trace off/on; it accepts neither an old report nor a skip-test flag. Invalid
inputs, incomplete references or failed examples create no artifact.

Each file is read into memory; compilation, testing and hashing use those same
bytes. Later filesystem edits cannot change that snapshot. This is not an atomic
multi-file checkout: callers still need a versioned/locked checkout if required.

Output parents must exist. Existing outputs, directories and dangling symlinks are
refused. A passing artifact is written to a same-directory temporary file, synced,
and persisted without clobbering a competing writer. Normal write failures clean
up pending files; power-loss and hostile-filesystem recovery are not guaranteed.
Input files are never rewritten.

Verify needs only the package and exact test file, not the original source files.
It validates the package and fingerprints, checks test/tool identity, then
**recompiles and reruns the embedded source snapshot**. Fresh results must match
stored report digest/counts. Embedded labels are never read as filesystem paths;
no archives are extracted. There is no server/SDK package-loader or deployment
endpoint in this increment.

## Contents and privacy

One JSON artifact conforms to the embedded [source-package schema](contracts/schema/source-package.json):

- `format: corint-core-source-package`, `format_version: "1"`, language `"0.1"`
  and profile `cdl-core-risk-draft-1` are separately identified.
- `policy` includes raw UTF-8 CDL sources, input-schema source, per-file SHA-256
  values and policy fingerprint. The complete explicit closure is included, with
  no unresolved external dependencies/imports; maximum 10,000 sources.
- `evidence` binds policy identity, test-file identity, exact executable identity,
  fresh report digest and passing-case counts. Validation and behavior have separate
  `passed` statuses; business evaluation is `not_performed`, publication approval
  is `not_granted`, and authenticity is `unsigned`.

The artifact does **not** embed test inputs or the full test report. Keep the exact
test file separately under its access controls; its hash cannot reconstruct it.
Verification without that file is not supported. Original host filenames and
Work sessions are not added as metadata.

Policy/schema text is included **verbatim**, including comments and descriptions.
Any secrets or personal data manually placed there are not automatically redacted.
Inspect those files before sharing. CLI stdout still contains per-case results,
which may expose sensitive expectations, action strings or diagnostic text. Hashing
and omitting test inputs do not make a package anonymous.

## Fingerprint specification

Digests are lowercase 64-character SHA-256 hex strings.

| Fingerprint | Exact scope |
|---|---|
| Per-file `sha256` | Original UTF-8 source bytes, without whitespace/newline/Unicode/YAML normalization. |
| `policy.sha256` | Structured digest of profile, language version, input-source hash and sorted resource-label/hash pairs; excludes tests, reports, tool identity and approvals. |
| `evidence.suite_sha256` | Exact test-file bytes, including comments and expectations. |
| `evidence.tool.executable_sha256` | Current host executable file bytes (CLI or embedding application), including statically linked engine/compiler code. Shared toolchain version is also recorded. |
| `evidence.test_report_sha256` | Structured digest of the fresh deterministic `TestResults` object. |
| Receipt `package_sha256` | Exact emitted/read JSON artifact bytes; returned externally, not embedded in itself. |

Canonical source labels are `rule/<id>.yaml`, `ruleset/<id>.yaml`,
`pipeline/<id>.yaml`, and `registry.yaml`, sorted lexicographically. They must
match resource kind/ID. The schema label is `input-schema.yaml`; test report
diagnostics use `test-suite.yaml`. Host paths and argument order are excluded.

Structured hashing uses these bytes (this is **not RFC 8785/JCS**):

```text
UTF8("corint-canonical-json-v1") + NUL
  + canonical_json({"domain": DOMAIN, "value": VALUE})
```

`canonical_json` recursively sorts object keys in UTF-8 lexicographic order,
preserves array order, emits no insignificant whitespace, and uses `serde_json`
primitive encoding, including number spellings. The policy domain is
`core-source-policy-v1`; its value has exactly these fields:

```json
{
  "profile": "cdl-core-risk-draft-1",
  "language_version": "0.1",
  "input_schema_sha256": "<input source digest>",
  "sources": [{"path": "<canonical label>", "sha256": "<source digest>"}]
}
```

This is a hash-description fragment, not runnable CDL. The report domain is
`core-test-report-v1`; its value is the complete `test_results` object from
build/verify, excluding the outer CLI envelope. Random request IDs/timings are
not in that object. Tests check encoding with an independent reference and a
SHA-256 known-answer vector.

Same source/test bytes and executable produce identical package bytes despite
changed locations, filenames, source argument order or output names. This is
**byte identity**, not semantic equivalence: editing a source comment changes
policy identity. Editing only tests changes evidence but not policy identity.
Reports never participate in the policy hash, avoiding circular dependencies.

Verify currently requires the **same executable fingerprint**, not just version
`0.1.0`. Rebuilding, signing, stripping or changing platform may change that hash.
Use the original executable or rebuild a new package under the new tool. This
strict initial rule is not a cross-platform compatibility policy or software
attestation. The fingerprint identifies the executable file, not the OS and all
dynamically linked libraries or execution environment.

Generator packages retain the same host binding. The [source exchange workflow](#source-exchange)
rebuilds reviewed sources under the target CLI to obtain fresh CLI-bound evidence; it does not
verify historical evidence from another executable.

## Reports and trust boundaries

JSON output retains the CLI envelope with `scope: build` or `scope: verify`,
`test_results` when tests ran, and an `artifact` receipt only after successful
write/verification. The receipt exposes content fingerprints and unsigned/no-approval
status. Fresh-run mismatches retain `execution_checked: true` while `valid` is false;
preflight failures leave execution unchecked. Exit codes are `0` success, `1`
invalid content/bindings/tests, and `2` usage or I/O failure.

Top-level diagnostics identify package/write failures; test assertion diagnostics
remain inside case results. Additional errors are:

- `E_PACKAGE_FORMAT`: shape, duplicate/unknown fields, version/profile or unsafe/
  noncanonical labels. Package files must be JSON, not permissively parsed YAML.
- `E_PACKAGE_INTEGRITY`: source bytes differ from recorded hash.
- `E_POLICY_BINDING`, `E_SUITE_MISMATCH`, `E_TOOL_MISMATCH`: stale/mismatched identities.
- `E_PACKAGE_TEST_FAILED`: fresh execution of embedded sources fails examples.
- `E_REPORT_MISMATCH`: fresh report digest/counts differ from stored evidence.
- `E_OUTPUT_EXISTS` / `E_IO` (stage `write`): output refused or persistence failure.

Hashes **do not authenticate** anything. Someone controlling an unsigned package
can change sources/tests and recompute hashes. Verification establishes only that
these bytes pass these examples under this executable; it does not establish who
created the package, sufficient/independent test coverage, a genuine historical
run, business effectiveness or deployment permission. An externally trusted
receipt can be used to compare the whole-package digest; without one, harmless
JSON reformatting may still verify but has a different file digest.

Package v1 does not embed authenticated publication approval, target bindings or raw import
provenance. [Target checks](contracts/README.md), [repository publication](contracts/core-server.md)
and [authoring import resolution](resolution.md) have separate contracts.

## Source exchange

Export/import moves a policy between a generator host, an editor or Agent, and the CLI.
The portable object is an editable source bundle. Starting with the package built above:

```sh
./target/debug/corint export --package payment.core-package.json --output editable.json --format json
# Review or edit YAML source strings in editable.json.
./target/debug/corint import --bundle editable.json --cases tests/conformance/cdl_core/behavior.yaml --output rebuilt.core-package.json --format json
./target/debug/corint verify --package rebuilt.core-package.json --cases tests/conformance/cdl_core/behavior.yaml --format json
```

Output paths must be new and follow the no-overwrite rules in [Build and verify](#build-and-verify).
Each output is one JSON file; no YAML directory is extracted and source labels are never used as
filesystem destinations. These commands do not scan repositories, resolve file imports, fetch
remote resources or deploy policies. Use [`corint resolve`](resolution.md) for authoring imports;
`corint import` consumes an already frozen source bundle.

### Editable bundle contract

The [source-bundle schema](contracts/schema/source-bundle.json) defines:

| Field | Meaning |
|---|---|
| `format` / `format_version` | `corint-core-source-bundle` / `1` |
| `profile` / `language_version` | `cdl-core-risk-draft-1` / `0.1` |
| `input_schema` | One `{path, yaml}` input-contract source |
| `sources` | 1–10000 `{path, yaml}` resources forming a complete Core collection |

YAML strings preserve their original bytes, including comments; JSON escaping is only transport
encoding. Export uses the package's canonical resource labels. Import also accepts other unique
logical `.yaml`/`.yml` labels with slash-separated ASCII letters, digits, underscores and hyphens.
Absolute paths, dot segments, URLs and backslashes are rejected; the input-schema label cannot
collide with a resource label. These in-memory labels have no filesystem path-length limit.

Bundles contain no hashes to update manually, test suite or evidence. Shared schema/compiler
checks reject unknown fields, duplicate keys, incompatible versions, missing dependencies,
duplicate resource IDs and unsupported semantics. Source text is preserved without redaction,
as described in [Contents and privacy](#contents-and-privacy).

### Export and import results

**Export** checks package shape, source hashes, policy binding, canonical labels and compilation
under the current compiler. It does not verify the original executable, historical test report
or original suite. Thus sources can be recovered from another host without accepting its evidence.
A broken source hash or policy binding fails; stale or fabricated historical test hashes do not
become trusted claims just because export succeeds.

The export report has `scope: export`, `execution_checked: false` and an `exported_bundle` receipt
with the output SHA-256, `source_package_evidence: not_verified`, `evidence_exported: false` and
`publication_approval: not_granted`. It contains no test results or package artifact.

**Import** validates the whole bundle, recompiles it and runs the caller's independent
[behavior suite](testing.md) with Trace off/on. A new package is written only when every case passes.
The report has `scope: import`, `test_results` when tests ran and an `artifact` after output creation.
Assertion failures retain results without a package; invalid suites fail before execution.

The supplied suite may differ from the original: import creates new evidence, whereas verify
requires the original suite and host binary. Callers review changes to both policy and expectations;
the tool never rewrites expectations. The input contract comes from the bundle, so callers also
review its changes. Import does not check a production target's schema or tenant authorization.
Business evaluation remains `not_performed`, authenticity `unsigned` and approval `not_granted`.
The shared exit conventions are 0 for success, 1 for invalid content/checks, and 2 for usage or I/O.

### Identity across hosts

Without source changes, export/import preserves policy identity and execution results. Reordering
or relabelling entries does not change the rebuilt policy hash because package construction uses
canonical resource labels. Changing source/schema bytes changes policy identity; changing the suite
changes evidence. Equal source bytes, suite bytes and executable yield the same package bytes.

A generator-host package still fails direct CLI verify with `E_TOOL_MISMATCH`. Export its sources,
review them, then import with the CLI and an independent suite. That is fresh validation under the
new executable, not authentication of a historical run or portable attestation.

## Shared API and verification assets

Packages, exchange and [generation](generation.md) share `corint-decision-toolchain`.
Its `transfer` module exposes `SourceBundle::new`, `export_sources`, `read_bundle`,
`import_sources` and `export`. Import uses synchronous test/build execution; async hosts should
use a blocking worker, as the strict generator does.

[Package process tests](../crates/corint-decision-cli/tests/package.rs) cover reproducible bytes,
relocation, changed sources/schema/suites/executable, forged reports, rehashed failing sources,
malformed packages, missing resources and no-overwrite behavior. [Transfer process tests](../crates/corint-decision-cli/tests/transfer.rs)
cover generator-host → CLI → library round trips with fixed responses, reviewed source/case edits,
Core negative mutations, stale evidence, unsafe labels and machine-readable errors.

```sh
cargo test -p corint-decision-cli --test package --test transfer --locked --offline
```

These fixtures establish the declared local workflow. Real model quality, Work interoperability,
business evaluation and production publication require the corresponding independent evidence
and [public contracts](contracts/README.md).
