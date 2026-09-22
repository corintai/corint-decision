<div align="center">

# Corint Decision

**High-performance, AI-augmented risk decision engine with unified DSL**

*Part of the **CORINT AI – Agentic Risk Operations Platform***

[Overview](#-overview) •
[Key Features](#-key-features) •
[Quick Start](#-quick-start) •
[Documentation](#-documentation)

[![License](https://img.shields.io/badge/license-Elastic-blue.svg)](LICENSE)
[![Documentation](https://img.shields.io/badge/docs-latest-green.svg)](CDL/)

</div>

---

## 🚀 Overview

For Agent/Skill authoring across all seven CDL resource kinds, use the
[offline static validation CLI](docs/cli.md). Experimental strict CDL Core also provides the
[behavior testing CLI](docs/testing.md), [source packages and exchange](docs/packages.md),
[strict generation API](docs/generation.md)
and [language references](CDL/overall.md). Compile-time validation is
not behavior testing, business evaluation or production certification.

**CORINT Decision** is a Rust decision engine with YAML policies, HTTP/gRPC APIs,
online feature computation, and tools for authoring and testing policies.

A Registry selects a Pipeline. Rules contribute scores, Rulesets turn rule results
into signals, and the Pipeline chooses the final decision and action intents.
Applications consume those intents to perform actions such as blocking a payment
or opening a review case.

The default server runs the compatibility engine with online Feature, List and
Service integrations. Strict CDL Core and the multi-tenant host use separate
configuration and admission paths; their capabilities are documented separately.

For a complete runnable example, see the
[simulated transaction policy](tests/policies/simulated-data/pipelines/transactions.yaml)
and its [Registry](tests/policies/simulated-data/registry.yaml). It defines two rules,
one Ruleset and one Pipeline, with explicit approval, review and decline outcomes.

---

## ✨ Key Features

### 🎯 Corint Definition Language (CDL)

CDL covers seven resource kinds: Rule, Ruleset, Pipeline, Registry, Feature, List
and Service. The [language overview](CDL/overall.md) describes their roles and the
separate static-validation and execution profiles.

Multiple rules or rulesets in one YAML document use a single `rule:` or `ruleset:`
key with a sequence of resource objects. A single resource may use an object.
Repeated keys are rejected; explicit `---` document separators are also supported
by the authoring loader.

Imports compose resources across files. Compatibility Rulesets can use `extends`:
parent rules precede additional child rules (duplicate IDs are removed), and a child's nonempty `conclusion` replaces the
parent conclusion. See [imports and inheritance](CDL/import.md) for profile limits.

### 📊 Feature Engineering

The [Feature executor](crates/corint-decision-runtime/src/feature/executor.rs)
implements the following methods; the configured datasource must also support them:

| Feature type | Implemented behavior |
| --- | --- |
| Aggregation | `count`, `sum`, `avg`, `min`, `max`, `distinct`, `stddev`, `median`, `percentile` |
| State | `time_since`: elapsed time since the earliest matching timestamp |
| Expression | Arithmetic over Feature values and numeric event fields, with dependency resolution |
| Lookup | Read a value from a configured Feature Store, with optional fallback |

Features can be computed on demand and cached according to their cache settings.
Sequence, Graph, statistical State methods and model inference are not implemented
in this Feature contract. See [Feature syntax and backend constraints](CDL/feature.md)
and the [authoring examples](tests/conformance/cdl_authoring/features/payment.yaml).

### 📋 Custom Lists

Rules can test membership using `in list.<id>` and `not in list.<id>`.
The runtime implements Memory, File, SQLite and PostgreSQL list backends;
PostgreSQL requires the SQL support feature and a configured pool. Redis appears
in the configuration enum but its loader currently returns an unsupported error.
Expiration, caching and update support depend on the backend. The Decision server
does not expose a list-management REST API.

See the [List contract](CDL/list.md) for declarations and lookup behavior. Event
fields in rule conditions use `event.<field>`; caller-supplied `user` context is not
accepted by the Decision HTTP API.

### 🤖 LLM Authoring and Service Integrations

The [LLM crate](crates/corint-decision-llm/src/lib.rs) supplies OpenAI, Anthropic,
Gemini and DeepSeek adapters for policy generation and offline analysis. Its
response cache belongs to these authoring workflows; it is not a decision-result
cache. Provider access and credentials must be configured by the host.

Online integrations use a logical `service` and `operation` with a runtime binding.
See the [Service contract](CDL/service.md). A generated policy still needs static
validation and appropriate behavior tests before use.

### 🔄 Modular Architecture

| Component | Responsibility |
| --- | --- |
| `corint-decision-model`, `corint-decision-dsl-parser`, `corint-decision-compiler` | Language types, parsing, validation and compilation |
| `corint-decision-runtime` | Execution, features, lists and datasource integrations |
| `corint-decision-repository` | Policy loading from filesystem, PostgreSQL or HTTP |
| `corint-decision-engine` | Decision orchestration, request IDs and shared policy snapshots |
| `corint-decision-sdk`, `corint-decision-ffi` | Rust API and foreign-language integration |
| `corint-decision-server` | HTTP/gRPC endpoints, authentication and server lifecycle |
| `corint-decision-cli`, `corint-decision-toolchain`, `corint-decision-mcp` | Validation, testing, packages and MCP tools |
| `corint-decision-llm` | LLM clients and policy generation |

The repository includes C FFI and Python, Java and Node.js/TypeScript binding
sources. See [FFI setup and examples](crates/corint-decision-ffi/README.md) for their
build requirements. The workspace has no browser/WASM execution entry point.

### 🗄️ Policy Storage and Reload

The compatibility repository layer supports:

| Backend | Implementation and requirements |
| --- | --- |
| Filesystem | `FileSystemRepository`; local YAML resources and ID lookup |
| PostgreSQL | `PostgresRepository`; enable the `postgres` Cargo feature and prepare its tables |
| HTTP API | `ApiRepository`; manifest-based resource discovery and optional Bearer authentication |

The server includes HTTP repository support. For PostgreSQL policy storage, build
with `cargo build --locked -p corint-decision-server --features postgres`.
Repository loads return the parsed resource and its raw source text. PostgreSQL
saves increment a version column on the current row; this is not an append-only
history of all policy versions or tenant isolation by itself.

An authenticated repository reload prepares a replacement engine and atomically
switches the shared HTTP/gRPC snapshot. In-flight decisions retain their original
snapshot. Use reload explicitly after policy changes; restarting is required for
server listener or authentication configuration changes. Strict Core publication
and tenant isolation have [separate contracts](docs/contracts/multi-tenancy.md).

### 🔍 Observability and Performance

- Structured logging controlled by `RUST_LOG` and HTTP request tracing.
- Optional execution traces and triggered-rule evidence in decision responses.
- Publisher-only metrics and persistence-status endpoints.
- Optional PostgreSQL result persistence through a background write queue.
- Feature caching and short-circuit condition evaluation.

Persistence requires `database_url` or `DATABASE_URL` and SQL support. The
`x-corint-persistence: queued` response header reports queue admission, not a
confirmed database commit. Check `/v1/persistence` for background write status.

Latency and throughput depend on policies, datasource calls, caching, hardware
and client concurrency. Use the simulation workflow below to measure your setup;
this README does not prescribe a fixed p99 or requests-per-second guarantee.

---

## 🎯 Use Cases

Policies and connected data can support the following workflows. Verification,
biometric processing and model scoring require application-specific integrations.

### Fraud Detection

- Real-time transaction monitoring
- Account takeover detection
- Payment fraud prevention

### Identity Verification

- KYC risk assessment
- Document verification
- Behavioral biometrics

### Credit Risk

- Loan application evaluation
- Credit scoring
- Income verification

### Compliance & AML

- Transaction monitoring
- Sanctions screening
- PEP detection

---

## 🚀 Quick Start

Start with the interactive demo or manual setup, then replay simulated transactions
to test decisions and concurrent requests.

### Option 1: Interactive Demo (Recommended)

The quickest way to see CORINT in action is using the interactive demo script:

```bash
# Clone repository
git clone https://github.com/corintai/corint-decision.git
cd corint-decision

# Requires Rust, protoc, jq, curl and lsof; SQLite mode also needs sqlite3
# The demo builds the server itself

# Run the interactive demo
./quickstart/decide_demo.sh
```

**The demo script will:**

- ✅ Let you choose data source (SQLite/PostgreSQL/ClickHouse/Redis/RisingWave)
- ✅ Automatically initialize test data
- ✅ Build and start the server
- ✅ Provide an interactive menu with 11 pre-configured fraud detection scenarios
- ✅ Show request/response for each test case
- ✅ Validate expected vs actual decisions

The demo bootstraps a temporary administrator, then requests decision and publisher
tokens from the credential API. Only their hashes are persisted in a private SQLite
control database under `temp/`; HTTP and gRPC share the database credential cache.
Previously exported decision/publisher tokens are not reused by the demo. The tenant
defaults to `local`; set `CORINT_TENANT_ID` to override it.
Occupied ports are left running; the demo selects
available ports and prints the endpoints. It prepares a repository under `temp/`
containing the transaction and login scenarios, so SQLite mode does not require
Redis or the PostgreSQL list examples. Unless Redis is selected, profile lookups
are disabled (their feature values are `null`); event history features still query
the selected database. HTTP errors are displayed with their status and original
response body. Auto-run exits with a nonzero status if any scenario fails.

Single-tenant deployments default to tenant `local` when the tenant ID is omitted.
For a shared HTTP host with tenant/environment isolation, Agent delegation and
independent lifecycle controls, see the [multi-tenancy guide](docs/contracts/multi-tenancy.md)
and run `python3 quickstart/tenant_demo.py --output /tmp/corint-tenants-demo`.

### Option 2: Manual Setup

For a minimal local server, use the bundled amount-screening policy. It needs no
external datasource and does not modify `repository/`. From the repository root,
stop any running workspace Decision service, back up any existing
`config/server.yaml`, then use this configuration:

```yaml
server:
  host: "127.0.0.1"
  port: 8080
  grpc_port: 50051
repository:
  type: filesystem
  path: "tests/policies/simulated-data"
datasource: {}
```

Install Rust, `protoc`, Python 3, `curl` and `lsof`. In a local shell without
previous CORINT or database configuration overrides, start the server with two
distinct credentials (each must contain 32–1024 printable non-space characters):

```bash
export CORINT_DECISION_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
export CORINT_PUBLISHER_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"

# Builds the server and waits for readiness; leaves it running in the background
./scripts/start.sh --only decision

curl http://127.0.0.1:8080/health

# This transaction matches the test policy's manual-review rule
curl -X POST http://127.0.0.1:8080/v1/decide \
  -H "Authorization: Bearer ${CORINT_DECISION_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"event":{"TRANSACTION_ID":1,"TX_AMOUNT":180}}'

# Stop when finished
./scripts/stop.sh --only decision
unset CORINT_DECISION_TOKEN CORINT_PUBLISHER_TOKEN
```

`CORINT_AUTH_CONFIG` selects database authentication instead of these environment
credentials. This minimal environment-auth setup does not initialize the SQLite
credential database required by managed CSV replay below; use Option 1 for that.
For the default `repository/` policies, configure their datasources and external
services before starting. See [service startup](docs/mcp.md#同时启动-decision-与-mcp)
for process management, logs and environment handling.

### Simulated Transaction Replay

Use [the replay script](scripts/replay_simulated_data.py) to send transactions from
[tests/data/simulated-data.csv](tests/data/simulated-data.csv) to `/v1/decide`.
It preserves the original field names and removes `TX_FRAUD` and
`TX_FRAUD_SCENARIO` before each request.

Run these commands from the repository root with Python 3 and the Rust toolchain:

```bash
# Build the local Decision server
cargo build --locked -p corint-decision-server

# Preview three request payloads without starting a server or sending requests
python3 scripts/replay_simulated_data.py --dry-run --limit 3
```

**Authentication setup:** Local replay requires an initialized SQLite credential
database. If you have not created one, run the interactive demo above with SQLite,
wait for the server and credentials to be ready, then choose `0` to exit and stop
the demo. Its authentication configuration and database remain under
`temp/demo_auth_*/`. Pass that run's `auth.json` using
`--auth-config /absolute/path/to/auth.json` on each replay command below. This
option can be omitted when your managed local service already has a SQLite
authentication configuration; the script reuses it automatically.

```bash
# Use local managed mode, with automatic credentials and test policy loading
unset CORINT_SERVER_URL

# Replay the first 100 rows in order, with a 0.1-second delay between requests
python3 scripts/replay_simulated_data.py --limit 100 --interval 0.1

# Replay 4,000 rows with up to 8 requests in flight and no additional delay
python3 scripts/replay_simulated_data.py --limit 4000 --concurrency 8 --interval 0
```

Before sending requests, the script registers a temporary decide-only credential
in SQLite, sets `CORINT_DECISION_TOKEN` for its own process, and starts or restarts
Decision with the [dedicated test policy repository](tests/policies/simulated-data/).
It does not modify the default `repository/` policies. On completion, errors, or
Ctrl+C, it stops the test service, removes the temporary credential, restores the
previous token environment, and restarts the original service if it was running.
There is no need to set a test token manually. Passing `--url` selects an existing
service instead and skips this automatic credential, service, and policy setup.

The test policy uses only `TX_AMOUNT`, in the dataset's amount units:

| Amount | Decision |
| --- | --- |
| `TX_AMOUNT > 220` | `decline` |
| `150 < TX_AMOUNT <= 220` | `review` |
| `TX_AMOUNT <= 150` | `approve` |

Each completed request produces a JSON result on stdout. The final summary on
stderr prints decision counts, rule hit counts and rates, throughput, and average
and maximum request latency. With the bundled dataset and test policy, the first
100 rows are all approved with no rule hits. The first 4,000 rows yield 3,907
approvals, 92 reviews, and 1 decline: 93 transactions hit a rule.

Concurrency defaults to `1`; higher values allow responses to finish out of order.
Use `--start-row` to select a different starting data row, or a positional CSV path
to replay another file with the same schema. Omitting `--limit` replays the entire
dataset of 1,754,155 transactions. Run
`python3 scripts/replay_simulated_data.py --help` for all options.

### API Endpoints

#### REST API

These endpoints describe the default compatibility server. `/health` is public;
other endpoints require `Authorization: Bearer <token>` with the indicated role.

| Method | Path | Purpose | Permission |
| --- | --- | --- | --- |
| GET | `/health` | Health and policy revision | None |
| POST | `/v1/decide` | Execute the selected Pipeline | `decide` |
| POST | `/v1/repo/reload` | Prepare and activate a replacement policy snapshot | `publish` |
| GET | `/v1/metrics` | Metrics for the current snapshot | `publish` |
| GET | `/v1/persistence` | Background persistence status | `publish` |

With database authentication, `/v1/tenancy/credentials` also provides credential
management under its own authorization rules. See the
[credential and tenancy contract](docs/contracts/multi-tenancy.md).

**Request example** for the test repository used in Manual Setup:

```json
{
  "event": {
    "TRANSACTION_ID": 1,
    "TX_AMOUNT": 180
  }
}
```

Only `event` is caller-owned. The optional `options.enable_trace` and
`options.return_features` flags request additional output. Non-null `user`,
`features`, `service`, `llm` or `vars` inputs, `event.tenant_id`, and
`options.async: true` are rejected.

**Response example** (request ID and elapsed time vary):

```json
{
  "request_id": "rq_qy1Gj3_0QdetsUXNJo",
  "status": 200,
  "process_time_ms": 1,
  "pipeline_id": "simulated_transaction_risk",
  "decision": {
    "result": "review",
    "actions": [],
    "scores": {
      "canonical": 11,
      "raw": 50
    },
    "evidence": {
      "triggered_rules": ["simulated_amount_review_band"]
    },
    "cognition": {
      "summary": "TX_AMOUNT is greater than 150 and at most 220",
      "reason_codes": []
    }
  }
}
```

Results are lowercase: `approve`, `decline`, `review`, `hold` or `pass`.
The canonical score uses the server's sigmoid normalization on a 0–1000 scale.
Request IDs have the form `rq_<6-character process segment>_<11-character ID>`;
the process segment is shared by requests generated within one process.

#### gRPC API

The compatibility server starts gRPC when `server.grpc_port` is configured.
Service `corint.decision.v1.DecisionService` provides `Decide`, `HealthCheck` and
`ReloadRepository`. Decide and reload use the same Bearer roles as HTTP, supplied
as `authorization` metadata; health is public.

With `grpcurl` installed and the Manual Setup server running, use the same
`CORINT_DECISION_TOKEN` in your shell:

```bash
grpcurl -plaintext localhost:50051 corint.decision.v1.DecisionService/HealthCheck

grpcurl -plaintext -H "authorization: Bearer ${CORINT_DECISION_TOKEN}" -d '{
  "event": {
    "TRANSACTION_ID": {"int_value": "1"},
    "TX_AMOUNT": {"double_value": 180.0}
  }
}' localhost:50051 corint.decision.v1.DecisionService/Decide
```

See the [gRPC protocol definition](crates/corint-decision-server/proto/decision.proto)
for message shapes. Server handlers enforce supported fields and options in
addition to the Protocol Buffers schema.

### Configuration

The compatibility server reads `config/server.yaml` (if present), `.env` and
`CORINT_*` environment settings. Nested `server:` settings take precedence over
legacy top-level listener settings. HTTP defaults to port 8080; gRPC is disabled
unless a port is configured. Use the actual startup endpoints when ports differ.

- `CORINT_AUTH_CONFIG` selects the authentication database configuration; otherwise
  `CORINT_DECISION_TOKEN` and `CORINT_PUBLISHER_TOKEN` are required.
- `CORINT_REPOSITORY_PATH` overrides the filesystem policy repository for one
  process without changing the saved configuration.
- `CORINT_CORE_CONFIG` and `CORINT_TENANT_CONFIG` select separate server modes and
  cannot be enabled together. The compatibility examples above do not apply to
  their full API and configuration contracts.
- Feature datasource names bind to entries under `datasource:`. Server-supplied
  entries take precedence over repository datasource configuration. Names such as
  `events_datasource` are bindings you configure, not automatic database discovery.

The HTTP router installs CORS middleware to expose policy/persistence headers,
but does not configure permissive cross-origin access for arbitrary browser sites.

### Logging

With configuration and credentials already set, run a foreground server with:

```bash
# Basic log levels
RUST_LOG=info cargo run -p corint-decision-server      # Info
RUST_LOG=debug cargo run -p corint-decision-server     # Debug (detailed)
RUST_LOG=trace cargo run -p corint-decision-server     # Trace (all details)
```

### Troubleshooting

**Server won't start:**

- Ensure credentials are configured for the selected authentication mode.
- Check if port is in use: `lsof -i :8080`
- Verify the configured repository directory and its `registry.yaml` exist.
- View detailed logs: `RUST_LOG=debug cargo run -p corint-decision-server`

**Rules not loading:**

- Ensure rule files have `.yaml` or `.yml` extension
- Check rule file syntax
- View server startup logs for rule loading information

**Feature calculation fails:**

- Verify the database connection
- Check data source configuration in `config/server.yaml` (datasource section)
- Check features configuration (`repository/features/*.yaml`)
- Verify test data exists in database
- Ensure logical datasource names (`events_datasource`, `lookup_datasource`) are properly mapped

**API errors:**

- `401`: supply a valid credential with the endpoint's required permission.
- `400`: check request fields and supported options; computed namespaces are server-owned.
- Execution errors: check the response and server logs, required event fields,
  datasource bindings and external dependencies.
- `pass` or no rule hits: check Registry routing, Pipeline decision defaults and
  the input values; an HTTP 200 alone does not mean a rule matched.

---

## 📚 Documentation

### CDL Documentation

#### CDL Overview

| Document | Description |
|----------|-------------|
| [**overall.md**](CDL/overall.md) | Resource roles, execution flow, capability boundaries and a complete Core example |

#### CDL Core Concepts

| Document | Description |
|----------|-------------|
| [**expression.md**](CDL/expression.md) | Expression language reference |
| [**rule.md**](CDL/rule.md) | Rule specification and patterns |
| [**ruleset.md**](CDL/ruleset.md) | Ruleset and decision logic |
| [**pipeline.md**](CDL/pipeline.md) | Pipeline orchestration |
| [**registry.md**](CDL/registry.md) | Pipeline Registry |

#### Advanced Features

| Document | Description |
|----------|-------------|
| [**import.md**](CDL/import.md) | Import syntax, resource composition and dependency constraints |
| [**context.md**](CDL/context.md) | Context and variable management |
| [**feature.md**](CDL/feature.md) ⭐ | **Feature definitions and supported semantics** |
| [**list.md**](CDL/list.md) ⭐ | **Custom lists (blocklists/allowlists)** |
| [**service.md**](CDL/service.md) | Service operations, HTTP bindings and custom adapters |

### Tools and Integration

| Document | Description |
|----------|-------------|
| [**Architecture**](docs/ARCHITECTURE.md) | System architecture reference |
| [**API Request**](docs/API_REQUEST.md) | Request namespaces and compatibility API reference |
| [**Static CLI**](docs/cli.md) | Validate all seven CDL resource kinds |
| [**Behavior testing**](docs/testing.md) | Strict Core behavior testing and execution limits |
| [**MCP**](docs/mcp.md) | Local MCP tools and service startup |
| [**CDL Studio**](web/README.md) | Pipeline visualization and source editing |
| [**Multi-tenancy**](docs/contracts/multi-tenancy.md) | Tenant scopes, credentials and publication |

---

## 🤝 Contributing

We welcome contributions! Here's how you can help:

- 🐛 **Report bugs** - Open an issue with reproduction steps
- 💡 **Suggest features** - Share your ideas in discussions
- 📝 **Improve docs** - Help make our documentation better
- 🔧 **Submit PRs** - Fix bugs or implement new features

## 📄 License

This project is licensed under the **Elastic License 2.0**.

---

<div align="center">

**Built with ❤️ for the risk control community**

If you find CORINT useful, please give us a ⭐ on GitHub!

</div>
