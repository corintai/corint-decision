# CDL generation

The `corint-decision-llm` crate provides two generation paths. Choose the path according to
the output and validation needed; all provider calls happen during authoring.

| Entry | Output and validation |
|---|---|
| [Strict Core generation](#strict-core-generation-experimental) | A complete resource collection, compiler diagnostics, independent behavior results and a package when the declared cases pass. Requires `core-generation`. |
| [Compatibility generators](#compatibility-generators) | YAML drafts for individual resources or a multi-document flow. The caller validates the resulting policy with the intended compiler and runtime. |

## Providers and configuration

The built-in adapters are `OpenAIProvider`, `AnthropicProvider`, `GeminiProvider` and
`DeepSeekProvider`; each accepts an API key `String` through `new`. The host supplies
credentials and the model ID. Provider constructors do not load environment variables.
`MockProvider::with_response(String)` supplies a fixed response for local tests.
These adapters implement `LLMClient`, which can also be implemented by the host.

This setup fragment assumes `api_key: String` and `model: String` come from the host:

```rust,ignore
use corint_decision_llm::{LLMClient, OpenAIProvider, RuleGeneratorConfig};
use std::sync::Arc;

let provider: Arc<dyn LLMClient> = Arc::new(OpenAIProvider::new(api_key));
let config = RuleGeneratorConfig::new(model)
    .with_max_tokens(2048)
    .with_temperature(0.3)
    .with_thinking(false);
```

For a compatible endpoint, construct the adapter with
`OpenAIProvider::with_base_url(api_key, base_url)`. This is an associated constructor;
the model is configured on `RuleGeneratorConfig`. `model` has type `String`;
`max_tokens` and `temperature` are optional values, and thinking defaults to `false`.
Use parameters supported by the selected model and endpoint. Adapter availability does
not establish live-provider quality, feature support or a fixed response latency.
See the [provider implementations](../crates/corint-decision-llm/src/provider/) and
[configuration type](../crates/corint-decision-llm/src/generator/rule_generator.rs).

## Strict Core generation (experimental)

The opt-in Rust `CoreGenerator` creates or revises a **complete** CDL Core
closure, not an isolated rule string. It reuses the same compiler, real-engine
behavior tests and source-package builder as `corint validate / test / build`.
It does not require Corint Work. Current evidence uses fixed, recorded model
responses, not live-provider quality measurements or a Work product integration.

### Contract and trust boundary

The caller chooses an `LLMClient`, model, requirements, the
[input contract](../CDL/schema/input.json), and independent
[acceptance cases](testing.md). On revision the caller also supplies the entire
existing closure. Invalid input contracts, suites or existing closures fail
before a provider call.

The prompt embeds the applicable sections of the [CDL references](../CDL/overall.md):
document rules, Rule, Ruleset, Pipeline, Registry, expressions, input/context and call semantics.
It selects the strict sections and excludes historical Registry fallback sketches and
compatibility namespace descriptions. It also includes the
[resource schema](../CDL/schema/core.json), [response schema](contracts/schema/generation-response.json)
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
one Registry. Use the [conformance fixture](../tests/conformance/cdl_core/manifest.yaml)
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

### Rust API

Optional [public target contracts](contracts/README.md) provide field meanings,
units and declared deployment constraints without changing CDL syntax:

```rust,ignore
let contracts = corint_decision_toolchain::contracts::TargetContracts::load(
    &business_context_source, &target_capabilities_source,
)?;
let result = generator.generate_for_target(
    requirements, &input_schema, &acceptance_cases, &contracts,
).await?;
let revision = generator.revise_for_target(
    change_request, &existing_sources, &input_schema, &acceptance_cases, &contracts,
).await?;
```

These methods send both declarations to the selected provider, but never the
acceptance cases. Input/context mismatch fails before a provider call; generated
sources must pass shared target compatibility and independent behavior checks.
`result.compatibility` binds the same policy identity as the resulting package.
It reports declared compatibility only, not live availability, business semantics
or authorization. A compatible candidate can still fail behavior tests and return
no package. Existing `generate` / `revise` remain target-independent and return no
compatibility report. Package v1 does not embed or authenticate this separate report.

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

### Portability and compatibility

The package format and policy hashes are unchanged. Evidence binds the **host
executable** running the shared toolchain, which is the CLI executable for CLI
builds, or the embedding application for generator builds. `corint verify`
therefore cannot certify evidence produced by a different host binary: it
returns `E_TOOL_MISMATCH`. Use [`corint export / import`](packages.md#source-exchange) to
export/review the embedded sources and rebuild with the target CLI and the
caller-owned suite to create fresh evidence; do not
remove or rewrite the fingerprint to bypass this check. Cross-host signed
attestation remains future work.

Enabling `core-generation` leaves the [compatibility generators](#compatibility-generators)
unchanged; their YAML output does not acquire a Core validation result.

### Regression gate

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

## Compatibility generators

`RuleGenerator`, `RulesetGenerator`, `PipelineGenerator` and `ServiceConfigGenerator`
return `Result<String>` from `generate(description)`. `DecisionFlowGenerator` returns
a `DecisionFlow`, with YAML strings in `documents` and per-type counts. Its `to_yaml()`
joins those documents with YAML separators. The generator-specific configuration names
are aliases of `RuleGeneratorConfig`; their `new` constructors take `Arc<dyn LLMClient>` and
the configuration.

Using the provider and configuration above, with `requirements: &str` supplied by the host:

```rust,ignore
let generator = corint_decision_llm::RuleGenerator::new(provider, config);
let yaml = generator.generate(requirements).await?;
```

Use `generate_with_metadata` instead when the provider response is needed; it returns
`(String, LLMResponse)`. It requests a generation rather than inspecting an existing draft.
Extraction and resource-prefix checks do not compile a policy, resolve dependencies or
execute acceptance cases. Even a `DecisionFlow` is not a verified complete Core collection.
Review the draft against the [current CDL definitions](../CDL/overall.md), validate it
through the [intended execution entry](runtime-validation.md), and run independent tests
before the caller stores or deploys it. For a complete Core candidate with those checks
built into generation, use `CoreGenerator`.
