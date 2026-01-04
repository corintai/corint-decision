#!/bin/bash

# Diagnose ClickHouse data issues

# Start ClickHouse if not running
docker ps | grep clickhouse/clickhouse-server || docker start clickhouse-test 2>/dev/null || {
    echo "Starting ClickHouse..."
    docker run -d --name clickhouse-test -p 8123:8123 clickhouse/clickhouse-server:latest
    sleep 5
}

CLICKHOUSE_URL="http://localhost:8123"

# Load test data
echo "Loading test data..."
python3 tests/scripts/generate_clickhouse_data.py

# Execute SQL file
echo "Executing SQL..."
sql_file="tests/data/clickhouse_test_data.sql"

statement=""
while IFS= read -r line || [ -n "$line" ]; do
    [[ -z "$line" || "$line" =~ ^[[:space:]]*--[[:space:]] ]] && continue
    statement+=" $line"

    if [[ "$line" =~ \;[[:space:]]*$ ]]; then
        statement=$(echo "$statement" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        if [ -n "$statement" ]; then
            response=$(curl -s -X POST "$CLICKHOUSE_URL/" --data-binary "$statement" 2>&1)
            if [[ "$response" =~ "Exception" ]]; then
                echo "Error: $response"
            fi
        fi
        statement=""
    fi
done < "$sql_file"

# Check data counts
echo ""
echo "=== Data Verification ==="
echo -n "Events count: "
curl -s "$CLICKHOUSE_URL/?query=SELECT%20count()%20FROM%20events"
echo ""

echo -n "List entries count: "
curl -s "$CLICKHOUSE_URL/?query=SELECT%20count()%20FROM%20list_entries"
echo ""

# Check for user_0001
echo ""
echo -n "user_0001 transactions: "
curl -s "$CLICKHOUSE_URL/?query=SELECT%20count()%20FROM%20events%20WHERE%20user_id%3D'user_0001'"
echo ""

# Check user distribution
echo ""
echo "=== Top 10 users by transaction count ==="
curl -s "$CLICKHOUSE_URL/?query=SELECT%20user_id%2C%20count()%20as%20cnt%20FROM%20events%20GROUP%20BY%20user_id%20ORDER%20BY%20cnt%20DESC%20LIMIT%2010&default_format=Pretty"

# Check timestamp range
echo ""
echo "=== Timestamp range ==="
curl -s "$CLICKHOUSE_URL/?query=SELECT%20min(timestamp)%20as%20min_ts%2C%20max(timestamp)%20as%20max_ts%2C%20now()%20as%20current_time%20FROM%20events&default_format=Pretty"

# Check recent events (within 24 hours)
echo ""
echo "=== Events in last 24 hours ==="
curl -s "$CLICKHOUSE_URL/?query=SELECT%20count()%20FROM%20events%20WHERE%20timestamp%20%3E%3D%20now()%20-%20INTERVAL%2024%20HOUR"
echo ""

# Check blocked users list
echo ""
echo "=== Blocked users list ==="
curl -s "$CLICKHOUSE_URL/?query=SELECT%20value%20FROM%20list_entries%20WHERE%20list_id%3D'blocked_users'&default_format=Pretty"

echo ""
echo "=== High risk countries list ==="
curl -s "$CLICKHOUSE_URL/?query=SELECT%20value%20FROM%20list_entries%20WHERE%20list_id%3D'high_risk_countries'&default_format=Pretty"
