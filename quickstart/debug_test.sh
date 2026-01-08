#!/bin/bash
# Debug script to check why rules are not triggering

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DB_FILE="${PROJECT_ROOT}/corint.db"

echo "=============================================="
echo "Debugging Rule Execution Issues"
echo "=============================================="
echo ""

echo "1. Checking database connection..."
if [ -f "${DB_FILE}" ]; then
    echo "✓ Database file exists: ${DB_FILE}"
    DB_SIZE=$(stat -f%z "${DB_FILE}" 2>/dev/null || stat -c%s "${DB_FILE}" 2>/dev/null)
    echo "  Size: ${DB_SIZE} bytes"
else
    echo "✗ Database file not found: ${DB_FILE}"
    exit 1
fi

echo ""
echo "2. Checking data in database..."
echo "   Total events:"
sqlite3 "${DB_FILE}" "SELECT COUNT(*) FROM events;" 2>/dev/null || echo "  Error querying database"

echo "   Events for test users:"
sqlite3 "${DB_FILE}" "SELECT user_id, COUNT(*) as count FROM events WHERE user_id LIKE '%_001' GROUP BY user_id;" 2>/dev/null || echo "  Error querying database"

echo ""
echo "3. Checking fraud_farm_pattern rule data..."
echo "   Devices per IP (5h window) for 192.168.100.1:"
sqlite3 "${DB_FILE}" "SELECT COUNT(DISTINCT device_id) as devices FROM events WHERE ip_address = '192.168.100.1' AND event_timestamp >= datetime('now', '-5 hours');" 2>/dev/null || echo "  Error querying database"

echo "   Users per IP (24h window) for 192.168.100.1:"
sqlite3 "${DB_FILE}" "SELECT COUNT(DISTINCT user_id) as users FROM events WHERE ip_address = '192.168.100.1' AND event_timestamp >= datetime('now', '-24 hours');" 2>/dev/null || echo "  Error querying database"

echo ""
echo "4. Checking account_takeover_pattern rule data..."
echo "   Failed logins (1h) for failed_login_001:"
sqlite3 "${DB_FILE}" "SELECT COUNT(*) as failed_logins FROM events WHERE user_id = 'failed_login_001' AND event_type = 'login' AND status = 'failed' AND event_timestamp >= datetime('now', '-1 hour');" 2>/dev/null || echo "  Error querying database"

echo "   Unique devices (7d) for multi_device_001:"
sqlite3 "${DB_FILE}" "SELECT COUNT(DISTINCT device_id) as devices FROM events WHERE user_id = 'multi_device_001' AND event_timestamp >= datetime('now', '-7 days');" 2>/dev/null || echo "  Error querying database"

echo ""
echo "5. Checking server configuration..."
if [ -f "${PROJECT_ROOT}/config/server.yaml" ]; then
    echo "✓ Server config exists"
    echo "   Datasource configuration:"
    if command -v yq &> /dev/null; then
        yq '.datasource.events_datasource' "${PROJECT_ROOT}/config/server.yaml" 2>/dev/null || echo "  Error reading datasource config"
    else
        grep -A 5 "events_datasource:" "${PROJECT_ROOT}/config/server.yaml" | head -6
    fi
else
    echo "✗ Server config not found: ${PROJECT_ROOT}/config/server.yaml"
fi

echo ""
echo "6. Testing API call..."
HTTP_HOST="127.0.0.1:8080"
if curl -s "http://${HTTP_HOST}/health" &>/dev/null; then
    echo "✓ Server is running"
    echo ""
    echo "   Testing with multi_device_001 (should trigger fraud_farm_pattern):"
    curl -s -X POST "http://${HTTP_HOST}/v1/decide" \
        -H "Content-Type: application/json" \
        -d '{
            "event": {
                "type": "transaction",
                "user_id": "multi_device_001",
                "amount": 999.99,
                "currency": "USD",
                "device_id": "device_md08",
                "ip_address": "192.168.100.1",
                "country": "CN",
                "city": "Shanghai",
                "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
            },
            "options": {
                "enable_trace": true,
                "return_features": true
            }
        }' | jq '.' 2>/dev/null || echo "  Error calling API or parsing response"
else
    echo "✗ Server is not running at http://${HTTP_HOST}"
    echo "  Please start the server first"
fi

echo ""
echo "=============================================="

