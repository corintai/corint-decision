# CDL implementation and conformance

This guide covers compiler APIs, engine integration, conformance, release criteria and performance measurement.
The language contract remains in [CDL reference](../CDL/overall.md); implementation details here do
not introduce additional syntax or capabilities.

## Compiler and engine APIs

The [Core compiler module](../crates/corint-decision-compiler/src/core.rs) provides the shared gates:

| API | Responsibility |
|---|---|
| `parse_core_input_schema(&source)` | Validate the input file against the public input schema and deserialize the existing model `Schema`. |
| `validate_core_document(&source)` | Check one source's YAML structure, version, fields and admitted capabilities. This does not validate a complete resource closure. |
| `compile_core(&sources, input_schema)` | Check the complete closure, input types, references and control flow, then compile all resources and Registry conditions. |
| `DecisionEngine::from_core(&sources, input_schema)` | Construct a closed-world engine from the shared compiler output. `decide` validates each input before policy execution. |

Callers supply `CoreSource { path, yaml }` for every resource and a
`corint_decision_model::types::Schema`. Source paths label diagnostics; these APIs do not
read the paths from disk, discover a repository, resolve imports or contact external services.
Supply exactly one Registry and its complete resource closure.

The [engine implementation](../crates/corint-decision-engine/src/decision_engine/engine.rs)
uses the offline Pipeline executor for Core. Compatibility builders retain their configured
connectors. Writing `version: "0.1"` alone does not switch a compatibility loader to Core.

## Compilation model

The current Core path performs these steps:

1. Validate the input schema and each resource's decoded YAML against the public resource schema.
2. Build the resource index, reject duplicate IDs and sources, and check Registry and call-graph requirements.
3. Check each resource's expressions, reference types, defaults and control-flow constraints;
   parse the existing AST and normalize conditions before generating instruction programs.
4. Attach source and condition maps, and compile Registry guards through the same compiler gate.
5. Assemble the engine's resource programs and execute them through the existing runtime.

The AST and instruction IR are Rust implementation types. Trace records are produced during
execution; they are not another compilation output between an AST and bytecode. Core disables
legacy dead-code elimination because it removes instructions without relocating jumps and
observation maps. See [condition observation internals](runtime-validation.md#条件观察的执行与数据边界)
and the broader [architecture guide](ARCHITECTURE.md).

## Public adapters

- [Validation CLI](cli.md) compiles an explicit file closure; [behavior testing](testing.md)
  executes declared examples through the Core engine with Trace off and on.
- [Source packages and exchange](packages.md) bind snapshots and evidence
  to their content. Their formats are tool contracts, separate from CDL resource syntax.
- [Import resolution](resolution.md) applies its own authoring profile and emits a frozen closure
  before Core compilation. Unresolved imports remain outside the execution profile.
- The [Core repository server](contracts/core-server.md) independently checks authorization and
  acceptance cases, then derives startup/reload state from the published repository.
- [Strict generation](generation.md) is opt-in. Compatibility generators do not provide Core
  acceptance evidence merely by emitting a language version.

Generation prompts assemble the relevant sections of the owning language references.
There is no separate copy of the language specification. Section boundaries are checked
by generation tests so a documentation reorganization cannot silently remove Registry
rejection rules or pull compatibility fallback sketches into strict prompts.

## Conformance and document checks

From the repository root, with dependencies cached:

```sh
python3 tests/scripts/check_docs.py
cargo test -p corint-decision-engine --test cdl_core_conformance --test document_assets --test registry_execution --locked --offline
```

Omit `--offline` if dependencies need downloading. With nextest installed, the equivalent engine
test selection is `cargo nextest run -p corint-decision-engine --test cdl_core_conformance --test document_assets --test registry_execution --locked`.
These synthetic suites do not require a Work account, business database or live LLM.
The full CI checks and feature-enabled workspace tests are defined in [ci.yml](../.github/workflows/ci.yml).

The [conformance runner](../crates/corint-decision-engine/tests/cdl_core_conformance.rs) runs complete
bundles through the public schema, real parsers, compiler and engine. It checks thresholds,
local and aggregate scores, first-match choices, action isolation, calls, paths, input errors and
Trace parity. Negative fixtures must fail at their registered stage and code.

The [example registry](../tests/conformance/documentation/examples.json) binds operational
documentation examples to the [Core fixture manifest](../tests/conformance/cdl_core/manifest.yaml)
and their executed sources. CDL pages link only within the language directory. Regression
resources and expected results live in `tests/conformance/`.

Historical pages have separate scope checks; the
[snippet registry](../tests/conformance/documentation/snippets.json) binds every fenced block
to its hash and admission classification. Executable wrappers keep their declared input
schemas and errors. Historical YAML in [legacy fixtures](../tests/conformance/documentation/legacy/)
must parse and produce its registered Core rejection. These checks do not certify
compatibility execution.

Examples still published in operational guides use `executable_examples` in the
[document inventory](inventory.json). The
[Registry tests](../crates/corint-decision-engine/tests/registry_execution.rs) execute complete
Pipeline/Ruleset/Rule fixtures and verify selection, scores, fallback, ordering, repeated target
IDs, guard rejection, short-circuiting, admission errors and Trace parity. Feature, Service
and List regression tests read their own fixtures without depending on example sections in CDL.

Internal keys such as `__executed_steps__`, `__ruleset_result__.<id>` and
`__core_rule_executions__` support test observations; they are not language namespaces or a
stable report/feedback format. Public result and Trace serialization are described in
[runtime validation](runtime-validation.md).

## Compatibility implementation boundaries

The shared implementation provides checked score overflow, nested Registry conditions,
expression precedence, Unicode string parsing and unary negation. Compatibility Ruleset
conclusions reject non-string conditions; missing List backends and invalid aggregation windows
fail explicitly. Other Core capabilities remain opt-in.

The earlier overall design listed runtime/server error types and AST error actions such as
Fallback, Skip, Fail and Retry. An enum or design sketch does not establish executable error
handling. Current Service fallback and timeout behavior are defined in the
[Service contract](../CDL/service.md); retry, circuit breakers and fallback chains must not be
inferred from those old implementation notes.

## Development and acceptance scope

The [capability inventory](contracts/schema/capabilities.json) is the current source for pending
work. It lists import dependency version ranges, full operand Trace, live-provider/Work generator
integration, multi-node distribution, cross-host publication trust and complete cross-product
interoperability. Full source spans and canonical audit serialization are not supplied by the
current condition observations. Local verification establishes evidence for the documented scopes.

Single-instance repository publication is implemented in its declared scope. Deployment and
operator approval are described in [Core server](contracts/core-server.md) and
[repository operations](contracts/core-operations.md); local compilation, tests and generation
results do not themselves authorize publication.

## Release acceptance

The current language version remains `0.1 / experimental`. In the capability inventory,
`implementation_status: implemented_in_declared_scope` describes the implemented scope;
`experimental` describes maturity. Pending multi-node or cross-host work does not negate an
implemented local entry point. Format and deployment identities are checked separately:

| Version or identity | Acceptance rule |
|---|---|
| Package, input, Trace and feedback format versions | Validate each serialization contract independently of the CDL language version. |
| Engine and checker identity | Recompile and validate under the new program after an upgrade; old evidence does not automatically authorize it. |
| Repository revision and policy fingerprint | Rebuild evidence when source content, dependencies or the input schema change. |

A stable-release candidate needs complete resource collections covering successful, boundary,
failing and unexecuted paths for every supported capability. Published supported examples and
generation templates must bind to executable assets. Check schema, parser, compiler and runtime
responsibilities separately, and compare shared semantics across optimization settings, Trace
modes and public entry points. Report external-dependency, product-integration and capacity
acceptance separately; an unconfigured environment is not a passing result.

Additional evidence includes the seeded [semantic property tests](../crates/corint-decision-engine/tests/semantic_properties.rs),
[generation prompt assets](../crates/corint-decision-llm/tests/prompt_assets.rs),
[Feature input binding](contracts/feature-pipeline.md) and [record/replay tests](replay.md).
HTTP Service configuration generation targets the online runtime and does not establish strict
Core Connector support.

## Performance measurement

Run from the repository root, choosing new output files:

```sh
python3 tests/scripts/run_core_benchmark.py --samples 1000 --output target/core-benchmark.json
python3 tests/scripts/run_core_http_benchmark.py --samples 200 --output target/http-benchmark.json
```

The engine report covers 16/128/512 rules, concurrency 1/4/16 and Trace off/on, recording compile
time, P50/P95/P99, throughput and child-process peak RSS. The HTTP report uses real CLI/server
processes, a random loopback port, synthetic inputs and a v3 SQLite journal, with and without
concurrent reload. After warmup each load group clears only its own temporary journal, starting
with no history. The HTTP figures include connection setup, the Python client and journal costs;
compare them separately from in-process engine results. `--profile debug` is for HTTP functional
checks; performance measurement defaults to release.

```sh
python3 tests/scripts/run_core_benchmark.py --samples 1000 --baseline target/core-benchmark.json --max-regression 0.25 --output target/core-benchmark-next.json
```

Compare the same platform, build mode, thread count and workload. Exceeding the threshold returns
nonzero while retaining the report. Confirm noise on controlled hardware before setting a gate;
deployment owners supply business capacity targets, and benchmark results do not define an SLA.
The [manual performance workflow](../.github/workflows/cdl-performance.yml) runs these measurements.

### Deployment coverage

Local PostgreSQL/SQLite, strict Core HTTP and existing compatibility-protocol tests have separate
acceptance evidence. FeaturePipeline is an SDK entry point. Strict Core gRPC/FFI, live Work,
online model inference and multi-node publication require their own target/version evidence
with fixed policies, inputs and dependencies.

### Recorded measurements (2026-09-06)

On the same machine with 200 samples per group, release engine P95 for 512 rules at concurrency 1
changed from 31.19 ms to 6.45 ms; with Trace enabled it changed from 39.16 ms to 7.70 ms.
These are observations for that run. The [before](../tests/performance/baselines/2026-09-06-macos-aarch64-before.json)
and [after](../tests/performance/baselines/2026-09-06-macos-aarch64-after.json) reports retain all
18 workloads and build state. The [HTTP functional report](../tests/performance/baselines/2026-09-06-macos-http-debug.json)
covers 12 workloads in debug mode and is not a production-throughput estimate.
