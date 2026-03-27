#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
CONFIG_DIR="${PROJECT_ROOT}/config"
ACTIVE_CONFIG="${CONFIG_DIR}/server.yaml"
TEST_CONFIG="${PROJECT_ROOT}/tests/config/german_credit_server.yaml"
BACKUP_CONFIG="${CONFIG_DIR}/server.yaml.bak.german_credit"
SERVER_LOG="${PROJECT_ROOT}/tests/.german_credit_server.log"
SERVER_URL="http://127.0.0.1:18080"
SERVER_PID=""

cleanup() {
    if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
        kill "${SERVER_PID}" 2>/dev/null || true
        wait "${SERVER_PID}" 2>/dev/null || true
    fi

    if [[ -f "${BACKUP_CONFIG}" ]]; then
        mv "${BACKUP_CONFIG}" "${ACTIVE_CONFIG}"
    else
        rm -f "${ACTIVE_CONFIG}"
    fi

    rm -f "${SERVER_LOG}"
}

trap cleanup EXIT

mkdir -p "${CONFIG_DIR}"

if [[ -f "${ACTIVE_CONFIG}" ]]; then
    cp "${ACTIVE_CONFIG}" "${BACKUP_CONFIG}"
fi

cp "${TEST_CONFIG}" "${ACTIVE_CONFIG}"

echo "[1/4] Building corint-server..."
cargo build --release -p corint-server >/dev/null

echo "[2/4] Starting corint-server on ${SERVER_URL}..."
"${PROJECT_ROOT}/target/release/corint-server" >"${SERVER_LOG}" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 30); do
    if curl -fsS "${SERVER_URL}/health" >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

if ! curl -fsS "${SERVER_URL}/health" >/dev/null 2>&1; then
    echo "Server failed to start. Log:"
    cat "${SERVER_LOG}"
    exit 1
fi

run_case() {
    local name="$1"
    local expected_result="$2"
    local expected_pipeline="$3"
    local payload="$4"

    local response
    response=$(curl -fsS -X POST "${SERVER_URL}/v1/decide" \
        -H "Content-Type: application/json" \
        -d "${payload}")

    local actual_result
    actual_result=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["decision"]["result"])' <<<"${response}")
    local actual_pipeline
    actual_pipeline=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["pipeline_id"])' <<<"${response}")

    if [[ "${actual_result}" != "${expected_result}" ]]; then
        echo "[FAIL] ${name}: expected result=${expected_result}, got result=${actual_result}"
        echo "${response}"
        exit 1
    fi

    if [[ "${actual_pipeline}" != "${expected_pipeline}" ]]; then
        echo "[FAIL] ${name}: expected pipeline=${expected_pipeline}, got pipeline=${actual_pipeline}"
        echo "${response}"
        exit 1
    fi

    echo "[PASS] ${name}: ${actual_result}"
}

PIPELINE_ID="german_credit_admission_risk_pipeline_executes_risk_assessment_ruleset_for_credit_a_0fe6aec7e79c"

echo "[3/4] Validating German Credit rule accuracy..."

run_case \
    "Stable applicant should approve" \
    "approve" \
    "${PIPELINE_ID}" \
    '{"event":{"type":"credit_application","application_id":"gc_approve_001","age":35,"credit_amount":2500,"duration":12,"housing":"own","saving_accounts":"rich","checking_account":"moderate"}}'

run_case \
    "High amount but stable profile should review" \
    "review" \
    "${PIPELINE_ID}" \
    '{"event":{"type":"credit_application","application_id":"gc_review_001","age":30,"credit_amount":9000,"duration":40,"housing":"own","saving_accounts":"rich","checking_account":"moderate"}}'

run_case \
    "Very long duration should review" \
    "review" \
    "${PIPELINE_ID}" \
    '{"event":{"type":"credit_application","application_id":"gc_review_002","age":41,"credit_amount":4200,"duration":40,"housing":"rent","saving_accounts":"moderate","checking_account":"moderate"}}'

run_case \
    "Young applicant with stacked high risk should decline" \
    "decline" \
    "${PIPELINE_ID}" \
    '{"event":{"type":"credit_application","application_id":"gc_decline_001","age":22,"credit_amount":9000,"duration":30,"housing":"rent","saving_accounts":"little","checking_account":"little"}}'

run_case \
    "High amount with strong profile should approve" \
    "approve" \
    "${PIPELINE_ID}" \
    '{"event":{"type":"credit_application","application_id":"gc_approve_002","age":33,"credit_amount":9000,"duration":12,"housing":"own","saving_accounts":"rich","checking_account":"moderate"}}'

echo "[4/4] German Credit rule validation passed."
