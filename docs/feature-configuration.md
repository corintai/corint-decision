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

## Validation and deployment

Validate datasource names, connection settings and supported methods against the selected runtime.
Keep connection credentials in deployment configuration. Test query timeouts, backend failures and
cache freshness with the intended database; strict Core admission of a policy cannot verify these effects.
See [Feature runtime evidence](contracts/feature-runtime.md) and the
[FeaturePipeline integration contract](contracts/feature-pipeline.md) for executable coverage.
