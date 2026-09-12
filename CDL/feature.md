# Feature

A Feature is a named value obtained by aggregation, state calculation, an expression,
or a lookup. Definitions are separate from the Rules and Pipelines that consume them.
`datasource` references a host binding; it does not declare a connection or credentials.

This page defines the online Feature extension. Strict [Core](overall.md) rejects
dynamic `features.*` access. To use a computed Feature in Core, the host supplies its value
as an ordinary declared event input before execution; reading that input performs no query.

## Documents and common fields

A Feature document contains a `features` sequence, an optional collection `version`
(default `"0.2"`), and optional `metadata` containing string values. The collection
version is a format label; it does not select Core admission or translate older syntax.
Each item defines one Feature. A bare `- name: ...` fragment needs the `features:` wrapper.
Rules are separate documents and are not registered by loading a Feature collection.

| Field | Requirement and meaning |
| --- | --- |
| `name` | Required non-empty Feature reference key. Use identifier names for expression access. |
| `type` | Required: `aggregation`, `state`, `expression` or `lookup` for the methods defined here. |
| `description` | Optional description; defaults to an empty string. |
| `dependencies` | Optional list of Feature names; expression dependencies are inferred and merged with this list. `depends_on` is not this field's name. |
| `enabled` | Optional boolean, default `true`. A disabled Feature returns null without computing its dependencies. |
| `tags` | Optional string list for organization. |
| `version` | Optional Feature revision label, default `"1.0"`; distinct from the collection format label. |

Names must be unique within a collection. Missing dependencies fail execution;
cycles are rejected. Batch registration and directory loading require a complete
dependency graph. Shared dependencies are computed once per execution request.
Registration of one definition may precede registration of its dependencies.

Sequence, Graph, statistical State methods and model inference are outside this contract.

## Aggregation

| Field | Requirement and meaning |
| --- | --- |
| `method` | Required: `count`, `sum`, `avg`, `min`, `max`, `distinct`, `stddev`, `median` or `percentile`. Backend capability checks also apply. |
| `datasource` | Required logical datasource name. |
| `entity` | Required table/entity name. |
| `dimension` | Required field used for an equality filter. |
| `dimension_value` | Required string literal, event path or template; see templates below. |
| `field` | Required for every method except `count`; `count` counts rows and does not use this field. |
| `window` | Optional relative duration; omission selects all history, subject to other filters. |
| `timestamp_field` | Optional time column, default `event_timestamp`; used when a window is present. |
| `when` | Optional row filter: one predicate or a flat `all` list. |
| `percentile` | Integer from 0 to 100 for `percentile`; defaults to 50. It has no effect on other methods. |

`dimension: user_id` with `dimension_value: "${event.user.id}"` adds
`WHERE user_id = <resolved value>`. It does not generate `GROUP BY` or return one row
per entity. Each Feature returns one aggregate value for the selected entity.
`distinct` counts distinct field values. Other methods use the datasource's aggregate
semantics; SQL aggregates ignore null inputs. With no matching SQL rows, count and
distinct return 0; sum, avg, min, max and statistical aggregates can return null.

Method names do not guarantee datasource availability. A host binding must support the
requested method; an unsupported method is an error.

### Row filters and templates

The left side of a `when` predicate selects a datasource field, without an `event.`
prefix. Dot-separated JSON field access depends on the datasource. The right side
supplies a value; it is not a general Rule expression.

- Comparisons: `==`, `!=`, `>`, `>=`, `<`, `<=`, `in`, `not in`.
- String predicates: `contains`, `starts_with`, `ends_with`, `matches`, subject to
  backend support. State has narrower filter support, described below.
- `all` combines a flat list of predicates. `any`, `not` and nested groups are rejected.
  Put separate predicates in `all` rather than combining them with `AND` or `OR` text.
- A complete `${event.path}` value, optionally quoted, resolves the request value
  with its type intact. Nested paths are traversed in full; a similarly named leaf
  elsewhere in the request is not a substitute. The legacy `{event.path}` spelling
  remains accepted in filters.
- For example, `amount > ${event.threshold}` compares with a request number;
  `country == "${event.country}"` compares with a request string.
- Partial string interpolation is rejected in filters. Templates inside literal
  arrays are also rejected; use a whole-array value such as `${event.countries}`.
- Missing template values fail before the query. Malformed placeholders are errors,
  not string literals or permission to remove a filter.

`dimension_value` and Lookup `key` use string templates: a bare `event.user.id`
reads the event path; `${event.user.id}` can also appear inside a larger string,
such as `customer:${event.user.id}`. Multiple substitutions are supported and inserted
values are not scanned again. Strings, finite numbers and booleans are converted to
text. Missing paths, null, arrays and objects are errors. Use `${...}` in these fields;
the legacy `{event.path}` filter alias does not apply to string templates.

### Time windows

| Unit | Duration |
| --- | --- |
| `s` | Second |
| `m` | Minute |
| `h` | Hour |
| `d` | Fixed 24-hour day |
| `w` | Fixed 7-day week |
| `mo` | Fixed 30 days, not a calendar month |

`window` is a positive integer followed by a unit, such as `30s`, `24h` or `7d`.
`last_` is an optional prefix. Zero, malformed values, `q`, `y` and durations exceeding
signed 64-bit seconds are rejected during validation and checked again before querying.
Only omission permits an unbounded query.

An ordinary SQL/ClickHouse relative query uses the database clock and the interval
`[now - window, now)`: the lower bound is included and the upper bound is excluded.
Future records are excluded. Separate queries may observe different clock instants.
The host can instead execute an explicit fixed-cutoff Feature plan using Unix seconds
`as_of`; its interval is `[as_of - window, as_of)`. The host selects that separate
execution mode explicitly; it is not implied by a Feature declaration.

## State

The implemented method is `time_since`: elapsed time since the **earliest** matching
record, obtained with `MIN(timestamp_field)`. It does not mean time since the last event.

| Field | Requirement and meaning |
| --- | --- |
| `method` | Required; `time_since` for the implemented method. |
| `datasource`, `entity` | Required datasource and table/entity. |
| `dimension`, `dimension_value` | Required entity equality filter and string template. |
| `timestamp_field` | Optional timestamp column, default `event_timestamp`. |
| `unit` | Required: `minutes`, `hours` or `days`; returns whole elapsed units truncated toward zero. |
| `when` | Optional single predicate or flat `all` list; comparison and membership operators only. |
| `fallback` | Optional JSON value used when the timestamp query/calculation fails; YAML null disables fallback. |

`window` is rejected for `time_since`; its query covers all matching history.
The host's UTC clock determines elapsed time. Timestamp text must be parseable as
an ISO timestamp or supported SQL timestamp text; text without an offset is treated
as UTC. A future earliest timestamp can produce a negative elapsed value.

No matching timestamp, an invalid timestamp or a query failure uses an explicit
fallback, otherwise execution fails. Invalid units, templates, filters, missing
bindings and unsupported methods fail without fallback. String-pattern operators
are rejected rather than converted into equality comparisons.

## Expression

An Expression consumes computed Feature values and numeric request fields. It does
not query a datasource or define a window. `expression` supplies the arithmetic;
`method` may be omitted (effective method `expression`).

Supported operators are `+`, `-`, `*`, `/`, `%`, unary minus and parentheses, with
standard arithmetic precedence and left associativity. Decimal and scientific
numeric literals are supported. Functions are `min(a,b)`, `max(a,b)`, `abs(x)`,
`sqrt(x)`, `ceil(x)`, `floor(x)` and `round(x)`.

Bare Feature names and `features.name` create dependencies. `event.nested.field`
reads the request without creating a Feature dependency. Registration rejects invalid
syntax, unsupported paths/operators and function arity. Inputs must be finite numbers
or null. Null propagates; division/remainder by zero returns null. Missing inputs,
wrong types and nonfinite results are errors. This does not extend Core's expression profile.

## Lookup

| Field | Requirement and meaning |
| --- | --- |
| `datasource` | Required logical Feature Store binding. |
| `key` | Required string template for the entity key, not a complete physical Redis key. |
| `fallback` | Optional JSON value; omission or YAML null means no explicit fallback. |

The Feature name and resolved entity key are passed separately to the store.
Mapping this pair to a physical storage key is the host binding's responsibility.
A found value is returned unchanged. A missing key returns an explicit fallback,
or null if none is configured. A store query failure uses an explicit fallback;
without one it propagates as an execution error. An unavailable Redis connection
is a store error, not a missing key. Invalid configuration, missing bindings and key
template errors fail before lookup and never use fallback. Fallback objects and arrays
retain their JSON types and are not interpolated or mapped.

### RisingWave materialized views

A Lookup can read a precomputed column directly from a RisingWave materialized view
through a host `feature_store` binding with provider `risingwave`. The runtime needs
the `sqlx` build feature. No Redis or HTTP service is required.

```yaml
features:
  - name: user_txn_count_1h
    type: lookup
    datasource: rw_features
    key: "${event.user.id}"
```

The host maps `user_txn_count_1h` to a schema, view, key column and value column.
The connector binds the resolved key as a query parameter and returns one value.
No row means missing; a row containing SQL NULL is a found null value and does not
activate fallback. Multiple matching rows are a query error. Missing mappings and
invalid entity key types fail before querying and cannot use fallback.

RisingWave maintains the computation independently of decision requests. The window
and filters belong to the materialized view definition; `lookup` does not accept
`window`, `when` or an `as_of` cutoff. Its result reflects the materialized state
visible to the query, which may not include the current request event. Separate
Feature lookups are separate queries and do not guarantee a shared snapshot.

RisingWave Lookup caching is disabled by default. The host can explicitly accept
stale values using `query_cache_ttl_secs`; Redis `default_ttl` and `namespace` do not
apply to this provider. Mapping fields and connection settings belong to host
configuration. This is an online extension; strict Core still requires the host
to supply values as declared event inputs.

See [Rule](rule.md), [Context](context.md) and [Expression](expression.md) for the
separate condition and input contracts.
