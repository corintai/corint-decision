#!/bin/bash
# CORINT Decision Engine Interactive Test Script
# ============================================================================
# This script provides an interactive way to test the CORINT Decision Engine
# with various scenarios and data sources.
#
# Features:
#   - Protocol selection: HTTP or gRPC
#   - Data source selection: PostgreSQL, ClickHouse, SQLite (+ Redis for lookups)
#   - Automatic data initialization with cleanup
#   - Server startup management
#   - Multiple test scenarios covering various fraud patterns
#
# Usage:
#   ./test_api.sh
# ============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIG_FILE="${PROJECT_ROOT}/config/server.yaml"
CONFIG_EXAMPLE="${PROJECT_ROOT}/config/server-example.yaml"

# Copy config file from example at script start
if [ -f "${CONFIG_EXAMPLE}" ]; then
    cp "${CONFIG_EXAMPLE}" "${CONFIG_FILE}"
    echo "Config file initialized from ${CONFIG_EXAMPLE}"
else
    echo "Error: Config example file not found: ${CONFIG_EXAMPLE}"
    exit 1
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Read server configuration from config/server.yaml
read_config() {
    if [ ! -f "${CONFIG_FILE}" ]; then
        print_error "Config file not found: ${CONFIG_FILE}"
        exit 1
    fi

    # Parse configuration using yq or grep/sed fallback
    if command -v yq &> /dev/null; then
        SERVER_HOST=$(yq -r '.host // "127.0.0.1"' "${CONFIG_FILE}")
        HTTP_PORT=$(yq -r '.port // 8080' "${CONFIG_FILE}")
        GRPC_PORT=$(yq -r '.grpc_port // 50051' "${CONFIG_FILE}")
    else
        # Fallback to grep/sed - improved regex to handle quoted and unquoted values
        SERVER_HOST=$(grep '^host:' "${CONFIG_FILE}" | head -1 | sed -E 's/^host:[[:space:]]*"?([^"]+)"?[[:space:]]*$/\1/' | tr -d ' ')
        HTTP_PORT=$(grep '^port:' "${CONFIG_FILE}" | head -1 | sed -E 's/^port:[[:space:]]*([0-9]+)[[:space:]]*$/\1/' | tr -d ' ')
        GRPC_PORT=$(grep '^grpc_port:' "${CONFIG_FILE}" | head -1 | sed -E 's/^grpc_port:[[:space:]]*([0-9]+)[[:space:]]*$/\1/' | tr -d ' ')
        SERVER_HOST="${SERVER_HOST:-127.0.0.1}"
        HTTP_PORT="${HTTP_PORT:-8080}"
        GRPC_PORT="${GRPC_PORT:-50051}"
    fi

    HTTP_HOST="${SERVER_HOST}:${HTTP_PORT}"
    GRPC_HOST="${SERVER_HOST}:${GRPC_PORT}"
}

# Configuration (will be populated by read_config)
HTTP_HOST=""
GRPC_HOST=""
PROTOCOL=""
DATASOURCE=""
SERVER_PID=""

# ============================================================================
# Helper Functions
# ============================================================================

print_header() {
    echo ""
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════════${NC}"
    echo ""
}

print_section() {
    echo ""
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}  $1${NC}"
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

print_warn() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

cleanup() {
    # Allow keeping the server alive by setting KEEP_SERVER_RUNNING=true
    if [ "${KEEP_SERVER_RUNNING:-}" = "true" ]; then
        print_warn "KEEP_SERVER_RUNNING=true set. Skipping server shutdown."
        return 0
    fi
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        print_info "Stopping server (PID: $SERVER_PID)..."
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
}

trap cleanup EXIT

# ============================================================================
# Protocol Selection
# ============================================================================

select_protocol() {
    print_section "Protocol Selection"

    # Check for environment variable first
    if [ -n "${TEST_PROTOCOL:-}" ]; then
        case "${TEST_PROTOCOL}" in
            http|1)
                PROTOCOL="http"
                print_success "Selected: HTTP protocol (from TEST_PROTOCOL)"
                return 0
                ;;
            grpc|2)
                PROTOCOL="grpc"
                # Check if grpcurl is available
                if ! command -v grpcurl &> /dev/null; then
                    print_error "grpcurl is required for gRPC testing but not found"
                    print_info "Install via: brew install grpcurl (macOS) or go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest"
                    exit 1
                fi
                print_success "Selected: gRPC protocol (from TEST_PROTOCOL)"
                return 0
                ;;
            *)
                print_warn "Invalid TEST_PROTOCOL value: ${TEST_PROTOCOL}, falling back to interactive mode"
                ;;
        esac
    fi

    # Non-interactive fallback
    if [ ! -t 0 ]; then
        PROTOCOL="http"
        print_warn "No TTY detected. Defaulting protocol to HTTP."
        return 0
    fi

    echo "Select test protocol:"
    echo ""
    echo "  1) HTTP (REST API) [default]"
    echo "  2) gRPC"
    echo ""

    while true; do
        # Avoid exiting the whole script on read error with set -e
        set +e
        read -p "Enter choice [1-2] (default: 1): " choice
        rc=$?
        set -e
        if [ $rc -ne 0 ]; then
            choice="1"
        fi
        choice="${choice:-1}"  # Default to 1 if empty
        case $choice in
            1)
                PROTOCOL="http"
                print_success "Selected: HTTP protocol"
                break
                ;;
            2)
                PROTOCOL="grpc"
                # Check if grpcurl is available
                if ! command -v grpcurl &> /dev/null; then
                    print_error "grpcurl is required for gRPC testing but not found"
                    print_info "Install via: brew install grpcurl (macOS) or go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest"
                    exit 1
                fi
                print_success "Selected: gRPC protocol"
                break
                ;;
            *)
                print_error "Invalid choice. Please enter 1 or 2."
                ;;
        esac
    done
}

# ============================================================================
# Data Source Selection
# ============================================================================

select_datasource() {
    print_section "Data Source Selection"

    # Check for environment variable first
    if [ -n "${TEST_DATASOURCE:-}" ]; then
        case "${TEST_DATASOURCE}" in
            sqlite|1)
                DATASOURCE="sqlite"
                print_success "Selected: SQLite (from TEST_DATASOURCE)"
                return 0
                ;;
            postgresql|2)
                DATASOURCE="postgresql"
                print_success "Selected: PostgreSQL (from TEST_DATASOURCE)"
                return 0
                ;;
            clickhouse|3)
                DATASOURCE="clickhouse"
                print_success "Selected: ClickHouse (from TEST_DATASOURCE)"
                return 0
                ;;
            redis|4)
                DATASOURCE="redis"
                print_success "Selected: Redis (from TEST_DATASOURCE)"
                return 0
                ;;
            *)
                print_warn "Invalid TEST_DATASOURCE value: ${TEST_DATASOURCE}, falling back to interactive mode"
                ;;
        esac
    fi

    # Non-interactive fallback
    if [ ! -t 0 ]; then
        DATASOURCE="${DATASOURCE:-sqlite}"
        print_warn "No TTY detected. Defaulting datasource to ${DATASOURCE}."
        return 0
    fi

    echo "Select event data source for aggregation features:"
    echo ""
    echo "  1) SQLite      - Lightweight file-based database [default]"
    echo "  2) PostgreSQL  - Production-grade RDBMS"
    echo "  3) ClickHouse  - High-performance OLAP database"
    echo "  4) Redis       - In-memory data store (lookup features only)"
    echo ""

    while true; do
        # Avoid exiting the whole script on read error with set -e
        set +e
        read -p "Enter choice [1-4] (default: 1): " choice
        rc=$?
        set -e
        if [ $rc -ne 0 ]; then
            choice="1"
        fi
        choice="${choice:-1}"  # Default to 1 if empty
        case $choice in
            1)
                DATASOURCE="sqlite"
                print_success "Selected: SQLite"
                break
                ;;
            2)
                DATASOURCE="postgresql"
                print_success "Selected: PostgreSQL"
                break
                ;;
            3)
                DATASOURCE="clickhouse"
                print_success "Selected: ClickHouse"
                break
                ;;
            4)
                DATASOURCE="redis"
                print_success "Selected: Redis"
                break
                ;;
            *)
                print_error "Invalid choice. Please enter 1, 2, 3, or 4."
                ;;
        esac
    done
}

# ============================================================================
# Data Initialization
# ============================================================================

init_event_data() {
    local init_script=""

    case $DATASOURCE in
        postgresql)
            init_script="${SCRIPT_DIR}/init_postgresql.sh"
            ;;
        clickhouse)
            init_script="${SCRIPT_DIR}/init_clickhouse.sh"
            ;;
        sqlite)
            init_script="${SCRIPT_DIR}/init_sqlite.sh"
            ;;
        redis)
            init_script="${SCRIPT_DIR}/init_redis.sh"
            ;;
    esac

    print_info "Initializing $DATASOURCE data..."

    if [ -f "$init_script" ]; then
        bash "$init_script"
        print_success "$DATASOURCE data initialized"
    else
        print_error "$init_script not found"
        return 1
    fi
}

initialize_data() {
    print_section "Data Initialization"

    echo "Initializing test data with automatic cleanup..."
    echo ""

    # Initialize primary data source
    init_event_data || {
        print_error "Event data initialization failed"
        exit 1
    }

    echo ""
    print_success "Data initialized successfully!"
}

# ============================================================================
# Server Management
# ============================================================================

start_server() {
    print_section "Starting CORINT Server"

    # Check if server is already running via health check
    if curl -s "http://${HTTP_HOST}/health" &>/dev/null; then
        print_warn "Server is already running at ${HTTP_HOST}"

        # Handle read with set +e to avoid script exit on EOF/error
        local use_existing="Y"
        if [ -t 0 ]; then
            set +e
            read -p "Use existing server? [Y/n]: " use_existing
            rc=$?
            set -e
            if [ $rc -ne 0 ]; then
                use_existing="Y"
            fi
        fi

        use_existing="${use_existing:-Y}"
        if [ "$use_existing" != "n" ] && [ "$use_existing" != "N" ]; then
            print_success "Using existing server"
            return 0
        fi
        print_info "Please stop the existing server manually and restart this script"
        exit 1
    fi

    # Check if port is occupied by another process (even if not responding to health check)
    local port_pid=""
    port_pid=$(lsof -t -i ":${HTTP_PORT}" 2>/dev/null || true)
    if [ -n "$port_pid" ]; then
        print_warn "Port ${HTTP_PORT} is occupied by process: $port_pid"
        print_info "Killing process $port_pid..."
        kill "$port_pid" 2>/dev/null || true
        sleep 1
        # Force kill if still running
        if kill -0 "$port_pid" 2>/dev/null; then
            kill -9 "$port_pid" 2>/dev/null || true
            sleep 1
        fi
        print_success "Process killed"
    fi

    # Build the server
    print_info "Building server (release mode)..."
    cd "$PROJECT_ROOT"
    if ! cargo build --release -p corint-server 2>&1 | tail -10; then
        print_error "Failed to build server"
        exit 1
    fi

    # Create log file
    local log_file="${PROJECT_ROOT}/server.log"

    # Start server in background with log output
    print_info "Starting server (log: ${log_file})..."
    export RUST_LOG="${RUST_LOG:-info}"

    "${PROJECT_ROOT}/target/release/corint-server" < /dev/null > "${log_file}" 2>&1 &
    SERVER_PID=$!

    # Wait for server to start
    print_info "Waiting for server to be ready..."
    local max_wait=60
    local waited=0

    # Disable set -e temporarily for health check loop
    set +e
    while true; do
        if curl -s "http://${HTTP_HOST}/health" &>/dev/null; then
            break
        fi

        # Check if process is still running
        if ! kill -0 "$SERVER_PID" 2>/dev/null; then
            print_error "Server process exited unexpectedly"
            echo ""
            echo -e "${YELLOW}Last 20 lines of server log:${NC}"
            tail -20 "${log_file}" 2>/dev/null || echo "(no log available)"
            exit 1
        fi

        sleep 1
        waited=$((waited + 1))
        if [ $waited -ge $max_wait ]; then
            print_error "Server failed to start within ${max_wait} seconds"
            echo ""
            echo -e "${YELLOW}Last 20 lines of server log:${NC}"
            tail -20 "${log_file}" 2>/dev/null || echo "(no log available)"
            exit 1
        fi
    done
    set -e

    print_success "Server started successfully (PID: $SERVER_PID)"
    echo "[DEBUG] start_server function completed"
}

# ============================================================================
# HTTP Test Functions
# ============================================================================

http_decide() {
    local user_id="$1"
    local event_type="$2"
    local amount="${3:-0}"
    local device_id="${4:-device_001}"
    local ip_address="${5:-192.168.1.1}"
    local country="${6:-US}"
    local city="${7:-New York}"
    local enable_trace="${8:-false}"
    local return_features="${9:-false}"

    local payload=$(cat <<EOF
{
  "event": {
    "type": "${event_type}",
    "user_id": "${user_id}",
    "amount": ${amount},
    "currency": "USD",
    "device_id": "${device_id}",
    "ip_address": "${ip_address}",
    "country": "${country}",
    "city": "${city}",
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  },
  "options": {
    "enable_trace": ${enable_trace},
    "return_features": ${return_features}
  }
}
EOF
)

    curl -s -X POST "http://${HTTP_HOST}/v1/decide" \
        -H "Content-Type: application/json" \
        -d "$payload"
}

http_health_check() {
    curl -s -X GET "http://${HTTP_HOST}/health"
}

# ============================================================================
# gRPC Test Functions
# ============================================================================

grpc_decide() {
    local user_id="$1"
    local event_type="$2"
    local amount="${3:-0}"
    local device_id="${4:-device_001}"
    local ip_address="${5:-192.168.1.1}"

    grpcurl -plaintext -d "{
        \"event\": {
            \"user_id\": {\"string_value\": \"${user_id}\"},
            \"type\": {\"string_value\": \"${event_type}\"},
            \"amount\": {\"double_value\": ${amount}},
            \"device_id\": {\"string_value\": \"${device_id}\"},
            \"ip_address\": {\"string_value\": \"${ip_address}\"}
        }
    }" "$GRPC_HOST" corint.decision.v1.DecisionService/Decide
}

grpc_health_check() {
    grpcurl -plaintext "$GRPC_HOST" corint.decision.v1.DecisionService/HealthCheck
}

# ============================================================================
# Test Scenarios
# ============================================================================

run_test() {
    local name="$1"
    local user_id="$2"
    local event_type="$3"
    local amount="$4"
    local device_id="$5"
    local ip_address="$6"
    local country="${7:-US}"
    local city="${8:-New York}"
    local expected="$9"

    echo -e "${BOLD}Testing: ${name}${NC}"
    echo -e "  User: ${user_id}, Event: ${event_type}, Amount: \$${amount}"
    echo -e "  Expected: ${YELLOW}${expected}${NC}"
    echo ""

    local response=""
    local request=""
    if [ "$PROTOCOL" = "http" ]; then
        request=$(cat <<EOF
{
  "event": {
    "type": "${event_type}",
    "user_id": "${user_id}",
    "amount": ${amount},
    "currency": "USD",
    "device_id": "${device_id}",
    "ip_address": "${ip_address}",
    "country": "${country}",
    "city": "${city}",
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  },
  "options": {
    "enable_trace": true,
    "return_features": true
  }
}
EOF
)
        echo -e "${CYAN}Request:${NC}"
        echo "$request" | jq . 2>/dev/null || echo "$request"
        echo ""

        response=$(curl -s -X POST "http://${HTTP_HOST}/v1/decide" \
            -H "Content-Type: application/json" \
            -d "$request")
    else
        request="{
  \"event\": {
    \"user_id\": {\"string_value\": \"${user_id}\"},
    \"type\": {\"string_value\": \"${event_type}\"},
    \"amount\": {\"double_value\": ${amount}},
    \"device_id\": {\"string_value\": \"${device_id}\"},
    \"ip_address\": {\"string_value\": \"${ip_address}\"}
  }
}"
        echo -e "${CYAN}Request:${NC}"
        echo "$request" | jq . 2>/dev/null || echo "$request"
        echo ""

        response=$(grpc_decide "$user_id" "$event_type" "$amount" "$device_id" "$ip_address")
    fi

    echo -e "${CYAN}Response:${NC}"
    if command -v jq &> /dev/null; then
        echo "$response" | jq '{
            request_id: .request_id,
            status: .status,
            decision: .decision,
            process_time_ms: .process_time_ms
        }' 2>/dev/null || echo "$response"
    else
        echo "$response"
    fi
    echo ""
    echo "───────────────────────────────────────────────────────────"
    echo ""
}

test_normal_user() {
    print_section "Scenario 1: Normal User (Low Risk)"
    echo "Description: User with typical behavior pattern, should be approved"
    echo ""

    run_test "Normal user transaction" \
        "normal_user_001" "transaction" "150" \
        "device_n001" "192.168.1.100" "US" "New York" \
        "APPROVE"
}

test_normal_user_2() {
    print_section "Scenario 2: Normal User with Moderate Activity"
    echo "Description: User with moderate activity, should be approved"
    echo ""

    run_test "Moderate activity user" \
        "normal_user_002" "transaction" "200" \
        "device_n002" "192.168.1.101" "US" "Los Angeles" \
        "APPROVE"
}

test_high_frequency_login() {
    print_section "Scenario 3: High Frequency Login"
    echo "Description: User with 15 logins/24h, 8 logins/1h - velocity detection"
    echo ""

    run_test "High frequency login user" \
        "high_freq_001" "login" "0" \
        "device_hf01" "10.0.0.50" "US" "Chicago" \
        "REVIEW (velocity pattern)"
}

test_failed_login() {
    print_section "Scenario 4: Multiple Failed Logins"
    echo "Description: User with 6 failed logins in 1 hour - account takeover attempt"
    echo ""

    run_test "Failed login user (transaction after failures)" \
        "failed_login_001" "transaction" "500" \
        "device_fl02" "172.16.0.20" "RU" "Moscow" \
        "REVIEW/DECLINE (account takeover)"
}

test_high_transaction_volume() {
    print_section "Scenario 5: High Transaction Volume"
    echo "Description: User with 12 transactions/24h, \$8500 total - velocity abuse"
    echo ""

    run_test "High transaction volume user" \
        "high_txn_001" "transaction" "500" \
        "device_ht01" "10.10.10.10" "US" "Miami" \
        "REVIEW (velocity abuse)"
}

test_multi_device() {
    print_section "Scenario 6: Multiple Devices (Fraud Farm Pattern)"
    echo "Description: User with 8 different devices in 7 days"
    echo ""

    run_test "Multi-device user" \
        "multi_device_001" "transaction" "999.99" \
        "device_md08" "192.168.100.1" "CN" "Shanghai" \
        "DECLINE (fraud farm)"
}

test_multi_ip() {
    print_section "Scenario 7: Multiple IPs (Suspicious Geography)"
    echo "Description: User from 5 different IPs/24h, 6 cities/30d - impossible travel"
    echo ""

    run_test "Multi-IP user" \
        "multi_ip_001" "transaction" "2500" \
        "device_mi01" "8.8.8.5" "BR" "Sao Paulo" \
        "REVIEW/DECLINE (impossible travel)"
}

test_new_user() {
    print_section "Scenario 8: New User High Value Transaction"
    echo "Description: 2-day-old account making \$5000 transaction"
    echo ""

    run_test "New user high value" \
        "new_user_001" "transaction" "5000" \
        "device_nu01" "203.0.113.10" "US" "Boston" \
        "REVIEW (new account risk)"
}

test_vip_user() {
    print_section "Scenario 9: VIP Verified User"
    echo "Description: Verified VIP user making \$10000 transaction - should bypass rules"
    echo ""

    run_test "VIP user high value" \
        "vip_user_001" "transaction" "10000" \
        "device_vip01" "10.20.30.40" "US" "Seattle" \
        "APPROVE (VIP bypass)"
}

test_suspicious_user() {
    print_section "Scenario 10: Highly Suspicious User"
    echo "Description: Multiple risk signals - failed logins + multiple devices/IPs + rapid transactions"
    echo ""

    run_test "Suspicious user" \
        "suspicious_001" "transaction" "999" \
        "device_sus05" "198.51.100.5" "VN" "Hanoi" \
        "DECLINE (fraud farm + account takeover)"
}

test_stats_user() {
    print_section "Scenario 11: Statistical Feature Testing"
    echo "Description: User for testing statistical aggregation features"
    echo ""

    run_test "Stats user" \
        "stats_user_001" "transaction" "250" \
        "device_st01" "10.100.0.1" "US" "Denver" \
        "APPROVE (baseline data)"
}

# ============================================================================
# Test Menu
# ============================================================================

show_test_menu() {
    print_section "Test Scenarios"

    echo "Select a test scenario to run:"
    echo ""
    echo -e "  ${BOLD}Individual Scenarios:${NC}"
    echo "   1) Normal User (Low Risk)            - Expected: APPROVE"
    echo "   2) Normal User with Moderate Activity - Expected: APPROVE"
    echo "   3) High Frequency Login              - Expected: REVIEW (velocity)"
    echo "   4) Multiple Failed Logins            - Expected: REVIEW/DECLINE (account takeover)"
    echo "   5) High Transaction Volume           - Expected: REVIEW (velocity abuse)"
    echo "   6) Multiple Devices (Fraud Farm)     - Expected: DECLINE"
    echo "   7) Multiple IPs (Suspicious Geo)     - Expected: REVIEW/DECLINE"
    echo "   8) New User High Value Transaction   - Expected: REVIEW"
    echo "   9) VIP Verified User                 - Expected: APPROVE (bypass)"
    echo "  10) Highly Suspicious User            - Expected: DECLINE"
    echo "  11) Statistical Feature Testing       - Expected: APPROVE"
    echo ""
    echo -e "  ${BOLD}Batch Operations:${NC}"
    echo "  12) Run ALL scenarios"
    echo "  13) Run only APPROVE scenarios (1, 2, 9, 11)"
    echo "  14) Run only DECLINE scenarios (6, 10)"
    echo "  15) Run only REVIEW scenarios (3, 4, 5, 7, 8)"
    echo ""
    echo "   0) Exit"
    echo ""
}

run_all_scenarios() {
    test_normal_user
    test_normal_user_2
    test_high_frequency_login
    test_failed_login
    test_high_transaction_volume
    test_multi_device
    test_multi_ip
    test_new_user
    test_vip_user
    test_suspicious_user
    test_stats_user

    print_section "All Scenarios Completed"
    echo "Review the results above to verify expected decisions."
}

run_approve_scenarios() {
    test_normal_user
    test_normal_user_2
    test_vip_user
    test_stats_user

    print_section "APPROVE Scenarios Completed"
}

run_decline_scenarios() {
    test_multi_device
    test_suspicious_user

    print_section "DECLINE Scenarios Completed"
}

run_review_scenarios() {
    test_high_frequency_login
    test_failed_login
    test_high_transaction_volume
    test_multi_ip
    test_new_user

    print_section "REVIEW Scenarios Completed"
}

# ============================================================================
# Main Loop
# ============================================================================

main() {
    echo "[DEBUG] main() started"
    print_header "CORINT Decision Engine - Interactive Test"

    # Read configuration from config/server.yaml
    echo "[DEBUG] calling read_config..."
    read_config
    echo "[DEBUG] read_config done: HTTP=${HTTP_HOST}, gRPC=${GRPC_HOST}"
    print_info "Config: HTTP=${HTTP_HOST}, gRPC=${GRPC_HOST}"

    # Step 1: Select data source
    echo "[DEBUG] calling select_datasource..."
    select_datasource
    echo "[DEBUG] select_datasource done: DATASOURCE=${DATASOURCE}"

    # Step 2: Initialize data (automatic)
    echo "[DEBUG] calling initialize_data..."
    print_info "Step 2/5: Initializing test data..."
    initialize_data
    echo "[DEBUG] initialize_data done"

    # Step 3: Start server (automatic)
    echo "[DEBUG] calling start_server..."
    print_info "Step 3/5: Starting server..."
    start_server
    echo "[DEBUG] start_server done"

    # Step 4: Health check
    print_info "Step 4/5: Running health check..."
    print_section "Health Check"
    local health
    health=$(http_health_check) || true
    echo "[DEBUG] Health response: $health"
    if echo "$health" | grep -q "healthy\|ok"; then
        print_success "Server is healthy"
        echo "$health" | jq . 2>/dev/null || echo "$health"
    else
        print_error "Health check failed"
        echo "$health"
        exit 1
    fi

    echo "[DEBUG] Health check passed, proceeding to protocol selection..."

    # Clear any buffered input before interactive prompts
    read -t 0.1 -n 10000 discard 2>/dev/null || true

    echo "[DEBUG] About to call select_protocol..."

    # Step 5: Select protocol
    print_info "Step 5/5: Ready for testing"
    select_protocol

    echo "[DEBUG] Protocol selected: $PROTOCOL"
    echo "[DEBUG] Entering test menu loop..."

    # Check for auto-run environment variable
    if [ -n "${TEST_AUTO_RUN:-}" ]; then
        print_section "Auto-Run Mode"
        print_info "TEST_AUTO_RUN is set, running all test scenarios..."

        # Run all test scenarios
        run_all_scenarios

        print_section "Auto-Run Complete"
        print_success "All test scenarios completed!"

        # Keep server running if requested
        if [ "${KEEP_SERVER_RUNNING:-}" = "true" ]; then
            print_info "KEEP_SERVER_RUNNING=true, server will continue running (PID: $SERVER_PID)"
            print_info "Press Ctrl+C to exit and stop the server"
            trap 'print_info "Exiting..."; exit 0' INT TERM
            while true; do
                sleep 1
            done
        else
            print_info "Exiting (server will be stopped)..."
            return 0
        fi
    fi

    # Check if running in non-interactive mode
    if [ ! -t 0 ]; then
        print_warn "Running in non-interactive mode. Server is running."
        print_info "You can test the API manually or run this script in an interactive terminal."
        print_info "Server PID: $SERVER_PID"
        print_info "HTTP endpoint: http://${HTTP_HOST}/v1/decide"
        print_info ""
        print_info "To keep the server running, press Ctrl+C to exit this script."
        print_info "The server will continue running in the background."
        print_info ""
        print_info "Waiting for manual termination (Ctrl+C)..."
        # Wait indefinitely until interrupted
        trap 'print_info "Exiting..."; exit 0' INT TERM
        while true; do
            sleep 1
        done
    fi

    # Test menu loop (interactive mode only)
    while true; do
        show_test_menu

        # Handle read with set +e to avoid script exit on EOF/error
        set +e
        read -p "Enter choice [0-15]: " choice
        rc=$?
        set -e

        # Exit gracefully on read error (e.g., EOF, no TTY)
        if [ $rc -ne 0 ]; then
            echo ""
            print_info "Input closed, exiting..."
            break
        fi

        echo "[DEBUG] User choice: '$choice'"

        case $choice in
            0)
                print_info "Exiting..."
                break
                ;;
            1) test_normal_user ;;
            2) test_normal_user_2 ;;
            3) test_high_frequency_login ;;
            4) test_failed_login ;;
            5) test_high_transaction_volume ;;
            6) test_multi_device ;;
            7) test_multi_ip ;;
            8) test_new_user ;;
            9) test_vip_user ;;
            10) test_suspicious_user ;;
            11) test_stats_user ;;
            12) run_all_scenarios ;;
            13) run_approve_scenarios ;;
            14) run_decline_scenarios ;;
            15) run_review_scenarios ;;
            *)
                print_error "Invalid choice. Please enter 0-15."
                ;;
        esac

    done

    print_header "Test Session Completed"
    echo "Thank you for testing CORINT Decision Engine!"
    echo ""
}

# Run main
main
