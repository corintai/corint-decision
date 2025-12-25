#!/usr/bin/env python3
"""
CORINT Decision Engine - E2E Test Data Generator

Generates SQL INSERT statements with diverse event data using relative timestamps
to avoid time window expiration issues.
"""

import random
import json
from datetime import datetime, timedelta
from pathlib import Path

# SQL output file path
SQL_OUTPUT = Path(__file__).parent.parent / "data" / "test_data.sql"

# User pools
NORMAL_USERS = [f"user_{i:04d}" for i in range(1, 51)]  # 50 normal users
SUSPICIOUS_USERS = [f"sus_{i:04d}" for i in range(1, 11)]  # 10 suspicious users
VIP_USERS = [f"vip_{i:04d}" for i in range(1, 6)]  # 5 VIP users

# Other data pools
MERCHANTS = [f"merchant_{i:03d}" for i in range(1, 21)]
DEVICES = [f"device_{i:05d}" for i in range(1, 101)]
COUNTRIES = ["US", "GB", "CA", "AU", "DE", "FR", "JP", "SG", "BR", "IN"]
SUSPICIOUS_COUNTRIES = ["NG", "RU", "CN", "KP"]
IPS = [f"192.168.{random.randint(1, 255)}.{random.randint(1, 255)}" for _ in range(100)]
SUSPICIOUS_IPS = ["45.142.212.61", "185.220.101.52", "91.109.190.28"]

# Event types
EVENT_TYPES = ["transaction", "login", "payment", "withdrawal", "transfer"]


def generate_schema_sql():
    """Generate SQL statements to create database schema"""
    return """-- Create events table
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    user_id TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    amount REAL,
    currency TEXT,
    merchant_id TEXT,
    device_id TEXT,
    ip_address TEXT,
    country TEXT,
    email TEXT,
    phone TEXT,
    status TEXT,
    metadata TEXT
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_user_id ON events(user_id);
CREATE INDEX IF NOT EXISTS idx_timestamp ON events(timestamp);
CREATE INDEX IF NOT EXISTS idx_event_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_user_timestamp ON events(user_id, timestamp);

-- Clear existing data
DELETE FROM events;
"""


def generate_timestamp(hours_ago=0, days_ago=0, minutes_ago=0):
    """Generate timestamp relative to current time"""
    now = datetime.now()
    offset = timedelta(hours=hours_ago, days=days_ago, minutes=minutes_ago)
    return (now - offset).isoformat()


def escape_sql_string(s):
    """Escape single quotes in SQL string"""
    if s is None:
        return "NULL"
    return f"'{str(s).replace(chr(39), chr(39)+chr(39))}'"

def generate_insert_statements(events):
    """Generate SQL INSERT statements from events"""
    sql_statements = []

    for event in events:
        event_type, user_id, timestamp, amount, currency, merchant_id, \
        device_id, ip_address, country, email, phone, status, metadata = event

        # Format values
        values = [
            escape_sql_string(event_type),
            escape_sql_string(user_id),
            escape_sql_string(timestamp),
            str(amount) if amount is not None else "NULL",
            escape_sql_string(currency),
            escape_sql_string(merchant_id),
            escape_sql_string(device_id),
            escape_sql_string(ip_address),
            escape_sql_string(country),
            escape_sql_string(email),
            escape_sql_string(phone),
            escape_sql_string(status),
            escape_sql_string(metadata)
        ]

        sql = f"INSERT INTO events (event_type, user_id, timestamp, amount, currency, merchant_id, device_id, ip_address, country, email, phone, status, metadata) VALUES ({', '.join(values)});"
        sql_statements.append(sql)

    return sql_statements


def generate_normal_transactions(count=100):
    """Generate normal transaction events"""
    events = []
    now = datetime.now()

    for i in range(count):
        user = random.choice(NORMAL_USERS + VIP_USERS)
        # Distribute events across last 30 days
        hours_ago = random.uniform(0, 720)  # 0-30 days

        metadata = {
            "payment_method": random.choice(["credit_card", "debit_card", "bank_transfer"]),
            "merchant_category": random.choice(["retail", "food", "travel", "online"]),
            "is_vip": user in VIP_USERS
        }

        events.append((
            "transaction",
            user,
            generate_timestamp(hours_ago=hours_ago),
            round(random.uniform(10, 500), 2),
            "USD",
            random.choice(MERCHANTS),
            random.choice(DEVICES),
            random.choice(IPS),
            random.choice(COUNTRIES),
            f"{user}@example.com",
            f"+1555{random.randint(1000000, 9999999)}",
            "completed",
            json.dumps(metadata)
        ))

    return events


def generate_suspicious_transactions(count=20):
    """Generate suspicious transaction events"""
    events = []

    for i in range(count):
        user = random.choice(SUSPICIOUS_USERS)
        # Recent suspicious activity
        hours_ago = random.uniform(0, 48)  # Last 2 days

        metadata = {
            "payment_method": random.choice(["crypto", "wire_transfer"]),
            "merchant_category": "suspicious",
            "high_risk": True,
            "new_recipient": True
        }

        events.append((
            "transaction",
            user,
            generate_timestamp(hours_ago=hours_ago),
            round(random.uniform(5000, 15000), 2),
            "USD",
            random.choice(MERCHANTS),
            random.choice(DEVICES),
            random.choice(SUSPICIOUS_IPS),
            random.choice(SUSPICIOUS_COUNTRIES),
            f"{user}@tempmail.com",
            None,
            "pending",
            json.dumps(metadata)
        ))

    return events


def generate_velocity_abuse(count=30):
    """Generate velocity abuse pattern - same user, multiple transactions in short time"""
    events = []

    # Pick a few users for velocity abuse
    for user in random.sample(SUSPICIOUS_USERS, 3):
        base_time = datetime.now()

        # Generate 10 transactions within 1 hour
        for i in range(10):
            minutes_ago = random.uniform(0, 60)

            metadata = {
                "payment_method": "credit_card",
                "velocity_pattern": True
            }

            events.append((
                "transaction",
                user,
                generate_timestamp(minutes_ago=minutes_ago),
                round(random.uniform(100, 500), 2),
                "USD",
                random.choice(MERCHANTS),
                random.choice(DEVICES),
                random.choice(IPS),
                random.choice(COUNTRIES),
                f"{user}@example.com",
                None,
                "completed",
                json.dumps(metadata)
            ))

    return events


def generate_login_events(count=200):
    """Generate login events"""
    events = []

    for i in range(count):
        user = random.choice(NORMAL_USERS + SUSPICIOUS_USERS + VIP_USERS)
        hours_ago = random.uniform(0, 168)  # Last week

        # Suspicious logins have more failed attempts
        is_suspicious = user in SUSPICIOUS_USERS
        success_rate = 0.5 if is_suspicious else 0.95

        metadata = {
            "login_method": random.choice(["password", "oauth", "sso"]),
            "failed_attempts": random.randint(0, 5) if is_suspicious else random.randint(0, 1),
            "user_agent": "Mozilla/5.0"
        }

        events.append((
            "login",
            user,
            generate_timestamp(hours_ago=hours_ago),
            None,
            None,
            None,
            random.choice(DEVICES),
            random.choice(SUSPICIOUS_IPS if is_suspicious else IPS),
            random.choice(SUSPICIOUS_COUNTRIES if is_suspicious else COUNTRIES),
            f"{user}@example.com",
            None,
            "success" if random.random() < success_rate else "failed",
            json.dumps(metadata)
        ))

    return events


def generate_payment_events(count=80):
    """Generate payment events"""
    events = []

    for i in range(count):
        user = random.choice(NORMAL_USERS + VIP_USERS)
        hours_ago = random.uniform(0, 240)  # Last 10 days

        amount = random.uniform(50, 2000) if user in VIP_USERS else random.uniform(10, 500)

        metadata = {
            "payment_type": random.choice(["subscription", "one_time", "recurring"]),
            "card_type": random.choice(["visa", "mastercard", "amex"]),
            "card_last4": f"{random.randint(1000, 9999)}"
        }

        events.append((
            "payment",
            user,
            generate_timestamp(hours_ago=hours_ago),
            round(amount, 2),
            random.choice(["USD", "EUR", "GBP"]),
            random.choice(MERCHANTS),
            random.choice(DEVICES),
            random.choice(IPS),
            random.choice(COUNTRIES),
            f"{user}@example.com",
            None,
            "completed",
            json.dumps(metadata)
        ))

    return events


def generate_account_takeover_pattern(count=15):
    """Generate account takeover pattern - location change, device change"""
    events = []

    for user in random.sample(NORMAL_USERS, 5):
        # Normal login
        events.append((
            "login",
            user,
            generate_timestamp(hours_ago=12),
            None,
            None,
            None,
            DEVICES[0],
            IPS[0],
            "US",
            f"{user}@example.com",
            None,
            "success",
            json.dumps({"normal_activity": True})
        ))

        # Suspicious login from different location shortly after
        events.append((
            "login",
            user,
            generate_timestamp(hours_ago=11.5),
            None,
            None,
            None,
            DEVICES[50],
            SUSPICIOUS_IPS[0],
            random.choice(SUSPICIOUS_COUNTRIES),
            f"{user}@example.com",
            None,
            "success",
            json.dumps({"possible_takeover": True, "location_change": True})
        ))

        # High-value transaction from suspicious location
        events.append((
            "transaction",
            user,
            generate_timestamp(hours_ago=11),
            round(random.uniform(8000, 15000), 2),
            "USD",
            random.choice(MERCHANTS),
            DEVICES[50],
            SUSPICIOUS_IPS[0],
            random.choice(SUSPICIOUS_COUNTRIES),
            f"{user}@example.com",
            None,
            "pending",
            json.dumps({"high_risk": True, "account_takeover_pattern": True})
        ))

    return events


def main():
    print("=" * 60)
    print("CORINT E2E Test Data Generator")
    print("=" * 60)
    print()

    # Create output directory
    SQL_OUTPUT.parent.mkdir(parents=True, exist_ok=True)

    # Generate all event types
    all_events = []

    print("Generating test data...")
    generators = [
        ("Normal transactions", generate_normal_transactions, 100),
        ("Suspicious transactions", generate_suspicious_transactions, 20),
        ("Velocity abuse patterns", generate_velocity_abuse, 30),
        ("Login events", generate_login_events, 200),
        ("Payment events", generate_payment_events, 80),
        ("Account takeover patterns", generate_account_takeover_pattern, 15),
    ]

    total_events = 0
    for name, generator, count in generators:
        events = generator(count)
        all_events.extend(events)
        total_events += len(events)
        print(f"  ✓ {name}: {len(events)} events")

    print()
    print(f"Generating SQL statements for {total_events} events...")

    # Write SQL file
    with open(SQL_OUTPUT, 'w') as f:
        # Write schema
        f.write("-- CORINT E2E Test Data\n")
        f.write(f"-- Generated at: {datetime.now().isoformat()}\n")
        f.write(f"-- Total events: {total_events}\n\n")

        f.write(generate_schema_sql())
        f.write("\n")

        # Write INSERT statements
        insert_statements = generate_insert_statements(all_events)
        for sql in insert_statements:
            f.write(sql + "\n")

    print(f"✓ SQL file generated: {SQL_OUTPUT}")
    print()

    # Print statistics
    print("Data Statistics:")
    print("-" * 60)
    print(f"Total events: {total_events}")

    # Count by type
    event_types = {}
    for event in all_events:
        event_type = event[0]
        event_types[event_type] = event_types.get(event_type, 0) + 1

    print("\nEvents by type:")
    for event_type, count in sorted(event_types.items()):
        print(f"  {event_type}: {count}")

    # Count unique users
    unique_users = set(event[1] for event in all_events)
    print(f"\nUnique users: {len(unique_users)}")

    # Time range
    timestamps = [event[2] for event in all_events]
    print(f"\nTime range:")
    print(f"  Earliest: {min(timestamps)}")
    print(f"  Latest: {max(timestamps)}")

    print()
    print("=" * 60)
    print("✓ SQL generation completed!")
    print("=" * 60)
    print()
    print(f"To create database, run:")
    print(f"  sqlite3 tests/data/e2e_test.db < {SQL_OUTPUT}")


if __name__ == "__main__":
    main()
