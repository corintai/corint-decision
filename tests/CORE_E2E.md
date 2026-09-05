# CDL Core process e2e

This suite exercises a fixed model response through the real Core generator,
the real `corint` CLI, and a real `corint-decision-server` child process over TCP.
It is separate from the legacy datasource suite and the in-process router tests.

## Run

From the repository root, with Rust/Cargo, Python 3 and `protoc` available:

```bash
bash tests/scripts/run_core_e2e_tests.sh
# Once dependencies are cached:
bash tests/scripts/run_core_e2e_tests.sh --offline
```

The runner builds the server with the locked dependency graph, reads its exact
executable path from Cargo JSON, and runs the `core_process_e2e` CLI test target.
It uses nextest when installed, otherwise `cargo test`. Cargo supplies the CLI
binary path; neither executable is selected from PATH or an old release build.

The test target is opt-in via the CLI's `process-e2e` feature. Ordinary
`cargo test -p corint-decision-cli` does not include it. For workspace
`--all-features` runs or coverage, first build and explicitly provide the server:

```bash
CORINT_E2E_SERVER=$(bash tests/scripts/run_core_e2e_tests.sh --build-only)
export CORINT_E2E_SERVER
cargo test --all-features --workspace --locked
```

If building the server fails, stop; do not run the next commands. The CI workflow
uses fail-fast shell execution. An enabled process test without a valid
`CORINT_E2E_SERVER` fails explicitly; it never silently skips. The Rust test does
not recursively invoke Cargo. `--build-only` also accepts `--offline`.

## Assertions

The two Rust tests cover:

- Fixed, synthetic model output → actual `CoreGenerator` acceptance → YAML files
  → CLI `validate`, `build`, `verify`, `export` → source bundle. CLI JSON reports,
  exit codes and absent business/publication approval are checked.
- Operator-owned initial bundle, context, target, acceptance cases and pinned
  allowlist → actual server bootstrap → authenticated readiness check.
- Real HTTP decisions at boundary/probe inputs: exact score, signal, actions,
  triggered rules, explanation, execution records and policy revision. Trace
  on/off must leave the full result unchanged; Trace must expose the observed
  rule condition outcome.
- Publisher-only activation: absent, wrong and decision-role credentials cannot
  activate. An approved candidate changes both the revision and probe decision.
- A candidate that passes deliberately weaker author cases but fails independent
  server cases is rejected. Unapproved content, stale revision and injected
  approval fields are also rejected. Each failure preserves the active receipt
  and the previous strategy's decision behavior.
- Invalid event input is rejected; legacy `/v1/decide` is unavailable in Core mode.
- An actual process restart reloads the configured initial bundle (activation is
  currently in-memory only), creates a fresh revision and rejects stale revisions.
- An unsupported startup config exits with its expected diagnostic, without
  logging a listening endpoint or falling back to legacy configuration.

Source: [core_process_e2e.rs](../crates/corint-decision-cli/tests/core_process_e2e.rs).
The [Core HTTP router tests](../crates/corint-decision-server/tests/core_activation.rs)
remain complementary, including concurrency and additional rejection cases.

## Isolation and diagnostics

Each test owns its temporary directory. Author files and operator acceptance files
are separate; existing fixtures are read-only. The server itself binds
`127.0.0.1:0`, so concurrent tests do not share a fixed port or a port reservation.
Children receive a cleared environment plus only the necessary synthetic test
credentials/configuration. HTTP proxies and redirects are disabled.

CLI waits and server readiness are bounded at 45 seconds (an in-flight HTTP
readiness request has its own 15-second timeout). Each HTTP request has a
15-second timeout. Child stdout/stderr go to per-child files, preventing pipe
buffer deadlocks. On success, panic or handled timeout, RAII kills/reaps only the
owned child and the temporary directory is removed. Panic diagnostics include
sanitized child logs in test output. As with normal RAII, forcibly killing the
test harness itself can bypass cleanup; CI job teardown is the final boundary.

The suite does not edit `config/server.yaml`, use a shared PID file, kill by
process name, download a database, truncate tables or connect to business data.

## Evidence boundary and CI

The [Core CI job](../.github/workflows/ci.yml) runs this suite after installing
`protoc`. The workspace all-features test and coverage jobs also prepare the
required server binary. A configured gate is not evidence of a completed remote
CI run, and subprocess execution is not a claim that coverage instrumentation
measures every line executed inside those children.

These tests prove the synthetic Core delivery/execution path, **not** live model
quality, real Corint Work integration, production data effectiveness, durable
publication, distributed rollout, audit persistence or datasource compatibility.
`business_evaluation` remains `not_performed`.
