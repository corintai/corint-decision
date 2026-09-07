# List configuration and operations

<!-- cdl-scope: compatibility-unverified -->
> This page is an unverified compatibility reference. Its snippets are not Core support evidence.
> For the executable contract and supported examples, use [CDL reference](../CDL/overall.md),
> [Pipeline](../CDL/pipeline.md) and the [capability inventory](contracts/schema/capabilities.json).

The [CDL List reference](../CDL/list.md) defines membership expressions and their error semantics.
Storage, loading and freshness belong to the host runtime. The configuration examples retain
their compatibility classification; backend behavior is covered by the linked regression tests.

## Current loader boundaries

The [List loader](../crates/corint-decision-runtime/src/lists/loader.rs) creates memory and file
backends. SQL backends require their configured database resources and build features.
The direct `backend: redis` and `backend: api` branches return explicit not-implemented errors;
their appearance in the [configuration enum](../crates/corint-decision-runtime/src/lists/config.rs)
is not evidence that they can be selected successfully. Datasource-backed loading is a separate path.

Directory loading visits YAML files directly under the repository root's `lists/` directory.
It does not recursively discover domain subdirectories; keep loadable files
directly under `lists/` or add explicit traversal in the host integration. Some file/backend
failures are logged and skipped, so check that every required List is installed before serving policies.
A missing List remains an execution error, including for `not in`, rather than becoming an empty list.

For direct VM integration, install the List service with `PipelineExecutor::with_list_service`.
The minimal `Executor` does not provide List service injection. Strict Core rejects dynamic List access.

## Regression fixture binding

The [List regression fixture](../tests/conformance/cdl_lists/) is exercised by
[list_execution.rs](../crates/corint-decision-engine/tests/list_execution.rs). The test parses and
compiles the fixture Rules, Ruleset and Pipeline through the compatibility APIs, registers the
compiled Rule/Ruleset programs with `PipelineExecutor::with_ruleset_programs`, and invokes the
Pipeline directly. This VM setup does not run strict Core admission or enable Lists in that profile.

The fixture's [lists.json](../tests/conformance/cdl_lists/lists.json) is test data, not loader YAML.
For each named collection, the test adds its string values to a `MemoryBackend`, registers that
backend under the exact ID in `ListService::new_with_backends`, and injects the service with
`with_list_service`. Register a backend even when the intended collection is empty;
`ListService::new_with_memory()` alone creates an empty registry, with no named lists installed.

The test uses the fixture events and checks the matched Rules, aggregate score and final decision.
Separate cases verify empty collections, missing service/ID, backend errors and Memory/File value
matching. Core rejection checks use the same complete resource graph and input schema, admit a
control with the List conditions replaced by `true`, then restore each original List condition and
require `E_UNSUPPORTED_CAPABILITY` at the type stage.

For a File backend, call `load()` successfully before registration. Constructing it does not prove
that the file was loaded. Its automatic reload path retains the previous snapshot on a read failure;
that failure does not become a lookup error while the cached snapshot remains available. Hosts that
need freshness guarantees must handle that loading policy explicitly.

## Configuration

Store single-list YAML files, or a document with a top-level `lists` array, directly under
`repository/lists/`. Each list needs an `id` and either `backend` or a registered `datasource`.
The loader uses `datasource` first if both are supplied; explicit configurations avoid ambiguity.

### Memory and File

Memory values are loaded from `initial_values` as strings:

```yaml
id: test_emails
backend: memory
initial_values:
  - "test@example.com"
  - "fraud@test.com"
```

File `path` is absolute or relative to the repository root, not the YAML file's directory.
`reload_interval` is in seconds. The referenced data file must exist and load successfully:

```yaml
id: high_risk_countries
backend: file
path: "lists/data/high_risk_countries.txt"
reload_interval: 3600
```

### Database backends

SQL operations require the runtime's `sqlx` build feature and a provisioned database/table.

| Configuration | Host requirement |
|---|---|
| `backend: postgresql` | Install a PostgreSQL pool with `ListLoader::with_db_pool`. |
| `backend: sqlite` | Set `db_path` or a loader default; relative database paths use the process working directory. |
| `datasource: <name>` | Register the source through `with_datasource` / `with_datasources`; the loader accepts `sqlite` and `postgresql`. PostgreSQL still requires the installed pool. |

`table` and `value_column` default to `list_entries` and `value`. Set `expiration_column`
according to the table; SQLite defaults to `expires_at`. Configuration fields and defaults are
owned by [ListConfig](../crates/corint-decision-runtime/src/lists/config.rs). A loaded configuration
is not proof that a SQL query succeeds; test the table schema and error behavior on the target backend.

## Freshness and operation

Check that every required list ID was installed after loading. File snapshots can remain in use
after a reload failure; plan monitoring and update policy around the application's freshness needs.
Choose capacity targets from measurements using representative list sizes, updates and concurrency.
The `cache_ttl` configuration field is not a universal cache switch implemented by every backend.

Boolean alternatives in a Rule do not request batch or parallel List queries. Membership and error
semantics are defined in [CDL List](../CDL/list.md#7-error-handling); implementation and backend
coverage live in the [List runtime](../crates/corint-decision-runtime/src/lists/).
