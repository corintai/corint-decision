#!/bin/bash
# Verify Database List Data
# This script queries the SQLite database to verify list entries

set -e

DB_PATH="${1:-tests/data/e2e_test.db}"

echo "============================================================"
echo "Database List Verification"
echo "============================================================"
echo ""
echo "Database: $DB_PATH"
echo ""

if [ ! -f "$DB_PATH" ]; then
    echo "Error: Database not found at $DB_PATH"
    exit 1
fi

# Function to check if value is in list
check_list_membership() {
    local list_id=$1
    local value=$2
    local count=$(sqlite3 "$DB_PATH" \
        "SELECT COUNT(*) FROM list_entries WHERE list_id = '$list_id' AND value = '$value'")

    if [ "$count" -gt 0 ]; then
        echo "  ✓ '$value' IS in list '$list_id'"
        return 0
    else
        echo "  ✗ '$value' is NOT in list '$list_id'"
        return 1
    fi
}

# Show all lists
echo "--- Available Lists ---"
echo ""
sqlite3 "$DB_PATH" "SELECT list_id, COUNT(*) as count FROM list_entries GROUP BY list_id" | \
while IFS='|' read -r list_id count; do
    echo "  $list_id: $count entries"
done

echo ""
echo "--- List Contents ---"
echo ""

# Show blocked users
echo "Blocked Users:"
sqlite3 "$DB_PATH" "SELECT value FROM list_entries WHERE list_id = 'blocked_users'" | \
while read -r value; do
    echo "  - $value"
done

echo ""
echo "Blocked IPs:"
sqlite3 "$DB_PATH" "SELECT value FROM list_entries WHERE list_id = 'blocked_ips'" | \
while read -r value; do
    echo "  - $value"
done

echo ""
echo "High Risk Countries:"
sqlite3 "$DB_PATH" "SELECT value FROM list_entries WHERE list_id = 'high_risk_countries'" | \
while read -r value; do
    echo "  - $value"
done

echo ""
echo "--- Membership Tests ---"
echo ""

# Test specific memberships
check_list_membership "blocked_users" "sus_0001" || true
check_list_membership "blocked_users" "user_0001" || true
check_list_membership "blocked_ips" "45.142.212.61" || true
check_list_membership "blocked_ips" "192.168.1.100" || true
check_list_membership "high_risk_countries" "RU" || true
check_list_membership "high_risk_countries" "US" || true

echo ""
echo "============================================================"
echo "✓ Database list verification completed"
echo "============================================================"
