#!/usr/bin/env python3
"""
Generate ClickHouse-compatible test data SQL from SQLite test data.

This script reads the SQLite test_data.sql and converts it to ClickHouse format:
- Converts CREATE TABLE syntax
- Converts INSERT statements
- Handles timestamp format conversion
"""

import re
import os
from datetime import datetime, timedelta

# Paths
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
SQLITE_SQL = os.path.join(PROJECT_ROOT, "tests/data/test_data.sql")
CLICKHOUSE_SQL = os.path.join(PROJECT_ROOT, "tests/data/clickhouse_test_data.sql")


def convert_sqlite_to_clickhouse(sqlite_sql: str) -> str:
    """Convert SQLite SQL to ClickHouse SQL."""

    lines = sqlite_sql.split('\n')
    output_lines = []

    # Add header
    output_lines.append("-- CORINT E2E Test Data for ClickHouse")
    output_lines.append(f"-- Generated at: {datetime.now().isoformat()}")
    output_lines.append("-- Converted from SQLite format")
    output_lines.append("")

    # Create events table for ClickHouse
    output_lines.append("-- Create events table")
    output_lines.append("""CREATE TABLE IF NOT EXISTS events (
    id UInt64,
    event_type String,
    user_id String,
    timestamp DateTime64(3),
    status Nullable(String),
    amount Nullable(Float64),
    currency Nullable(String),
    merchant_id Nullable(String),
    device_id Nullable(String),
    ip_address Nullable(String),
    country Nullable(String),
    email Nullable(String),
    phone Nullable(String),
    metadata Nullable(String),
    attributes Nullable(String)
) ENGINE = MergeTree()
ORDER BY (user_id, timestamp)
PARTITION BY toYYYYMM(timestamp);
""")

    # Create list_entries table for ClickHouse
    output_lines.append("-- Create list_entries table")
    output_lines.append("""CREATE TABLE IF NOT EXISTS list_entries (
    id UInt64,
    list_id String,
    value String,
    created_at DateTime64(3) DEFAULT now64(3),
    expires_at Nullable(DateTime64(3)),
    metadata Nullable(String)
) ENGINE = MergeTree()
ORDER BY (list_id, value);
""")

    # Create indexes note (ClickHouse uses ORDER BY instead)
    output_lines.append("-- Note: ClickHouse uses ORDER BY for primary indexing")
    output_lines.append("-- Secondary indexes can be added with ALTER TABLE if needed")
    output_lines.append("")

    # Truncate tables
    output_lines.append("-- Clear existing data")
    output_lines.append("TRUNCATE TABLE IF EXISTS events;")
    output_lines.append("TRUNCATE TABLE IF EXISTS list_entries;")
    output_lines.append("")

    # Process INSERT statements
    event_id = 0
    list_id = 0

    for line in lines:
        line = line.strip()

        # Skip SQLite-specific statements
        if line.startswith("CREATE TABLE") or line.startswith("CREATE INDEX"):
            continue
        if line.startswith("DELETE FROM"):
            continue
        if line.startswith("--") and "Generated at:" not in line:
            continue
        if not line:
            continue

        # Process events INSERT
        if line.startswith("INSERT INTO events"):
            event_id += 1
            # Extract values from INSERT statement
            match = re.search(r"VALUES \((.+)\);?$", line)
            if match:
                values_str = match.group(1)
                # Parse the values - handle quoted strings and nulls
                converted = convert_events_insert(event_id, values_str)
                if converted:
                    output_lines.append(converted)

        # Process list_entries INSERT
        elif line.startswith("INSERT INTO list_entries"):
            list_id += 1
            match = re.search(r"VALUES \((.+)\);?$", line)
            if match:
                values_str = match.group(1)
                converted = convert_list_insert(list_id, values_str)
                if converted:
                    output_lines.append(converted)

    output_lines.append("")
    output_lines.append(f"-- Total events inserted: {event_id}")
    output_lines.append(f"-- Total list entries inserted: {list_id}")

    return '\n'.join(output_lines)


def convert_events_insert(event_id: int, values_str: str) -> str:
    """Convert SQLite events INSERT values to ClickHouse format."""

    # Parse values carefully handling quoted strings
    values = parse_sql_values(values_str)

    if len(values) < 14:
        return None

    # SQLite columns: event_type, user_id, timestamp, status, amount, currency,
    #                 merchant_id, device_id, ip_address, country, email, phone, metadata, attributes

    event_type = values[0]
    user_id = values[1]
    timestamp = convert_timestamp(values[2])
    status = values[3] if values[3] != 'NULL' else 'NULL'
    amount = values[4] if values[4] != 'NULL' else 'NULL'
    currency = values[5] if values[5] != 'NULL' else 'NULL'
    merchant_id = values[6] if values[6] != 'NULL' else 'NULL'
    device_id = values[7] if values[7] != 'NULL' else 'NULL'
    ip_address = values[8] if values[8] != 'NULL' else 'NULL'
    country = values[9] if values[9] != 'NULL' else 'NULL'
    email = values[10] if values[10] != 'NULL' else 'NULL'
    phone = values[11] if values[11] != 'NULL' else 'NULL'
    metadata = escape_json(values[12]) if values[12] != 'NULL' else 'NULL'
    attributes = escape_json(values[13]) if len(values) > 13 and values[13] != 'NULL' else 'NULL'

    return (f"INSERT INTO events (id, event_type, user_id, timestamp, status, amount, currency, "
            f"merchant_id, device_id, ip_address, country, email, phone, metadata, attributes) "
            f"VALUES ({event_id}, {event_type}, {user_id}, {timestamp}, {status}, {amount}, {currency}, "
            f"{merchant_id}, {device_id}, {ip_address}, {country}, {email}, {phone}, {metadata}, {attributes});")


def convert_list_insert(list_id: int, values_str: str) -> str:
    """Convert SQLite list_entries INSERT values to ClickHouse format."""

    values = parse_sql_values(values_str)

    if len(values) < 2:
        return None

    # SQLite columns (created_at omitted, uses DEFAULT): list_id, value, expires_at, metadata
    list_name = values[0]
    value = values[1]
    expires_at = convert_timestamp(values[2]) if len(values) > 2 and values[2] != 'NULL' else 'NULL'
    metadata = values[3] if len(values) > 3 and values[3] != 'NULL' else 'NULL'

    return (f"INSERT INTO list_entries (id, list_id, value, created_at, expires_at, metadata) "
            f"VALUES ({list_id}, {list_name}, {value}, now64(3), {expires_at}, {metadata});")


def parse_sql_values(values_str: str) -> list:
    """Parse SQL VALUES string into a list of values, handling quoted strings."""

    values = []
    current = ""
    in_quote = False
    quote_char = None
    i = 0

    while i < len(values_str):
        char = values_str[i]

        if not in_quote:
            if char == "'" or char == '"':
                in_quote = True
                quote_char = char
                current += char
            elif char == ',':
                values.append(current.strip())
                current = ""
            else:
                current += char
        else:
            current += char
            if char == quote_char:
                # Check for escaped quote
                if i + 1 < len(values_str) and values_str[i + 1] == quote_char:
                    current += values_str[i + 1]
                    i += 1
                else:
                    in_quote = False
                    quote_char = None
        i += 1

    if current.strip():
        values.append(current.strip())

    return values


def convert_timestamp(ts_str: str) -> str:
    """Convert SQLite timestamp string to ClickHouse DateTime64 format.

    Adjusts timestamps to be relative to current time to ensure they fall within
    time windows used by features (e.g., 90 days, 30 days, 7 days, 24 hours).
    Preserves the relative time differences between events.
    """

    if ts_str == 'NULL' or not ts_str:
        return 'NULL'

    # Remove quotes
    ts = ts_str.strip("'\"")

    # Handle ISO format: 2025-12-13T05:00:17.019788
    if 'T' in ts:
        # Convert to ClickHouse format: 2025-12-13 05:00:17.019
        ts = ts.replace('T', ' ')
        # Truncate microseconds to milliseconds
        if '.' in ts:
            parts = ts.split('.')
            if len(parts[1]) > 3:
                ts = parts[0] + '.' + parts[1][:3]

    # Parse the timestamp to calculate offset from now
    try:
        # Parse the timestamp (SQLite timestamps are in UTC)
        if '.' in ts:
            ts_dt = datetime.strptime(ts, '%Y-%m-%d %H:%M:%S.%f')
        else:
            ts_dt = datetime.strptime(ts, '%Y-%m-%d %H:%M:%S')

        # Calculate how long ago this timestamp should be from now
        # IMPORTANT: Use UTC time to match SQLite data generation which uses UTC
        from datetime import timezone
        now = datetime.now(timezone.utc).replace(tzinfo=None)
        time_diff = now - ts_dt

        # Convert to total seconds
        total_seconds = int(time_diff.total_seconds())

        # If the timestamp is in the future or very recent (within 1 minute), place it in the recent past
        if total_seconds < 60:
            # Put it 10 minutes to 1 hour ago, preserving relative ordering
            # Use microseconds to maintain ordering
            microseconds = ts_dt.microsecond
            return f"now64(3) - INTERVAL 600 SECOND - INTERVAL {microseconds} MICROSECOND"

        # For timestamps in the past, preserve the exact time difference
        # Break down into days, hours, minutes, seconds, milliseconds
        days = total_seconds // 86400
        remaining = total_seconds % 86400
        hours = remaining // 3600
        remaining = remaining % 3600
        minutes = remaining // 60
        seconds = remaining % 60
        milliseconds = ts_dt.microsecond // 1000

        # Build the interval expression
        intervals = []
        if days > 0:
            intervals.append(f"INTERVAL {days} DAY")
        if hours > 0:
            intervals.append(f"INTERVAL {hours} HOUR")
        if minutes > 0:
            intervals.append(f"INTERVAL {minutes} MINUTE")
        if seconds > 0:
            intervals.append(f"INTERVAL {seconds} SECOND")
        if milliseconds > 0:
            intervals.append(f"INTERVAL {milliseconds} MILLISECOND")

        if intervals:
            return "now64(3) - " + " - ".join(intervals)
        else:
            return "now64(3)"

    except Exception as e:
        # Fallback to original behavior if parsing fails
        return f"parseDateTimeBestEffort('{ts}')"


def escape_json(json_str: str) -> str:
    """Escape JSON string for ClickHouse."""
    if json_str == 'NULL':
        return 'NULL'

    # Already quoted, keep as is but escape internal quotes if needed
    return json_str


def main():
    """Main function."""

    print(f"Reading SQLite SQL from: {SQLITE_SQL}")

    if not os.path.exists(SQLITE_SQL):
        print(f"Error: SQLite SQL file not found: {SQLITE_SQL}")
        print("Please run generate_test_data.py first.")
        return 1

    with open(SQLITE_SQL, 'r') as f:
        sqlite_sql = f.read()

    print("Converting to ClickHouse format...")
    clickhouse_sql = convert_sqlite_to_clickhouse(sqlite_sql)

    print(f"Writing ClickHouse SQL to: {CLICKHOUSE_SQL}")
    with open(CLICKHOUSE_SQL, 'w') as f:
        f.write(clickhouse_sql)

    print("Done!")
    return 0


if __name__ == "__main__":
    exit(main())
