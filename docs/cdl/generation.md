# Strict Core generation (experimental)

The opt-in Rust `CoreGenerator` creates or revises a **complete** CDL Core
closure, not an isolated rule string. It reuses the same compiler, real-engine
behavior tests and source-package builder as `corint validate / test / build`.
It does not require Corint Work. Current evidence uses fixed, recorded model
responses, not live-provider quality measurements or a Work product integration.

## Contract and trust boundary

The caller chooses an `LLMClient`, model, requirements, the
[input contract](schema/input.json), and independent
[acceptance cases](testing.md). On revision the caller also supplies the entire
existing closure. Invalid input contracts, suites or existing closures fail
before a provider call.

The prompt embeds the current [Core specification](cdl-core.md),
[resource schema](schema/core.json), [response schema](schema/generation-response.json)
and the actual conformance fixture sources. Requirements, the input contract and
existing strategy sources **are sent to the selected provider**; review them for
sensitive material first. Acceptance cases, their inputs and expected outputs are
**not sent**. There is no real-data connection or Work lookup.

The model returns one JSON object:

```json
{
  "profile": "cdl-core-risk-draft-1",
  "sources": [
    {"path": "rule.yaml", "yaml": "version: \"0.1\"\nrule: ..."}
  ]
}
```

This is an **envelope illustration, not an executable CDL example**. A runnable
response must include a valid, complete Rule/Ruleset/Pipeline closure and exactly
one Registry. Use the [conformance fixture](../../tests/conformance/cdl_core/manifest.yaml)
for executable examples.

Envelope limits are 1–256 sources, source labels of at most 256 characters,
YAML strings of at most 1,048,576 characters, and 4 MiB for the entire response.
Labels must be relative slash-separated ASCII alphanumeric/underscore/hyphen
names ending in `.yaml` or `.yml`. They are unique diagnostic labels,
**never destinations that the generator reads or writes**.

Unknown fields (including tests, input schemas, packages or validation claims),
duplicate JSON/YAML keys, Markdown fences, trailing prose, incompatible profiles,
incomplete closures and unsupported Core semantics fail closed. No document is
silently discarded. Only normal completion reasons `stop`, `end_turn` and
`STOP` are accepted; truncation, refusal and unknown reasons are rejected.
These are adapter contracts, not certification of any live provider.

## Rust API

Enable `core-generation` on `corint-decision-llm`. The public types for input
sources come from `corint-decision-compiler::core`; shared tests/packages live
in the independent `corint-decision-toolchain` crate.

```rust,ignore
let generator = CoreGenerator::new(provider, RuleGeneratorConfig::new(model));
let result = generator.generate(requirements, &input_schema, &acceptance_cases).await?;

// Revisions return a complete new candidate and never overwrite the original.
let revision = generator.revise(
    change_request, &existing_sources, &input_schema, &acceptance_cases,
).await?;

if let Some(package) = result.package {
    // Optional, caller-authorized output; fails if the destination exists.
    corint_decision_toolchain::package::write(&package, output_path)?;
} else {
    // Behavior was checked, but at least one assertion failed.
    // result.tests retains expected/actual results and structured diagnostics.
}
```

There is exactly one provider call per generation/revision. There are no
automatic repairs, retries, provider fallbacks, publication or activation.
If the caller wants different expected behavior, the caller must explicitly
review and update the acceptance suite; the model cannot revise it to pass.
Stable IDs on revision are requested in the prompt, not mechanically guaranteed:
review candidate changes before adopting them.

Results distinguish:

- `Err(CoreGenerationError::Core)`: preserves shared structural, reference and
  type diagnostics, or a generation-envelope error. No package is returned.
- `Err(Provider)` / `Err(Worker)`: provider or validation-worker failure.
- `Ok` with `package: None`: all cases ran, but behavior assertions failed.
- `Ok` with a package: all declared cases passed with Trace off/on parity.
  Business evaluation remains `not_performed`, authenticity `unsigned`, and
  publication approval `not_granted`.

The generator uses a blocking worker for the shared synchronous test/build
implementation, so it is safe to call from a Tokio async context. Cancellation
does not forcibly stop an already-started blocking validation worker; it performs
local checks only, without writing artifacts or deploying strategies.

## Portability and compatibility

The package format and policy hashes are unchanged. Evidence binds the **host
executable** running the shared toolchain, which is the CLI executable for CLI
builds, or the embedding application for generator builds. `corint verify`
therefore cannot certify evidence produced by a different host binary: it
returns `E_TOOL_MISMATCH`. Export/review the embedded sources and rebuild with
the target CLI and the caller-owned suite to create fresh evidence; do not
remove or rewrite the fingerprint to bypass this check. Cross-host signed
attestation remains future work.

Existing `RuleGenerator`, `RulesetGenerator`, `PipelineGenerator` and
`DecisionFlowGenerator` remain compatibility APIs. Their YAML output is not
strict Core acceptance evidence. They are not silently redirected or granted a
Core success claim by enabling this feature.

## Regression gate

```bash
cargo test -p corint-decision-llm --features core-generation --test core_generation --locked
cargo test -p corint-decision-toolchain -p corint-decision-cli --locked
```

Tests use fixed responses and independent cases: full generate/test/package/
verify round trip, all 24 shared negative fixture mutations with exact compiler
diagnostic parity, wrong boundaries, missing dependencies, forged claims,
malformed/oversized responses, private-case omission, provider failure without
retry, and revision without original-source mutation. Neither a mock response
nor passing synthetic cases proves production business effectiveness.
