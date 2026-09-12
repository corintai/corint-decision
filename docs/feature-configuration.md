# Feature data sources and compatibility configuration

<!-- cdl-scope: compatibility-unverified -->
> This page is an unverified compatibility reference. Its snippets are not Core support evidence.
> For the executable contract and supported examples, use [CDL reference](../CDL/overall.md),
> [Pipeline](../CDL/pipeline.md) and the [capability inventory](contracts/schema/capabilities.json).

Feature definitions and field semantics belong in the [CDL Feature reference](../CDL/feature.md).
This page covers authoring conventions and host configuration. The connection example is a
configuration reference; parsing or strict Core rejection does not verify a database connection.

## Current backend boundaries

The [Feature runtime contract](contracts/feature-runtime.md) records method support, filtering,
dependency checks and cache behavior. SQLite rejects `stddev`, `median` and `percentile`;
it does not use the historical `STDEV` or subquery workarounds. MySQL SQL execution is not
implemented. PostgreSQL SQL generation and acceptance against a target database are separate checks.

The [server configuration model](../crates/corint-decision-server/src/config.rs) maps
`options.max_connections` to pool size and `options.connection_timeout` from seconds to
runtime milliseconds. `options.query_cache_ttl_secs` defaults to zero; a nonzero TTL
explicitly accepts stale query results. These settings are host configuration, not Feature DSL fields.
Use the [server configuration example](../config/server-example.yaml) as the repository entry point.

## Use cases and naming

Choose a computation from the [Feature runtime contract](contracts/feature-runtime.md), then
define the entity, source fields, filters and window explicitly. These examples describe how
to compose supported method families; datasource acceptance still depends on the selected backend.

| Business question | Feature composition | Suggested name |
|---|---|---|
| Failed logins for one user in the last hour | `count`, user dimension, login/failure row filters, `1h` window | `cnt_userid_login_1h_failed` |
| Total transaction amount for one user in a day | `sum` over the amount column, user dimension, `24h` window | `sum_userid_txn_amt_24h` |
| Distinct devices using one IP in a day | `distinct` over the device column, IP dimension, `24h` window | `distinct_ip_device_24h` |
| Failed-login fraction | Numeric Expression dividing failed and total counts with matching filters and windows | `rate_userid_login_1h_failure` |
| Precomputed device reputation | Lookup using the device ID as the entity key | `device_reputation_score` |

For ratios, the caller must define how to handle a zero denominator; arithmetic errors propagate.
Session analysis requires the business system to supply session IDs and any session boundaries.
State `time_since` measures elapsed time from the **earliest** matching timestamp, so it cannot
stand in for time since the most recent activity. Keep thresholds and decision conditions in
Rules, with units and missing-data expectations documented by the application.

Use consistent `snake_case` names. Aggregates can follow
`<method>_<entity>_<event>[_<field>]_<window>[_<modifier>]`; Lookup names can describe the
stored value. Abbreviations such as `cnt` and `txn` are optional team conventions. A name
does not set the Feature type, method, datasource, dimensions, window or key. Configuration
must retain the actual source column names and event values, including their abbreviations.

Unimplemented Feature directions are summarized in the
[architecture roadmap](ARCHITECTURE.md#feature-directions).

## Feature Store keys

Lookup definitions pass a Feature name and a resolved entity key to the connector.
The Redis connector uses `feature_name:entity_key` when namespace is empty, and
`namespace:feature_name:entity_key` otherwise. The DSL `key` supplies only the entity
part. For `name: user_risk_score`, `key: "${event.user.id}"`, namespace `profiles` and
user ID `u1`, store the value at `profiles:user_risk_score:u1`.

The connector does not interpret `key` as an already complete Redis key. Existing
definitions that repeat a Feature prefix inside `key` retain that prefix as entity
data; migrate stored keys and definitions together if changing this layout.
The [executable Feature examples](../crates/corint-decision-engine/tests/feature_documentation.rs)
check the actual GET key against a local RESP test server, including missing values
and explicit server errors. This is connector coverage, not production Redis acceptance.

## Datasource configuration

Datasource connections belong in the host's `config/server.yaml`. This fragment shows PostgreSQL
aggregation and Redis Lookup settings; select a backend and validate it using the current runtime
contract. Expression features evaluate supplied values and do not query an SQL provider directly.

```yaml
datasource:
  # Events datasource (for aggregation features)
  postgres_events:
    type: sql
    provider: postgresql
    connection_string: "postgresql://user:password@localhost:5432/corint_risk"
    database: "corint_risk"
    events_table: "events"
    options:
      max_connections: "20"
      connection_timeout: "30"

  # Lookup datasource (for feature lookups)
  redis_features:
    type: feature_store
    provider: redis
    connection_string: "redis://localhost:6379/0"
    options:
      namespace: "user_features"
      default_ttl: "86400"
```

The compatibility engine builder can install `events_datasource` and `lookup_datasource` aliases
for configured sources. This is a loader convention. Explicit datasource names avoid relying on it;
see the [engine builder](../crates/corint-decision-engine/src/builder.rs).

## RisingWave direct Lookup

Use `type: feature_store`, `provider: risingwave` to read materialized views through
the PostgreSQL wire protocol. The runtime must be built with `sqlx` (enabled by the
server). This provider implements named Lookup, not Aggregation or arbitrary SQL.
It does not create sources, materialized views, indexes or credentials.

```yaml
datasource:
  rw_features:
    type: feature_store
    provider: risingwave
    connection_string: "postgresql://root@localhost:4566/dev"
    feature_mappings:
      user_txn_count_1h:
        schema: public
        view: user_features_mv
        key_column: user_id
        key_type: text
        value_column: txn_count_1h
    options:
      max_connections: "10"
      connection_timeout: "2"
      query_cache_ttl_secs: "0"
```

`feature_mappings` is a nonempty map keyed by the exact CDL Feature name. Each entry
requires `view`, `key_column` and `value_column`; `schema` defaults to `public` and
`key_type` defaults to `text`. `text` binds a VARCHAR entity key; `int64` parses the
resolved string as a signed 64-bit integer and binds BIGINT. Composite keys are not
supported; expose a single key column in the view. Identifiers are individually
quoted: put the schema in `schema`, not in a dotted `view` string. Unknown mapping
fields, empty identifiers, missing bindings and invalid int64 keys are rejected.

The key must uniquely identify a row. Queries select one value column with a bound
`$1` key and `LIMIT 2`, detecting duplicates rather than silently choosing a row.
Prepared statements are not retained across lookups, avoiding stale relation plans
when a materialized view is dropped and recreated on RisingWave 2.8.
Result types include boolean, smallint/integer/bigint, real/double, decimal/numeric,
text/varchar, JSON/JSONB, date, timestamp and timestamptz. Numbers use Corint's f64
representation (large integers and decimals may lose precision); nonfinite numbers
are errors. Strings remain strings even when they contain digits. JSON retains
arrays, objects and nested nulls. Dates and timestamps return ISO strings, with UTC
offsets for timestamptz and no invented offset for timestamp without time zone.
Unsupported non-null types fail; expose structured values as JSONB when needed.

For an existing ingested relation `transactions` with a VARCHAR `user_id` and
TIMESTAMPTZ `event_timestamp`, provision this view separately:

```sql
CREATE MATERIALIZED VIEW public.user_features_mv AS
SELECT user_id, COUNT(*) AS txn_count_1h
FROM public.transactions
WHERE event_timestamp >= NOW() - INTERVAL '1 hour'
  AND event_timestamp < NOW()
GROUP BY user_id;
```

The view maintains a rolling `[now - 1h, now)` window using RisingWave's streaming
clock. It is not a request-specific fixed-cutoff query. A user's group may disappear
when its final event expires; Lookup then returns null unless an explicit fallback
is configured. `fallback: 0` also covers query failures, so it cannot distinguish
"no activity" from "database unavailable". Sources, lag and late-event policies remain
deployment responsibilities. See RisingWave's [temporal filter documentation](https://docs.risingwave.com/processing/sql/temporal-filters).

Connections are opened lazily. The runtime `timeout_ms` bounds each lookup including
connection acquisition; server `options.connection_timeout` supplies this deadline
in seconds. Runtime `pool_size` comes from `options.max_connections`. RisingWave
Lookup uses runtime `query_cache_ttl_secs` (default 0), also set through server options;
its nonzero TTL caches found values, including SQL NULL. Missing rows and failures
are not cached. `default_ttl` and `namespace` are Redis settings and are ignored here.
Separate lookups can observe different streaming snapshots.

For standalone runtime datasource YAML, use the same `feature_mappings` alongside
`name`, `type`, `provider` and `connection_string`, and put `pool_size`, `timeout_ms`
and `query_cache_ttl_secs` at the top level rather than under server `options`.

## Validation and deployment

Validate datasource names, connection settings and supported methods against the selected runtime.
Keep connection credentials in deployment configuration. Test query timeouts, backend failures and
cache freshness with the intended database; strict Core admission of a policy cannot verify these effects.
See [Feature runtime evidence](contracts/feature-runtime.md) and the
[FeaturePipeline integration contract](contracts/feature-pipeline.md) for executable coverage.

## Revision History

| Date | Changes |
| --- | --- |
| 2026-09-12 | Document RisingWave direct Lookup configuration, freshness, result types and validation. |
