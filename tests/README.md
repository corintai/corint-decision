# CORINT Decision Engine - E2E Tests

These suites cover different execution paths; neither certifies the entire solution.

## Datasource and business-policy E2E

`run_e2e_tests.sh` preserves the original fixture generation → real database → real
server → HTTP business-decision workflow. It runs 43 SQL business/list cases
(the original 38 plus five database-list cases), the Redis lookup suite, or the
RisingWave materialized-view lookup suite.

```bash
python3 -m venv .venv-e2e
source .venv-e2e/bin/activate
python -m pip install -r tests/scripts/requirements-e2e.txt
bash tests/scripts/run_e2e_tests.sh --sqlite
bash tests/scripts/run_e2e_tests.sh --redis
```

Prerequisites: Cargo, Python 3, PyYAML, curl, jq and sqlite3. Redis additionally
requires the Python redis package and redis-cli; install redis-server to let the
runner start a private, ephemeral Redis instance. Cargo builds the current checkout
with the engine's Redis feature. Set `CARGO_NET_OFFLINE=true` when dependencies are
already cached. The runner works from any directory; noninteractive runs default
to SQLite.

Each run copies fixtures into a temporary directory, uses dynamic loopback ports,
generates distinct decision/publisher credentials, and cleans up only its own
processes. It leaves repository configuration, fixtures and existing services alone.
Background data uses `CORINT_E2E_SEED` (default `20260907`); timestamps remain relative
to the current UTC time. The two spending-spike fixtures include multiple events
within 24 hours so their intended review scenarios do not also trigger the
concentration rule. These cases assert both the decision and the target rule hit.
The high-total-spending, micro-pattern and wide-range cases assert the target rule
and approve: their individual scores (40/35/35) are below the policy review
threshold of 50. The missing-fields case omits optional metadata while retaining the policy's
required amount and aggregation dimensions.

Additional backends require explicit **disposable test endpoints**:

```bash
POSTGRES_URL=postgresql://localhost/corint_e2e bash tests/scripts/run_e2e_tests.sh --postgres
CLICKHOUSE_URL=http://localhost:8123 bash tests/scripts/run_e2e_tests.sh --clickhouse
RISINGWAVE_URL=postgresql://root@localhost:4566/dev bash tests/scripts/run_e2e_tests.sh --risingwave
bash tests/scripts/run_e2e_tests.sh --all
```

PostgreSQL requires psql and tests acknowledged decision persistence using the
checked-in PostgreSQL schema. External PostgreSQL/ClickHouse test tables are reset;
external Redis, if selected with `REDIS_URL`, has its `e2e_features:*` keys replaced.
Use dedicated test databases, never business endpoints. ClickHouse uses its default
database for aggregations and a private SQLite sidecar for SQL-backed lists.
The runner never downloads or starts ClickHouse automatically. `--all` reports
unconfigured backends as **skipped**, continues after a backend failure, and returns
nonzero if any executed backend fails. Skipped backends are not passing evidence.

### RisingWave tests

RisingWave additionally requires `psql` and an explicitly configured `RISINGWAVE_URL`
for a running test instance. The runner verifies the server identifies itself as
RisingWave; a PostgreSQL instance cannot substitute for this E2E suite. It creates
a random `corint_e2e_*` schema containing a transaction table and a rolling one-hour
materialized view, generates the host `feature_mappings`, and starts a private
Corint HTTP server. The database role needs permission to create and drop these
objects. The runner removes its schema on exit without stopping RisingWave.

The 10 HTTP cases cover low/high velocity, entity isolation, old/future event
exclusion, missing-row fallback, SQL-injection-like keys, missing-key errors,
updates without stale cache, deleted groups, view-query failure and recovery.
Mutations are followed by `FLUSH` before assertions; these tests do not measure
production ingestion latency. Decisions and relevant Rule hits are asserted.
`--all` includes RisingWave when `RISINGWAVE_URL` is set; otherwise it reports it
as skipped. Explicit `--risingwave` with a missing/unavailable endpoint fails.

For the faster connector/FeatureExecutor checks, keep the Rust tests as well:

```bash
cargo test -p corint-decision-runtime --features sqlx,redis --test risingwave_lookup --locked
CORINT_TEST_RISINGWAVE_URL=postgresql://root@localhost:4566/dev \
  cargo test -p corint-decision-runtime --features sqlx,redis --test risingwave_lookup --locked -- --ignored
```

Those tests cover type decoding, SQL NULL, key validation, cache TTL and timeout
semantics. They complement the HTTP E2E suite rather than replacing it.

### Reports

Reports default to an ignored `tests/results.XXXXXX/` directory. Override with
`CORINT_E2E_RESULTS_DIR`. Each report contains case counts and results, per-case
request/response JSON, and server logs. Setup/build/startup errors produce a
`setup_failed` report and nonzero exit. HTTP transport errors, HTTP error statuses,
malformed responses and wrong decisions fail the case. All business cases continue
after individual assertion failures. Set `CORINT_E2E_LOG_LEVEL=debug` for diagnostics.
Server startup allows 60 seconds; override `CORINT_E2E_STARTUP_TIMEOUT_SECS` for
slower hosts (for example, a first launch of a newly built macOS binary).

## Strict CDL Core process E2E

```bash
bash tests/scripts/run_core_e2e_tests.sh --offline
```

Fixed model output → real generator → real CLI → real loopback HTTP server,
including activation rejection, decision/Trace parity and process restart.
Uses temporary directories and dynamic ports, without business databases.
See [Core E2E instructions and evidence boundaries](CORE_E2E.md).

Test the fixture conversion tools without a database service:

```bash
python3 -m unittest discover -s tests/scripts -p test_e2e_tools.py
```
