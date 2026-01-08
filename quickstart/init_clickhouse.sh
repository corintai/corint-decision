#!/bin/bash
# CORINT ClickHouse Data Initialization Script
# ============================================================================
# This script loads E2E test data from events.json into ClickHouse
# Database configuration is read from config/server.yaml (datasources.clickhouse_events)
# For backward compatibility, also supports reading from repository/configs/datasources/clickhouse_events.yaml
#
# Usage:
#   ./init_clickhouse.sh                    # Use config from server.yaml or datasources yaml
#   ./init_clickhouse.sh -c /path/to/config.yaml  # Use custom config file
# ============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DATA_FILE="${SCRIPT_DIR}/data/events.json"
TEMP_DIR="${PROJECT_ROOT}/temp"

# Ensure temp directory exists
mkdir -p "${TEMP_DIR}"
SERVER_CONFIG="${PROJECT_ROOT}/config/server.yaml"
LEGACY_CONFIG="${PROJECT_ROOT}/repository/configs/datasources/clickhouse_events.yaml"

# Parse command line arguments
while getopts "c:" opt; do
    case $opt in
        c) CONFIG_FILE="$OPTARG" ;;
        *) echo "Usage: $0 [-c config_file]" && exit 1 ;;
    esac
done

# Determine config file to use
if [ -n "${CONFIG_FILE:-}" ]; then
    # Use explicitly provided config file (assume legacy format)
    CONFIG_PATH=""
elif [ -f "${SERVER_CONFIG}" ]; then
    # Use server.yaml (preferred)
    CONFIG_FILE="${SERVER_CONFIG}"
    CONFIG_PATH=".datasource.events_datasource"
elif [ -f "${LEGACY_CONFIG}" ]; then
    # Fallback to legacy config file
    CONFIG_FILE="${LEGACY_CONFIG}"
    CONFIG_PATH=""
else
    echo "Error: No config file found. Tried:"
    echo "  - ${SERVER_CONFIG}"
    echo "  - ${LEGACY_CONFIG}"
    exit 1
fi

# Check for required tools
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed"
    exit 1
fi

# Check for clickhouse-client or curl (for HTTP interface)
USE_HTTP=false
if ! command -v clickhouse-client &> /dev/null; then
    if command -v curl &> /dev/null; then
        USE_HTTP=true
        echo "Warning: clickhouse-client not found, will use HTTP interface instead"
    else
        echo "Error: Neither clickhouse-client nor curl is available"
        echo "Please install one of:"
        echo "  - clickhouse-client (recommended)"
        echo "  - curl (for HTTP interface)"
        exit 1
    fi
fi

# Check if data file exists
if [ ! -f "${DATA_FILE}" ]; then
    echo "Error: Data file not found: ${DATA_FILE}"
    exit 1
fi

# Check if config file exists
if [ ! -f "${CONFIG_FILE}" ]; then
    echo "Error: Config file not found: ${CONFIG_FILE}"
    exit 1
fi

echo "=============================================="
echo "CORINT ClickHouse Data Initialization"
echo "=============================================="
echo "Config file: ${CONFIG_FILE}"

# Parse configuration from YAML
if command -v yq &> /dev/null; then
    if [ -n "${CONFIG_PATH:-}" ]; then
        # Read from nested path in server.yaml (e.g., .datasource.events_datasource)
        CONNECTION_STRING=$(yq -r "${CONFIG_PATH}.connection_string // empty" "${CONFIG_FILE}")
        DATABASE=$(yq -r "${CONFIG_PATH}.database // \"default\"" "${CONFIG_FILE}")
        EVENTS_TABLE=$(yq -r "${CONFIG_PATH}.events_table // \"events\"" "${CONFIG_FILE}")
        CH_USER=$(yq -r "${CONFIG_PATH}.options.user // \"default\"" "${CONFIG_FILE}")
    else
        # Read from root level (legacy config file or explicit config)
        CONNECTION_STRING=$(yq -r '.connection_string // empty' "${CONFIG_FILE}")
        DATABASE=$(yq -r '.database // "default"' "${CONFIG_FILE}")
        EVENTS_TABLE=$(yq -r '.events_table // "events"' "${CONFIG_FILE}")
        CH_USER=$(yq -r '.options.user // "default"' "${CONFIG_FILE}")
    fi
else
    # Fallback to grep/sed for simple YAML parsing
    if [ -n "${CONFIG_PATH:-}" ]; then
        # For server.yaml, we need to find the events_datasource section
        # Extract the section between events_datasource: and the next top-level key
        CONNECTION_STRING=$(awk '/events_datasource:/{flag=1; next} /^[a-zA-Z_][a-zA-Z0-9_]*:/ && flag {exit} flag && /connection_string:/ {gsub(/^[[:space:]]*connection_string:[[:space:]]*"?|"$/, "", $0); gsub(/^"|"$/, "", $0); print; exit}' "${CONFIG_FILE}")
        DATABASE=$(awk '/events_datasource:/{flag=1; next} /^[a-zA-Z_][a-zA-Z0-9_]*:/ && flag {exit} flag && /database:/ {gsub(/^[[:space:]]*database:[[:space:]]*"?|"$/, "", $0); gsub(/^"|"$/, "", $0); print; exit}' "${CONFIG_FILE}")
        EVENTS_TABLE=$(awk '/events_datasource:/{flag=1; next} /^[a-zA-Z_][a-zA-Z0-9_]*:/ && flag {exit} flag && /events_table:/ {gsub(/^[[:space:]]*events_table:[[:space:]]*"?|"$/, "", $0); gsub(/^"|"$/, "", $0); print; exit}' "${CONFIG_FILE}")
        CH_USER=$(awk '/events_datasource:/{flag=1; next} /^[a-zA-Z_][a-zA-Z0-9_]*:/ && flag {exit} flag && /options:/ {opt_flag=1; next} opt_flag && /user:/ {gsub(/^[[:space:]]*user:[[:space:]]*"?|"$/, "", $0); gsub(/^"|"$/, "", $0); print; exit}' "${CONFIG_FILE}")
    else
        # Legacy config file format
        CONNECTION_STRING=$(grep '^connection_string:' "${CONFIG_FILE}" | sed 's/^connection_string:[[:space:]]*"\?\([^"]*\)"\?/\1/')
        DATABASE=$(grep '^database:' "${CONFIG_FILE}" | sed 's/^database:[[:space:]]*"\?\([^"]*\)"\?/\1/')
        EVENTS_TABLE=$(grep '^events_table:' "${CONFIG_FILE}" | sed 's/^events_table:[[:space:]]*"\?\([^"]*\)"\?/\1/')
        CH_USER=$(grep -A10 '^options:' "${CONFIG_FILE}" | grep 'user:' | head -1 | sed 's/.*user:[[:space:]]*"\?\([^"]*\)"\?/\1/')
    fi
    # Clean up extracted values (remove quotes and whitespace)
    CONNECTION_STRING=$(echo "${CONNECTION_STRING}" | sed 's/^[[:space:]]*"\(.*\)"[[:space:]]*$/\1/' | sed 's/^[[:space:]]*//' | sed 's/[[:space:]]*$//')
    DATABASE=$(echo "${DATABASE}" | sed 's/^[[:space:]]*"\(.*\)"[[:space:]]*$/\1/' | sed 's/^[[:space:]]*//' | sed 's/[[:space:]]*$//')
    EVENTS_TABLE=$(echo "${EVENTS_TABLE}" | sed 's/^[[:space:]]*"\(.*\)"[[:space:]]*$/\1/' | sed 's/^[[:space:]]*//' | sed 's/[[:space:]]*$//')
    CH_USER=$(echo "${CH_USER}" | sed 's/^[[:space:]]*"\(.*\)"[[:space:]]*$/\1/' | sed 's/^[[:space:]]*//' | sed 's/[[:space:]]*$//')
    DATABASE="${DATABASE:-default}"
    EVENTS_TABLE="${EVENTS_TABLE:-events}"
    CH_USER="${CH_USER:-default}"
fi

if [ -z "${CONNECTION_STRING}" ]; then
    echo "Error: connection_string not found in config file"
    exit 1
fi

# Parse host and port from connection string (http://host:port format)
CH_HOST=$(echo "${CONNECTION_STRING}" | sed 's|http://||' | cut -d':' -f1)
CH_HTTP_PORT=$(echo "${CONNECTION_STRING}" | sed 's|http://||' | cut -d':' -f2)
CH_TCP_PORT=9000  # Default TCP port for clickhouse-client

echo "Host: ${CH_HOST}"
echo "HTTP Port: ${CH_HTTP_PORT}"
echo "Database: ${DATABASE}"
echo "Events table: ${EVENTS_TABLE}"
echo "User: ${CH_USER}"
echo ""

# Function to execute ClickHouse query via HTTP
execute_ch_query() {
    local query="$1"
    local method="${2:-GET}"  # Default to GET, use POST for modifying queries
    local encoded_query=$(echo -n "$query" | jq -sRr @uri)
    
    if [ "$method" = "POST" ]; then
        # POST method for modifying queries (CREATE, INSERT, ALTER, etc.)
        echo -n "$query" | curl -s -X POST "http://${CH_HOST}:${CH_HTTP_PORT}/" --data-binary @-
    else
        # GET method for read-only queries
        curl -s "http://${CH_HOST}:${CH_HTTP_PORT}/?query=${encoded_query}"
    fi
}

# Function to execute ClickHouse query via client
execute_ch_client() {
    local query="$1"
    clickhouse-client --host "${CH_HOST}" --port "${CH_TCP_PORT}" --user "${CH_USER}" -q "$query"
}

# Test connection
echo "Testing database connection..."
if [ "$USE_HTTP" = true ]; then
    if ! curl -s "http://${CH_HOST}:${CH_HTTP_PORT}/ping" | grep -q "Ok"; then
        echo "Error: Cannot connect to ClickHouse at ${CH_HOST}:${CH_HTTP_PORT}"
        exit 1
    fi
    CH_EXEC="execute_ch_query"
else
    CH_CMD="clickhouse-client --host ${CH_HOST} --port ${CH_TCP_PORT} --user ${CH_USER} --database ${DATABASE}"
    if ! ${CH_CMD} -q "SELECT 1" &> /dev/null; then
        echo "Error: Cannot connect to ClickHouse at ${CH_HOST}"
        exit 1
    fi
    CH_EXEC="execute_ch_client"
fi
echo "Connection successful!"
echo ""

# Create database if not exists
echo "Creating database if not exists..."
if [ "$USE_HTTP" = true ]; then
    execute_ch_query "CREATE DATABASE IF NOT EXISTS ${DATABASE}" "POST"
else
    clickhouse-client --host ${CH_HOST} --port ${CH_TCP_PORT} --user ${CH_USER} -q "CREATE DATABASE IF NOT EXISTS ${DATABASE}"
fi

# Create events table if not exists
echo "Creating events table..."
TABLE_SQL=$(cat <<EOF
CREATE TABLE IF NOT EXISTS ${DATABASE}.${EVENTS_TABLE} (
    id UInt64,
    event_type String,
    user_id String,
    event_timestamp DateTime64(3),
    status Nullable(String),
    amount Nullable(Decimal(15, 2)),
    currency Nullable(String),
    merchant_id Nullable(String),
    device_id Nullable(String),
    ip_address Nullable(String),
    country Nullable(String),
    city Nullable(String),
    email Nullable(String),
    phone Nullable(String),
    metadata Nullable(String),
    created_at DateTime64(3) DEFAULT now64()
) ENGINE = MergeTree()
ORDER BY (user_id, event_timestamp)
PARTITION BY toYYYYMM(event_timestamp);
EOF
)

if [ "$USE_HTTP" = true ]; then
    execute_ch_query "$TABLE_SQL" "POST" > /dev/null
else
    ${CH_CMD} <<EOF
${TABLE_SQL}
EOF
fi
echo "Table created successfully!"
echo ""

# Clean up existing test data
echo "Cleaning up existing test data..."
if [ "$USE_HTTP" = true ]; then
    execute_ch_query "ALTER TABLE ${DATABASE}.${EVENTS_TABLE} DELETE WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%'" "POST" > /dev/null
else
    ${CH_CMD} -q "ALTER TABLE ${EVENTS_TABLE} DELETE WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%'"
fi
# Wait for mutations to complete
sleep 2
echo "Cleanup complete!"
echo ""

# Function to convert timestamp offset to seconds
convert_offset_to_seconds() {
    local offset="$1"
    local value="${offset#-}"
    local unit="${value: -1}"
    local num="${value%?}"

    case "$unit" in
        m) echo $((num * 60)) ;;
        h) echo $((num * 3600)) ;;
        d) echo $((num * 86400)) ;;
        *) echo "0" ;;
    esac
}

# Load events from JSON
echo "Loading events from ${DATA_FILE}..."

# Generate SQL for all events - use a temporary file to track ID
ID_FILE=$(mktemp "${TEMP_DIR}/clickhouse_id_XXXXXX")
echo "1" > "${ID_FILE}"

# Process events and generate INSERT statements
jq -c '.events[]' "${DATA_FILE}" | while read -r event; do
    ID=$(cat "${ID_FILE}")
    event_type=$(echo "$event" | jq -r '.event_type')
    user_id=$(echo "$event" | jq -r '.user_id')
    timestamp_offset=$(echo "$event" | jq -r '.timestamp_offset')
    status=$(echo "$event" | jq -r '.status // empty')
    amount=$(echo "$event" | jq -r '.amount // empty')
    currency=$(echo "$event" | jq -r '.currency // empty')
    merchant_id=$(echo "$event" | jq -r '.merchant_id // empty')
    device_id=$(echo "$event" | jq -r '.device_id // empty')
    ip_address=$(echo "$event" | jq -r '.ip_address // empty')
    country=$(echo "$event" | jq -r '.country // empty')
    city=$(echo "$event" | jq -r '.city // empty')
    email=$(echo "$event" | jq -r '.email // empty')
    phone=$(echo "$event" | jq -r '.phone // empty')
    metadata=$(echo "$event" | jq -c '.metadata // empty')

    # Convert offset to seconds
    seconds=$(convert_offset_to_seconds "$timestamp_offset")

    # Build INSERT statement
    # Escape single quotes in string values
    event_type_escaped=$(echo "$event_type" | sed "s/'/''/g")
    user_id_escaped=$(echo "$user_id" | sed "s/'/''/g")
    status_escaped=$(echo "$status" | sed "s/'/''/g")
    currency_escaped=$(echo "$currency" | sed "s/'/''/g")
    merchant_id_escaped=$(echo "$merchant_id" | sed "s/'/''/g")
    device_id_escaped=$(echo "$device_id" | sed "s/'/''/g")
    ip_address_escaped=$(echo "$ip_address" | sed "s/'/''/g")
    country_escaped=$(echo "$country" | sed "s/'/''/g")
    city_escaped=$(echo "$city" | sed "s/'/''/g")
    email_escaped=$(echo "$email" | sed "s/'/''/g")
    phone_escaped=$(echo "$phone" | sed "s/'/''/g")
    metadata_escaped=$(echo "$metadata" | sed "s/'/''/g")
    
    # Generate INSERT statement and send directly via HTTP or append to file
    INSERT_SQL="INSERT INTO ${DATABASE}.${EVENTS_TABLE} (id, event_type, user_id, event_timestamp, status, amount, currency, merchant_id, device_id, ip_address, country, city, email, phone, metadata) VALUES (${ID}, '${event_type_escaped}', '${user_id_escaped}', now64() - INTERVAL ${seconds} SECOND, $([ -z "$status" ] && echo "NULL" || echo "'${status_escaped}'"), $([ -z "$amount" ] && echo "NULL" || echo "${amount}"), $([ -z "$currency" ] && echo "NULL" || echo "'${currency_escaped}'"), $([ -z "$merchant_id" ] && echo "NULL" || echo "'${merchant_id_escaped}'"), $([ -z "$device_id" ] && echo "NULL" || echo "'${device_id_escaped}'"), $([ -z "$ip_address" ] && echo "NULL" || echo "'${ip_address_escaped}'"), $([ -z "$country" ] && echo "NULL" || echo "'${country_escaped}'"), $([ -z "$city" ] && echo "NULL" || echo "'${city_escaped}'"), $([ -z "$email" ] && echo "NULL" || echo "'${email_escaped}'"), $([ -z "$phone" ] && echo "NULL" || echo "'${phone_escaped}'"), $([ -z "$metadata" ] && echo "NULL" || echo "'${metadata_escaped}'"));"
    
    if [ "$USE_HTTP" = true ]; then
        # Send each INSERT directly via HTTP
        execute_ch_query "$INSERT_SQL" "POST" > /dev/null 2>&1
    else
        # Append to SQL file for batch execution
        echo "$INSERT_SQL" >> "${SQL_FILE}"
    fi
    
    # Increment ID
    echo $((ID + 1)) > "${ID_FILE}"
done

rm -f "${ID_FILE}"

# Execute the SQL file (only if using client)
if [ "$USE_HTTP" != true ]; then
    ${CH_CMD} --multiquery < "${SQL_FILE}"
    rm -f "${SQL_FILE}"
fi

# Count inserted rows
if [ "$USE_HTTP" = true ]; then
    INSERTED=$(execute_ch_query "SELECT COUNT(*) FROM ${DATABASE}.${EVENTS_TABLE} WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%'" | tr -d ' ')
else
    INSERTED=$(${CH_CMD} -q "SELECT COUNT(*) FROM ${EVENTS_TABLE} WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%'" | tr -d ' ')
fi

echo ""
echo "=============================================="
echo "Data loaded successfully!"
echo "Total events inserted: ${INSERTED}"
echo ""
echo "Sample verification:"
if [ "$USE_HTTP" = true ]; then
    execute_ch_query "SELECT user_id, event_type, COUNT(*) as count FROM ${DATABASE}.${EVENTS_TABLE} WHERE user_id LIKE '%_001' OR user_id LIKE 'suspicious_001' GROUP BY user_id, event_type ORDER BY user_id, event_type LIMIT 20"
else
    ${CH_CMD} -q "SELECT user_id, event_type, COUNT(*) as count FROM ${EVENTS_TABLE} WHERE user_id LIKE '%_001' OR user_id LIKE 'suspicious_001' GROUP BY user_id, event_type ORDER BY user_id, event_type LIMIT 20"
fi
echo "=============================================="
