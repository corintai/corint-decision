#!/usr/bin/env bash
# Original datasource E2E suite: real fixtures -> server -> authenticated HTTP decisions.
# Run from any directory. Generated files and services belong to this run only.
# External POSTGRES_URL/CLICKHOUSE_URL/REDIS_URL must point to disposable test databases.
set -eo pipefail
REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
for tool in python3 cargo curl jq sqlite3; do
    command -v "$tool" >/dev/null || { echo "Missing prerequisite: $tool" >&2; exit 2; }
done
python3 -c 'import yaml' || { echo 'Install tests/scripts/requirements-e2e.txt' >&2; exit 2; }
case "${1:-}" in
    --sqlite|--postgres|--clickhouse|--redis|--all) DATASOURCE=${1#--} ;;
    "")
        if [ -t 0 ]; then
            read -r -p 'Datasource (sqlite/postgres/clickhouse/redis/all) [sqlite]: ' DATASOURCE
            DATASOURCE=${DATASOURCE:-sqlite}
        else DATASOURCE=sqlite; fi ;;
    *) echo "Usage: $0 [--sqlite|--postgres|--clickhouse|--redis|--all]" >&2; exit 2 ;;
esac
case "$DATASOURCE" in sqlite|postgres|clickhouse|redis|all) ;; *) exit 2 ;; esac
RESULTS_DIR=${CORINT_E2E_RESULTS_DIR:-$(mktemp -d "$REPO_ROOT/tests/results.XXXXXX")}
mkdir -p "$RESULTS_DIR"
RESULTS_DIR=$(cd "$RESULTS_DIR" && pwd)
export CARGO_INCREMENTAL=${CARGO_INCREMENTAL:-0}
export CORINT_E2E_SEED=${CORINT_E2E_SEED:-20260907}
log_info() { echo "[INFO] $*"; }
log_success() { echo "[PASS] $*"; }
log_error() { echo "[FAIL] $*" >&2; }

if [ "$DATASOURCE" = all ]; then
    : > "$RESULTS_DIR/datasources.jsonl"
    failed=0
    for ds in sqlite postgres clickhouse redis; do
        reason=""
        case "$ds" in
            postgres) [ -n "${POSTGRES_URL:-}" ] || reason='POSTGRES_URL not configured' ;;
            clickhouse) [ -n "${CLICKHOUSE_URL:-}" ] || reason='CLICKHOUSE_URL not configured' ;;
            redis) if ! command -v redis-server >/dev/null && [ -z "${REDIS_URL:-}" ]; then reason='redis-server and REDIS_URL unavailable'; fi ;;
        esac
        if [ -n "$reason" ]; then
            status=skipped
            log_info "$ds: SKIPPED ($reason)"
        elif CORINT_E2E_RESULTS_DIR="$RESULTS_DIR/$ds" bash "$REPO_ROOT/tests/scripts/run_e2e_tests.sh" "--$ds"; then
            status=passed
        else status=failed; failed=1; fi
        jq -nc --arg datasource "$ds" --arg status "$status" --arg reason "$reason" \
            '{datasource:$datasource,status:$status,reason:$reason}' >> "$RESULTS_DIR/datasources.jsonl"
    done
    jq -s '.' "$RESULTS_DIR/datasources.jsonl" > "$RESULTS_DIR/summary.json"
    log_info "Reports: $RESULTS_DIR"
    exit "$failed"
fi

RUN_DIR=$(mktemp -d "${TMPDIR:-/tmp}/corint-e2e.XXXXXX")
rm -f "$RESULTS_DIR/$DATASOURCE-report.json"
SERVER_PID=""
REDIS_PID=""
cleanup() {
    for pid in "$SERVER_PID" "$REDIS_PID"; do
        if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
    done
    rm -rf "$RUN_DIR"
}
finish() {
    local code=$?
    trap - EXIT
    if [ ! -f "$RESULTS_DIR/$DATASOURCE-report.json" ]; then
        jq -nc --arg datasource "$DATASOURCE" --argjson exit_code "$code" \
            '{datasource:$datasource,status:"setup_failed",exit_code:$exit_code,total:0,passed:0,failed:0}' > "$RESULTS_DIR/$DATASOURCE-report.json"
    fi
    cleanup
    log_info "Reports: $RESULTS_DIR"
    exit "$code"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
python3 - "$REPO_ROOT" "$RUN_DIR" <<'PY_COPY'
import pathlib, shutil, sys
repo, run = map(pathlib.Path, sys.argv[1:])
shutil.copytree(repo / "tests", run / "tests", ignore=shutil.ignore_patterns("results", "results.*", "__pycache__"))
(run / "config").mkdir()
(run / "docs/schema").mkdir(parents=True)
shutil.copy2(repo / "docs/schema/postgres-schema.sql", run / "docs/schema/postgres-schema.sql")
for path in ["tests/data", "tests/e2e_repo/features", "tests/e2e_repo/pipelines"]:
    (run / path).mkdir(parents=True, exist_ok=True)
PY_COPY
cd "$RUN_DIR"
read -r SERVER_PORT GRPC_PORT REDIS_PORT < <(python3 - <<'PY_PORT'
import socket
sockets = [socket.socket() for _ in range(3)]
try:
    for sock in sockets: sock.bind(("127.0.0.1", 0))
    print(*(sock.getsockname()[1] for sock in sockets))
finally:
    for sock in sockets: sock.close()
PY_PORT
)
# Do not inherit caller routing, credentials, or persistence configuration.
for variable in $(env | cut -d= -f1); do
    case "$variable" in CORINT_*) case "$variable" in CORINT_E2E_*) ;; *) unset "$variable" ;; esac ;; esac
done
unset DATABASE_URL
export CORINT_DECISION_TOKEN=$(python3 -c 'import secrets; print(secrets.token_hex(32))')
export CORINT_PUBLISHER_TOKEN=$(python3 -c 'import secrets; print(secrets.token_hex(32))')
export CORINT_TENANT_ID=e2e_local
export NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost
API_URL="http://127.0.0.1:$SERVER_PORT"
CURRENT_DATASOURCE=$DATASOURCE
TOTAL_TESTS=0 PASSED_TESTS=0 FAILED_TESTS=0
declare -a PASSED_TEST_NAMES=() FAILED_TEST_NAMES=() FAILED_TEST_DETAILS=()
: > "$RESULTS_DIR/$DATASOURCE-cases.jsonl"

log_info "Generating deterministic fixtures (seed=$CORINT_E2E_SEED)..."
# SQLite also supplies database lists for the ClickHouse variant.
python3 tests/scripts/generate_test_data.py
sqlite3 tests/data/e2e_test.db < tests/data/test_data.sql
case "$DATASOURCE" in
    sqlite) export DATABASE_URL="sqlite://$RUN_DIR/tests/data/e2e_test.db" ;;
    postgres)
        : "${POSTGRES_URL:?Set POSTGRES_URL to a disposable PostgreSQL test database}"
        command -v psql >/dev/null
        python3 tests/scripts/generate_postgres_data.py
        psql -X -q -v ON_ERROR_STOP=1 "$POSTGRES_URL" -f tests/data/postgres_test_data.sql
        export DATABASE_URL="$POSTGRES_URL" ;;
    clickhouse)
        : "${CLICKHOUSE_URL:?Set CLICKHOUSE_URL to a disposable ClickHouse test endpoint}"
        curl -fsS --connect-timeout 2 --max-time 10 "$CLICKHOUSE_URL" --data 'SELECT 1' >/dev/null
        python3 tests/scripts/generate_clickhouse_data.py
        python3 tests/scripts/load_clickhouse_data.py tests/data/clickhouse_test_data.sql "$CLICKHOUSE_URL"
        export DATABASE_URL="sqlite://$RUN_DIR/tests/data/e2e_test.db" ;;
    redis)
        python3 -c 'import redis' || { echo 'Install tests/scripts/requirements-e2e.txt' >&2; exit 2; }
        command -v redis-cli >/dev/null
        if [ -z "${REDIS_URL:-}" ]; then
            command -v redis-server >/dev/null
            redis-server --bind 127.0.0.1 --port "$REDIS_PORT" --save '' --appendonly no \
                --dir "$RUN_DIR" > "$RESULTS_DIR/redis.log" 2>&1 &
            REDIS_PID=$!
            export REDIS_URL="redis://127.0.0.1:$REDIS_PORT/0"
        fi
        ready=false
        for attempt in {1..30}; do
            if redis-cli -u "$REDIS_URL" ping >/dev/null 2>&1; then ready=true; break; fi
            sleep 0.2
        done
        [ "$ready" = true ] || { log_error 'Redis failed to start'; exit 1; }
        python3 tests/scripts/generate_redis_data.py ;;
esac

# Generate actual connection values, not unevaluated shell placeholders in YAML.
python3 - "$DATASOURCE" "$SERVER_PORT" "$GRPC_PORT" <<'PY_CONFIG'
import os, pathlib, shutil, sys, yaml
kind, http, grpc = sys.argv[1:]
repo = pathlib.Path('tests/e2e_repo')
for directory in ['features', 'pipelines', 'configs/datasources']:
    for path in (repo / directory).glob('*.yaml'): path.unlink()
shutil.copy2(repo / f'templates/features/e2e_features_{kind}.yaml', repo / 'features/e2e_features.yaml')
for name in (['redis_test'] if kind == 'redis' else ['transaction_test', 'payment_test', 'login_test', 'db_list_test']):
    shutil.copy2(repo / f'templates/pipelines/{name}.yaml', repo / f'pipelines/{name}.yaml')
shutil.copy2(repo / ('templates/registry/registry_redis.yaml' if kind == 'redis' else 'templates/registry/registry_default.yaml'), repo / 'registry.yaml')
name = kind + '_e2e'
source = {'type': 'sql', 'provider': kind, 'database': 'corint_e2e', 'connection_string': os.environ.get('DATABASE_URL', ''), 'options': {'max_connections': '3'}}
if kind == 'postgres': source['provider'] = 'postgresql'
if kind == 'clickhouse': source.update(type='olap', provider='clickhouse', connection_string=os.environ['CLICKHOUSE_URL'], database='default', events_table='events')
if kind == 'redis': source.update(type='feature_store', provider='redis', connection_string=os.environ['REDIS_URL'], options={'namespace': 'e2e_features', 'max_connections': '3'})
sources = {name: source}
# List backends support SQL; ClickHouse uses a documented SQLite sidecar.
if kind == 'clickhouse': sources['sqlite_e2e'] = {'type': 'sql', 'provider': 'sqlite', 'database': 'corint_e2e', 'connection_string': os.environ['DATABASE_URL']}
if kind == 'redis': (repo / 'lists/db_lists.yaml').unlink()
elif kind == 'postgres':
    path = repo / 'lists/db_lists.yaml'
    path.write_text(path.read_text().replace('sqlite_e2e', 'postgres_e2e'))
for ds_name, ds in sources.items():
    runtime = dict(ds, name=ds_name)
    if ds['type'] == 'feature_store': runtime['namespace'] = 'e2e_features'
    (repo / f'configs/datasources/{ds_name}.yaml').write_text(yaml.safe_dump(runtime))
config = {'host': '127.0.0.1', 'port': int(http), 'grpc_port': int(grpc), 'enable_tracing': False,
          'repository': {'type': 'filesystem', 'path': str(repo)}, 'datasource': sources}
pathlib.Path('config/server.yaml').write_text(yaml.safe_dump(config))
PY_CONFIG

log_info 'Building server from the current checkout...'
SERVER_BINARY=$(cd "$REPO_ROOT" && cargo build -p corint-decision-server --bin corint-decision-server \
    --features corint-decision-engine/redis --locked --message-format=json | python3 -c '
import json, sys
paths = []
for line in sys.stdin:
    value = json.loads(line)
    if value.get("reason") == "compiler-message": sys.stderr.write(value["message"].get("rendered") or "")
    if value.get("reason") == "compiler-artifact" and value.get("target", {}).get("name") == "corint-decision-server" and value.get("executable"): paths.append(value["executable"])
if len(paths) != 1: sys.exit("Expected one freshly built server executable")
print(paths[0])
')
RUST_LOG=${CORINT_E2E_LOG_LEVEL:-info} "$SERVER_BINARY" > "$RESULTS_DIR/server_$DATASOURCE.log" 2>&1 &
SERVER_PID=$!
ready=false
for attempt in {1..30}; do
    kill -0 "$SERVER_PID" 2>/dev/null || break
    if curl --noproxy '*' --connect-timeout 1 --max-time 2 -fsS "$API_URL/health" >/dev/null 2>&1; then ready=true; break; fi
    sleep 1
done
if [ "$ready" != true ]; then
    log_error 'Server failed to become ready'
    cat "$RESULTS_DIR/server_$DATASOURCE.log" >&2
    exit 1
fi

request_decision() {
    local payload="$1"
    printf '%s\n' "$payload" > "$RESULTS_DIR/${CURRENT_DATASOURCE}-request-${TOTAL_TESTS}.json"
    RESPONSE_FILE="$RESULTS_DIR/${CURRENT_DATASOURCE}-case-${TOTAL_TESTS}.json"
    HTTP_STATUS=$(curl --noproxy '*' --connect-timeout 2 --max-time 30 -sS \
        -X POST "$API_URL/v1/decide" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $CORINT_DECISION_TOKEN" \
        -d "$payload" -o "$RESPONSE_FILE" -w '%{http_code}') || HTTP_STATUS=000
}

record_case() {
    local name="$1" expected="$2" passed="$3" detail="$4"
    jq -nc --arg name "$name" --arg expected "$expected" --argjson passed "$passed" \
        --arg status "$HTTP_STATUS" --arg detail "$detail" --arg response "$RESPONSE_FILE" \
        '{name:$name,expected:$expected,passed:$passed,http_status:$status,detail:$detail,response:$response}' \
        >> "$RESULTS_DIR/${CURRENT_DATASOURCE}-cases.jsonl"
    if [ "$passed" = true ]; then
        PASSED_TESTS=$((PASSED_TESTS + 1)); PASSED_TEST_NAMES+=("$name")
        log_success "$name: PASSED ($detail)"
    else
        FAILED_TESTS=$((FAILED_TESTS + 1)); FAILED_TEST_NAMES+=("$name")
        FAILED_TEST_DETAILS+=("$name|FAILED|$detail")
        log_error "$name: FAILED ($detail)"
    fi
    # Record every failure, then continue executing the remaining cases.
    return 0
}

run_test_case() {
    local name="$1" payload="$2" expected="$3" required_rule="${4:-}" actual
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    request_decision "$payload"
    actual=$(jq -er '.decision.result | ascii_downcase' "$RESPONSE_FILE" 2>/dev/null) || actual=INVALID_RESPONSE
    if [ "$HTTP_STATUS" = 200 ] && [ "$actual" = "$expected" ] &&
       { [ -z "$required_rule" ] || jq -e --arg rule "$required_rule" ' .decision.evidence.triggered_rules | index($rule) != null' "$RESPONSE_FILE" >/dev/null 2>&1; }; then
        record_case "$name" "$expected" true "$actual"
    else
        record_case "$name" "$expected" false "HTTP $HTTP_STATUS; expected $expected, got $actual; required rule: ${required_rule:-none}"
    fi
}

run_error_test_case() {
    local name="$1" payload="$2" expected="$3" passed=false
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    request_decision "$payload"
    case "$expected" in
        default_fallback)
            if [ "$HTTP_STATUS" = 200 ] && jq -e '.pipeline_id == "default" and .decision.result == "pass"' "$RESPONSE_FILE" >/dev/null 2>&1; then passed=true; fi ;;
        no_pipeline)
            if [ "$HTTP_STATUS" != 200 ] && jq -e '.error.code == "E_NO_PIPELINE_MATCH"' "$RESPONSE_FILE" >/dev/null 2>&1; then passed=true; fi ;;
        error)
            if [[ "$HTTP_STATUS" == 4?? ]] && jq -e '.error.code | type == "string"' "$RESPONSE_FILE" >/dev/null 2>&1; then passed=true; fi ;;
    esac
    record_case "$name" "$expected" "$passed" "HTTP $HTTP_STATUS; expected $expected"
}


CURRENT_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
if [ "$DATASOURCE" = redis ]; then
    source tests/scripts/e2e_test_cases_redis.sh
else
    source tests/scripts/e2e_test_cases.sh
fi
jq -s --arg datasource "$DATASOURCE" --arg seed "$CORINT_E2E_SEED" \
    '{datasource:$datasource,seed:$seed,total:length,passed:map(select(.passed))|length,failed:map(select(.passed|not))|length,cases:.}' \
    "$RESULTS_DIR/$DATASOURCE-cases.jsonl" > "$RESULTS_DIR/$DATASOURCE-report.json"
log_info "$DATASOURCE: $PASSED_TESTS/$TOTAL_TESTS passed; $FAILED_TESTS failed"
[ "$FAILED_TESTS" -eq 0 ]
