#!/bin/bash
# CORINT SQLite Data Initialization Script
# ============================================================================
# This script loads E2E test data from events.json into SQLite
# Database file is created in project root directory: corint.db
#
# Usage:
#   ./init_sqlite.sh
# ============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DATA_FILE="${SCRIPT_DIR}/data/events.json"
SQLITE_DB="${PROJECT_ROOT}/corint.db"
TEMP_DIR="${PROJECT_ROOT}/temp"

# Ensure temp directory exists
mkdir -p "${TEMP_DIR}"
EVENTS_TABLE="events"

# Check for required tools
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed"
    exit 1
fi

if ! command -v sqlite3 &> /dev/null; then
    echo "Error: sqlite3 is required but not installed"
    exit 1
fi

# Check if data file exists
if [ ! -f "${DATA_FILE}" ]; then
    echo "Error: Data file not found: ${DATA_FILE}"
    exit 1
fi

echo "=============================================="
echo "CORINT SQLite Data Initialization"
echo "=============================================="

echo "Database: ${SQLITE_DB}"
echo "Events table: ${EVENTS_TABLE}"
echo ""

# Create events table if not exists
echo "Creating events table..."
sqlite3 "${SQLITE_DB}" <<EOF
CREATE TABLE IF NOT EXISTS ${EVENTS_TABLE} (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    user_id TEXT NOT NULL,
    event_timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    status TEXT,
    amount REAL,
    currency TEXT,
    merchant_id TEXT,
    device_id TEXT,
    ip_address TEXT,
    country TEXT,
    city TEXT,
    email TEXT,
    phone TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
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
sqlite3 "${SQLITE_DB}" "DELETE FROM ${EVENTS_TABLE} WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%';"
echo "Cleanup complete!"
echo ""

# Function to convert timestamp offset to SQLite modifier
convert_offset_to_modifier() {
    local offset="$1"
    local value="${offset#-}"
    local unit="${value: -1}"
    local num="${value%?}"

    case "$unit" in
        m) echo "-${num} minutes" ;;
        h) echo "-${num} hours" ;;
        d) echo "-${num} days" ;;
        *) echo "-0 minutes" ;;
    esac
}

# Load events from JSON
echo "Loading events from ${DATA_FILE}..."

# Generate SQL for all events
SQL_FILE=$(mktemp "${TEMP_DIR}/sqlite_init_XXXXXX.sql")
echo "BEGIN TRANSACTION;" > "${SQL_FILE}"

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

    # Convert offset to SQLite modifier
    modifier=$(convert_offset_to_modifier "$timestamp_offset")

    # Build INSERT statement
    cat >> "${SQL_FILE}" <<EOSQL
INSERT INTO ${EVENTS_TABLE} (event_type, user_id, event_timestamp, status, amount, currency, merchant_id, device_id, ip_address, country, city, email, phone, metadata)
VALUES (
    '${event_type}',
    '${user_id}',
    datetime('now', '${modifier}'),
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
done

echo "COMMIT;" >> "${SQL_FILE}"

# Execute the SQL file
sqlite3 "${SQLITE_DB}" < "${SQL_FILE}"
rm -f "${SQL_FILE}"

# Count inserted rows
INSERTED=$(sqlite3 "${SQLITE_DB}" "SELECT COUNT(*) FROM ${EVENTS_TABLE} WHERE user_id LIKE 'normal_user_%' OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%' OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%' OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%' OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%' OR user_id LIKE 'stats_user_%';")

echo ""
echo "=============================================="
echo "Data loaded successfully!"
echo "Total events inserted: ${INSERTED}"
echo ""
echo "Sample verification:"
sqlite3 -header -column "${SQLITE_DB}" "SELECT user_id, event_type, COUNT(*) as count FROM ${EVENTS_TABLE} WHERE user_id LIKE '%_001' OR user_id LIKE 'suspicious_001' GROUP BY user_id, event_type ORDER BY user_id, event_type LIMIT 20;"
echo "=============================================="
