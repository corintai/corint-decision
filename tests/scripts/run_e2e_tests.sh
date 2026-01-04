#!/bin/bash

# ============================================================================
# CORINT Decision Engine - E2E Test Runner
# ============================================================================
#
# This script runs end-to-end tests for the CORINT Decision Engine:
# 1. Selects data source (SQLite, PostgreSQL, ClickHouse, Redis, or All)
# 2. Generates test data with relative timestamps
# 3. Builds and starts the server with test configuration
# 4. Runs test cases covering all feature types
# 5. Collects results and generates report
# 6. Cleans up server process
#
# Usage:
#   cd tests
#   ./scripts/run_e2e_tests.sh              # Interactive mode
#   ./scripts/run_e2e_tests.sh --sqlite     # SQLite only
#   ./scripts/run_e2e_tests.sh --postgres   # PostgreSQL only
#   ./scripts/run_e2e_tests.sh --clickhouse # ClickHouse only
#   ./scripts/run_e2e_tests.sh --all        # All datasources
#
# ============================================================================

set -e  # Exit on error

# Determine project root directory
# If we're in the tests directory, move to parent
if [ -d "../crates" ] && [ -f "../Cargo.toml" ]; then
    cd ..
fi

# Verify we're in the project root
if [ ! -d "crates" ] || [ ! -f "Cargo.toml" ]; then
    echo "Error: Must run from project root or tests directory"
    exit 1
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Test configuration
API_URL="http://localhost:8080"
SERVER_PORT=8080
TEST_REPO="tests/e2e_repo"
RESULTS_DIR="tests/results"
SERVER_PID_FILE="/tmp/corint_e2e_server.pid"

# Database files
SQLITE_DB="tests/data/e2e_test.db"
SQLITE_SQL="tests/data/test_data.sql"
POSTGRES_SQL="tests/data/postgres_test_data.sql"
CLICKHOUSE_SQL="tests/data/clickhouse_test_data.sql"

# Configuration file paths
CONFIG_DIR="config"
CONFIG_FILE="$CONFIG_DIR/server.yaml"
CONFIG_BACKUP="$CONFIG_DIR/server.yaml.backup"
TEST_CONFIG_FILE="tests/e2e_server.yaml"

# Feature files
FEATURES_DIR="$TEST_REPO/configs/features"
FEATURES_TEMPLATES="$TEST_REPO/templates/features"
ACTIVE_FEATURES="$FEATURES_DIR/e2e_features.yaml"

# Datasource selection
DATASOURCE=""

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

log_header() {
    echo -e "${CYAN}$1${NC}"
}

show_menu() {
    echo ""
    log_header "============================================================================"
    log_header "CORINT Decision Engine - E2E Test Suite"
    log_header "============================================================================"
    echo ""
    echo "Select data source for testing:"
    echo ""
    echo "  1) SQLite      (default, in-memory)"
    echo "  2) PostgreSQL  (requires running PostgreSQL server)"
    echo "  3) ClickHouse  (requires running ClickHouse server)"
    echo "  4) Redis       (requires running Redis server - coming soon)"
    echo "  5) All         (run tests on all available datasources)"
    echo ""
    echo "  q) Quit"
    echo ""
}

select_datasource() {
    # Check command line arguments first
    case "$1" in
        --sqlite)
            DATASOURCE="sqlite"
            return
            ;;
        --postgres|--postgresql)
            DATASOURCE="postgres"
            return
            ;;
        --clickhouse|--ch)
            DATASOURCE="clickhouse"
            return
            ;;
        --redis)
            DATASOURCE="redis"
            return
            ;;
        --all)
            DATASOURCE="all"
            return
            ;;
        "")
            # Interactive mode
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--sqlite|--postgres|--clickhouse|--redis|--all]"
            exit 1
            ;;
    esac

    # Interactive selection
    show_menu
    read -p "Enter choice [1-5, q]: " choice

    case $choice in
        1|"")
            DATASOURCE="sqlite"
            ;;
        2)
            DATASOURCE="postgres"
            ;;
        3)
            DATASOURCE="clickhouse"
            ;;
        4)
            DATASOURCE="redis"
            ;;
        5)
            DATASOURCE="all"
            ;;
        q|Q)
            echo "Exiting."
            exit 0
            ;;
        *)
            echo "Invalid choice. Using SQLite (default)."
            DATASOURCE="sqlite"
            ;;
    esac
}

backup_config() {
    # Check if current config is actually the test config (from a previous incomplete run)
    if [ -f "$CONFIG_FILE" ] && grep -q "E2E Tests" "$CONFIG_FILE" 2>/dev/null; then
        log_warning "Current config appears to be test config from previous run"
        if [ -f "$CONFIG_BACKUP" ]; then
            log_info "Found backup from previous run, restoring it first"
            restore_config
        else
            log_warning "No backup found, current test config will be overwritten"
            rm -f "$CONFIG_FILE"
        fi
    fi

    if [ -f "$CONFIG_BACKUP" ]; then
        if grep -q "E2E Tests" "$CONFIG_BACKUP" 2>/dev/null; then
            log_warning "Backup appears to be test config, removing it"
            rm -f "$CONFIG_BACKUP"
        else
            log_warning "Old backup exists: $CONFIG_BACKUP (keeping it)"
        fi
    fi

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
        if grep -q "E2E Tests" "$CONFIG_BACKUP" 2>/dev/null; then
            log_error "Backup file appears to be test config, not restoring"
            log_warning "Please manually restore from config/server-example.yaml"
            rm -f "$CONFIG_BACKUP"
            return 1
        fi

        log_info "Restoring original config: $CONFIG_BACKUP -> $CONFIG_FILE"
        if [ -f "$CONFIG_FILE" ]; then
            rm -f "$CONFIG_FILE"
        fi
        mv "$CONFIG_BACKUP" "$CONFIG_FILE"
        log_success "Config restored successfully"
    else
        if [ -f "$CONFIG_FILE" ] && grep -q "E2E Tests" "$CONFIG_FILE" 2>/dev/null; then
            log_warning "No backup config found, but test config is still in place"
            log_warning "Restoring from server-example.yaml"
            cp "$CONFIG_DIR/server-example.yaml" "$CONFIG_FILE"
            log_success "Config restored from example"
        else
            log_info "No backup to restore (config may already be restored)"
        fi
    fi
}

setup_test_config() {
    local datasource=$1

    if [ ! -f "$TEST_CONFIG_FILE" ]; then
        log_error "Test config file not found: $TEST_CONFIG_FILE"
        exit 1
    fi

    log_info "Setting up test config for datasource: $datasource"

    # Ensure config directory exists
    mkdir -p "$CONFIG_DIR"

    # Create config based on datasource
    case $datasource in
        sqlite)
            cp "$TEST_CONFIG_FILE" "$CONFIG_FILE"
            ;;
        postgres)
            create_postgres_config
            ;;
        clickhouse)
            create_clickhouse_config
            ;;
        redis)
            log_error "Redis datasource not yet implemented"
            exit 1
            ;;
    esac

    log_success "Test config installed for $datasource"
}

create_postgres_config() {
    cat > "$CONFIG_FILE" << 'EOF'
# CORINT Decision Engine Server Configuration for E2E Tests (PostgreSQL)
host: "127.0.0.1"
port: 8080
grpc_port: 50051

repository:
  type: filesystem
  path: "tests/e2e_repo"

datasources:
  postgres_e2e:
    type: sql
    provider: postgresql
    connection_string: "${POSTGRES_URL:-postgresql://postgres:postgres@localhost:5432/corint_test}"
    database: "corint_test"
    options:
      max_connections: "5"

enable_metrics: true
enable_tracing: false
log_level: "error"

llm:
  default_provider: openai
  enable_cache: false
  enable_thinking: false
  openai:
    api_key: "${OPENAI_API_KEY:-dummy-key-for-testing}"
    default_model: "gpt-4o-mini"
    max_tokens: 100
    temperature: 0.0
EOF
}

create_clickhouse_config() {
    cat > "$CONFIG_FILE" << 'EOF'
# CORINT Decision Engine Server Configuration for E2E Tests (ClickHouse)
host: "127.0.0.1"
port: 8080
grpc_port: 50051

repository:
  type: filesystem
  path: "tests/e2e_repo"

datasources:
  clickhouse_e2e:
    type: olap
    provider: clickhouse
    connection_string: "${CLICKHOUSE_URL:-http://localhost:8123}"
    database: "default"
    options:
      max_connections: "5"

enable_metrics: true
enable_tracing: false
log_level: "error"

llm:
  default_provider: openai
  enable_cache: false
  enable_thinking: false
  openai:
    api_key: "${OPENAI_API_KEY:-dummy-key-for-testing}"
    default_model: "gpt-4o-mini"
    max_tokens: 100
    temperature: 0.0
EOF
}

setup_features() {
    local datasource=$1
    local template_file="$FEATURES_TEMPLATES/e2e_features_${datasource}.yaml"

    log_info "Setting up features for datasource: $datasource"

    if [ -f "$template_file" ]; then
        cp "$template_file" "$ACTIVE_FEATURES"
        log_success "$datasource features installed"
    else
        log_error "Features template not found: $template_file"
        exit 1
    fi
}

cleanup_features() {
    # Remove the copied features file
    if [ -f "$ACTIVE_FEATURES" ]; then
        rm -f "$ACTIVE_FEATURES"
        log_info "Features file removed"
    fi
}

setup_database() {
    local datasource=$1

    log_info "Setting up database for: $datasource"

    case $datasource in
        sqlite)
            setup_sqlite_database
            ;;
        postgres)
            setup_postgres_database
            ;;
        clickhouse)
            setup_clickhouse_database
            ;;
    esac
}

setup_sqlite_database() {
    # Always regenerate SQL statements with fresh timestamps
    log_info "Generating fresh SQLite test data with current timestamps..."
    python3 tests/scripts/generate_test_data.py
    if [ $? -ne 0 ]; then
        log_error "Failed to generate SQL data"
        exit 1
    fi

    # Remove old database
    rm -f "$SQLITE_DB"

    # Execute SQL file
    sqlite3 "$SQLITE_DB" < "$SQLITE_SQL"
    if [ $? -ne 0 ]; then
        log_error "Failed to create SQLite database"
        exit 1
    fi

    # Verify data insertion
    EVENT_COUNT=$(sqlite3 "$SQLITE_DB" "SELECT COUNT(*) FROM events;")
    LIST_COUNT=$(sqlite3 "$SQLITE_DB" "SELECT COUNT(*) FROM list_entries;")
    log_success "SQLite database created with $EVENT_COUNT events and $LIST_COUNT list entries"

    # Set environment variable
    export DATABASE_URL="sqlite://$(pwd)/$SQLITE_DB"
}

setup_postgres_database() {
    # Check PostgreSQL connection
    POSTGRES_URL="${POSTGRES_URL:-postgresql://postgres:postgres@localhost:5432/corint_test}"

    log_info "Checking PostgreSQL connection..."
    if ! psql "$POSTGRES_URL" -c "SELECT 1;" > /dev/null 2>&1; then
        log_error "Cannot connect to PostgreSQL at: $POSTGRES_URL"
        log_warning "Please ensure PostgreSQL is running and accessible"
        log_warning "Set POSTGRES_URL environment variable if using non-default connection"
        exit 1
    fi

    # First, regenerate SQLite test data (PostgreSQL generation depends on it)
    log_info "Generating fresh test data with current timestamps..."
    python3 tests/scripts/generate_test_data.py
    if [ $? -ne 0 ]; then
        log_error "Failed to generate test data"
        exit 1
    fi

    # Then convert to PostgreSQL format
    log_info "Converting to PostgreSQL format..."
    python3 tests/scripts/generate_postgres_data.py
    if [ $? -ne 0 ]; then
        log_error "Failed to convert to PostgreSQL format"
        exit 1
    fi

    # Execute SQL file
    log_info "Loading test data into PostgreSQL..."
    psql "$POSTGRES_URL" < "$POSTGRES_SQL" > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        log_error "Failed to load PostgreSQL data"
        exit 1
    fi

    # Verify data insertion
    EVENT_COUNT=$(psql "$POSTGRES_URL" -t -c "SELECT COUNT(*) FROM events;" | tr -d ' ')
    LIST_COUNT=$(psql "$POSTGRES_URL" -t -c "SELECT COUNT(*) FROM list_entries;" | tr -d ' ')
    log_success "PostgreSQL loaded with $EVENT_COUNT events and $LIST_COUNT list entries"

    export DATABASE_URL="$POSTGRES_URL"
}

setup_clickhouse_database() {
    # Check ClickHouse connection
    CLICKHOUSE_URL="${CLICKHOUSE_URL:-http://localhost:8123}"

    log_info "Checking ClickHouse connection..."
    if ! curl -s "$CLICKHOUSE_URL/?query=SELECT%201" > /dev/null 2>&1; then
        log_warning "ClickHouse not running at: $CLICKHOUSE_URL"

        # Check if clickhouse binary exists in common locations
        CLICKHOUSE_CMD=""
        if command -v clickhouse &> /dev/null; then
            CLICKHOUSE_CMD="clickhouse"
        elif [ -f "$HOME/clickhouse" ]; then
            CLICKHOUSE_CMD="$HOME/clickhouse"
        elif [ -f "./clickhouse" ]; then
            CLICKHOUSE_CMD="./clickhouse"
        fi

        if [ -z "$CLICKHOUSE_CMD" ]; then
            log_info "ClickHouse not found. Downloading to home directory..."
            (cd "$HOME" && curl https://clickhouse.com/ | sh)
            if [ $? -ne 0 ]; then
                log_error "Failed to download ClickHouse"
                exit 1
            fi
            log_success "ClickHouse downloaded successfully"
            CLICKHOUSE_CMD="$HOME/clickhouse"
        else
            log_info "Found ClickHouse at: $CLICKHOUSE_CMD"
        fi

        # Start ClickHouse server in background
        log_info "Starting ClickHouse server..."
        $CLICKHOUSE_CMD server --daemon 2>/dev/null || $CLICKHOUSE_CMD server &
        CLICKHOUSE_SERVER_PID=$!

        # Wait for server to start
        local max_attempts=30
        local attempt=0
        while [ $attempt -lt $max_attempts ]; do
            if curl -s "$CLICKHOUSE_URL/?query=SELECT%201" > /dev/null 2>&1; then
                log_success "ClickHouse server started!"
                break
            fi
            attempt=$((attempt + 1))
            sleep 1
        done

        if [ $attempt -eq $max_attempts ]; then
            log_error "ClickHouse server failed to start"
            exit 1
        fi
    fi

    # First, regenerate SQLite test data (ClickHouse generation depends on it)
    log_info "Generating fresh test data with current timestamps..."
    python3 tests/scripts/generate_test_data.py
    if [ $? -ne 0 ]; then
        log_error "Failed to generate test data"
        exit 1
    fi

    # Then convert SQLite format to ClickHouse format
    log_info "Converting to ClickHouse format..."
    python3 tests/scripts/generate_clickhouse_data.py
    if [ $? -ne 0 ]; then
        log_error "Failed to convert to ClickHouse format"
        exit 1
    fi

    # Clean up old data before loading new test data
    log_info "Cleaning up old data from ClickHouse..."
    curl -s "$CLICKHOUSE_URL" --data "TRUNCATE TABLE IF EXISTS events" > /dev/null 2>&1 || true
    curl -s "$CLICKHOUSE_URL" --data "TRUNCATE TABLE IF EXISTS list_entries" > /dev/null 2>&1 || true
    log_success "Old data cleaned"

    # Execute SQL file efficiently (use optimized batch method)
    log_info "Loading test data into ClickHouse..."

    # Try using clickhouse-client if available (fastest method)
    CLICKHOUSE_CLIENT=""
    if command -v clickhouse-client &> /dev/null; then
        CLICKHOUSE_CLIENT="clickhouse-client"
    elif [ -n "$CLICKHOUSE_CMD" ] && [ -f "${CLICKHOUSE_CMD}-client" ]; then
        CLICKHOUSE_CLIENT="${CLICKHOUSE_CMD}-client"
    fi

    if [ -n "$CLICKHOUSE_CLIENT" ]; then
        log_info "Using clickhouse-client for faster data loading..."
        if $CLICKHOUSE_CLIENT --host localhost --port 9000 < "$CLICKHOUSE_SQL" > /dev/null 2>&1; then
            log_success "Data loaded via clickhouse-client"
        else
            log_warning "clickhouse-client failed, using HTTP API with batch optimization"
            # Fall through to HTTP method with batch optimization
            CLICKHOUSE_CLIENT=""
        fi
    fi

    # If clickhouse-client not available or failed, use optimized HTTP API method
    if [ -z "$CLICKHOUSE_CLIENT" ]; then
        log_info "Using HTTP API with batch optimization (reduces HTTP requests from ~850 to ~10)..."
        
        # Use Python script to batch INSERT statements for better performance
        # This reduces the number of HTTP requests from ~850 to ~10 (100 statements per batch)
        python3 tests/scripts/load_clickhouse_data.py "$CLICKHOUSE_SQL" "$CLICKHOUSE_URL" 2>&1
        
        if [ $? -eq 0 ]; then
            log_success "Data loaded via HTTP API (batch optimization)"
        else
            log_error "Failed to load data via HTTP API"
            exit 1
        fi
    fi

    # Verify data insertion
    EVENT_COUNT=$(curl -s "$CLICKHOUSE_URL/?query=SELECT%20count()%20FROM%20events" | tr -d '\n')
    LIST_COUNT=$(curl -s "$CLICKHOUSE_URL/?query=SELECT%20count()%20FROM%20list_entries" | tr -d '\n')
    log_success "ClickHouse loaded with $EVENT_COUNT events and $LIST_COUNT list entries"

    export CLICKHOUSE_URL="$CLICKHOUSE_URL"
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

    # Remove copied features file
    cleanup_features
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

    local response=$(curl -s -X POST "$API_URL/v1/decide" \
        -H "Content-Type: application/json" \
        -d "$test_data")

    local actual_decision=$(echo "$response" | jq -r '.decision.result // "UNKNOWN"' | tr '[:upper:]' '[:lower:]')
    local error=$(echo "$response" | jq -r '.error // empty')

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

run_error_test_case() {
    local test_name="$1"
    local test_data="$2"
    local expected_pattern="$3"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    local response=$(curl -s -X POST "$API_URL/v1/decide" \
        -H "Content-Type: application/json" \
        -d "$test_data")

    local has_error=$(echo "$response" | jq -r '.error // empty')
    local decision=$(echo "$response" | jq -r '.decision.result // empty' | tr '[:upper:]' '[:lower:]')
    local reason=$(echo "$response" | jq -r '.decision.reason // empty')

    case "$expected_pattern" in
        "no_pipeline")
            if [[ -n "$has_error" ]] || [[ "$decision" == "approve" && "$reason" == *"no matching"* ]]; then
                log_success "$test_name: PASSED (no pipeline matched)"
                PASSED_TESTS=$((PASSED_TESTS + 1))
                PASSED_TEST_NAMES+=("$test_name")
                return 0
            fi
            ;;
        "error")
            if [ -n "$has_error" ]; then
                log_success "$test_name: PASSED (error returned: $has_error)"
                PASSED_TESTS=$((PASSED_TESTS + 1))
                PASSED_TEST_NAMES+=("$test_name")
                return 0
            fi
            ;;
        "default_fallback")
            local pipeline_id=$(echo "$response" | jq -r '.pipeline_id // empty')
            if [[ "$pipeline_id" == "default" ]] && [[ "$decision" == "pass" ]]; then
                log_success "$test_name: PASSED (default pipeline fallback)"
                PASSED_TESTS=$((PASSED_TESTS + 1))
                PASSED_TEST_NAMES+=("$test_name")
                return 0
            fi
            ;;
    esac

    log_error "$test_name: FAILED (expected: $expected_pattern, response: $response)"
    FAILED_TESTS=$((FAILED_TESTS + 1))
    FAILED_TEST_NAMES+=("$test_name")
    FAILED_TEST_DETAILS+=("$test_name|UNEXPECTED_RESPONSE|Expected: $expected_pattern")
    return 1
}

run_tests_for_datasource() {
    local datasource=$1

    echo ""
    log_header "============================================================================"
    log_header "Running E2E Tests for: $datasource"
    log_header "============================================================================"
    echo ""

    # Reset counters for this datasource
    TOTAL_TESTS=0
    PASSED_TESTS=0
    FAILED_TESTS=0
    PASSED_TEST_NAMES=()
    FAILED_TEST_NAMES=()
    FAILED_TEST_DETAILS=()

    # Step 1: Backup config
    log_info "Step 1: Setting up configuration..."
    backup_config
    setup_test_config "$datasource"
    setup_features "$datasource"
    log_success "Configuration ready"
    echo ""

    # Step 2: Setup database
    log_info "Step 2: Setting up database..."
    setup_database "$datasource"
    echo ""

    # Step 3: Build server
    log_info "Step 3: Building server..."
    cargo build --bin corint-server --quiet
    if [ $? -ne 0 ]; then
        log_error "Failed to build server"
        return 1
    fi
    log_success "Server built successfully"
    echo ""

    # Step 4: Start server
    log_info "Step 4: Starting test server..."
    # Enable detailed performance logging for feature execution and datasource queries
    RUST_LOG=error,corint_runtime::feature::executor=debug,corint_runtime::datasource=debug target/debug/corint-server > "$RESULTS_DIR/server_${datasource}.log" 2>&1 &
    SERVER_PID=$!
    echo $SERVER_PID > "$SERVER_PID_FILE"
    log_info "Server started (PID: $SERVER_PID)"

    if ! wait_for_server; then
        log_error "Server startup failed. Check logs at $RESULTS_DIR/server_${datasource}.log"
        return 1
    fi
    echo ""

    # Step 5: Run test cases
    log_info "Step 5: Running test cases..."
    echo "============================================================================"
    echo ""

    set +e

    CURRENT_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    # Include the test cases (same as before)
    source tests/scripts/e2e_test_cases.sh

    set -e

    # Step 6: Print report
    print_test_report "$datasource"

    # Stop server
    if [ -f "$SERVER_PID_FILE" ]; then
        PID=$(cat "$SERVER_PID_FILE")
        kill $PID 2>/dev/null || true
        rm -f "$SERVER_PID_FILE"
    fi

    return $FAILED_TESTS
}

print_test_report() {
    local datasource=$1

    echo ""
    log_header "============================================================================"
    log_header "Test Report for: $datasource"
    log_header "============================================================================"
    echo ""

    echo -e "${BLUE}Test Summary:${NC}"
    echo "  Total Tests:  $TOTAL_TESTS"
    echo -e "  ${GREEN}Passed:       $PASSED_TESTS${NC}"
    echo -e "  ${RED}Failed:       $FAILED_TESTS${NC}"
    echo ""

    if [ $PASSED_TESTS -gt 0 ]; then
        echo -e "${GREEN}✓ Passed Tests ($PASSED_TESTS):${NC}"
        for test_name in "${PASSED_TEST_NAMES[@]}"; do
            echo -e "  ${GREEN}✓${NC} $test_name"
        done
        echo ""
    fi

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

    if [ $FAILED_TESTS -eq 0 ]; then
        log_success "All tests passed for $datasource! 🎉"
    else
        log_error "$FAILED_TESTS test(s) failed for $datasource"
    fi
    echo ""
}

# ============================================================================
# Main
# ============================================================================

main() {
    # Select datasource
    select_datasource "$1"

    # Create results directory
    mkdir -p "$RESULTS_DIR"

    # Track overall results
    declare -a DATASOURCE_RESULTS=()
    TOTAL_FAILED=0

    case $DATASOURCE in
        sqlite)
            run_tests_for_datasource "sqlite"
            TOTAL_FAILED=$?
            ;;
        postgres)
            run_tests_for_datasource "postgres"
            TOTAL_FAILED=$?
            ;;
        clickhouse)
            run_tests_for_datasource "clickhouse"
            TOTAL_FAILED=$?
            ;;
        redis)
            log_error "Redis datasource not yet implemented"
            exit 1
            ;;
        all)
            echo ""
            log_header "Running tests on all available datasources..."
            echo ""

            # SQLite
            run_tests_for_datasource "sqlite"
            DATASOURCE_RESULTS+=("sqlite:$FAILED_TESTS")

            # PostgreSQL (if available)
            if psql "${POSTGRES_URL:-postgresql://postgres:postgres@localhost:5432/corint_test}" -c "SELECT 1;" > /dev/null 2>&1; then
                run_tests_for_datasource "postgres"
                DATASOURCE_RESULTS+=("postgres:$FAILED_TESTS")
            else
                log_warning "PostgreSQL not available, skipping"
            fi

            # ClickHouse (auto-install and start if needed)
            run_tests_for_datasource "clickhouse"
            DATASOURCE_RESULTS+=("clickhouse:$FAILED_TESTS")

            # Print overall summary
            echo ""
            log_header "============================================================================"
            log_header "Overall Summary"
            log_header "============================================================================"
            for result in "${DATASOURCE_RESULTS[@]}"; do
                ds=$(echo "$result" | cut -d: -f1)
                failures=$(echo "$result" | cut -d: -f2)
                if [ "$failures" -eq 0 ]; then
                    echo -e "  ${GREEN}✓${NC} $ds: All tests passed"
                else
                    echo -e "  ${RED}✗${NC} $ds: $failures test(s) failed"
                    TOTAL_FAILED=$((TOTAL_FAILED + failures))
                fi
            done
            echo ""
            ;;
    esac

    exit $TOTAL_FAILED
}

# Run main function
main "$@"
