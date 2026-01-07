#!/bin/bash
# CORINT PostgreSQL Data Initialization Script
# ============================================================================
# This script loads E2E test data from events.json into PostgreSQL
# Database configuration is read from config/server.yaml (datasources.postgres_events)
# For backward compatibility, also supports reading from repository/configs/datasources/postgres_events.yaml
#
# Usage:
#   ./init_postgresql.sh                    # Use config from server.yaml or datasources yaml
#   ./init_postgresql.sh -c /path/to/config.yaml  # Use custom config file
# ============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DATA_FILE="${SCRIPT_DIR}/data/events.json"
CONFIG_FILE="${PROJECT_ROOT}/repository/configs/datasources/postgres_events.yaml"

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

if ! command -v psql &> /dev/null; then
    echo "Error: psql is required but not installed"
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
echo "CORINT PostgreSQL Data Initialization"
echo "=============================================="
echo "Config file: ${CONFIG_FILE}"

# Parse configuration from YAML
# Try yq first, fall back to grep/sed if not available
if command -v yq &> /dev/null; then
    CONNECTION_STRING=$(yq -r '.connection_string // empty' "${CONFIG_FILE}")
    DATABASE=$(yq -r '.database // empty' "${CONFIG_FILE}")
    EVENTS_TABLE=$(yq -r '.events_table // "events"' "${CONFIG_FILE}")
else
    # Fallback to grep/sed for simple YAML parsing
    CONNECTION_STRING=$(grep '^connection_string:' "${CONFIG_FILE}" | sed 's/^connection_string:[[:space:]]*"\?\([^"]*\)"\?/\1/')
    DATABASE=$(grep '^database:' "${CONFIG_FILE}" | sed 's/^database:[[:space:]]*"\?\([^"]*\)"\?/\1/')
    EVENTS_TABLE=$(grep '^events_table:' "${CONFIG_FILE}" | sed 's/^events_table:[[:space:]]*"\?\([^"]*\)"\?/\1/')
    EVENTS_TABLE="${EVENTS_TABLE:-events}"
fi

if [ -z "${CONNECTION_STRING}" ]; then
    echo "Error: connection_string not found in config file"
    exit 1
fi

echo "Connection: ${CONNECTION_STRING}"
echo "Database: ${DATABASE}"
echo "Events table: ${EVENTS_TABLE}"
echo ""

# Build psql command
PSQL_CMD="psql ${CONNECTION_STRING}"

# Test connection
echo "Testing database connection..."
if ! ${PSQL_CMD} -c "SELECT 1" &> /dev/null; then
    echo "Error: Cannot connect to PostgreSQL"
    exit 1
fi
echo "Connection successful!"
echo ""

# Create events table if not exists
echo "Creating events table..."
${PSQL_CMD} <<EOF
CREATE TABLE IF NOT EXISTS ${EVENTS_TABLE} (
    id SERIAL PRIMARY KEY,
    event_type VARCHAR(50) NOT NULL,
    user_id VARCHAR(100) NOT NULL,
    event_timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status VARCHAR(50),
    amount DECIMAL(15, 2),
    currency VARCHAR(10),
    merchant_id VARCHAR(100),
    device_id VARCHAR(100),
    ip_address VARCHAR(50),
    country VARCHAR(10),
    city VARCHAR(100),
    email VARCHAR(255),
    phone VARCHAR(50),
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_${EVENTS_TABLE}_user_id ON ${EVENTS_TABLE}(user_id);
CREATE INDEX IF NOT EXISTS idx_${EVENTS_TABLE}_event_timestamp ON ${EVENTS_TABLE}(event_timestamp);
CREATE INDEX IF NOT EXISTS idx_${EVENTS_TABLE}_event_type ON ${EVENTS_TABLE}(event_type);
CREATE INDEX IF NOT EXISTS idx_${EVENTS_TABLE}_user_timestamp ON ${EVENTS_TABLE}(user_id, event_timestamp);
EOF
echo "Table created successfully!"
echo ""

# Clean up existing test data
echo "Cleaning up existing test data..."
${PSQL_CMD} -c "DELETE FROM ${EVENTS_TABLE} WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%';"
echo "Cleanup complete!"
echo ""

# Function to convert timestamp offset to PostgreSQL interval
convert_offset() {
    local offset="$1"
    local value="${offset#-}"
    local unit="${value: -1}"
    local num="${value%?}"

    case "$unit" in
        m) echo "${num} minutes" ;;
        h) echo "${num} hours" ;;
        d) echo "${num} days" ;;
        *) echo "0 minutes" ;;
    esac
}

# Load events from JSON
echo "Loading events from ${DATA_FILE}..."

# Generate SQL for all events
SQL_FILE=$(mktemp)

jq -c '.events[]' "${DATA_FILE}" | while read -r event; do
    event_type=$(echo "$event" | jq -r '.event_type')
    user_id=$(echo "$event" | jq -r '.user_id')
    timestamp_offset=$(echo "$event" | jq -r '.timestamp_offset')
    status=$(echo "$event" | jq -r '.status // "null"')
    amount=$(echo "$event" | jq -r '.amount // "null"')
    currency=$(echo "$event" | jq -r '.currency // "null"')
    merchant_id=$(echo "$event" | jq -r '.merchant_id // "null"')
    device_id=$(echo "$event" | jq -r '.device_id // "null"')
    ip_address=$(echo "$event" | jq -r '.ip_address // "null"')
    country=$(echo "$event" | jq -r '.country // "null"')
    city=$(echo "$event" | jq -r '.city // "null"')
    email=$(echo "$event" | jq -r '.email // "null"')
    phone=$(echo "$event" | jq -r '.phone // "null"')
    metadata=$(echo "$event" | jq -c '.metadata // null')

    # Convert offset to interval
    interval=$(convert_offset "$timestamp_offset")

    # Build INSERT statement
    cat >> "${SQL_FILE}" <<EOSQL
INSERT INTO ${EVENTS_TABLE} (event_type, user_id, event_timestamp, status, amount, currency, merchant_id, device_id, ip_address, country, city, email, phone, metadata)
VALUES (
    '${event_type}',
    '${user_id}',
    NOW() - INTERVAL '${interval}',
    $([ "$status" = "null" ] && echo "NULL" || echo "'${status}'"),
    $([ "$amount" = "null" ] && echo "NULL" || echo "${amount}"),
    $([ "$currency" = "null" ] && echo "NULL" || echo "'${currency}'"),
    $([ "$merchant_id" = "null" ] && echo "NULL" || echo "'${merchant_id}'"),
    $([ "$device_id" = "null" ] && echo "NULL" || echo "'${device_id}'"),
    $([ "$ip_address" = "null" ] && echo "NULL" || echo "'${ip_address}'"),
    $([ "$country" = "null" ] && echo "NULL" || echo "'${country}'"),
    $([ "$city" = "null" ] && echo "NULL" || echo "'${city}'"),
    $([ "$email" = "null" ] && echo "NULL" || echo "'${email}'"),
    $([ "$phone" = "null" ] && echo "NULL" || echo "'${phone}'"),
    $([ "$metadata" = "null" ] && echo "NULL" || echo "'${metadata}'")
);
EOSQL
done

# Execute the SQL file
${PSQL_CMD} -f "${SQL_FILE}"
rm -f "${SQL_FILE}"

# Count inserted rows
INSERTED=$(${PSQL_CMD} -t -c "SELECT COUNT(*) FROM ${EVENTS_TABLE} WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%';" | tr -d ' ')

echo ""
echo "=============================================="
echo "Data loaded successfully!"
echo "Total events inserted: ${INSERTED}"
echo ""
echo "Sample verification:"
${PSQL_CMD} -c "SELECT user_id, event_type, COUNT(*) as count FROM ${EVENTS_TABLE} WHERE user_id LIKE '%_001' OR user_id LIKE 'suspicious_001' GROUP BY user_id, event_type ORDER BY user_id, event_type LIMIT 20;"
echo "=============================================="
