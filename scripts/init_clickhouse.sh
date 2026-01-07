#!/bin/bash
# CORINT ClickHouse Data Initialization Script
# ============================================================================
# This script loads E2E test data from events.json into ClickHouse
# Database configuration is read from repository/configs/datasources/clickhouse_events.yaml
#
# Usage:
#   ./init_clickhouse.sh                    # Use config from datasources yaml
#   ./init_clickhouse.sh -c /path/to/config.yaml  # Use custom config file
# ============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DATA_FILE="${SCRIPT_DIR}/data/events.json"
CONFIG_FILE="${PROJECT_ROOT}/repository/configs/datasources/clickhouse_events.yaml"

# Parse command line arguments
while getopts "c:" opt; do
    case $opt in
        c) CONFIG_FILE="$OPTARG" ;;
        *) echo "Usage: $0 [-c config_file]" && exit 1 ;;
    esac
done

# Check for required tools
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed"
    exit 1
fi

if ! command -v clickhouse-client &> /dev/null; then
    echo "Error: clickhouse-client is required but not installed"
    exit 1
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
    CONNECTION_STRING=$(yq -r '.connection_string // empty' "${CONFIG_FILE}")
    DATABASE=$(yq -r '.database // "default"' "${CONFIG_FILE}")
    EVENTS_TABLE=$(yq -r '.events_table // "events"' "${CONFIG_FILE}")
    CH_USER=$(yq -r '.options.user // "default"' "${CONFIG_FILE}")
else
    # Fallback to grep/sed for simple YAML parsing
    CONNECTION_STRING=$(grep '^connection_string:' "${CONFIG_FILE}" | sed 's/^connection_string:[[:space:]]*"\?\([^"]*\)"\?/\1/')
    DATABASE=$(grep '^database:' "${CONFIG_FILE}" | sed 's/^database:[[:space:]]*"\?\([^"]*\)"\?/\1/')
    EVENTS_TABLE=$(grep '^events_table:' "${CONFIG_FILE}" | sed 's/^events_table:[[:space:]]*"\?\([^"]*\)"\?/\1/')
    CH_USER=$(grep -A10 '^options:' "${CONFIG_FILE}" | grep 'user:' | head -1 | sed 's/.*user:[[:space:]]*"\?\([^"]*\)"\?/\1/')
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

# Build clickhouse-client command
CH_CMD="clickhouse-client --host ${CH_HOST} --port ${CH_TCP_PORT} --user ${CH_USER} --database ${DATABASE}"

# Test connection
echo "Testing database connection..."
if ! ${CH_CMD} -q "SELECT 1" &> /dev/null; then
    echo "Error: Cannot connect to ClickHouse at ${CH_HOST}"
    exit 1
fi
echo "Connection successful!"
echo ""

# Create database if not exists
echo "Creating database if not exists..."
clickhouse-client --host ${CH_HOST} --port ${CH_TCP_PORT} --user ${CH_USER} -q "CREATE DATABASE IF NOT EXISTS ${DATABASE}"

# Create events table if not exists
echo "Creating events table..."
${CH_CMD} <<EOF
CREATE TABLE IF NOT EXISTS ${EVENTS_TABLE} (
    id UInt64,
    event_type String,
    user_id String,
    timestamp DateTime64(3),
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
ORDER BY (user_id, timestamp)
PARTITION BY toYYYYMM(timestamp);
EOF
echo "Table created successfully!"
echo ""

# Clean up existing test data
echo "Cleaning up existing test data..."
${CH_CMD} -q "ALTER TABLE ${EVENTS_TABLE} DELETE WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%'"
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

# Generate SQL for all events
SQL_FILE=$(mktemp)
ID=1

jq -c '.events[]' "${DATA_FILE}" | while read -r event; do
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
    cat >> "${SQL_FILE}" <<EOSQL
INSERT INTO ${EVENTS_TABLE} (id, event_type, user_id, timestamp, status, amount, currency, merchant_id, device_id, ip_address, country, city, email, phone, metadata)
VALUES (
    ${ID},
    '${event_type}',
    '${user_id}',
    now64() - ${seconds},
    $([ -z "$status" ] && echo "NULL" || echo "'${status}'"),
    $([ -z "$amount" ] && echo "NULL" || echo "${amount}"),
    $([ -z "$currency" ] && echo "NULL" || echo "'${currency}'"),
    $([ -z "$merchant_id" ] && echo "NULL" || echo "'${merchant_id}'"),
    $([ -z "$device_id" ] && echo "NULL" || echo "'${device_id}'"),
    $([ -z "$ip_address" ] && echo "NULL" || echo "'${ip_address}'"),
    $([ -z "$country" ] && echo "NULL" || echo "'${country}'"),
    $([ -z "$city" ] && echo "NULL" || echo "'${city}'"),
    $([ -z "$email" ] && echo "NULL" || echo "'${email}'"),
    $([ -z "$phone" ] && echo "NULL" || echo "'${phone}'"),
    $([ -z "$metadata" ] && echo "NULL" || echo "'${metadata}'")
);
EOSQL
    ((ID++)) || true
done

# Execute the SQL file
${CH_CMD} --multiquery < "${SQL_FILE}"
rm -f "${SQL_FILE}"

# Count inserted rows
INSERTED=$(${CH_CMD} -q "SELECT COUNT(*) FROM ${EVENTS_TABLE} WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%'" | tr -d ' ')

echo ""
echo "=============================================="
echo "Data loaded successfully!"
echo "Total events inserted: ${INSERTED}"
echo ""
echo "Sample verification:"
${CH_CMD} -q "SELECT user_id, event_type, COUNT(*) as count FROM ${EVENTS_TABLE} WHERE user_id LIKE '%_001' OR user_id LIKE 'suspicious_001' GROUP BY user_id, event_type ORDER BY user_id, event_type LIMIT 20"
echo "=============================================="
