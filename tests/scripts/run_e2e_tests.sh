#!/bin/bash

# ============================================================================
# CORINT Decision Engine - E2E Test Runner
# ============================================================================
#
# This script runs end-to-end tests for the CORINT Decision Engine:
# 1. Generates test data with relative timestamps
# 2. Builds and starts the server with test configuration
# 3. Runs test cases covering all feature types
# 4. Collects results and generates report
# 5. Cleans up server process
#
# Usage:
#   cd tests
#   ./scripts/run_e2e_tests.sh
#
# ============================================================================

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
API_URL="http://localhost:8080"
SERVER_PORT=8080
TEST_REPO="tests/e2e_repo"
TEST_DB="tests/data/e2e_test.db"
TEST_SQL="tests/data/test_data.sql"
RESULTS_DIR="tests/results"
SERVER_PID_FILE="/tmp/corint_e2e_server.pid"

# Configuration file paths
CONFIG_DIR="config"
CONFIG_FILE="$CONFIG_DIR/server.yaml"
CONFIG_BACKUP="$CONFIG_DIR/server.yaml.backup.$(date +%s)"
TEST_CONFIG_FILE="tests/e2e_server.yaml"

# Counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Arrays to track test results
declare -a PASSED_TEST_NAMES=()
declare -a FAILED_TEST_NAMES=()
declare -a FAILED_TEST_DETAILS=()

# ============================================================================
# Helper Functions
# ============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

backup_config() {
    if [ -f "$CONFIG_FILE" ]; then
        log_info "Backing up existing config: $CONFIG_FILE -> $CONFIG_BACKUP"
        cp "$CONFIG_FILE" "$CONFIG_BACKUP"
        log_success "Config backed up"
        return 0
    else
        log_warning "No existing config file to backup"
        return 0
    fi
}

restore_config() {
    if [ -f "$CONFIG_BACKUP" ]; then
        log_info "Restoring original config: $CONFIG_BACKUP -> $CONFIG_FILE"
        mv "$CONFIG_BACKUP" "$CONFIG_FILE"
        log_success "Config restored"
    else
        log_warning "No backup config to restore"
    fi
}

setup_test_config() {
    if [ ! -f "$TEST_CONFIG_FILE" ]; then
        log_error "Test config file not found: $TEST_CONFIG_FILE"
        exit 1
    fi

    log_info "Setting up test config: $TEST_CONFIG_FILE -> $CONFIG_FILE"

    # Ensure config directory exists
    mkdir -p "$CONFIG_DIR"

    # Copy test config to config directory
    cp "$TEST_CONFIG_FILE" "$CONFIG_FILE"
    log_success "Test config installed"
}

cleanup() {
    log_info "Cleaning up..."

    # Stop server if running
    if [ -f "$SERVER_PID_FILE" ]; then
        PID=$(cat "$SERVER_PID_FILE")
        if ps -p $PID > /dev/null 2>&1; then
            log_info "Stopping server (PID: $PID)..."
            kill $PID 2>/dev/null || true
            sleep 2

            # Force kill if still running
            if ps -p $PID > /dev/null 2>&1; then
                kill -9 $PID 2>/dev/null || true
            fi
        fi
        rm -f "$SERVER_PID_FILE"
    fi

    # Also try pkill as fallback
    pkill -f "corint-server" 2>/dev/null || true

    # Restore original config
    restore_config
}

# Trap to ensure cleanup on exit
trap cleanup EXIT INT TERM

wait_for_server() {
    log_info "Waiting for server to start..."
    local max_attempts=30
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        if curl -s "$API_URL/health" > /dev/null 2>&1; then
            log_success "Server is ready!"
            return 0
        fi
        attempt=$((attempt + 1))
        sleep 1
    done

    log_error "Server failed to start within timeout"
    return 1
}

run_test_case() {
    local test_name="$1"
    local test_data="$2"
    local expected_decision="$3"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    # Run API call and capture response
    local response=$(curl -s -X POST "$API_URL/v1/decide" \
        -H "Content-Type: application/json" \
        -d "$test_data")

    # Extract decision from response (nested in .decision.result)
    local actual_decision=$(echo "$response" | jq -r '.decision.result // "UNKNOWN"' | tr '[:upper:]' '[:lower:]')
    local error=$(echo "$response" | jq -r '.error // empty')

    # Check result
    if [ -n "$error" ]; then
        log_error "$test_name: API ERROR - $error"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        FAILED_TEST_NAMES+=("$test_name")
        FAILED_TEST_DETAILS+=("$test_name|API_ERROR|Expected: $expected_decision, Got: API error - $error")
        return 1
    elif [ "$actual_decision" = "$expected_decision" ]; then
        log_success "$test_name: PASSED (decision: $actual_decision)"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        PASSED_TEST_NAMES+=("$test_name")
        return 0
    else
        log_error "$test_name: FAILED (expected: $expected_decision, got: $actual_decision)"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        FAILED_TEST_NAMES+=("$test_name")
        FAILED_TEST_DETAILS+=("$test_name|WRONG_DECISION|Expected: $expected_decision, Got: $actual_decision")
        return 1
    fi
}

# ============================================================================
# Main Test Flow
# ============================================================================

main() {
    echo "============================================================================"
    echo "CORINT Decision Engine - E2E Test Suite"
    echo "============================================================================"
    echo ""

    # Create results directory
    mkdir -p "$RESULTS_DIR"

    # Step 0: Backup and setup configuration
    log_info "Step 0: Setting up test configuration..."
    backup_config
    setup_test_config
    log_success "Configuration ready"
    echo ""

    # Step 1: Generate SQL statements
    log_info "Step 1: Generating SQL test data..."
    python3 tests/scripts/generate_test_data.py
    if [ $? -ne 0 ]; then
        log_error "Failed to generate SQL data"
        exit 1
    fi
    log_success "SQL data generated"
    echo ""

    # Step 1.5: Create database and insert data
    log_info "Step 1.5: Creating database and inserting test data..."

    # Remove old database
    rm -f "$TEST_DB"

    # Execute SQL file
    sqlite3 "$TEST_DB" < "$TEST_SQL"
    if [ $? -ne 0 ]; then
        log_error "Failed to create database from SQL"
        exit 1
    fi

    # Verify data insertion
    EVENT_COUNT=$(sqlite3 "$TEST_DB" "SELECT COUNT(*) FROM events;")
    LIST_COUNT=$(sqlite3 "$TEST_DB" "SELECT COUNT(*) FROM list_entries;")
    log_success "Database created with $EVENT_COUNT events and $LIST_COUNT list entries"

    # Verify database lists
    log_info "Step 1.6: Verifying database list data..."
    bash tests/scripts/verify_db_lists.sh > /tmp/db_list_verification.log 2>&1
    if [ $? -eq 0 ]; then
        log_success "Database list verification passed"
        echo "  Lists created:"
        sqlite3 "$TEST_DB" "SELECT '  - ' || list_id || ': ' || COUNT(*) || ' entries' FROM list_entries GROUP BY list_id;"
    else
        log_error "Database list verification failed - see /tmp/db_list_verification.log"
    fi
    echo ""

    # Step 2: Build server
    log_info "Step 2: Building server..."
    cargo build --release --quiet
    if [ $? -ne 0 ]; then
        log_error "Failed to build server"
        exit 1
    fi
    log_success "Server built successfully"
    echo ""

    # Step 3: Start server with test configuration
    log_info "Step 3: Starting test server..."

    # Use absolute path for test database
    TEST_DB_ABSOLUTE="$(pwd)/$TEST_DB"

    # Set environment variables for server
    export DATABASE_URL="sqlite://$TEST_DB_ABSOLUTE"

    # Start server in background using config/server.yaml (already set up with test config)
    RUST_LOG=error \
        target/release/corint-server > "$RESULTS_DIR/server.log" 2>&1 &

    SERVER_PID=$!
    echo $SERVER_PID > "$SERVER_PID_FILE"

    log_info "Server started with test config (PID: $SERVER_PID)"

    # Wait for server to be ready
    if ! wait_for_server; then
        log_error "Server startup failed. Check logs at $RESULTS_DIR/server.log"
        exit 1
    fi
    echo ""

    # Step 4: Run test cases
    log_info "Step 4: Running test cases..."
    echo "============================================================================"
    echo ""

    # Get current timestamp in ISO format
    CURRENT_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    # -------------------------------------------------------------------------
    # Transaction Tests
    # -------------------------------------------------------------------------
    echo "--- Transaction Flow Tests ---"
    echo ""

    # Test 1: Normal transaction - should approve
    run_test_case "Normal Transaction" '{
        "event": {
            "type": "transaction",
            "user_id": "user_9998",
            "amount": 150.50,
            "country": "US",
            "ip_address": "192.168.1.100",
            "device_id": "device_00001",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "approve"

    # Test 2: Blocked user - should decline
    run_test_case "Blocked User Transaction" '{
        "event": {
            "type": "transaction",
            "user_id": "sus_0001",
            "amount": 100.00,
            "country": "US",
            "ip_address": "192.168.1.100",
            "device_id": "device_00001",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "decline"

    # Test 3: High value from new user - should review
    run_test_case "High Value New User" '{
        "event": {
            "type": "transaction",
            "user_id": "user_0050",
            "amount": 5000.00,
            "country": "US",
            "ip_address": "192.168.1.100",
            "device_id": "device_00001",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "review"

    # Test 4: High-risk country - should review
    run_test_case "High Risk Country" '{
        "event": {
            "type": "transaction",
            "user_id": "user_0002",
            "amount": 200.00,
            "country": "NG",
            "ip_address": "192.168.1.100",
            "device_id": "device_00001",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "review"

    echo ""
    echo "--- Login Flow Tests ---"
    echo ""

    # Test 5: Normal login - should approve
    run_test_case "Normal Login" '{
        "event": {
            "type": "login",
            "user_id": "user_9999",
            "country": "US",
            "ip_address": "192.168.1.100",
            "device_id": "device_00001",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "approve"

    # Test 6: Blocked IP - should decline
    run_test_case "Blocked IP Login" '{
        "event": {
            "type": "login",
            "user_id": "user_0001",
            "country": "US",
            "ip_address": "45.142.212.61",
            "device_id": "device_00001",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "decline"

    # Test 7: High-risk country login - should review
    run_test_case "High Risk Country Login" '{
        "event": {
            "type": "login",
            "user_id": "user_0002",
            "country": "RU",
            "ip_address": "192.168.1.200",
            "device_id": "device_00002",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "review"

    echo ""
    echo "--- Payment Flow Tests ---"
    echo ""

    # Test 8: Normal payment - should approve
    run_test_case "Normal Payment" '{
        "event": {
            "type": "payment",
            "user_id": "user_9997",
            "amount": 299.99,
            "country": "US",
            "ip_address": "192.168.1.100",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "approve"

    # Test 9: Very high payment - should review
    run_test_case "Very High Payment" '{
        "event": {
            "type": "payment",
            "user_id": "user_0005",
            "amount": 8000.00,
            "country": "US",
            "ip_address": "192.168.1.100",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "review"

    # Test 10: Blocked user payment - should decline
    run_test_case "Blocked User Payment" '{
        "event": {
            "type": "payment",
            "user_id": "sus_0002",
            "amount": 50.00,
            "country": "US",
            "ip_address": "192.168.1.100",
            "timestamp": "'"$CURRENT_TIME"'"
        }
    }' "decline"

    echo ""
    echo "============================================================================"
    echo ""

    # Step 5: Print detailed test report
    log_info "Step 5: Test Report"
    echo "============================================================================"
    echo ""

    # Summary
    echo -e "${BLUE}Test Summary:${NC}"
    echo "  Total Tests:  $TOTAL_TESTS"
    echo -e "  ${GREEN}Passed:       $PASSED_TESTS${NC}"
    echo -e "  ${RED}Failed:       $FAILED_TESTS${NC}"
    echo ""

    # Passed tests
    if [ $PASSED_TESTS -gt 0 ]; then
        echo -e "${GREEN}✓ Passed Tests ($PASSED_TESTS):${NC}"
        for test_name in "${PASSED_TEST_NAMES[@]}"; do
            echo -e "  ${GREEN}✓${NC} $test_name"
        done
        echo ""
    fi

    # Failed tests
    if [ $FAILED_TESTS -gt 0 ]; then
        echo -e "${RED}✗ Failed Tests ($FAILED_TESTS):${NC}"
        for detail in "${FAILED_TEST_DETAILS[@]}"; do
            IFS='|' read -r name error_type message <<< "$detail"
            echo -e "  ${RED}✗${NC} $name"
            echo -e "    ${YELLOW}→${NC} $message"
        done
        echo ""
    fi

    echo "============================================================================"
    echo ""

    if [ $FAILED_TESTS -eq 0 ]; then
        log_success "All tests passed! 🎉"
        echo ""
        exit 0
    else
        log_error "$FAILED_TESTS test(s) failed"
        echo ""
        exit 1
    fi
}

# Run main function
main
