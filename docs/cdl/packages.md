# Experimental source packages: `corint build` / `corint verify`

These offline commands bind Core source content to freshly generated test evidence.
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

One JSON artifact conforms to the embedded [source-package schema](schema/source-package.json):

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

The [source exchange commands](exchange.md), strict [generation API](generation.md) and CLI share
`corint-decision-toolchain`. Generator packages retain this same host binding:
use `corint export` and `corint import` to rebuild reviewed sources under the
target CLI and obtain CLI-bound evidence. Cross-host historical evidence
verification is not implemented; source exchange does not bypass this restriction.

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

Production signatures, trusted run records, real-data evaluation, environment
bindings, approvals, distribution/activation, import locking and full
BusinessContext/Feature/Model/feedback contracts remain pending.

## Verification assets

[Process tests](../../crates/corint-decision-cli/tests/package.rs) cover build/verify,
relocation, reproducible bytes, changed sources/schema/tests/tool identity, forged
reports, rehashed sources failing real tests, malformed packages, duplicate/missing
resources, no extraction, no-overwrite and competing writers. Run with:

```sh
cargo test -p corint-decision-cli --locked --offline
```

This implements the first local validate/test/source-build workflow, not all
phase-0 or W01–W10 cross-product acceptance requirements.
