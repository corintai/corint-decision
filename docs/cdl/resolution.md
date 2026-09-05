# Strict authoring imports: `corint resolve` (experimental)

Source profile: **`cdl-core-import-draft-1`**. Output execution profile:
**`cdl-core-risk-draft-1`**. This is the bounded C08 authoring/load increment,
not a new execution capability or completion of phase 0.

`corint import` reads an already complete source bundle. `corint resolve` instead
loads explicit local entry files and their declared transitive imports, validates
them and emits a **frozen source bundle v1**. Existing `compile_core`, validation,
strict generation and server activation continue to reject unresolved imports.
No runtime path reads, repository discovery, remote fetches or implicit ID lookup
are added to those entry points.

## Source contract

Import declarations use the existing `import.rules / rulesets / pipelines` names,
but this explicit profile checks the [header schema](schema/import-header.json)
before removing any import fields. It does not inherit the compatibility parser's
permissive merging, implicit versions or multi-definition document selection.

Each file contains exactly one of:

- A single strict Core document with explicit string `version: "0.1"`, optionally
  adding an `import` mapping next to its single resource definition.
- Exactly two YAML documents: a header containing **only** explicit `version`
  and `import`, then exactly one Core resource. The second document can omit its
  version, inheriting the header's; an explicit version must agree exactly.

The mapping must contain at least one known import group, each a nonempty array
of unique paths. Empty/null/wrong-type groups, unknown fields, duplicate YAML keys,
extra documents and version conflicts fail closed. Arbitrary extra resources
cannot be hidden in the header or a third document.

Runnable examples are the [C08 fixture](../../tests/conformance/cdl_imports/manifest.json):
Registry → Pipeline → Ruleset → Rule, with a shared Rule imported by two parents.
An import declares availability, **not execution**: importing a Pipeline does not
enable subpipeline calls. Imported resource kinds must match their group. Registry
files are explicit entries, not import targets. Multiple entries are permitted;
the complete resolved closure must still have exactly one Registry.

Paths are relative to the **explicit repository root**, not each importing file.
Labels match `[A-Za-z0-9_-]+(/[A-Za-z0-9_-]+)*.yaml` (or `.yml`), at most 256 bytes.
Absolute paths, `.`/`..`, empty components, backslashes, URLs and globs are rejected.
The same exact label reached from multiple parents is loaded once; different files
with colliding resource IDs fail the shared compiler rather than replacing each
other. Labels are case-sensitive; filesystem aliases are not silently merged.
Every resource reference must resolve within this closed set, not a global catalog.

## CLI and library

Run from the repository root, choosing new output filenames:

```sh
cargo build -p corint-decision-cli --locked --offline
./target/debug/corint resolve \
  --source-profile cdl-core-import-draft-1 \
  --root tests/conformance/cdl_imports \
  --input-schema input-schema.yaml \
  --output payment.resolved.json --format json registry.yaml

./target/debug/corint import --bundle payment.resolved.json \
  --cases tests/conformance/cdl_core/behavior.yaml \
  --output payment.core-package.json --format json
```

`--root` and `--output` are relative to the current working directory; the input
label and all entry/import labels are root-relative. The source profile is required,
not inferred from YAML version. No existing file is overwritten, including symlinks.
On resolution failure no output artifact is created. Output is a regular v1 bundle
usable by existing package/target/server gates; those gates do not need the original
directory. Resolution itself compiles but does **not** execute acceptance cases.

JSON stdout is one report with `scope: resolve`, `execution_checked: false`, and
`resolution` containing the manifest and resolution/policy/bundle SHA-256 values.
Its outer `profile` identifies the output execution profile; `manifest.source_profile`
identifies the authoring profile. Exit 0 means resolved/compiled, 1 means an invalid
dependency/source/closure or exceeded bound, 2 means usage, root/file I/O or output
failure. Missing or inaccessible declared dependencies are resolution failures (1).
`E_INVALID_IMPORT`, `E_IMPORT_KIND` and `E_IMPORT_LIMIT` distinguish import failures;
Core structural/type/reference errors retain shared diagnostics. Dependency read
failures and cycles include an import chain. Normalized-document semantic errors
use the original file label and normalized field pointer, without fabricated raw
line/column positions; YAML parsing errors retain their available raw positions.

Shared Rust entry points in `corint_decision_toolchain::resolve`:

- `resolve(root, input_label, entries)`: confined filesystem loading on Unix.
- `resolve_sources(input_label, entries, &[CoreSource])`: the same graph, header
  and compiler checks on an explicit virtual repository; no filesystem access.
- The immutable result exposes `bundle()`, `receipt()` and `originals()`; `write()`
  writes a new frozen bundle. Async callers should use a blocking worker.

Filesystem loading uses directory-relative `openat` with `O_NOFOLLOW` for every
repository-relative component; even symlinks pointing back inside the root are
rejected. Files are opened nonblocking and checked to be regular files. The root is
operator-selected, and its final component cannot be a symlink. The filesystem
adapter currently requires Unix (tested locally on macOS; Linux CI configured);
the in-memory resolver is portable. No fallback to less-constrained file reads exists.
Use a stable checkout: the resolver captures each file once but does not promise
a transactional snapshot of a directory being concurrently rewritten.

Bounds: 256 resource files, maximum import stack depth 32, 1 MiB per resource/input
file and 16 MiB combined resource/input bytes. Unreachable files are not discovered
or parsed. This does not grant execution of Connector/Feature/Model extensions.

## Identity and evidence

The versioned resolution manifest records sorted entry labels, the input label and
raw SHA-256, and each reachable file's path, raw SHA-256, resource kind/ID and typed
import edges. It excludes absolute root/output paths. Each file retains its original
bytes in the library's `originals()` result for an explicitly authorized provenance
store. CLI stdout contains hashes/graph only; the output bundle does **not** include
the original import layout or the manifest. Retain the source checkout and save the
JSON report if that provenance is needed; reconstructing originals from the bundle
alone is not supported.

`resolution_sha256` uses the package toolchain's canonical JSON hash with domain
`core-import-resolution-v1` and the manifest as its value. Every normalized source
gets a `# corint-resolution-sha256: ...` comment. Since ordinary package identity
already hashes exact source bytes, changes to original files (including comments),
input, entry set or dependency graph change the frozen policy identity. Existing
target bindings and operator approvals then no longer match. Directory relocation,
entry argument ordering or virtual repository ordering do not change the result.

These comments and reports are **not signatures or proof of origin**. The runtime
does not interpret or trust them as permissions. Editing/removing a comment produces
another policy identity; no caller-supplied manifest or “passed” claim is accepted
as verification. After editing original sources, run resolution again and rebuild
evidence. Different textual closures can behave identically without having identical
policy hashes. A previously built package remains evidence for its own frozen bytes,
not for a subsequently modified live source directory.

No new fields are added to source bundle/package v1; exporting a resolved package
preserves normalized frozen sources, not the original import layout. Portable raw
source provenance, version-range dependency resolution and signed lock manifests
remain future work.

## Verification

[Resolver tests](../../crates/corint-decision-cli/tests/resolve.rs) cover real file
loading and in-memory equivalence; shared dependencies, malformed headers, cycles,
wrong kinds, missing files, version/ID conflicts, path confinement, bounds, no-clobber
output and identity invalidation. The existing real-engine boundary cases compare
results, paths, local results and Trace off/on behavior with the explicit closure.
[Server tests](../../crates/corint-decision-server/tests/core_activation.rs) additionally
pass the frozen result through the operator approval gate and actual decision engine.
This is synthetic fixture evidence, not live Work integration or business evaluation.
