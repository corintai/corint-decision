#!/usr/bin/env python3
"""
Generate PostgreSQL-compatible test data SQL from SQLite test data.

This script reads the SQLite test_data.sql and converts it to PostgreSQL format:
- Converts CREATE TABLE syntax (SERIAL, TIMESTAMP, etc.)
- Converts INSERT statements
- Handles timestamp format conversion
"""

import re
import os
from datetime import datetime

# Paths
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
SQLITE_SQL = os.path.join(PROJECT_ROOT, "tests/data/test_data.sql")
POSTGRES_SQL = os.path.join(PROJECT_ROOT, "tests/data/postgres_test_data.sql")


def convert_sqlite_to_postgres(sqlite_sql: str) -> str:
    """Convert SQLite SQL to PostgreSQL SQL."""

    lines = sqlite_sql.split('\n')
    output_lines = []

    # Add header
    output_lines.append("-- CORINT E2E Test Data for PostgreSQL")
    output_lines.append(f"-- Generated at: {datetime.now().isoformat()}")
    output_lines.append("-- Converted from SQLite format")
    output_lines.append("")

    # Create events table for PostgreSQL
    output_lines.append("-- Create events table")
    output_lines.append("""CREATE TABLE IF NOT EXISTS events (
    id SERIAL PRIMARY KEY,
    event_type VARCHAR(64) NOT NULL,
    user_id VARCHAR(64) NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    status VARCHAR(32),
    amount DECIMAL(18, 2),
    currency VARCHAR(8),
    merchant_id VARCHAR(64),
    device_id VARCHAR(64),
    ip_address VARCHAR(64),
    country VARCHAR(8),
    email VARCHAR(128),
    phone VARCHAR(32),
    metadata TEXT,
    attributes TEXT
);
""")

    # Create indexes for events
    output_lines.append("-- Create indexes for events")
    output_lines.append("CREATE INDEX IF NOT EXISTS idx_events_user_id ON events(user_id);")
    output_lines.append("CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);")
    output_lines.append("CREATE INDEX IF NOT EXISTS idx_events_event_type ON events(event_type);")
    output_lines.append("CREATE INDEX IF NOT EXISTS idx_events_user_timestamp ON events(user_id, timestamp);")
    output_lines.append("")

    # Create list_entries table for PostgreSQL
    output_lines.append("-- Create list_entries table")
    output_lines.append("""CREATE TABLE IF NOT EXISTS list_entries (
    id SERIAL PRIMARY KEY,
    list_id VARCHAR(64) NOT NULL,
    value VARCHAR(256) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    metadata TEXT
);
""")

    # Create indexes for list_entries
    output_lines.append("-- Create indexes for list_entries")
    output_lines.append("CREATE INDEX IF NOT EXISTS idx_list_entries_list_id ON list_entries(list_id);")
    output_lines.append("CREATE INDEX IF NOT EXISTS idx_list_entries_value ON list_entries(list_id, value);")
    output_lines.append("CREATE INDEX IF NOT EXISTS idx_list_entries_expires ON list_entries(expires_at);")
    output_lines.append("")

    # Truncate tables
    output_lines.append("-- Clear existing data")
    output_lines.append("TRUNCATE TABLE events RESTART IDENTITY CASCADE;")
    output_lines.append("TRUNCATE TABLE list_entries RESTART IDENTITY CASCADE;")
    output_lines.append("")

    # Process INSERT statements
    event_count = 0
    list_count = 0

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
            event_count += 1
            # Convert timestamp format in INSERT
            converted = convert_events_insert_pg(line)
            if converted:
                output_lines.append(converted)

        # Process list_entries INSERT
        elif line.startswith("INSERT INTO list_entries"):
            list_count += 1
            converted = convert_list_insert_pg(line)
            if converted:
                output_lines.append(converted)

    output_lines.append("")
    output_lines.append(f"-- Total events inserted: {event_count}")
    output_lines.append(f"-- Total list entries inserted: {list_count}")

    return '\n'.join(output_lines)


def convert_events_insert_pg(line: str) -> str:
    """Convert SQLite events INSERT to PostgreSQL format."""

    # PostgreSQL INSERT format is mostly compatible with SQLite
    # Main changes:
    # 1. Remove column list to use SERIAL id
    # 2. Convert timestamp format if needed

    # Extract the VALUES part
    match = re.search(r"VALUES \((.+)\);?$", line)
    if not match:
        return line

    values_str = match.group(1)

    # Parse and convert timestamps (ISO format with T to space)
    # Replace 'T' with space in timestamp values
    converted = re.sub(r"'(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})", r"'\1 \2", values_str)

    return f"INSERT INTO events (event_type, user_id, timestamp, status, amount, currency, merchant_id, device_id, ip_address, country, email, phone, metadata, attributes) VALUES ({converted});"


def convert_list_insert_pg(line: str) -> str:
    """Convert SQLite list_entries INSERT to PostgreSQL format."""

    # Extract the VALUES part
    match = re.search(r"VALUES \((.+)\);?$", line)
    if not match:
        return line

    values_str = match.group(1)

    # Convert timestamp format
    converted = re.sub(r"'(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})", r"'\1 \2", values_str)

    return f"INSERT INTO list_entries (list_id, value, created_at, expires_at, metadata) VALUES ({converted});"


def main():
    """Main function."""

    print(f"Reading SQLite SQL from: {SQLITE_SQL}")

    if not os.path.exists(SQLITE_SQL):
        print(f"Error: SQLite SQL file not found: {SQLITE_SQL}")
        print("Please run generate_test_data.py first.")
        return 1

    with open(SQLITE_SQL, 'r') as f:
        sqlite_sql = f.read()

    print("Converting to PostgreSQL format...")
    postgres_sql = convert_sqlite_to_postgres(sqlite_sql)

    print(f"Writing PostgreSQL SQL to: {POSTGRES_SQL}")
    with open(POSTGRES_SQL, 'w') as f:
        f.write(postgres_sql)

    print("Done!")
    return 0


if __name__ == "__main__":
    exit(main())
